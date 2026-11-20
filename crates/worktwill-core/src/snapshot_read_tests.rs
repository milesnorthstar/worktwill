//! 在正式查询准备版本 SQL 时协调另一连接提交，验证 DTO 与版本没有拼接不同快照。
use crate::{model::ResourceQuery, Store};
use rusqlite::{
    hooks::{AuthAction, AuthContext, Authorization},
    Connection,
};
use std::{cell::RefCell, fs, sync::mpsc, thread, time::Duration};

type ConnectionSetup = Box<dyn FnOnce(&Connection)>;
thread_local! {
    static NEXT_CONNECTION: RefCell<Option<ConnectionSetup>> = const { RefCell::new(None) };
}

pub(crate) fn configure(db: &Connection) {
    if let Some(setup) = NEXT_CONNECTION.with(|next| next.borrow_mut().take()) {
        setup(db);
    }
}

fn check_snapshot(overview: bool) {
    let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
    let directory = tempfile::Builder::new()
        .prefix("read-snapshot-")
        .tempdir_in(approved)
        .unwrap();
    let docs = directory.path().join("documents");
    fs::create_dir(&docs).unwrap();
    let path = docs.join("原始资料.txt");
    fs::write(&path, b"synthetic fixture").unwrap();
    let database = directory.path().join("app/catalog.sqlite3");
    let store = Store::open(&database).unwrap();
    store
        .import_files(&[path.to_string_lossy().into_owned()], None, true)
        .unwrap();
    store.grant_root(&docs).unwrap();
    let writer = Connection::open(&database).unwrap();
    writer.execute("UPDATE settings SET value=10", []).unwrap();
    let read = || {
        if overview {
            serde_json::to_value(store.overview().unwrap()).unwrap()
        } else {
            serde_json::to_value(
                store
                    .resources(ResourceQuery {
                        workspace_id: None,
                        kind: "file".into(),
                        mode: "modified".into(),
                        offset: 0,
                    })
                    .unwrap(),
            )
            .unwrap()
        }
    };
    let before = read();
    let (started, ready) = mpsc::channel();
    let (committed, finished) = mpsc::channel();
    let worker = thread::spawn(move || {
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        writer.execute_batch("BEGIN IMMEDIATE; UPDATE resources SET name='外部提交的新名称',favorite=1,modified_at=999999999,size=55; UPDATE search_documents SET modified_at=999999999 WHERE kind<>'workspace'; UPDATE scan_roots SET scanned=99; UPDATE settings SET value=11 WHERE key='revision'; COMMIT;").unwrap();
        committed.send(()).unwrap();
    });
    NEXT_CONNECTION.with(|next| {
        *next.borrow_mut() = Some(Box::new(move |db| {
            let mut fired = false;
            db.authorizer(Some(move |context: AuthContext<'_>| {
                if !fired
                    && matches!(
                        context.action,
                        AuthAction::Read {
                            table_name: "settings",
                            column_name: "value"
                        }
                    )
                {
                    fired = true;
                    started.send(()).unwrap();
                    finished.recv_timeout(Duration::from_secs(5)).unwrap();
                }
                Authorization::Allow
            }));
        }))
    });
    let during = read();
    worker.join().unwrap();
    assert_eq!(during, before);
    let after = read();
    assert_eq!(after["revision"], 11);
    assert_ne!(after, before);
    assert_eq!(fs::read(path).unwrap(), b"synthetic fixture");
}

#[test]
fn resources_keep_candidates_details_and_versions_in_one_snapshot() {
    check_snapshot(false);
}

#[test]
fn overview_keeps_roots_activities_grants_and_versions_in_one_snapshot() {
    check_snapshot(true);
}
