//! 历史来源、schema7/8迁移及恢复结构验收；只操作批准目录内自建夹具。
use crate::{model::*, timeline::TimelineRequest, Store};
use rusqlite::{params, types::Value, Connection};
use std::{
    fs,
    path::{Path, PathBuf},
};

const ORIGIN_TABLES: &[(&str, &[&str])] = &[
    (
        "activity_origins",
        &["activity_id", "observed_resource_id", "identity"],
    ),
    (
        "activity_root_scopes",
        &[
            "activity_id",
            "root_id",
            "root_identity",
            "relative_path",
            "retired_location_valid",
        ],
    ),
    (
        "resource_relinks",
        &[
            "event_id",
            "source_resource_id",
            "merged_resource_id",
            "previous_identity",
            "selected_identity",
        ],
    ),
];

fn create_without_column(table: &str, columns: &[&str], missing: &str) -> String {
    // 保留其余真实约束，避免缺普通列的用例因额外破坏父键而提前失败。
    let mut definitions = columns
        .iter()
        .filter(|column| **column != missing)
        .map(|column| {
            let suffix = match (table, *column) {
                ("activity_root_scopes", "retired_location_valid") => {
                    "INTEGER NOT NULL DEFAULT 1 CHECK(retired_location_valid IN(0,1))"
                }
                ("activity_origins", "activity_id") | ("resource_relinks", "event_id") => {
                    "TEXT PRIMARY KEY REFERENCES activities(id) ON DELETE CASCADE"
                }
                ("activity_root_scopes", "activity_id") => {
                    "TEXT NOT NULL REFERENCES activity_origins(activity_id) ON DELETE CASCADE"
                }
                ("activity_root_scopes", "root_id") => {
                    "TEXT NOT NULL REFERENCES scan_roots(id) ON DELETE CASCADE"
                }
                ("activity_root_scopes", "relative_path")
                | ("resource_relinks", "source_resource_id" | "selected_identity") => {
                    "TEXT NOT NULL"
                }
                _ => "TEXT",
            };
            format!("{column} {suffix}")
        })
        .collect::<Vec<_>>();
    if table == "activity_root_scopes" && !["activity_id", "root_id"].contains(&missing) {
        definitions.push("PRIMARY KEY(activity_id,root_id)".into());
    }
    format!("CREATE TABLE {table}({})", definitions.join(","))
}

struct Fixture {
    _directory: tempfile::TempDir,
    home: PathBuf,
    root: PathBuf,
    database: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("activity-origins-")
            .tempdir_in(approved)
            .unwrap();
        let home = directory.path().canonicalize().unwrap();
        let root = home.join("documents");
        fs::create_dir(&root).unwrap();
        // 应用数据独立于资料目录，避免扫描器正确的自排除规则把测试资料也排除。
        fs::create_dir(home.join("app")).unwrap();
        let database = home.join("app/catalog.sqlite3");
        Self {
            _directory: directory,
            home,
            root,
            database,
        }
    }
    fn file(&self, path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"synthetic activity origin fixture").unwrap();
    }
}

// 逐段采用实际旧版DDL；不从schema7删表再伪造user_version=6。
fn genuine_v6(db: &Connection) {
    db.execute_batch(include_str!("schema.sql")).unwrap();
    db.execute_batch("ALTER TABLE activities ADD COLUMN source TEXT NOT NULL DEFAULT 'app';
        ALTER TABLE activities ADD COLUMN file_modified_at REAL;
        CREATE INDEX activities_time ON activities(at DESC,id);
        CREATE TABLE file_grants(resource_id TEXT PRIMARY KEY REFERENCES resources(id),active INTEGER NOT NULL DEFAULT 1);
        ALTER TABLE workspaces ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE search_documents ADD COLUMN modified_at REAL;
        CREATE INDEX search_documents_modified ON search_documents(modified_at DESC,name,entity_id);
        CREATE INDEX search_documents_kind_modified ON search_documents(kind,modified_at DESC,name,entity_id);
        CREATE TABLE file_locators(resource_id TEXT PRIMARY KEY REFERENCES resources(id) ON DELETE CASCADE,identity TEXT NOT NULL,bookmark BLOB NOT NULL);
        CREATE TABLE root_locators(root_id TEXT PRIMARY KEY REFERENCES scan_roots(id) ON DELETE CASCADE,identity TEXT NOT NULL,bookmark BLOB,valid INTEGER NOT NULL CHECK(valid IN(0,1)));
        CREATE INDEX root_locators_identity ON root_locators(identity);
        CREATE TABLE root_aliases(root_id TEXT NOT NULL REFERENCES scan_roots(id) ON DELETE CASCADE,path TEXT NOT NULL,PRIMARY KEY(root_id,path));
        PRAGMA user_version=6; PRAGMA foreign_keys=ON;").unwrap();
    assert_eq!(schema_version(db), 6);
    for (table, _) in ORIGIN_TABLES {
        assert_eq!(table_exists(db, table), 0);
    }
}
fn schema_version(db: &Connection) -> u32 {
    db.pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}
fn table_exists(db: &Connection, table: &str) -> usize {
    db.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?",
        [table],
        |row| row.get(0),
    )
    .unwrap()
}
fn schema(db: &Connection) -> Vec<(String, String, Option<String>)> {
    db.prepare("SELECT type,name,sql FROM sqlite_master ORDER BY type,name")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}
