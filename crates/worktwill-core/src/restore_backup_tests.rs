//! 仅批准临时目录中的真实SQLite锁与恢复故障，不操作用户数据。
use super::*;
use std::{
    fs,
    sync::{mpsc, Arc},
    thread,
    time::Instant,
};

struct Fixture {
    directory: tempfile::TempDir,
    store: Arc<Store>,
}
impl Fixture {
    fn new() -> Self {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("restore-lock-")
            .tempdir_in(approved)
            .unwrap();
        let store = Arc::new(Store::open(directory.path().join("app/data.sqlite3")).unwrap());
        Self { directory, store }
    }
    fn workspace(&self, name: &str) {
        self.store
            .mutate(MutationRequest {
                operation_id: id(),
                mutation: Mutation::CreateWorkspace {
                    name: name.into(),
                    resource_ids: None,
                },
            })
            .unwrap();
    }
}

#[test]
fn locked_restore_returns_error_releases_gate_and_allows_explicit_retry() {
    let fixture = Fixture::new();
    fixture.workspace("备份状态");
    let backup = fixture.directory.path().join("backup.sqlite3");
    fixture.store.backup_to(&backup).unwrap();
    fixture.workspace("当前状态必须保留");
    let before = serde_json::to_value(fixture.store.overview().unwrap()).unwrap();
    let epoch = fixture.store.scan_epoch.load(Ordering::SeqCst);
    let locker = Connection::open(&fixture.store.path).unwrap();
    locker.execute_batch("BEGIN IMMEDIATE").unwrap();
    let (sent, received) = mpsc::channel();
    let store = fixture.store.clone();
    let source = backup.clone();
    let started = Instant::now();
    let worker = thread::spawn(move || {
        sent.send(store.restore_from(&source)).unwrap();
    });
    let observed = received.recv_timeout(Duration::from_secs(3));
    let completed_while_locked = observed.is_ok();
    // 即使旧实现无限重试，也先释放外锁、收完线程，再断言，避免遗留挂起任务。
    let while_locked = completed_while_locked
        .then(|| serde_json::to_value(fixture.store.overview().unwrap()).unwrap());
    locker.execute_batch("ROLLBACK").unwrap();
    let result =
        observed.unwrap_or_else(|_| received.recv_timeout(Duration::from_secs(5)).unwrap());
    worker.join().unwrap();
    println!(
        "LOCKED_RESTORE completed_while_locked={completed_while_locked} elapsed_ms={}",
        started.elapsed().as_secs_f64() * 1000.
    );
    assert!(
        completed_while_locked,
        "restore kept retrying while holding the Store gate"
    );
    assert_eq!(result.unwrap_err().code, "database_busy");
    assert_eq!(while_locked.unwrap(), before);
    assert_eq!(
        serde_json::to_value(fixture.store.overview().unwrap()).unwrap(),
        before
    );
    assert_eq!(fixture.store.scan_epoch.load(Ordering::SeqCst), epoch);
    fixture.workspace("失败后仍可写入");
    fixture.store.restore_from(&backup).unwrap();
    let after = fixture.store.overview().unwrap();
    assert_eq!(after.workspaces.len(), 1);
    assert_eq!(after.workspaces[0].name, "备份状态");
    assert_eq!(fixture.store.scan_epoch.load(Ordering::SeqCst), epoch + 1);
}

