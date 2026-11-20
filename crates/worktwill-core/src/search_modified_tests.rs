use crate::{migrations, model::*, search, Store};
use rusqlite::{params, Connection, OpenFlags};
use std::{fs, path::Path};

fn fixture_directory(prefix: &str) -> tempfile::TempDir {
    let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(approved)
        .unwrap()
}

fn columns(db: &Connection, table: &str) -> Vec<String> {
    db.prepare(&format!("PRAGMA table_info({table})"))
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn version(db: &Connection) -> u32 {
    db.pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

fn legacy_v3(db: &Connection) {
    // 从历史结构升级到真实v3；不用新库改版本号模拟缺列的旧库。
    db.execute_batch(include_str!("schema.sql")).unwrap();
    assert_eq!(version(db), 1);
    db.execute_batch(
        "ALTER TABLE activities ADD COLUMN source TEXT NOT NULL DEFAULT 'app';
         ALTER TABLE activities ADD COLUMN file_modified_at REAL;
         CREATE INDEX activities_time ON activities(at DESC,id);
         CREATE TABLE file_grants(resource_id TEXT PRIMARY KEY REFERENCES resources(id),active INTEGER NOT NULL DEFAULT 1);
         PRAGMA user_version=2;",
    )
    .unwrap();
    assert_eq!(version(db), 2);
    db.execute_batch(
        "ALTER TABLE workspaces ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
         PRAGMA user_version=3;",
    )
    .unwrap();
    assert_eq!(version(db), 3);
    assert!(!columns(db, "search_documents")
        .iter()
        .any(|c| c == "modified_at"));
}

fn legacy_project(db: &Connection, id: &str, kind: &str, name: &str, path: &str) {
    let name = search::normalize(name);
    let path = search::normalize(path);
    db.execute(
        "INSERT INTO search_documents(entity_id,kind,name,location) VALUES(?1,?2,?3,?4)",
        params![id, kind, name, path],
    )
    .unwrap();
    let rowid = db.last_insert_rowid();
    db.execute(
        "INSERT INTO search_tri(rowid,text) VALUES(?1,?2)",
        params![rowid, format!("{name}\n{path}")],
    )
    .unwrap();
    db.execute(
        "INSERT INTO search_short(rowid,tokens) VALUES(?1,?2)",
        params![rowid, search::short_tokens(&name, &path)],
    )
    .unwrap();
}

fn seed_legacy(db: &Connection, directory: &Path) {
    let root = directory.join("documents");
    let single = directory.join("individual.txt");
    db.execute(
        "INSERT INTO scan_roots(id,path,active) VALUES('root',?1,1)",
        [root.to_string_lossy().as_ref()],
    )
    .unwrap();
    for (id, kind, name, path, modified) in [
        (
            "zero",
            "file",
            "资料_零时间.txt",
            root.join("zero.txt").to_string_lossy().into_owned(),
            0.,
        ),
        (
            "negative",
            "file",
            "资料_负时间.txt",
            single.to_string_lossy().into_owned(),
            -10.,
        ),
        (
            "web",
            "web",
            "资料_网页",
            "https://example.com/library".into(),
            42.,
        ),
    ] {
        db.execute("INSERT INTO resources(id,kind,name,location,modified_at,favorite,created_at) VALUES(?1,?2,?3,?4,?5,1,0)",params![id,kind,name,path,modified]).unwrap();
        legacy_project(db, id, kind, name, &path);
    }
    db.execute("INSERT INTO workspaces(id,name,created_at,sort_order) VALUES('workspace','资料_工作区',0,7)", []).unwrap();
    legacy_project(db, "workspace", "workspace", "资料_工作区", "");
    db.execute_batch(
        "INSERT INTO file_roots VALUES('root','zero',0);
         INSERT INTO file_grants VALUES('negative',1);
         INSERT INTO workspace_resources VALUES('workspace','zero',0);
         INSERT INTO workspace_resources VALUES('workspace','negative',0);
         INSERT INTO workspace_resources VALUES('workspace','web',0);
         INSERT INTO activities(id,kind,name,resource_id,at) VALUES('opened','opened','','zero',100);",
    )
    .unwrap();
}

fn projection_times(db: &Connection) -> Vec<(String, Option<f64>)> {
    db.prepare("SELECT entity_id,modified_at FROM search_documents ORDER BY entity_id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn expected_times() -> Vec<(String, Option<f64>)> {
    vec![
        ("negative".into(), Some(-10.)),
        ("web".into(), Some(42.)),
        ("workspace".into(), None),
        ("zero".into(), Some(0.)),
    ]
}

fn fts_content(db: &Connection, table: &str, field: &str) -> Vec<(i64, String)> {
    db.prepare(&format!("SELECT rowid,{field} FROM {table} ORDER BY rowid"))
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn genuine_v3_backfills_nullable_times_without_changing_fts_or_relationships() {
    let directory = fixture_directory("modified-v3-");
    let mut db = Connection::open_in_memory().unwrap();
    legacy_v3(&db);
    seed_legacy(&db, directory.path());
    let tri = fts_content(&db, "search_tri", "text");
    let short = fts_content(&db, "search_short", "tokens");
    migrations::migrate(&mut db).unwrap();
    assert_eq!(version(&db), migrations::VERSION);
    assert_eq!(projection_times(&db), expected_times());
    assert_eq!(fts_content(&db, "search_tri", "text"), tri);
    assert_eq!(fts_content(&db, "search_short", "tokens"), short);
    let ordered = db
        .prepare("SELECT entity_id FROM search_documents ORDER BY modified_at DESC,name,entity_id")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(ordered, ["web", "zero", "negative", "workspace"]);
    for index in [
        "search_documents_modified",
        "search_documents_kind_modified",
    ] {
        let count: u32 = db
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='index' AND name=?1",
                [index],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
    let counts: (u32, u32, u32, u32) = db.query_row("SELECT (SELECT count(*) FROM workspace_resources),(SELECT count(*) FROM resources WHERE favorite=1),(SELECT count(*) FROM file_grants WHERE active=1),(SELECT sort_order FROM workspaces WHERE id='workspace')", [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap();
    assert_eq!(counts, (3, 3, 1, 7));
    assert_eq!(
        db.query_row("SELECT sum(value) FROM settings", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
}

#[test]
fn failed_v3_migration_rolls_back_alter_and_backfill_then_can_retry() {
    let directory = fixture_directory("modified-rollback-");
    let mut db = Connection::open_in_memory().unwrap();
    legacy_v3(&db);
    seed_legacy(&db, directory.path());
    let tri = fts_content(&db, "search_tri", "text");
    // 在ALTER和回填之后的索引阶段注入确定性故障，验证整段迁移原子回滚。
    db.execute(
        "CREATE INDEX search_documents_modified ON search_documents(name)",
        [],
    )
    .unwrap();
    assert!(migrations::migrate(&mut db).is_err());
    assert_eq!(version(&db), 3);
    assert!(!columns(&db, "search_documents")
        .iter()
        .any(|c| c == "modified_at"));
    assert_eq!(
        db.query_row("SELECT count(*) FROM search_documents", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        4
    );
    assert_eq!(fts_content(&db, "search_tri", "text"), tri);
    db.execute("DROP INDEX search_documents_modified", [])
        .unwrap();
    migrations::migrate(&mut db).unwrap();
    assert_eq!(version(&db), migrations::VERSION);
    assert_eq!(projection_times(&db), expected_times());
}

#[test]
fn reopening_migrated_database_does_not_repeat_projection_backfill() {
    let directory = fixture_directory("modified-reopen-");
    let path = directory.path().join("legacy.sqlite3");
    {
        let db = Connection::open(&path).unwrap();
        legacy_v3(&db);
        seed_legacy(&db, directory.path());
    }
    let first = Store::open(&path).unwrap();
    assert_eq!(
        projection_times(&first.connection().unwrap()),
        expected_times()
    );
    drop(first);
    {
        let db = Connection::open(&path).unwrap();
        // 相同值的全表UPDATE也会触发；只用于夹具，不进入正式schema。
        db.execute_batch("CREATE TRIGGER forbid_projection_refill BEFORE UPDATE ON search_documents BEGIN SELECT RAISE(ABORT,'unexpected repeated projection backfill'); END;").unwrap();
    }
    for _ in 0..2 {
        let reopened = Store::open(&path).unwrap();
        let db = reopened.connection().unwrap();
        assert_eq!(version(&db), migrations::VERSION);
        assert_eq!(projection_times(&db), expected_times());
    }
}

#[test]
fn restoring_genuine_v3_backup_migrates_rebuilds_and_revokes_both_grant_types() {
    let directory = fixture_directory("modified-restore-");
    let legacy = directory.path().join("legacy-backup.sqlite3");
    {
        let db = Connection::open(&legacy).unwrap();
        legacy_v3(&db);
        seed_legacy(&db, directory.path());
        // 备份派生投影不完整且FTS内容错误，恢复必须从业务事实重建。
        search::remove(&db, "web").unwrap();
        db.execute("UPDATE search_tri SET text='stale_content'", [])
            .unwrap();
    }
    let source_bytes = fs::read(&legacy).unwrap();
    let store = Store::open(directory.path().join("app/current.sqlite3")).unwrap();
    store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation: Mutation::CreateWorkspace {
                name: "恢复前".into(),
                resource_ids: None,
            },
        })
        .unwrap();
    let before = store.overview().unwrap();
    store.restore_from(&legacy).unwrap();
    let after = store.overview().unwrap();
    assert!(after.permission_version > before.permission_version);
    assert!(after.revision > before.revision);
    assert_eq!(after.workspaces.len(), 1);
    assert_eq!(after.workspaces[0].id, "workspace");
    assert!(after.roots.iter().all(|root| !root.active));
    assert!(after.file_grants.is_empty());
    let db = store.connection().unwrap();
    assert_eq!(version(&db), migrations::VERSION);
    assert_eq!(projection_times(&db), expected_times());
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM file_grants WHERE active=1",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM workspace_resources", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM search_tri WHERE search_tri MATCH 'stale_content'",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    let response = store
        .search_page(SearchPageRequest {
            request: SearchRequest {
                request_id: 1,
                query: "资料".into(),
                workspace_id: None,
                kind: "all".into(),
            },
            offset: 0,
            sort: "modified".into(),
            cursor: None,
            modified_from: None,
            modified_to: None,
            path_prefix: None,
        })
        .unwrap();
    assert_eq!(
        response
            .page
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["web", "workspace"]
    );
    assert!(!response.page.has_more);
    assert!(response.next_cursor.is_none());
    assert_eq!(fs::read(&legacy).unwrap(), source_bytes);
    let source = Connection::open_with_flags(&legacy, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(version(&source), 3);
    assert!(!columns(&source, "search_documents")
        .iter()
        .any(|c| c == "modified_at"));
}

#[test]
fn version_four_adds_only_private_locator_table_and_failure_keeps_its_version() {
    let mut db = Connection::open_in_memory().unwrap();
    migrations::migrate(&mut db).unwrap();
    db.execute_batch("DROP TABLE activity_root_scopes; DROP TABLE activity_origins; DROP TABLE resource_relinks; DROP INDEX activities_resource_kind_time; DROP TABLE root_aliases; DROP TABLE root_locators; DROP TABLE file_locators; PRAGMA user_version=4;")
        .unwrap();
    let projection_columns = columns(&db, "search_documents");
    // 在真实v4结构上制造同名对象冲突；建表失败不能提前推进版本。
    db.execute("CREATE VIEW file_locators AS SELECT 1 AS resource_id", [])
        .unwrap();
    assert!(migrations::migrate(&mut db).is_err());
    assert_eq!(version(&db), 4);
    db.execute("DROP VIEW file_locators", []).unwrap();
    migrations::migrate(&mut db).unwrap();
    assert_eq!(version(&db), migrations::VERSION);
    assert_eq!(
        columns(&db, "file_locators"),
        ["resource_id", "identity", "bookmark"]
    );
    assert_eq!(columns(&db, "search_documents"), projection_columns);
    assert_eq!(
        db.query_row("SELECT count(*) FROM file_locators", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
}