fn seed_v6_root(db: &Connection, id: &str, path: &Path, active: bool, anchored: Option<bool>) {
    db.execute(
        "INSERT INTO scan_roots(id,path,active) VALUES(?1,?2,?3)",
        params![id, path.to_string_lossy(), active],
    )
    .unwrap();
    if let Some(valid) = anchored {
        let identity = crate::platform::identity(path, &path.metadata().unwrap()).unwrap();
        db.execute(
            "INSERT INTO root_locators VALUES(?1,?2,NULL,?3)",
            params![id, identity, valid],
        )
        .unwrap();
    }
}
fn seed_v6_file(db: &Connection, root: &str, path: &Path, name: &str) -> (String, String, String) {
    let rid = uuid::Uuid::new_v4().to_string();
    let aid = uuid::Uuid::new_v4().to_string();
    let metadata = path.metadata().unwrap();
    let identity = crate::platform::identity(path, &metadata).unwrap();
    let current_name = path.file_name().unwrap().to_str().unwrap();
    db.execute("INSERT INTO resources(id,kind,name,location,identity,modified_at,size,created_at) VALUES(?1,'file',?2,?3,?4,?5,?6,1)",params![rid,current_name,path.to_string_lossy(),identity,crate::catalog::mtime(&metadata),metadata.len() as f64]).unwrap();
    db.execute("INSERT INTO file_roots VALUES(?1,?2,0)", params![root, rid])
        .unwrap();
    db.execute("INSERT INTO activities(id,kind,name,resource_id,at,source,file_modified_at) VALUES(?1,'file_discovered',?2,?3,1,'filesystem',NULL)",params![aid,name,rid]).unwrap();
    crate::search::project(
        db,
        &rid,
        "file",
        current_name,
        &path.to_string_lossy(),
        Some(crate::catalog::mtime(&metadata)),
    )
    .unwrap();
    (rid, aid, identity)
}
fn mutate(store: &Store, mutation: Mutation) -> MutationResponse {
    store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation,
        })
        .unwrap()
}
fn raw_table(db: &Connection, table: &str) -> Vec<Vec<Value>> {
    let mut query = db
        .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
        .unwrap();
    let columns = query.column_count();
    query
        .query_map([], |row| {
            (0..columns)
                .map(|column| row.get(column))
                .collect::<rusqlite::Result<Vec<Value>>>()
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}
fn live_snapshot(store: &Store) -> Vec<(String, Vec<Vec<Value>>)> {
    let db = store.connection().unwrap();
    [
        "settings",
        "resources",
        "scan_roots",
        "root_locators",
        "root_aliases",
        "scan_exclusions",
        "file_roots",
        "file_grants",
        "file_locators",
        "workspaces",
        "workspace_resources",
        "activities",
        "operations",
        "search_documents",
        "activity_origins",
        "activity_root_scopes",
        "resource_relinks",
    ]
    .into_iter()
    .map(|table| (table.into(), raw_table(&db, table)))
    .collect()
}
fn request() -> TimelineRequest {
    TimelineRequest {
        from: Some(0.),
        to: Some(1000.),
        month_from: 0.,
        month_to: 1000.,
        day_boundaries: vec![0., 1000.],
        filter: "all".into(),
        basis: "observed".into(),
        workspace_id: None,
        offset: 0,
        cursor: None,
    }
}
fn assert_no_internal_origin_fields(value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                assert!(
                    ![
                        "observed_resource_id",
                        "identity",
                        "root_identity",
                        "relative_path",
                        "source_resource_id",
                        "merged_resource_id",
                        "previous_identity",
                        "selected_identity",
                        "activity_origins",
                        "activity_root_scopes",
                        "resource_relinks"
                    ]
                    .contains(&key.as_str()),
                    "private provenance field leaked: {key}"
                );
                assert_no_internal_origin_fields(value);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                assert_no_internal_origin_fields(value);
            }
        }
        _ => {}
    }
}

