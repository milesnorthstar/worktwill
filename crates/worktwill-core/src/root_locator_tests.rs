#![cfg(target_os = "macos")]

use crate::{model::*, Store};
use rusqlite::{types::Value, Connection};
use std::{fs, path::PathBuf};

struct Fixture {
    _directory: tempfile::TempDir,
    home: PathBuf,
    root: PathBuf,
    database: PathBuf,
    store: Store,
}

impl Fixture {
    fn new() -> Self {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("root-locator-domain-")
            .tempdir_in(approved)
            .unwrap();
        let home = directory.path().canonicalize().unwrap();
        let root = home.join("documents");
        fs::create_dir_all(root.join("nested")).unwrap();
        let database = home.join("app/catalog.sqlite3");
        let store = Store::open(&database).unwrap();
        Self {
            _directory: directory,
            home,
            root,
            database,
            store,
        }
    }

    fn change(&self, mutation: Mutation) -> MutationResponse {
        self.store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation,
            })
            .unwrap()
    }

    fn seed(&self) -> (String, String, Vec<String>) {
        let file = self.root.join("nested/contract.txt");
        fs::write(&file, b"synthetic directory original").unwrap();
        let root_id = self.store.grant_root(&self.root).unwrap();
        self.store.scan_all().unwrap();
        let rid: String = self
            .store
            .connection()
            .unwrap()
            .query_row(
                "SELECT id FROM resources WHERE location=?",
                [file.to_string_lossy().as_ref()],
                |row| row.get(0),
            )
            .unwrap();
        let mut workspaces = Vec::new();
        for name in ["客户", "合同"] {
            workspaces.push(
                self.change(Mutation::CreateWorkspace {
                    name: name.into(),
                    resource_ids: Some(vec![rid.clone()]),
                })
                .id
                .unwrap(),
            );
        }
        self.change(Mutation::Favorite {
            id: rid.clone(),
            value: true,
        });
        (root_id, rid, workspaces)
    }

    fn root_state(&self, id: &str) -> Root {
        self.store
            .roots()
            .unwrap()
            .into_iter()
            .find(|root| root.id == id)
            .unwrap()
    }

    fn assert_reference(&self, rid: &str, location: &std::path::Path, workspaces: &[String]) {
        let resource = Store::resource_db(&self.store.connection().unwrap(), rid, false).unwrap();
        assert_eq!(resource.id, rid);
        assert_eq!(
            resource.location,
            location.canonicalize().unwrap().to_string_lossy()
        );
        assert!(resource.favorite);
        assert_eq!(resource.status, "available");
        let mut actual = resource.workspace_ids;
        let mut expected = workspaces.to_vec();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
        assert_eq!(
            self.store.resolve_resource(rid).unwrap().0,
            location.to_string_lossy()
        );
    }
}

