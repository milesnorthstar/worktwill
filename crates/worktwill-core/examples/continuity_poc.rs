//! 隔离实验库：测量保留UUID后的关系与历史行为，不提供生产自动续接接口。
#[cfg(windows)]
fn main() {
    use rusqlite::{params, Connection};
    use serde_json::{json, Value};
    use std::{fs, path::PathBuf, process::Command};
    use worktwill_core::{capture_import_selections, model::*, Store};
    fn mutate(store: &Store, mutation: Mutation) {
        store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation,
            })
            .unwrap();
    }
    fn object_id(path: &std::path::Path) -> Option<String> {
        let out = Command::new("fsutil")
            .args(["objectid", "query"])
            .arg(path)
            .output()
            .unwrap();
        if !out.status.success() {
            return None;
        }
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .and_then(|line| line.split(':').nth(1))
            .map(|s| s.trim().to_owned())
    }
    let args: Vec<_> = std::env::args().collect();
    let root = PathBuf::from(std::env::var_os("WORKTWILL_TEST_ROOT").unwrap())
        .canonicalize()
        .unwrap();
    let dir = PathBuf::from(&args[2]).canonicalize().unwrap();
    assert!(dir != root && dir.starts_with(&root));
    let path = dir.join("documents/contract.docx").canonicalize().unwrap();
    let database = dir.join("app/poc.sqlite3");
    let state_path = dir.join("state.json");
    let store = Store::open(&database).unwrap();
    if args[1] == "inspect-discovery" {
        // 仅观察隔离夹具，不创建授权或锚点；记录扫描跳过临时源的具体采样边界。
        for entry in fs::read_dir(dir.join("documents")).unwrap() {
            let entry = entry.unwrap();
            let candidate = entry.path();
            let metadata = fs::symlink_metadata(&candidate);
            let canonical = candidate.canonicalize();
            let selection =
                capture_import_selections(&[candidate.to_string_lossy().into()]).remove(0);
            println!(
                "{}",
                json!({"phase":"inspect-discovery","path":candidate,"metadata":metadata.as_ref().map(|m|json!({"file":m.is_file(),"size":m.len()})).map_err(|e|e.to_string()),"canonical":canonical.map_err(|e|e.to_string()),"identity":selection.identity,"capture_error":selection.capture_error,"reconcile":store.reconcile_document(&candidate)})
            );
        }
        store.scan_all().unwrap();
        println!(
            "{}",
            json!({"phase":"scan-roots","roots":store.roots().unwrap()})
        );
        return;
    }
    if args[1] == "scan-once" {
        store.scan_all().unwrap();
        let db = Connection::open(&database).unwrap();
        let rows = db.prepare("SELECT id,name,identity FROM resources WHERE kind='file'").unwrap().query_map([], |r| Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"identity":r.get::<_,Option<String>>(2)?}))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
        println!(
            "{}",
            json!({"phase":"scan-once","resources":rows,"roots":store.roots().unwrap()})
        );
        return;
    }
    if args[1] == "scan-race" {
        // 同一独立Store连续扫描，尝试捕获真实WPS临时源提前入库的时序。
        for _ in 0..300 {
            store.scan_all().unwrap();
            let db = Connection::open(&database).unwrap();
            let rows = db.prepare("SELECT id,name,identity FROM resources WHERE kind='file'").unwrap().query_map([], |r| Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"identity":r.get::<_,Option<String>>(2)?}))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
            println!("{}", json!({"phase":"scan-race","resources":rows}));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        return;
    }
    if args[1] == "init" || args[1] == "init-mixed" || args[1] == "init-service" {
        assert!(!state_path.exists());
        if args[1] == "init-mixed" {
            store
                .import_files(&[path.to_string_lossy().into()], None, true)
                .unwrap();
        }
        let grant = store.grant_root(path.parent().unwrap()).unwrap();
        store.scan_all().unwrap();
        let file = store
            .resources(ResourceQuery {
                workspace_id: None,
                kind: "file".into(),
                mode: "modified".into(),
                offset: 0,
            })
            .unwrap()
            .items
            .remove(0);
        for name in ["PoC A", "PoC B"] {
            mutate(
                &store,
                Mutation::CreateWorkspace {
                    name: name.into(),
                    resource_ids: Some(vec![file.id.clone()]),
                },
            );
        }
        mutate(
            &store,
            Mutation::Favorite {
                id: file.id.clone(),
                value: true,
            },
        );
        if args[1] == "init-service" {
            store.set_document_tracking(&grant, true).unwrap();
            store.establish_document_anchor(&file.id).unwrap();
        }
        // 只测试活动记账，不声称本调用已请求系统编辑器打开。
        let opened = store.prepare_open(&file.id).unwrap();
        store.record_open_checked(opened).unwrap();
        let identity = capture_import_selections(&[path.to_string_lossy().into()])
            .remove(0)
            .identity
            .unwrap();
        fs::write(state_path,serde_json::to_vec_pretty(&json!({"id":file.id,"identity":identity,"object_id":object_id(&path),"root_id":grant})).unwrap()).unwrap();
    } else {
        let mut state: Value = serde_json::from_slice(&fs::read(&state_path).unwrap()).unwrap();
        let rid = state["id"].as_str().unwrap().to_owned();
        if args[1] == "revoke" {
            mutate(
                &store,
                Mutation::RevokeRoot {
                    id: state["root_id"].as_str().unwrap().into(),
                },
            );
        }
        let current = capture_import_selections(&[path.to_string_lossy().into()])
            .remove(0)
            .identity
            .unwrap();
        let oid = object_id(&path);
        let db = Connection::open(&database).unwrap();
        let active: bool = db
            .query_row(
                "SELECT active FROM scan_roots WHERE id=?",
                [state["root_id"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        let duplicate: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM resources WHERE identity=? AND id<>?)",
                params![current, rid],
                |r| r.get(0),
            )
            .unwrap();
        let decision = if !active {
            "reject_revoked"
        } else if duplicate {
            "reject_existing_resource"
        } else if oid.is_none() || oid.as_deref() != state["object_id"].as_str() {
            "ambiguous_no_marker"
        } else if args[1] == "apply" {
            "experimental_marker_policy"
        } else {
            "observe_only"
        };
        drop(db);
        drop(store);
        if decision == "experimental_marker_policy" {
            // 独占夹具且Store已释放；这是显式候选策略实验，不是身份已证明。
            let mut db = Connection::open(&database).unwrap();
            let tx = db.transaction().unwrap();
            let changed=tx.execute("UPDATE resources SET identity=?1,status='available' WHERE id=?2 AND identity=?3",params![current,rid,state["identity"].as_str().unwrap()]).unwrap();
            assert_eq!(changed, 1);
            tx.execute("UPDATE settings SET value=value+1 WHERE key='revision'", [])
                .unwrap();
            tx.commit().unwrap();
            state["identity"] = json!(current);
            fs::write(&state_path, serde_json::to_vec_pretty(&state).unwrap()).unwrap();
        }
        let store = Store::open(&database).unwrap();
        store.scan_all().unwrap();
        let list = |mode: &str| {
            store
                .resources(ResourceQuery {
                    workspace_id: None,
                    kind: "file".into(),
                    mode: mode.into(),
                    offset: 0,
                })
                .unwrap()
        };
        let db = Connection::open(&database).unwrap();
        let links: i64 = db
            .query_row(
                "SELECT count(*) FROM workspace_resources WHERE resource_id=?",
                [&rid],
                |r| r.get(0),
            )
            .unwrap();
        let count: i64 = db
            .query_row(
                "SELECT count(*) FROM resources WHERE kind='file'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let result = json!({"decision":decision,"resource_id":rid,"resolved":store.resolve_resource(&rid),"links":links,"resource_count":count,"favorites":list("favorites"),"recent":list("recent"),"object_id":oid,"warning":"experimental isolated SQL binding only; no lineage or grant transfer implemented"});
        println!("{}", result);
    }
}
#[cfg(not(windows))]
fn main() {
    panic!("Windows-only isolated experiment");
}