#[test]
fn genuine_v6_migration_binds_only_current_permitted_scopes_without_inventing_observed_uuid() {
    let f = Fixture::new();
    let legacy = f.home.join("legacy");
    let revoked = f.home.join("revoked");
    let invalid = f.home.join("invalid");
    for folder in [&legacy, &revoked, &invalid] {
        fs::create_dir(folder).unwrap();
    }
    let paths = [
        f.root.join("allowed.txt"),
        legacy.join("legacy.txt"),
        revoked.join("revoked.txt"),
        f.root.join("private/hidden.txt"),
        invalid.join("invalid.txt"),
    ];
    for path in &paths {
        f.file(path);
    }
    let mut db = Connection::open(&f.database).unwrap();
    genuine_v6(&db);
    seed_v6_root(&db, "anchored", &f.root, true, Some(true));
    seed_v6_root(&db, "legacy", &legacy, true, None);
    seed_v6_root(&db, "revoked", &revoked, false, Some(true));
    seed_v6_root(&db, "invalid", &invalid, true, Some(false));
    db.execute(
        "INSERT INTO scan_exclusions VALUES('anchored','private')",
        [],
    )
    .unwrap();
    let mut seeded = Vec::new();
    for (index, (root, path)) in ["anchored", "legacy", "revoked", "anchored", "invalid"]
        .into_iter()
        .zip(&paths)
        .enumerate()
    {
        seeded.push(seed_v6_file(
            &db,
            root,
            path,
            if index == 1 {
                ""
            } else {
                "旧观察名称.txt"
            },
        ));
    }
    crate::migrations::migrate(&mut db).unwrap();
    assert_eq!(schema_version(&db), crate::migrations::VERSION);
    let before = raw_table(&db, "activity_origins");
    assert_eq!(before.len(), 5);
    for (rid, aid, identity) in &seeded {
        let observed: (Option<String>, Option<String>) = db
            .query_row(
                "SELECT observed_resource_id,identity FROM activity_origins WHERE activity_id=?",
                [aid],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(observed, (None, Some(identity.clone())));
        assert_ne!(observed.0.as_deref(), Some(rid.as_str()));
    }
    let scopes:Vec<(String,String,Option<String>,String)>=db.prepare("SELECT activity_id,root_id,root_identity,relative_path FROM activity_root_scopes ORDER BY root_id").unwrap().query_map([],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    assert_eq!(scopes.len(), 2);
    assert_eq!(
        scopes[0],
        (
            seeded[0].1.clone(),
            "anchored".into(),
            Some(crate::platform::identity(&f.root, &f.root.metadata().unwrap()).unwrap()),
            "allowed.txt".into()
        )
    );
    assert_eq!(
        scopes[1],
        (
            seeded[1].1.clone(),
            "legacy".into(),
            None,
            "legacy.txt".into()
        )
    );
    assert!(raw_table(&db, "resource_relinks").is_empty());
    let old_name: String = db
        .query_row(
            "SELECT name FROM activities WHERE id=?",
            [&seeded[1].1],
            |row| row.get(0),
        )
        .unwrap();
    assert!(old_name.is_empty());
    crate::migrations::migrate(&mut db).unwrap();
    assert!(before == raw_table(&db, "activity_origins"));
}

#[test]
fn genuine_v6_origin_table_conflicts_roll_back_all_ddl_and_data_then_retry() {
    for (conflicting, _) in ORIGIN_TABLES {
        let f = Fixture::new();
        let path = f.root.join("old.txt");
        f.file(&path);
        let mut db = Connection::open(&f.database).unwrap();
        genuine_v6(&db);
        seed_v6_root(&db, "legacy-root", &f.root, true, None);
        let (_, aid, _) = seed_v6_file(&db, "legacy-root", &path, "旧事件.txt");
        db.execute(&format!("CREATE TABLE {conflicting}(conflict TEXT)"), [])
            .unwrap();
        let before_schema = schema(&db);
        let before_resources = raw_table(&db, "resources");
        let before_activities = raw_table(&db, "activities");
        assert!(
            crate::migrations::migrate(&mut db).is_err(),
            "{conflicting}"
        );
        assert_eq!(schema_version(&db), 6);
        assert_eq!(schema(&db), before_schema);
        assert_eq!(raw_table(&db, "resources"), before_resources);
        assert_eq!(raw_table(&db, "activities"), before_activities);
        for (table, _) in ORIGIN_TABLES {
            assert_eq!(table_exists(&db, table), usize::from(table == conflicting));
        }
        db.execute(&format!("DROP TABLE {conflicting}"), [])
            .unwrap();
        crate::migrations::migrate(&mut db).unwrap();
        assert_eq!(schema_version(&db), crate::migrations::VERSION);
        let observed: Option<String> = db
            .query_row(
                "SELECT observed_resource_id FROM activity_origins WHERE activity_id=?",
                [&aid],
                |row| row.get(0),
            )
            .unwrap();
        assert!(observed.is_none());
        assert_eq!(raw_table(&db, "resources"), before_resources);
        assert_eq!(raw_table(&db, "activities"), before_activities);
    }
}

#[test]
fn migrated_empty_history_names_fall_back_only_while_current_resource_is_authorized() {
    let f = Fixture::new();
    let path = f.root.join("legacy-current-name.txt");
    f.file(&path);
    let db = Connection::open(&f.database).unwrap();
    genuine_v6(&db);
    seed_v6_root(&db, "legacy-root", &f.root, true, None);
    let (rid, aid, _) = seed_v6_file(&db, "legacy-root", &path, "");
    drop(db);
    let store = Store::open(&f.database).unwrap();
    let overview = store.overview().unwrap();
    let activity = overview
        .activities
        .iter()
        .find(|activity| activity.id == aid)
        .unwrap();
    assert_eq!(activity.name, "legacy-current-name.txt");
    assert_eq!(activity.resource_id.as_deref(), Some(rid.as_str()));
    let timeline = store.timeline(request()).unwrap();
    assert_eq!(timeline.events.len(), 1);
    assert_eq!(timeline.events[0].activity.name, "legacy-current-name.txt");
    let raw_name: String = store
        .connection()
        .unwrap()
        .query_row("SELECT name FROM activities WHERE id=?", [&aid], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(raw_name.is_empty());
    assert_no_internal_origin_fields(&serde_json::to_value(overview).unwrap());
    assert_no_internal_origin_fields(&serde_json::to_value(timeline).unwrap());
    mutate(
        &store,
        Mutation::RevokeRoot {
            id: "legacy-root".into(),
        },
    );
    assert!(store
        .overview()
        .unwrap()
        .activities
        .iter()
        .all(|activity| activity.id != aid));
    assert!(store.timeline(request()).unwrap().events.is_empty());
}

#[test]
fn current_schema_backups_missing_each_origin_table_or_required_column_never_replace_live_data() {
    let f = Fixture::new();
    let store = Store::open(&f.database).unwrap();
    mutate(
        &store,
        Mutation::CreateWorkspace {
            name: "备份点".into(),
            resource_ids: None,
        },
    );
    let valid = f.home.join("valid-empty-origins.sqlite3");
    store.backup_to(&valid).unwrap();
    let path = f.root.join("current-live-file.txt");
    f.file(&path);
    store.grant_root(&f.root).unwrap();
    store.scan_all().unwrap();
    mutate(
        &store,
        Mutation::CreateWorkspace {
            name: "当前必须保留".into(),
            resource_ids: None,
        },
    );
    let before = live_snapshot(&store);
    let before_schema = schema(&store.connection().unwrap());
    let before_session = store.session.clone();
    for (table, columns) in ORIGIN_TABLES {
        for missing in std::iter::once(None).chain(columns.iter().copied().map(Some)) {
            let invalid = f.home.join(format!(
                "missing-{table}-{}.sqlite3",
                missing.unwrap_or("table")
            ));
            fs::copy(&valid, &invalid).unwrap();
            let db = Connection::open(&invalid).unwrap();
            db.execute_batch(&format!("PRAGMA foreign_keys=OFF; DROP TABLE {table};"))
                .unwrap();
            if let Some(missing) = missing {
                db.execute(&create_without_column(table, columns, missing), [])
                    .unwrap();
                let column_names: Vec<String> = db
                    .prepare(&format!("PRAGMA table_info({table})"))
                    .unwrap()
                    .query_map([], |row| row.get(1))
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap();
                assert!(!column_names.iter().any(|column| column == missing));
            } else {
                assert_eq!(table_exists(&db, table), 0);
            }
            assert_eq!(schema_version(&db), crate::migrations::VERSION);
            drop(db);
            assert!(
                store.restore_from(&invalid).is_err(),
                "accepted current schema without {table}.{}",
                missing.unwrap_or("whole_table")
            );
            assert!(
                before == live_snapshot(&store),
                "live rows changed: {table}/{missing:?}"
            );
            assert_eq!(schema(&store.connection().unwrap()), before_schema);
            assert_eq!(store.session, before_session);
        }
    }
    mutate(
        &store,
        Mutation::CreateWorkspace {
            name: "拒绝恢复后仍可写入".into(),
            resource_ids: None,
        },
    );
    assert!(store
        .overview()
        .unwrap()
        .workspaces
        .iter()
        .any(|workspace| workspace.name == "拒绝恢复后仍可写入"));
}

#[test]
fn merge_keeps_original_event_origins_and_relink_provenance_after_duplicate_deletion_reopen_and_restore(
) {
    let f = Fixture::new();
    let source_path = f.root.join("observed-source.txt");
    let target_path = f.root.join("selected-target.txt");
    f.file(&source_path);
    f.file(&target_path);
    let store = Store::open(&f.database).unwrap();
    let root_id = store.grant_root(&f.root).unwrap();
    store.scan_all().unwrap();
    let db = store.connection().unwrap();
    let source: (String, String) = db
        .query_row(
            "SELECT id,identity FROM resources WHERE location=?",
            [source_path.to_string_lossy().as_ref()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let target: (String, String) = db
        .query_row(
            "SELECT id,identity FROM resources WHERE location=?",
            [target_path.to_string_lossy().as_ref()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    drop(db);
    mutate(
        &store,
        Mutation::CreateWorkspace {
            name: "原引用工作区".into(),
            resource_ids: Some(vec![source.0.clone()]),
        },
    );
    mutate(
        &store,
        Mutation::CreateWorkspace {
            name: "目标引用工作区".into(),
            resource_ids: Some(vec![target.0.clone()]),
        },
    );
    mutate(
        &store,
        Mutation::Favorite {
            id: target.0.clone(),
            value: true,
        },
    );
    store.record_open(&source.0).unwrap();
    store.record_open(&target.0).unwrap();
    let db = store.connection().unwrap();
    let previous_origins = raw_table(&db, "activity_origins");
    let previous_scopes = raw_table(&db, "activity_root_scopes");
    let target_events: Vec<String> = db
        .prepare(
            "SELECT activity_id FROM activity_origins WHERE observed_resource_id=? AND identity=?",
        )
        .unwrap()
        .query_map(params![target.0, target.1], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let source_events: Vec<String> = db
        .prepare(
            "SELECT activity_id FROM activity_origins WHERE observed_resource_id=? AND identity=?",
        )
        .unwrap()
        .query_map(params![source.0, source.1], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(!target_events.is_empty() && !source_events.is_empty());
    let anchored:usize=db.query_row("SELECT count(*) FROM activity_root_scopes WHERE root_id=? AND root_identity=(SELECT identity FROM root_locators WHERE root_id=?)",params![root_id,root_id],|row|row.get(0)).unwrap();
    assert_eq!(anchored, previous_origins.len());
    let actual_root_identity =
        crate::platform::identity(&f.root, &f.root.metadata().unwrap()).unwrap();
    let captured: Vec<(String, String, String, String)> = db.prepare("SELECT o.observed_resource_id,o.identity,s.root_identity,s.relative_path FROM activity_origins o JOIN activity_root_scopes s ON s.activity_id=o.activity_id WHERE s.root_id=?").unwrap()
        .query_map([&root_id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    assert_eq!(captured.len(), previous_origins.len());
    for (observed, identity, root_identity, relative) in captured {
        let (expected_identity, expected_relative) = if observed == source.0 {
            (&source.1, "observed-source.txt")
        } else {
            assert_eq!(observed, target.0);
            (&target.1, "selected-target.txt")
        };
        assert_eq!(&identity, expected_identity);
        assert_eq!(root_identity, actual_root_identity);
        assert_eq!(relative, expected_relative);
    }
    drop(db);
    store
        .relink_confirmed(&source.0, &target_path, true)
        .unwrap();
    let db = store.connection().unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM resources WHERE id=?",
            [&target.0],
            |row| row.get::<_, usize>(0)
        )
        .unwrap(),
        0
    );
    let provenance:(String,String,Option<String>,Option<String>,String)=db.query_row("SELECT event_id,source_resource_id,merged_resource_id,previous_identity,selected_identity FROM resource_relinks",[],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).unwrap();
    assert_eq!(provenance.1, source.0);
    assert_eq!(provenance.2.as_deref(), Some(target.0.as_str()));
    assert_eq!(provenance.3.as_deref(), Some(source.1.as_str()));
    assert_eq!(provenance.4, target.1);
    for row in &previous_origins {
        assert!(raw_table(&db, "activity_origins").contains(row));
    }
    for row in &previous_scopes {
        assert!(raw_table(&db, "activity_root_scopes").contains(row));
    }
    for aid in &target_events {
        let current: String = db
            .query_row(
                "SELECT resource_id FROM activities WHERE id=?",
                [aid],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(current, source.0);
        let original: (String, String) = db
            .query_row(
                "SELECT observed_resource_id,identity FROM activity_origins WHERE activity_id=?",
                [aid],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(original, target);
    }
    let current_origin: (String, String) = db
        .query_row(
            "SELECT observed_resource_id,identity FROM activity_origins WHERE activity_id=?",
            [&provenance.0],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(current_origin, (source.0.clone(), target.1.clone()));
    let persisted_origins = raw_table(&db, "activity_origins");
    let persisted_scopes = raw_table(&db, "activity_root_scopes");
    let persisted_relinks = raw_table(&db, "resource_relinks");
    drop(db);
    let overview = serde_json::to_value(store.overview().unwrap()).unwrap();
    assert_no_internal_origin_fields(&overview);
    let serialized = serde_json::to_string(&overview).unwrap();
    assert!(!serialized.contains(&target.0));
    assert!(!serialized.contains(&source.1));
    assert!(!serialized.contains(&target.1));
    drop(store);
    let store = Store::open(&f.database).unwrap();
    for (table, rows) in [
        ("activity_origins", &persisted_origins),
        ("activity_root_scopes", &persisted_scopes),
        ("resource_relinks", &persisted_relinks),
    ] {
        assert_eq!(&raw_table(&store.connection().unwrap(), table), rows);
    }
    let backup = f.home.join("merged-origins.sqlite3");
    store.backup_to(&backup).unwrap();
    store.restore_from(&backup).unwrap();
    for (table, rows) in [
        ("activity_origins", &persisted_origins),
        ("activity_root_scopes", &persisted_scopes),
        ("resource_relinks", &persisted_relinks),
    ] {
        assert_eq!(&raw_table(&store.connection().unwrap(), table), rows);
    }
    let restored = store.overview().unwrap();
    assert!(restored
        .activities
        .iter()
        .all(|activity| !source_events.contains(&activity.id)
            && !target_events.contains(&activity.id)));
    assert_no_internal_origin_fields(&serde_json::to_value(restored).unwrap());
    assert_eq!(
        fs::read(source_path).unwrap(),
        b"synthetic activity origin fixture"
    );
    assert_eq!(
        fs::read(target_path).unwrap(),
        b"synthetic activity origin fixture"
    );
}

// 复现已发布的两种真实v7结构；无列版本不能靠从当前库删列后改版本号冒充。
fn advance_genuine_v6_to_v7(db: &Connection, with_retired_flag: bool) {
    assert_eq!(schema_version(db), 6);
    let flag = if with_retired_flag {
        ",retired_location_valid INTEGER NOT NULL DEFAULT 1 CHECK(retired_location_valid IN(0,1))"
    } else {
        ""
    };
    db.execute_batch(&format!(
        "CREATE TABLE activity_origins(activity_id TEXT PRIMARY KEY REFERENCES activities(id) ON DELETE CASCADE,observed_resource_id TEXT,identity TEXT);
         CREATE TABLE activity_root_scopes(activity_id TEXT NOT NULL REFERENCES activity_origins(activity_id) ON DELETE CASCADE,root_id TEXT NOT NULL REFERENCES scan_roots(id) ON DELETE CASCADE,root_identity TEXT,relative_path TEXT NOT NULL{flag},PRIMARY KEY(activity_id,root_id));
         CREATE TABLE resource_relinks(event_id TEXT PRIMARY KEY REFERENCES activities(id) ON DELETE CASCADE,source_resource_id TEXT NOT NULL,merged_resource_id TEXT,previous_identity TEXT,selected_identity TEXT NOT NULL);
         PRAGMA user_version=7;"
    ))
    .unwrap();
    assert_eq!(schema_version(db), 7);
}

struct V7History {
    current: (String, String, String),
    retired: (String, String, String),
    replacement_identity: String,
    relink_event: String,
    workspace: String,
}

fn seeded_genuine_v7(f: &Fixture, with_retired_flag: bool) -> (Connection, V7History) {
    let current_path = f.root.join("v7-current.txt");
    let retired_path = f.root.join("v7-retired-personnel.txt");
    let replacement = f.root.join("v7-selected-replacement.txt");
    for path in [&current_path, &retired_path, &replacement] {
        f.file(path);
    }
    let db = Connection::open(&f.database).unwrap();
    genuine_v6(&db);
    seed_v6_root(&db, "v7-root", &f.root, true, Some(true));
    let current = seed_v6_file(&db, "v7-root", &current_path, "当前对象旧观察.txt");
    let retired = seed_v6_file(&db, "v7-root", &retired_path, "退休对象旧观察.txt");
    advance_genuine_v6_to_v7(&db, with_retired_flag);
    // 这些为v7已存在的观察事实，迁移只能保留，不能将原UUID或identity改绑给替代对象。
    crate::activity_origin::capture(&db, &current.1, &current.0).unwrap();
    crate::activity_origin::capture(&db, &retired.1, &retired.0).unwrap();
    let outside = f.home.join("retired-outside-managed-root");
    fs::create_dir(&outside).unwrap();
    fs::rename(&retired_path, outside.join("v7-retired-personnel.txt")).unwrap();
    let metadata = replacement.metadata().unwrap();
    let replacement_identity = crate::platform::identity(&replacement, &metadata).unwrap();
    db.execute(
        "UPDATE resources SET identity=?1,name='v7-selected-replacement.txt',location=?2,modified_at=?3,size=?4,favorite=1 WHERE id=?5",
        params![replacement_identity,replacement.to_string_lossy(),crate::catalog::mtime(&metadata),metadata.len() as f64,retired.0],
    )
    .unwrap();
    crate::search::project(
        &db,
        &retired.0,
        "file",
        "v7-selected-replacement.txt",
        &replacement.to_string_lossy(),
        Some(crate::catalog::mtime(&metadata)),
    )
    .unwrap();
    let relink_event = uuid::Uuid::new_v4().to_string();
    db.execute(
        "INSERT INTO activities(id,kind,name,resource_id,at,source) VALUES(?1,'relinked','v7-selected-replacement.txt',?2,2,'app')",
        params![relink_event,retired.0],
    )
    .unwrap();
    crate::activity_origin::capture(&db, &relink_event, &retired.0).unwrap();
    db.execute(
        "INSERT INTO resource_relinks VALUES(?1,?2,NULL,?3,?4)",
        params![relink_event, retired.0, retired.2, replacement_identity],
    )
    .unwrap();
    let workspace = uuid::Uuid::new_v4().to_string();
    db.execute(
        "INSERT INTO workspaces(id,name,created_at,sort_order) VALUES(?1,'v7关系与收藏必须保留',3,4)",
        [&workspace],
    )
    .unwrap();
    for rid in [&current.0, &retired.0] {
        db.execute(
            "INSERT INTO workspace_resources VALUES(?1,?2,3)",
            params![workspace, rid],
        )
        .unwrap();
        db.execute("UPDATE resources SET favorite=1 WHERE id=?", [rid])
            .unwrap();
    }
    if with_retired_flag {
        // 已记录的拒绝必须持久保留；另两行仍为默认1，形成真实混合0/1旧数据。
        db.execute(
            "UPDATE activity_root_scopes SET retired_location_valid=0 WHERE activity_id=?",
            [&retired.1],
        )
        .unwrap();
    }
    (
        db,
        V7History {
            current,
            retired,
            replacement_identity,
            relink_event,
            workspace,
        },
    )
}

fn four_scope_columns(db: &Connection) -> Vec<(String, String, Option<String>, String)> {
    db.prepare("SELECT activity_id,root_id,root_identity,relative_path FROM activity_root_scopes ORDER BY activity_id,root_id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn assert_v7_history_visibility(store: &Store, seed: &V7History) {
    let overview = store.overview().unwrap();
    assert!(overview.activities.iter().any(|activity| {
        activity.id == seed.current.1 && activity.name == "当前对象旧观察.txt"
    }));
    assert!(overview
        .activities
        .iter()
        .any(|activity| activity.id == seed.relink_event));
    assert!(overview
        .activities
        .iter()
        .all(|activity| activity.id != seed.retired.1));
    let timeline = store.timeline(request()).unwrap();
    assert!(timeline
        .events
        .iter()
        .any(|entry| entry.activity.id == seed.current.1));
    assert!(timeline
        .events
        .iter()
        .all(|entry| entry.activity.id != seed.relink_event));
    assert!(timeline
        .events
        .iter()
        .all(|entry| entry.activity.id != seed.retired.1));
    assert_eq!(timeline.events.len(), 1);
    assert_eq!(
        timeline
            .calendar
            .iter()
            .map(|point| point.count)
            .sum::<u32>(),
        1
    );
    // 应用管理事件只在次级筛选中出现，不能为迁移测试改变默认文件动态的产品语义。
    let mut system_request = request();
    system_request.filter = "system".into();
    let system = store.timeline(system_request).unwrap();
    assert_eq!(system.events.len(), 1);
    assert_eq!(system.events[0].activity.id, seed.relink_event);
    assert_eq!(
        system.calendar.iter().map(|point| point.count).sum::<u32>(),
        1
    );
    assert_no_internal_origin_fields(&serde_json::to_value(overview).unwrap());
    assert_no_internal_origin_fields(&serde_json::to_value(timeline).unwrap());
    assert_no_internal_origin_fields(&serde_json::to_value(system).unwrap());
}

#[test]
fn genuine_v7_four_column_scope_migrates_to_v8_without_losing_current_history_or_reviving_retired_history(
) {
    let f = Fixture::new();
    let (db, seed) = seeded_genuine_v7(&f, false);
    let old_origins = raw_table(&db, "activity_origins");
    let old_scopes = four_scope_columns(&db);
    let old_resources = raw_table(&db, "resources");
    let old_activities = raw_table(&db, "activities");
    let old_workspaces = raw_table(&db, "workspaces");
    let old_links = raw_table(&db, "workspace_resources");
    let old_relinks = raw_table(&db, "resource_relinks");
    let old_settings = raw_table(&db, "settings");
    drop(db);
    let store = Store::open(&f.database).unwrap();
    // 真正执行共用权限SQL，修复前这里复现原生overview的缺列错误。
    assert_v7_history_visibility(&store, &seed);
    let db = store.connection().unwrap();
    assert_eq!(schema_version(&db), crate::migrations::VERSION);
    assert_eq!(raw_table(&db, "activity_origins"), old_origins);
    assert_eq!(four_scope_columns(&db), old_scopes);
    assert_eq!(raw_table(&db, "resources"), old_resources);
    assert_eq!(raw_table(&db, "activities"), old_activities);
    assert_eq!(raw_table(&db, "workspaces"), old_workspaces);
    assert_eq!(raw_table(&db, "workspace_resources"), old_links);
    assert_eq!(raw_table(&db, "resource_relinks"), old_relinks);
    assert_eq!(raw_table(&db, "settings"), old_settings);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM activity_root_scopes WHERE retired_location_valid<>0",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    let before_reopen = raw_table(&db, "activity_root_scopes");
    drop(db);
    drop(store);
    let reopened = Store::open(&f.database).unwrap();
    assert_v7_history_visibility(&reopened, &seed);
    assert_eq!(
        raw_table(&reopened.connection().unwrap(), "activity_root_scopes"),
        before_reopen
    );
    let resources = reopened
        .resources(ResourceQuery {
            workspace_id: Some(seed.workspace),
            kind: "file".into(),
            mode: "all".into(),
            offset: 0,
        })
        .unwrap();
    assert_eq!(resources.items.len(), 2);
    assert!(resources.items.iter().all(|item| item.favorite));
}

#[test]
fn genuine_v7_existing_retired_flags_keep_both_zero_and_one_while_advancing_to_v8() {
    let f = Fixture::new();
    let (mut db, seed) = seeded_genuine_v7(&f, true);
    let before = raw_table(&db, "activity_root_scopes");
    let origins = raw_table(&db, "activity_origins");
    assert_eq!(
        db.query_row(
            "SELECT count(DISTINCT retired_location_valid) FROM activity_root_scopes",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        2
    );
    crate::migrations::migrate(&mut db).unwrap();
    assert_eq!(schema_version(&db), crate::migrations::VERSION);
    assert_eq!(raw_table(&db, "activity_root_scopes"), before);
    assert_eq!(raw_table(&db, "activity_origins"), origins);
    crate::migrations::migrate(&mut db).unwrap();
    assert_eq!(raw_table(&db, "activity_root_scopes"), before);
    drop(db);
    let store = Store::open(&f.database).unwrap();
    assert_v7_history_visibility(&store, &seed);
}

#[test]
fn migrated_v7_scopes_keep_new_capture_default_and_allow_fresh_known_retirement_only() {
    let f = Fixture::new();
    let (db, seed) = seeded_genuine_v7(&f, false);
    drop(db);
    let store = Store::open(&f.database).unwrap();
    store.record_open(&seed.current.0).unwrap();
    let db = store.connection().unwrap();
    let fresh: String = db
        .query_row(
            "SELECT id FROM activities WHERE kind='opened' AND resource_id=?",
            [&seed.current.0],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        db.query_row(
            "SELECT retired_location_valid FROM activity_root_scopes WHERE activity_id=?",
            [&fresh],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT retired_location_valid FROM activity_root_scopes WHERE activity_id=?",
            [&seed.current.1],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    drop(db);
    let selected = f.root.join("fresh-known-retirement-target.txt");
    f.file(&selected);
    store.relink(&seed.current.0, &selected).unwrap();
    let db = store.connection().unwrap();
    for aid in [&seed.current.1, &fresh] {
        assert_eq!(
            db.query_row(
                "SELECT retired_location_valid FROM activity_root_scopes WHERE activity_id=?",
                [aid],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
            1
        );
    }
    assert_eq!(
        db.query_row(
            "SELECT retired_location_valid FROM activity_root_scopes WHERE activity_id=?",
            [&seed.retired.1],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    let observed: (Option<String>, Option<String>) = db
        .query_row(
            "SELECT observed_resource_id,identity FROM activity_origins WHERE activity_id=?",
            [&seed.current.1],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(observed, (Some(seed.current.0), Some(seed.current.2)));
    assert_eq!(
        db.query_row(
            "SELECT identity FROM resources WHERE id=?",
            [&seed.retired.0],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        seed.replacement_identity
    );
    drop(db);
    let visible = store.overview().unwrap();
    assert!(visible
        .activities
        .iter()
        .any(|activity| activity.id == seed.current.1));
    assert!(visible
        .activities
        .iter()
        .all(|activity| activity.id != seed.retired.1));
}

#[test]
fn genuine_v7_backup_restores_via_v8_staging_without_changing_source_or_reviving_unknown_scope() {
    let f = Fixture::new();
    let (db, seed) = seeded_genuine_v7(&f, false);
    let origins = raw_table(&db, "activity_origins");
    let scopes = four_scope_columns(&db);
    let links = raw_table(&db, "workspace_resources");
    let relinks = raw_table(&db, "resource_relinks");
    drop(db);
    let source_before = fs::read(&f.database).unwrap();
    let destination = f.home.join("app/restored.sqlite3");
    let store = Store::open(&destination).unwrap();
    mutate(
        &store,
        Mutation::CreateWorkspace {
            name: "live-before-restore".into(),
            resource_ids: None,
        },
    );
    let permission_before = store.overview().unwrap().permission_version;
    store.restore_from(&f.database).unwrap();
    assert_eq!(fs::read(&f.database).unwrap(), source_before);
    let db = store.connection().unwrap();
    assert_eq!(schema_version(&db), crate::migrations::VERSION);
    assert_eq!(raw_table(&db, "activity_origins"), origins);
    assert_eq!(four_scope_columns(&db), scopes);
    assert_eq!(raw_table(&db, "workspace_resources"), links);
    assert_eq!(raw_table(&db, "resource_relinks"), relinks);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM activity_root_scopes WHERE retired_location_valid<>0",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    drop(db);
    let restored = store.overview().unwrap();
    assert!(restored.permission_version > permission_before);
    assert!(restored.roots.iter().all(|root| !root.active));
    assert!(restored.activities.is_empty());
    let root_id = store.grant_root(&f.root).unwrap();
    assert_eq!(root_id, "v7-root");
    assert_v7_history_visibility(&store, &seed);
    let db = store.connection().unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM resources WHERE favorite=1",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        2
    );
    let grant_count: u32 = db
        .query_row(
            "SELECT count(*) FROM file_grants WHERE active=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(grant_count, 0);
    assert_eq!(fs::read(&f.database).unwrap(), source_before);
}

#[test]
fn genuine_v7_retired_flag_upgrade_rolls_back_alter_rows_and_version_then_retries() {
    let f = Fixture::new();
    let (mut db, _seed) = seeded_genuine_v7(&f, false);
    db.execute_batch("CREATE TRIGGER stop_unknown_retired_scope_update BEFORE UPDATE ON activity_root_scopes BEGIN SELECT RAISE(ABORT,'deterministic v8 migration failure'); END;").unwrap();
    let before_schema = schema(&db);
    let before_scopes = raw_table(&db, "activity_root_scopes");
    let before_origins = raw_table(&db, "activity_origins");
    let before_resources = raw_table(&db, "resources");
    let before_links = raw_table(&db, "workspace_resources");
    assert!(crate::migrations::migrate(&mut db).is_err());
    assert_eq!(schema_version(&db), 7);
    assert_eq!(schema(&db), before_schema);
    assert_eq!(raw_table(&db, "activity_root_scopes"), before_scopes);
    assert_eq!(raw_table(&db, "activity_origins"), before_origins);
    assert_eq!(raw_table(&db, "resources"), before_resources);
    assert_eq!(raw_table(&db, "workspace_resources"), before_links);
    assert!(db
        .prepare("SELECT retired_location_valid FROM activity_root_scopes LIMIT 0")
        .is_err());
    db.execute("DROP TRIGGER stop_unknown_retired_scope_update", [])
        .unwrap();
    crate::migrations::migrate(&mut db).unwrap();
    assert_eq!(schema_version(&db), crate::migrations::VERSION);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM activity_root_scopes WHERE retired_location_valid<>0",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(raw_table(&db, "activity_origins"), before_origins);
    assert_eq!(raw_table(&db, "resources"), before_resources);
    assert_eq!(raw_table(&db, "workspace_resources"), before_links);
}