fn count(store: &Store, table: &str) -> u32 {
    store
        .connection()
        .unwrap()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

#[test]
fn targeted_root_refresh_honors_pause_then_resume_without_scanning_other_roots() {
    let f = Fixture::new();
    let (root, rid, workspaces) = f.seed();
    let other = f.home.join("unrelated");
    fs::create_dir(&other).unwrap();
    fs::write(other.join("unrelated.txt"), b"unrelated synthetic fixture").unwrap();
    let other_id = f.store.grant_root(&other).unwrap();
    f.change(Mutation::SetPaused {
        id: root.clone(),
        value: true,
    });
    let moved = f.home.join("paused-moved");
    fs::rename(&f.root, &moved).unwrap();
    fs::write(
        moved.join("new.txt"),
        b"not yet authorized for automatic discovery while paused",
    )
    .unwrap();
    f.store.refresh_root_locations().unwrap();
    f.store.refresh_root(&root).unwrap();
    f.assert_reference(&rid, &moved.join("nested/contract.txt"), &workspaces);
    assert_eq!(count(&f.store, "resources"), 1);
    assert!(f.root_state(&root).paused);
    assert_eq!(f.root_state(&other_id).scan_state, "idle");
    f.change(Mutation::SetPaused {
        id: root.clone(),
        value: false,
    });
    f.store.refresh_root(&root).unwrap();
    assert_eq!(count(&f.store, "resources"), 2);
    assert_eq!(f.root_state(&root).scan_state, "completed");
    assert_eq!(f.root_state(&other_id).scan_state, "idle");
    f.assert_reference(&rid, &moved.join("nested/contract.txt"), &workspaces);
}

#[test]
fn targeted_refresh_cannot_restore_a_revoked_root() {
    let f = Fixture::new();
    let (root, _, _) = f.seed();
    f.change(Mutation::SetPaused {
        id: root.clone(),
        value: true,
    });
    f.change(Mutation::RevokeRoot { id: root.clone() });
    let before = snapshot(&f.store);
    assert_eq!(
        f.store.refresh_root(&root).unwrap_err().code,
        "not_authorized"
    );
    assert_eq!(snapshot(&f.store), before);
}

#[test]
fn restored_scope_can_refresh_known_references_before_the_full_root_loop() {
    let f = Fixture::new();
    let (root, rid, workspaces) = f.seed();
    let moved = f.home.join("moved-documents");
    fs::rename(&f.root, &moved).unwrap();
    let mut notified = false;
    f.store
        .scan_all_with_root_updates(|roots| {
            let restored = roots.iter().find(|candidate| candidate.id == root).unwrap();
            assert_eq!(restored.path, moved.to_string_lossy());
            assert_eq!(restored.scan_state, "idle");
            assert_eq!(restored.location_status, "available");
            let old: String = f
                .store
                .connection()
                .unwrap()
                .query_row("SELECT location FROM resources WHERE id=?", [&rid], |row| {
                    row.get(0)
                })
                .unwrap();
            assert!(old.starts_with(f.root.to_str().unwrap()));
            // 同步调用也不能持锁死锁；实际原生调度由固定工作任务并行执行。
            f.store.scan_root(&root).unwrap();
            f.assert_reference(&rid, &moved.join("nested/contract.txt"), &workspaces);
            notified = true;
        })
        .unwrap();
    assert!(notified);
    f.assert_reference(&rid, &moved.join("nested/contract.txt"), &workspaces);
}

fn snapshot(store: &Store) -> Vec<Vec<Vec<Value>>> {
    let db = store.connection().unwrap();
    let mut result = Vec::new();
    for table in [
        "settings",
        "resources",
        "scan_roots",
        "root_locators",
        "root_aliases",
        "file_roots",
        "file_grants",
        "file_locators",
        "workspaces",
        "workspace_resources",
        "activities",
        "operations",
        "search_documents",
    ] {
        let mut statement = db
            .prepare(&format!("SELECT * FROM {table} ORDER BY 1,2"))
            .unwrap();
        let width = statement.column_count();
        result.push(
            statement
                .query_map([], |row| {
                    (0..width)
                        .map(|index| row.get::<_, Value>(index))
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap(),
        );
    }
    result
}

#[test]
fn managed_directory_rename_move_and_reopen_keep_root_uuid_and_resource_relationships() {
    let mut f = Fixture::new();
    let (root_id, rid, workspaces) = f.seed();
    let original = f.root.join("nested/contract.txt");
    let bytes = fs::read(&original).unwrap();
    let modified = original.metadata().unwrap().modified().unwrap();
    let identity = crate::platform::identity(&f.root, &f.root.metadata().unwrap()).unwrap();
    let renamed = f.home.join("renamed");
    fs::rename(&f.root, &renamed).unwrap();
    f.store = Store::open(&f.database).unwrap();
    f.store.refresh_roots().unwrap();
    f.store.scan_all().unwrap();
    assert_eq!(f.root_state(&root_id).location_status, "available");
    assert_eq!(f.root_state(&root_id).path, renamed.to_string_lossy());
    f.assert_reference(&rid, &renamed.join("nested/contract.txt"), &workspaces);
    let destination = f.home.join("elsewhere/deep");
    fs::create_dir_all(&destination).unwrap();
    let moved = destination.join("renamed");
    fs::rename(&renamed, &moved).unwrap();
    f.store = Store::open(&f.database).unwrap();
    f.store.scan_all().unwrap();
    let file = moved.join("nested/contract.txt");
    f.assert_reference(&rid, &file, &workspaces);
    assert_eq!(f.root_state(&root_id).path, moved.to_string_lossy());
    assert!(identity == crate::platform::identity(&moved, &moved.metadata().unwrap()).unwrap());
    assert_eq!(fs::read(&file).unwrap(), bytes);
    assert_eq!(file.metadata().unwrap().modified().unwrap(), modified);
    assert_eq!(f.store.roots().unwrap().len(), 1);
    assert_eq!(count(&f.store, "resources"), 1);
    assert!(f.store.file_grants().unwrap().is_empty());
    assert_eq!(count(&f.store, "file_locators"), 0);
}

#[test]
fn paused_root_recovers_known_paths_but_does_not_enumerate_new_files_until_resumed() {
    let mut f = Fixture::new();
    let (root_id, rid, workspaces) = f.seed();
    f.change(Mutation::SetPaused {
        id: root_id.clone(),
        value: true,
    });
    fs::write(f.root.join("not-yet-indexed.txt"), b"new while paused").unwrap();
    let moved = f.home.join("paused-moved");
    fs::rename(&f.root, &moved).unwrap();
    f.store = Store::open(&f.database).unwrap();
    f.store.refresh_roots().unwrap();
    f.store.scan_all().unwrap();
    assert!(f.root_state(&root_id).paused);
    assert_eq!(f.root_state(&root_id).location_status, "available");
    f.assert_reference(&rid, &moved.join("nested/contract.txt"), &workspaces);
    assert_eq!(count(&f.store, "resources"), 1);
    f.change(Mutation::SetPaused {
        id: root_id.clone(),
        value: false,
    });
    f.store.scan_all().unwrap();
    assert_eq!(count(&f.store, "resources"), 2);
    f.assert_reference(&rid, &moved.join("nested/contract.txt"), &workspaces);
    assert_eq!(
        fs::read(moved.join("not-yet-indexed.txt")).unwrap(),
        b"new while paused"
    );
}

#[test]
fn consecutive_directory_moves_and_explicit_regrant_reuse_physical_root_identity() {
    let f = Fixture::new();
    let (root_id, rid, workspaces) = f.seed();
    let first = f.home.join("first");
    let second = f.home.join("second");
    let third = f.home.join("third");
    fs::rename(&f.root, &first).unwrap();
    fs::rename(&first, &second).unwrap();
    f.store.refresh_roots().unwrap();
    assert_eq!(f.store.grant_root(&second).unwrap(), root_id);
    fs::rename(&second, &third).unwrap();
    f.store.refresh_roots().unwrap();
    f.store.scan_all().unwrap();
    assert_eq!(f.store.grant_root(&third).unwrap(), root_id);
    assert_eq!(f.store.roots().unwrap().len(), 1);
    assert_eq!(count(&f.store, "root_locators"), 1);
    let aliases: u32 = f
        .store
        .connection()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM root_aliases WHERE root_id=?",
            [&root_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(aliases >= 2);
    f.assert_reference(&rid, &third.join("nested/contract.txt"), &workspaces);
}

#[test]
fn relative_exclusions_and_revocation_remain_effective_after_root_movement() {
    let f = Fixture::new();
    let (root_id, rid, workspaces) = f.seed();
    fs::create_dir(f.root.join("secret")).unwrap();
    let secret = f.root.join("secret/private.txt");
    fs::write(&secret, b"excluded private fixture").unwrap();
    f.store.scan_all().unwrap();
    let secret_id: String = f
        .store
        .connection()
        .unwrap()
        .query_row(
            "SELECT id FROM resources WHERE location=?",
            [secret.to_string_lossy().as_ref()],
            |row| row.get(0),
        )
        .unwrap();
    f.change(Mutation::Link {
        workspace_id: workspaces[0].clone(),
        resource_ids: vec![secret_id.clone()],
    });
    f.change(Mutation::Exclude {
        root_id: root_id.clone(),
        relative_path: "secret".into(),
    });
    let moved = f.home.join("excluded-moved");
    fs::rename(&f.root, &moved).unwrap();
    f.store.refresh_roots().unwrap();
    f.store.scan_all().unwrap();
    f.assert_reference(&rid, &moved.join("nested/contract.txt"), &workspaces);
    assert!(f.root_state(&root_id).exclusions.contains(&"secret".into()));
    assert!(f.store.resolve_resource(&secret_id).is_err());
    assert!(f
        .store
        .needs_file_grant(&moved.join("secret/private.txt"))
        .unwrap());
    let hidden = Store::resource_db(&f.store.connection().unwrap(), &secret_id, true).unwrap();
    assert_eq!(hidden.status, "permission_denied");
    assert!(hidden.location.is_empty());

    f.change(Mutation::RevokeRoot {
        id: root_id.clone(),
    });
    let revoked = f.home.join("revoked-moved");
    fs::rename(&moved, &revoked).unwrap();
    f.store.refresh_roots().unwrap();
    f.store.scan_all().unwrap();
    assert!(!f.root_state(&root_id).active);
    assert!(f.store.resolve_resource(&rid).is_err());
    assert!(f
        .store
        .needs_file_grant(&revoked.join("nested/contract.txt"))
        .unwrap());
    assert_eq!(f.store.grant_root(&revoked).unwrap(), root_id);
    f.store.scan_all().unwrap();
    f.assert_reference(&rid, &revoked.join("nested/contract.txt"), &workspaces);
    assert!(f.store.resolve_resource(&secret_id).is_err());
}

#[test]
fn replacement_at_old_directory_path_is_not_authorized_or_given_old_references() {
    let f = Fixture::new();
    let (root_id, rid, workspaces) = f.seed();
    let actual = f.home.join("actual-original");
    fs::rename(&f.root, &actual).unwrap();
    fs::create_dir_all(f.root.join("nested")).unwrap();
    let unrelated = f.root.join("nested/contract.txt");
    fs::write(&unrelated, b"unrelated same-name replacement").unwrap();
    let permission = f.store.overview().unwrap().permission_version;
    match f.store.refresh_roots() {
        Ok(()) => {
            assert_eq!(f.root_state(&root_id).location_status, "available");
            assert_eq!(f.root_state(&root_id).path, actual.to_string_lossy());
        }
        Err(error) => {
            // 普通Bookmark可能回退到已被占用的旧路径；身份拒绝后必须明确重选原目录。
            assert_eq!(error.code, "root_unavailable");
            assert_eq!(f.root_state(&root_id).location_status, "unavailable");
            assert!(f.store.overview().unwrap().permission_version > permission);
            let db = f.store.connection().unwrap();
            assert_eq!(
                db.query_row(
                    "SELECT valid FROM root_locators WHERE root_id=?",
                    [&root_id],
                    |row| row.get::<_, u32>(0)
                )
                .unwrap(),
                0
            );
            let hidden = Store::resource_db(&db, &rid, true).unwrap();
            assert_eq!(hidden.status, "permission_denied");
            assert!(hidden.location.is_empty());
            assert!(f.store.resolve_resource(&rid).is_err());
            assert!(f.store.needs_file_grant(&unrelated).unwrap());
            assert_eq!(count(&f.store, "resources"), 1);
            assert_eq!(f.store.grant_root(&actual).unwrap(), root_id);
        }
    }
    f.store.scan_all().unwrap();
    f.assert_reference(&rid, &actual.join("nested/contract.txt"), &workspaces);
    assert_eq!(f.root_state(&root_id).path, actual.to_string_lossy());
    assert!(f.store.needs_file_grant(&unrelated).unwrap());
    assert_eq!(count(&f.store, "resources"), 1);
    let new_root = f.store.grant_root(&f.root).unwrap();
    assert_ne!(new_root, root_id);
    f.store.scan_all().unwrap();
    let new = f
        .store
        .resources(ResourceQuery {
            workspace_id: None,
            kind: "file".into(),
            mode: "modified".into(),
            offset: 0,
        })
        .unwrap()
        .items
        .into_iter()
        .find(|resource| resource.id != rid)
        .unwrap();
    assert!(new.workspace_ids.is_empty() && !new.favorite);
    f.assert_reference(&rid, &actual.join("nested/contract.txt"), &workspaces);
    assert_eq!(
        fs::read(unrelated).unwrap(),
        b"unrelated same-name replacement"
    );
}

#[test]
fn vanished_root_recreated_at_same_name_fails_closed_without_identity_inheritance() {
    let f = Fixture::new();
    let (root_id, rid, _) = f.seed();
    let held_directory = fs::File::open(&f.root).unwrap();
    fs::remove_dir_all(&f.root).unwrap();
    fs::create_dir_all(f.root.join("nested")).unwrap();
    let replacement = f.root.join("nested/contract.txt");
    fs::write(&replacement, b"different directory and file identity").unwrap();
    // 定位失败是否汇总为Err由调用方定义；无论返回形式，授权必须关闭。
    let _result = f.store.refresh_roots();
    assert_eq!(f.root_state(&root_id).location_status, "unavailable");
    assert_eq!(
        f.store
            .connection()
            .unwrap()
            .query_row(
                "SELECT valid FROM root_locators WHERE root_id=?",
                [&root_id],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
    assert!(f.store.needs_file_grant(&replacement).unwrap());
    assert!(f.store.resolve_resource(&rid).is_err());
    assert_eq!(count(&f.store, "resources"), 1);
    assert_eq!(
        fs::read(&replacement).unwrap(),
        b"different directory and file identity"
    );
    drop(held_directory);
}

#[test]
fn root_and_descendant_symlinks_cannot_expand_directory_authorization() {
    let f = Fixture::new();
    let (root_id, rid, workspaces) = f.seed();
    let actual = f.home.join("actual-root");
    fs::rename(&f.root, &actual).unwrap();
    let foreign = f.home.join("foreign");
    fs::create_dir(&foreign).unwrap();
    fs::write(foreign.join("private.txt"), b"foreign private fixture").unwrap();
    std::os::unix::fs::symlink(&foreign, &f.root).unwrap();
    f.store.refresh_roots().unwrap();
    f.store.scan_all().unwrap();
    assert_eq!(f.root_state(&root_id).path, actual.to_string_lossy());
    f.assert_reference(&rid, &actual.join("nested/contract.txt"), &workspaces);
    assert!(f
        .store
        .needs_file_grant(&foreign.join("private.txt"))
        .unwrap());
    assert_eq!(count(&f.store, "resources"), 1);

    f.change(Mutation::SetPaused {
        id: root_id,
        value: true,
    });
    let escaped = f.home.join("escaped-nested");
    fs::rename(actual.join("nested"), &escaped).unwrap();
    std::os::unix::fs::symlink(&escaped, actual.join("nested")).unwrap();
    f.store.refresh_roots().unwrap();
    assert!(f.store.resolve_resource(&rid).is_err());
    assert!(f
        .store
        .needs_file_grant(&escaped.join("contract.txt"))
        .unwrap());
    assert_eq!(
        fs::read(escaped.join("contract.txt")).unwrap(),
        b"synthetic directory original"
    );
    assert_eq!(count(&f.store, "resources"), 1);
}

fn genuine_v5(db: &Connection) {
    // 历史v1逐段构造到v5，绝不把当前新库降版本号伪装成旧库。
    db.execute_batch(include_str!("schema.sql")).unwrap();
    db.execute_batch("ALTER TABLE activities ADD COLUMN source TEXT NOT NULL DEFAULT 'app'; ALTER TABLE activities ADD COLUMN file_modified_at REAL; CREATE INDEX activities_time ON activities(at DESC,id); CREATE TABLE file_grants(resource_id TEXT PRIMARY KEY REFERENCES resources(id),active INTEGER NOT NULL DEFAULT 1); PRAGMA user_version=2;").unwrap();
    db.execute_batch("ALTER TABLE workspaces ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0; PRAGMA user_version=3;").unwrap();
    db.execute_batch("ALTER TABLE search_documents ADD COLUMN modified_at REAL; CREATE INDEX search_documents_modified ON search_documents(modified_at DESC,name,entity_id); CREATE INDEX search_documents_kind_modified ON search_documents(kind,modified_at DESC,name,entity_id); PRAGMA user_version=4;").unwrap();
    db.execute_batch("CREATE TABLE file_locators(resource_id TEXT PRIMARY KEY REFERENCES resources(id) ON DELETE CASCADE,identity TEXT NOT NULL,bookmark BLOB NOT NULL); PRAGMA user_version=5;").unwrap();
}

#[test]
fn historical_v5_roots_are_legacy_and_not_retroactively_assigned_bookmarks() {
    let f = Fixture::new();
    let legacy = f.home.join("legacy.sqlite3");
    {
        let db = Connection::open(&legacy).unwrap();
        genuine_v5(&db);
        db.execute(
            "INSERT INTO scan_roots(id,path,active) VALUES('legacy-root',?1,1)",
            [f.root.to_string_lossy().as_ref()],
        )
        .unwrap();
    }
    let migrated = Store::open(&legacy).unwrap();
    assert_eq!(
        migrated
            .connection()
            .unwrap()
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        crate::migrations::VERSION
    );
    assert_eq!(migrated.roots().unwrap()[0].location_status, "legacy");
    assert_eq!(count(&migrated, "root_locators"), 0);
    migrated.refresh_roots().unwrap();
    assert_eq!(count(&migrated, "root_locators"), 0);
    assert_eq!(migrated.roots().unwrap()[0].location_status, "legacy");
    migrated.grant_root(&f.root).unwrap();
    assert_eq!(count(&migrated, "root_locators"), 1);
    assert!(migrated
        .roots()
        .unwrap()
        .iter()
        .any(|root| root.location_status == "available"));
}

#[test]
fn malformed_schema6_root_locator_backups_never_replace_live_state() {
    let f = Fixture::new();
    let (_root_id, rid, workspaces) = f.seed();
    let backup = f.home.join("valid.sqlite3");
    f.store.backup_to(&backup).unwrap();
    let cases = [
        "DROP TABLE root_locators;",
        "DROP TABLE root_aliases;",
        "DROP TABLE root_locators; CREATE TABLE root_locators(root_id TEXT PRIMARY KEY,bookmark BLOB,valid INTEGER);",
        "DROP TABLE root_locators; CREATE TABLE root_locators(root_id TEXT PRIMARY KEY,identity TEXT,valid INTEGER);",
        "DROP TABLE root_locators; CREATE TABLE root_locators(root_id TEXT PRIMARY KEY,identity TEXT,bookmark BLOB);",
        "DROP TABLE root_aliases; CREATE TABLE root_aliases(root_id TEXT);",
        "DROP TABLE root_aliases; CREATE TABLE root_aliases(path TEXT);",
    ];
    for (index, corruption) in cases.iter().enumerate() {
        let corrupt = f.home.join(format!("corrupt-{index}.sqlite3"));
        fs::copy(&backup, &corrupt).unwrap();
        let source = Connection::open(&corrupt).unwrap();
        source.execute_batch(corruption).unwrap();
        drop(source);
        let before = snapshot(&f.store);
        assert!(f.store.restore_from(&corrupt).is_err());
        assert!(
            before == snapshot(&f.store),
            "invalid root locator backup changed live data"
        );
        f.assert_reference(&rid, &f.root.join("nested/contract.txt"), &workspaces);
    }
}

#[test]
fn schema6_migration_conflict_rolls_back_new_tables_and_retries_without_retroactive_trust() {
    let f = Fixture::new();
    let mut db = Connection::open_in_memory().unwrap();
    genuine_v5(&db);
    db.execute(
        "INSERT INTO scan_roots(id,path,active) VALUES('legacy-root',?1,1)",
        [f.root.to_string_lossy().as_ref()],
    )
    .unwrap();
    // 第二张表冲突发生在root_locators及索引创建后，验证整段DDL原子回滚。
    db.execute("CREATE TABLE root_aliases(conflict INTEGER)", [])
        .unwrap();
    assert!(crate::migrations::migrate(&mut db).is_err());
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        5
    );
    for name in ["root_locators", "root_locators_identity"] {
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM sqlite_master WHERE name=?",
                [name],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
            0
        );
    }
    assert_eq!(
        db.query_row(
            "SELECT active FROM scan_roots WHERE id='legacy-root'",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
    db.execute("DROP TABLE root_aliases", []).unwrap();
    crate::migrations::migrate(&mut db).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        crate::migrations::VERSION
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM root_locators", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM root_aliases", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(Store::roots_db(&db).unwrap()[0].location_status, "legacy");
}

#[test]
fn broken_single_file_bookmark_does_not_prevent_moved_root_recovery_and_scan() {
    let f = Fixture::new();
    let (root_id, rid, workspaces) = f.seed();
    let single = f.home.join("individual.txt");
    fs::write(&single, b"independent single file fixture").unwrap();
    let grant = f
        .store
        .import_files(&[single.to_string_lossy().into_owned()], None, true)
        .unwrap();
    let single_id = grant.items[0].resource_id.clone().unwrap();
    let single_moved = f.home.join("individual-moved.txt");
    fs::rename(&single, &single_moved).unwrap();
    f.store
        .connection()
        .unwrap()
        .execute(
            "UPDATE file_locators SET bookmark=X'00' WHERE resource_id=?",
            [&single_id],
        )
        .unwrap();
    let moved = f.home.join("root-moved");
    fs::rename(&f.root, &moved).unwrap();
    assert_eq!(
        f.store.scan_all().unwrap_err().code,
        "bookmark_resolve_failed"
    );
    assert_eq!(f.root_state(&root_id).location_status, "available");
    assert_eq!(f.root_state(&root_id).scan_state, "completed");
    assert_eq!(f.root_state(&root_id).path, moved.to_string_lossy());
    f.assert_reference(&rid, &moved.join("nested/contract.txt"), &workspaces);
    let unavailable =
        Store::resource_db(&f.store.connection().unwrap(), &single_id, false).unwrap();
    assert_eq!(unavailable.status, "missing");
    assert_eq!(count(&f.store, "resources"), 2);
    assert_eq!(
        fs::read(single_moved).unwrap(),
        b"independent single file fixture"
    );
}

#[test]
fn paused_mapping_recovers_every_known_resource_across_more_than_two_batches() {
    let mut f = Fixture::new();
    for index in 0..205 {
        fs::write(
            f.root.join("nested").join(format!("item-{index:03}.txt")),
            format!("synthetic paused fixture {index}"),
        )
        .unwrap();
    }
    let root_id = f.store.grant_root(&f.root).unwrap();
    f.store.scan_all().unwrap();
    let expected = f
        .store
        .connection()
        .unwrap()
        .prepare("SELECT id,location,identity,modified_at,size FROM resources ORDER BY id")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(expected.len(), 205);
    let workspace = f
        .change(Mutation::CreateWorkspace {
            name: "全部已知资料".into(),
            resource_ids: Some(expected.iter().map(|row| row.0.clone()).collect()),
        })
        .id
        .unwrap();
    let favorite = expected[100].0.clone();
    f.change(Mutation::Favorite {
        id: favorite.clone(),
        value: true,
    });
    f.change(Mutation::SetPaused {
        id: root_id.clone(),
        value: true,
    });
    fs::write(
        f.root.join("new-root-file.txt"),
        b"new root file while paused",
    )
    .unwrap();
    fs::write(
        f.root.join("nested/new-nested-file.txt"),
        b"new nested file while paused",
    )
    .unwrap();
    let moved = f.home.join("large-paused-moved");
    fs::rename(&f.root, &moved).unwrap();
    f.store = Store::open(&f.database).unwrap();
    f.store.refresh_roots().unwrap();
    assert!(f.root_state(&root_id).paused);
    assert_eq!(count(&f.store, "resources"), 205);
    let db = f.store.connection().unwrap();
    // 先查持久路径，不调用resolve补救，避免掩盖100条批次边界的遗漏。
    for (rid, previous, identity, modified, size) in &expected {
        let relative = std::path::Path::new(previous)
            .strip_prefix(&f.root)
            .unwrap();
        let current = moved.join(relative);
        let row = db
            .query_row(
                "SELECT location,identity,modified_at,size FROM resources WHERE id=?",
                [rid],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, f64>(2)?,
                        row.get::<_, f64>(3)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(row.0, current.to_string_lossy());
        assert_eq!(&row.1, identity);
        assert_eq!((row.2, row.3), (*modified, *size));
        let resource = Store::resource_db(&db, rid, false).unwrap();
        assert_eq!(resource.workspace_ids, vec![workspace.clone()]);
        assert_eq!(resource.favorite, *rid == favorite);
        let filename = current.file_stem().unwrap().to_str().unwrap();
        let index: usize = filename.strip_prefix("item-").unwrap().parse().unwrap();
        assert_eq!(
            fs::read(&current).unwrap(),
            format!("synthetic paused fixture {index}").as_bytes()
        );
    }
    drop(db);
    f.store.scan_all().unwrap();
    assert_eq!(count(&f.store, "resources"), 205);
    assert_eq!(count(&f.store, "file_locators"), 0);
    f.change(Mutation::SetPaused {
        id: root_id,
        value: false,
    });
    f.store.scan_all().unwrap();
    assert_eq!(count(&f.store, "resources"), 207);
}

#[test]
fn unavailable_mixed_root_and_file_grant_preserves_references_and_scans_healthy_root() {
    let f = Fixture::new();
    let original = f.root.join("nested/contract.txt");
    fs::write(&original, b"mixed grant synthetic file").unwrap();
    // 先明确授权单文件，再接管目录；公共API保留两种真实授权关系。
    let imported = f
        .store
        .import_files(&[original.to_string_lossy().into_owned()], None, true)
        .unwrap();
    let rid = imported.items[0].resource_id.clone().unwrap();
    let (root_id, same_rid, workspaces) = f.seed();
    assert_eq!(rid, same_rid);
    assert_eq!(count(&f.store, "file_grants"), 1);
    assert_eq!(count(&f.store, "file_locators"), 1);

    let healthy = f.home.join("healthy-root");
    fs::create_dir(&healthy).unwrap();
    let healthy_id = f.store.grant_root(&healthy).unwrap();
    f.store.scan_all().unwrap();
    let added = healthy.join("new-in-healthy.txt");
    fs::write(&added, b"healthy new fixture").unwrap();
    f.store
        .connection()
        .unwrap()
        .execute(
            "UPDATE file_locators SET bookmark=X'00' WHERE resource_id=?",
            [&rid],
        )
        .unwrap();
    fs::remove_dir_all(&f.root).unwrap();
    let error = f.store.scan_all().unwrap_err();
    assert!(crate::root_locator::is_root_error(&error));
    assert_eq!(f.root_state(&root_id).location_status, "unavailable");
    assert_eq!(f.root_state(&healthy_id).scan_state, "completed");
    let db = f.store.connection().unwrap();
    let reference = Store::resource_db(&db, &rid, false).unwrap();
    let mut actual = reference.workspace_ids;
    let mut expected = workspaces;
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    assert!(reference.favorite);
    assert_eq!(reference.status, "missing");
    assert_eq!(count(&f.store, "resources"), 2);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM resources WHERE location=? AND status='available'",
            [added.to_string_lossy().as_ref()],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(fs::read(added).unwrap(), b"healthy new fixture");
}

fn startup_search_request() -> SearchPageRequest {
    SearchPageRequest {
        request: SearchRequest {
            request_id: 1,
            query: "contract".into(),
            workspace_id: None,
            kind: "file".into(),
        },
        offset: 0,
        sort: "name".into(),
        cursor: None,
        modified_from: None,
        modified_to: None,
        path_prefix: None,
    }
}

fn add_startup_search_page(f: &Fixture) {
    for index in 0..100 {
        fs::write(
            f.root
                .join("nested")
                .join(format!("contract-search-{index:03}.txt")),
            b"startup synthetic fixture",
        )
        .unwrap();
    }
    f.store.scan_all().unwrap();
}

fn root_generation(store: &Store, root_id: &str) -> i64 {
    store
        .connection()
        .unwrap()
        .query_row(
            "SELECT generation FROM scan_roots WHERE id=?",
            [root_id],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn opening_unchanged_root_preserves_versions_and_search_projection_without_scanning() {
    let mut f = Fixture::new();
    let (root_id, rid, workspaces) = f.seed();
    add_startup_search_page(&f);
    let before = f.store.overview().unwrap();
    let generation = root_generation(&f.store, &root_id);
    let persisted = snapshot(&f.store);
    let mut request = startup_search_request();
    let first = f.store.search_page(request.clone()).unwrap();
    assert_eq!(first.page.items.len(), 100);
    let mut cursor = first.next_cursor.unwrap();
    // 未扫描的额外文件不能由启动枚举带入索引。
    fs::write(
        f.root.join("contract-not-enumerated.txt"),
        b"unscanned fixture",
    )
    .unwrap();
    f.store = Store::open(&f.database).unwrap();
    let reopened = f.store.overview().unwrap();
    assert_eq!(reopened.permission_version, before.permission_version);
    assert_eq!(reopened.revision, before.revision);
    assert_eq!(root_generation(&f.store, &root_id), generation);
    assert!(persisted == snapshot(&f.store));
    assert_eq!(f.root_state(&root_id).location_status, "available");
    assert_eq!(
        f.store
            .search_page(request.clone())
            .unwrap()
            .page
            .items
            .len(),
        100
    );
    // 新会话有独立session；同步它后验证未变化的权限/数据版本仍可续页。
    cursor.session = reopened.session;
    request.offset = cursor.next_offset;
    request.cursor = Some(cursor);
    let next = f.store.search_page(request).unwrap();
    assert_eq!(next.page.items.len(), 1);
    assert!(next.next_cursor.is_none());
    assert_eq!(count(&f.store, "resources"), 101);
    f.assert_reference(&rid, &f.root.join("nested/contract.txt"), &workspaces);
}

#[test]
fn opening_moved_or_replaced_root_only_invalidates_metadata_until_explicit_refresh() {
    for replacement in [false, true] {
        let mut f = Fixture::new();
        let (root_id, rid, workspaces) = f.seed();
        add_startup_search_page(&f);
        let before = f.store.overview().unwrap();
        let generation = root_generation(&f.store, &root_id);
        let mut request = startup_search_request();
        let mut cursor = f
            .store
            .search_page(request.clone())
            .unwrap()
            .next_cursor
            .unwrap();
        let moved = f.home.join("moved-before-startup");
        fs::rename(&f.root, &moved).unwrap();
        if replacement {
            fs::create_dir_all(f.root.join("nested")).unwrap();
            fs::write(f.root.join("nested/contract.txt"), b"untrusted replacement").unwrap();
        }
        f.store = Store::open(&f.database).unwrap();
        let reopened = f.store.overview().unwrap();
        assert_eq!(reopened.permission_version, before.permission_version + 1);
        assert_eq!(reopened.revision, before.revision + 1);
        assert_eq!(root_generation(&f.store, &root_id), generation + 1);
        assert_eq!(f.root_state(&root_id).location_status, "unavailable");
        assert_eq!(f.root_state(&root_id).path, f.root.to_string_lossy());
        assert_eq!(count(&f.store, "root_aliases"), 0);
        assert_eq!(count(&f.store, "resources"), 101);
        let db = f.store.connection().unwrap();
        assert_eq!(
            db.query_row(
                "SELECT valid FROM root_locators WHERE root_id=?",
                [&root_id],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
            0
        );
        let hidden = Store::resource_db(&db, &rid, true).unwrap();
        assert_eq!(hidden.status, "permission_denied");
        assert!(hidden.location.is_empty());
        // 排除会话变更因素，明确证明启动的p/r更新使旧搜索游标失效。
        cursor.session = reopened.session;
        request.offset = cursor.next_offset;
        request.cursor = Some(cursor);
        assert_eq!(
            f.store.search_page(request).unwrap_err().code,
            "stale_cursor"
        );
        drop(db);
        match f.store.refresh_roots() {
            Ok(()) => {
                assert_eq!(f.root_state(&root_id).path, moved.to_string_lossy());
                assert_eq!(f.root_state(&root_id).location_status, "available");
            }
            Err(error) => {
                // 被占用旧路径的Bookmark可能拒绝恢复，必须显式重选原身份。
                assert!(replacement);
                assert_eq!(error.code, "root_unavailable");
                assert_eq!(f.store.grant_root(&moved).unwrap(), root_id);
            }
        }
        f.store.scan_all().unwrap();
        f.assert_reference(&rid, &moved.join("nested/contract.txt"), &workspaces);
    }
}

#[test]
fn explicit_regrant_of_different_identity_clears_old_directory_aliases() {
    let f = Fixture::new();
    let (root_id, rid, workspaces) = f.seed();
    let renamed = f.home.join("renamed-with-alias");
    fs::rename(&f.root, &renamed).unwrap();
    assert_eq!(f.store.grant_root(&renamed).unwrap(), root_id);
    f.store.scan_all().unwrap();
    assert_eq!(count(&f.store, "root_aliases"), 1);
    let old_identity: String = f
        .store
        .connection()
        .unwrap()
        .query_row(
            "SELECT identity FROM root_locators WHERE root_id=?",
            [&root_id],
            |row| row.get(0),
        )
        .unwrap();
    let old_original = f.home.join("old-original-retained");
    fs::rename(&renamed, &old_original).unwrap();
    fs::create_dir_all(renamed.join("nested")).unwrap();
    fs::write(
        renamed.join("nested/contract.txt"),
        b"new directory fixture",
    )
    .unwrap();
    assert_eq!(f.store.grant_root(&renamed).unwrap(), root_id);
    let db = f.store.connection().unwrap();
    let identity: String = db
        .query_row(
            "SELECT identity FROM root_locators WHERE root_id=?",
            [&root_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_ne!(identity, old_identity);
    assert_eq!(count(&f.store, "root_aliases"), 0);
    assert_eq!(f.root_state(&root_id).location_status, "available");
    assert!(f.store.resolve_resource(&rid).is_err());
    let reference = Store::resource_db(&db, &rid, false).unwrap();
    let mut actual = reference.workspace_ids;
    let mut expected = workspaces;
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    assert!(reference.favorite);
    assert_eq!(count(&f.store, "resources"), 1);
}

#[test]
fn explicit_regrant_conflicting_existing_root_identity_and_path_is_atomic() {
    let f = Fixture::new();
    let (first_id, _, _) = f.seed();
    let second = f.home.join("second-existing-root");
    fs::create_dir(&second).unwrap();
    let second_id = f.store.grant_root(&second).unwrap();
    assert_ne!(first_id, second_id);
    let retained = f.home.join("first-root-retained");
    fs::rename(&f.root, &retained).unwrap();
    fs::rename(&second, &f.root).unwrap();
    let before = snapshot(&f.store);
    assert_eq!(f.store.grant_root(&f.root).unwrap_err().code, "conflict");
    assert!(before == snapshot(&f.store));
    assert_eq!(f.root_state(&first_id).path, f.root.to_string_lossy());
    assert_eq!(f.root_state(&second_id).path, second.to_string_lossy());
    assert_eq!(count(&f.store, "root_aliases"), 0);
}