fn populated_source(path: &Path) -> Connection {
    let source = Connection::open(path).unwrap();
    source.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE marker(value TEXT); INSERT INTO marker VALUES('source'); CREATE TABLE payload(value BLOB); INSERT INTO payload VALUES(zeroblob(2000000));").unwrap();
    source
}
fn marker(db: &Connection) -> String {
    db.query_row("SELECT value FROM marker", [], |row| row.get(0))
        .unwrap()
}
fn integrity(db: &Connection) {
    assert_eq!(
        db.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
}

#[test]
fn copy_busy_finishes_backup_handle_and_preserves_target() {
    let fixture = Fixture::new();
    let source = populated_source(&fixture.directory.path().join("source.sqlite3"));
    let target_path = fixture.directory.path().join("target.sqlite3");
    let mut target = Connection::open(&target_path).unwrap();
    target
        .execute_batch("CREATE TABLE marker(value TEXT); INSERT INTO marker VALUES('target');")
        .unwrap();
    let locker = Connection::open(&target_path).unwrap();
    locker.execute_batch("BEGIN IMMEDIATE").unwrap();
    let started = Instant::now();
    let result = copy_snapshot(&source, &mut target);
    let elapsed = started.elapsed();
    locker.execute_batch("ROLLBACK").unwrap();
    assert_eq!(result.unwrap_err().code, "database_busy");
    assert!(elapsed < Duration::from_secs(3));
    assert_eq!(marker(&target), "target");
    integrity(&target);
    copy_snapshot(&source, &mut target).unwrap();
    assert_eq!(marker(&target), "source");
    integrity(&target);
    println!("COPY_BUSY elapsed_ms={}", elapsed.as_secs_f64() * 1000.);
}

#[test]
fn dropping_partial_backup_rolls_back_actual_written_pages() {
    let fixture = Fixture::new();
    let source = populated_source(&fixture.directory.path().join("source.sqlite3"));
    let target_path = fixture.directory.path().join("target.sqlite3");
    let mut target = Connection::open(&target_path).unwrap();
    target
        .execute_batch("CREATE TABLE marker(value TEXT); INSERT INTO marker VALUES('target');")
        .unwrap();
    let before = fs::read(&target_path).unwrap();
    {
        let backup = Backup::new(&source, &mut target).unwrap();
        assert_eq!(backup.step(1).unwrap(), StepResult::More);
        assert!(backup.progress().remaining > 0);
    }
    assert_eq!(marker(&target), "target");
    integrity(&target);
    assert_eq!(fs::read(&target_path).unwrap(), before);
    target
        .execute("UPDATE marker SET value='still writable'", [])
        .unwrap();
    assert_eq!(marker(&target), "still writable");
}

#[test]
fn pinned_source_keeps_validated_snapshot_during_external_wal_commit() {
    let fixture = Fixture::new();
    let source_path = fixture.directory.path().join("source.sqlite3");
    let source = populated_source(&source_path);
    let snapshot = source.unchecked_transaction().unwrap();
    assert_eq!(marker(&snapshot), "source");
    let mut target = Connection::open_in_memory().unwrap();
    {
        let backup = Backup::new(&snapshot, &mut target).unwrap();
        assert_eq!(backup.step(1).unwrap(), StepResult::More);
        let writer = Connection::open(&source_path).unwrap();
        writer
            .execute("UPDATE marker SET value='external commit'", [])
            .unwrap();
        assert_eq!(marker(&writer), "external commit");
        loop {
            match backup.step(100).unwrap() {
                StepResult::Done => break,
                StepResult::More => {}
                other => panic!("unexpected snapshot result {other:?}"),
            }
        }
    }
    assert_eq!(marker(&target), "source");
    integrity(&target);
    let mut through_helper = Connection::open_in_memory().unwrap();
    copy_snapshot(&snapshot, &mut through_helper).unwrap();
    assert_eq!(marker(&through_helper), "source");
    drop(snapshot);
    assert_eq!(marker(&source), "external commit");
}

#[test]
fn restoring_live_database_path_drops_source_read_lock_before_target_copy() {
    let fixture = Fixture::new();
    fixture.workspace("当前库来源");
    let before = fixture.store.overview().unwrap();
    fixture.store.restore_from(&fixture.store.path).unwrap();
    let after = fixture.store.overview().unwrap();
    assert_eq!(after.workspaces[0].id, before.workspaces[0].id);
    assert_eq!(after.permission_version, before.permission_version + 1);
    assert_eq!(after.revision, before.revision + 1);
}

#[test]
fn sqlite_full_during_copy_rolls_back_target_and_allows_retry() {
    let fixture = Fixture::new();
    let source = populated_source(&fixture.directory.path().join("source.sqlite3"));
    let target_path = fixture.directory.path().join("full-target.sqlite3");
    let mut target = Connection::open(&target_path).unwrap();
    target.execute_batch("CREATE TABLE marker(value TEXT); INSERT INTO marker VALUES('target'); PRAGMA max_page_count=2;").unwrap();
    let before = fs::read(&target_path).unwrap();
    let result = copy_snapshot(&source, &mut target);
    assert_eq!(result.unwrap_err().code, "storage_full");
    assert_eq!(marker(&target), "target");
    integrity(&target);
    assert_eq!(fs::read(&target_path).unwrap(), before);
    target
        .pragma_update(None, "max_page_count", 1_000_000)
        .unwrap();
    copy_snapshot(&source, &mut target).unwrap();
    assert_eq!(marker(&target), "source");
    integrity(&target);
}

#[test]
fn current_schema_backup_missing_locator_table_or_columns_is_rejected_before_live_write() {
    let f = Fixture::new();
    f.workspace("当前资料保留");
    for case in 0..2 {
        let backup = f
            .directory
            .path()
            .join(format!("invalid-locator-{case}.sqlite3"));
        f.store.backup_to(&backup).unwrap();
        {
            let db = Connection::open(&backup).unwrap();
            db.execute("DROP TABLE file_locators", []).unwrap();
            if case == 1 {
                db.execute(
                    "CREATE TABLE file_locators(resource_id TEXT PRIMARY KEY,identity TEXT)",
                    [],
                )
                .unwrap();
            }
            assert_eq!(
                db.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                    .unwrap(),
                "ok"
            );
        }
        let before = serde_json::to_value(f.store.overview().unwrap()).unwrap();
        assert_eq!(
            f.store.restore_from(&backup).unwrap_err().code,
            "database_error"
        );
        assert_eq!(
            serde_json::to_value(f.store.overview().unwrap()).unwrap(),
            before
        );
        f.workspace("拒绝备份后仍可写");
    }
}
