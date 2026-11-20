#![cfg(target_os = "macos")]

use crate::{file_locator::FileRecovery, model::*, Store};
use rusqlite::{params, types::Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

struct Fixture {
    _directory: tempfile::TempDir,
    home: PathBuf,
    database: PathBuf,
    store: Store,
}

impl Fixture {
    fn new() -> Self {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("file-locator-domain-")
            .tempdir_in(approved)
            .unwrap();
        let home = directory.path().canonicalize().unwrap();
        let database = home.join("app/catalog.sqlite3");
        let store = Store::open(&database).unwrap();
        Self {
            _directory: directory,
            home,
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

    fn import(&self, path: &Path, bytes: &[u8]) -> String {
        fs::write(path, bytes).unwrap();
        let output = self
            .store
            .import_files(&[path.to_string_lossy().into_owned()], None, true)
            .unwrap();
        assert_eq!(output.items[0].status, "added");
        output.items[0].resource_id.clone().unwrap()
    }

    fn linked_file(&self) -> (PathBuf, String, Vec<String>) {
        let path = self.home.join("合同 original.txt");
        let rid = self.import(&path, b"synthetic original document");
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
        (path, rid, workspaces)
    }

    fn assert_reference(&self, rid: &str, path: &Path, workspaces: &[String]) {
        let db = self.store.connection().unwrap();
        let resource = Store::resource_db(&db, rid, false).unwrap();
        assert_eq!(resource.id, rid);
        assert_eq!(
            resource.location,
            path.canonicalize().unwrap().to_string_lossy()
        );
        assert!(resource.favorite);
        let mut actual = resource.workspace_ids;
        let mut expected = workspaces.to_vec();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
        assert_eq!(resource.status, "available");
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

fn locator(store: &Store, rid: &str) -> (String, Vec<u8>) {
    store
        .connection()
        .unwrap()
        .query_row(
            "SELECT identity,bookmark FROM file_locators WHERE resource_id=?",
            [rid],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

fn assert_locator_points_to(store: &Store, rid: &str, path: &Path) {
    let (identity, bookmark) = locator(store, rid);
    assert!(!bookmark.is_empty());
    assert!(identity == crate::platform::identity(path, &path.metadata().unwrap()).unwrap());
    let (resolved, _) = crate::macos_bookmark::resolve(&bookmark).unwrap();
    assert!(resolved.canonicalize().unwrap() == path.canonicalize().unwrap());
}

fn recovery_snapshot(store: &Store, rid: &str, candidate: &Path) -> FileRecovery {
    let mut connection = store.connection().unwrap();
    let tx = connection.transaction().unwrap();
    let mut recovery = tx.query_row("SELECT r.identity,r.location,l.bookmark FROM resources r JOIN file_locators l ON l.resource_id=r.id WHERE r.id=?", [rid], |row| Ok(FileRecovery {
        resource_id: rid.into(), identity: row.get(0)?, location: row.get(1)?, bookmark: row.get(2)?, refreshed_bookmark: Vec::new(), permission_version: 0, revision: 0,
    })).unwrap();
    recovery.permission_version = crate::store::version(&tx, "permission_version").unwrap();
    recovery.revision = crate::store::version(&tx, "revision").unwrap();
    drop(tx);
    recovery.refreshed_bookmark = crate::macos_bookmark::create(candidate).unwrap();
    recovery
}

fn logical_snapshot(store: &Store) -> Vec<Vec<Vec<Value>>> {
    let db = store.connection().unwrap();
    let mut result = Vec::new();
    for table in [
        "settings",
        "resources",
        "scan_roots",
        "file_roots",
        "file_grants",
        "file_locators",
        "workspaces",
        "workspace_resources",
        "activities",
        "operations",
        "search_documents",
        "search_tri",
        "search_short",
    ] {
        let sql = match table {
            "search_tri" => "SELECT rowid,text FROM search_tri ORDER BY 1,2".into(),
            "search_short" => "SELECT rowid,tokens FROM search_short ORDER BY 1,2".into(),
            _ => format!("SELECT * FROM {table} ORDER BY 1,2"),
        };
        let mut statement = db.prepare(&sql).unwrap();
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

fn reject_recovery_without_writes(store: &Store, recovery: &FileRecovery, candidate: &Path) {
    let before = logical_snapshot(store);
    let result = store.import_one_checked(
        candidate,
        None,
        false,
        Some(&recovery.identity),
        Some(recovery),
    );
    assert_eq!(result.unwrap_err().code, "permission_changed");
    // 不通过assert_eq输出私有Bookmark字节或逻辑快照。
    assert!(
        before == logical_snapshot(store),
        "rejected recovery changed persistent state"
    );
}

#[test]
fn single_file_rename_move_reopen_and_scan_keep_uuid_references_and_original_bytes() {
    let mut f = Fixture::new();
    let (original, rid, workspaces) = f.linked_file();
    let bytes = fs::read(&original).unwrap();
    let modified = original.metadata().unwrap().modified().unwrap();
    let identity = crate::platform::identity(&original, &original.metadata().unwrap()).unwrap();
    let neighbor = f.home.join("private-neighbor.txt");
    fs::write(&neighbor, b"not authorized").unwrap();
    assert_locator_points_to(&f.store, &rid, &original);

    let renamed = f.home.join("合同 renamed.txt");
    fs::rename(&original, &renamed).unwrap();
    f.store = Store::open(&f.database).unwrap();
    f.store.scan_all().unwrap();
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap().0,
        renamed.to_string_lossy()
    );
    f.assert_reference(&rid, &renamed, &workspaces);

    let destination = f.home.join("another/subdirectory");
    fs::create_dir_all(&destination).unwrap();
    let moved = destination.join("合同 moved.txt");
    fs::rename(&renamed, &moved).unwrap();
    f.store = Store::open(&f.database).unwrap();
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap().0,
        moved.to_string_lossy()
    );
    f.store.scan_all().unwrap();
    f.assert_reference(&rid, &moved, &workspaces);
    assert_locator_points_to(&f.store, &rid, &moved);
    assert!(identity == crate::platform::identity(&moved, &moved.metadata().unwrap()).unwrap());
    assert_eq!(fs::read(&moved).unwrap(), bytes);
    assert_eq!(moved.metadata().unwrap().modified().unwrap(), modified);
    assert!(f.store.needs_file_grant(&neighbor).unwrap());
    assert!(f.store.roots().unwrap().is_empty());
    assert_eq!(count(&f.store, "resources"), 1);
    assert_eq!(count(&f.store, "file_locators"), 1);
    assert_eq!(f.store.file_grants().unwrap()[0].id, rid);
}

#[test]
fn copies_and_same_path_replacements_do_not_inherit_original_identity_or_references() {
    let f = Fixture::new();
    let (original, rid, workspaces) = f.linked_file();
    let copy = f.home.join("copy.txt");
    fs::copy(&original, &copy).unwrap();
    assert!(f.store.needs_file_grant(&copy).unwrap());
    assert_eq!(
        f.store.import_one(&copy, None, false).unwrap_err().code,
        "not_authorized"
    );
    let copy_id = f.store.import_one(&copy, None, true).unwrap().0;
    assert_ne!(copy_id, rid);
    let copied = Store::resource_db(&f.store.connection().unwrap(), &copy_id, false).unwrap();
    assert!(copied.workspace_ids.is_empty() && !copied.favorite);

    // 持有原对象句柄，避免删除后inode立即复用干扰负例。
    let held_original = fs::File::open(&original).unwrap();
    fs::remove_file(&original).unwrap();
    fs::write(&original, b"replacement is a different physical file").unwrap();
    assert!(f.store.resolve_resource(&rid).is_err());
    assert!(f.store.needs_file_grant(&original).unwrap());
    assert_eq!(
        f.store.import_one(&original, None, false).unwrap_err().code,
        "not_authorized"
    );
    let replacement = f.store.import_one(&original, None, true).unwrap().0;
    assert_ne!(replacement, rid);
    f.store.scan_all().unwrap();
    assert!(f.store.resolve_resource(&rid).is_err());
    let db = f.store.connection().unwrap();
    let old = Store::resource_db(&db, &rid, false).unwrap();
    let new = Store::resource_db(&db, &replacement, false).unwrap();
    assert_eq!(old.workspace_ids.len(), workspaces.len());
    assert!(old.favorite);
    assert!(new.workspace_ids.is_empty() && !new.favorite);
    assert_eq!(fs::read(&copy).unwrap(), b"synthetic original document");
    assert_eq!(
        fs::read(&original).unwrap(),
        b"replacement is a different physical file"
    );
    drop(held_original);
}

#[test]
fn revoke_and_restore_keep_saved_locators_from_reactivating_single_file_authorization() {
    for restore in [false, true] {
        let f = Fixture::new();
        let (original, rid, _) = f.linked_file();
        let backup = f.home.join("snapshot.sqlite3");
        f.store.backup_to(&backup).unwrap();
        let moved = f.home.join("moved.txt");
        fs::rename(&original, &moved).unwrap();
        let recovery = recovery_snapshot(&f.store, &rid, &moved);
        if restore {
            f.store.restore_from(&backup).unwrap();
        } else {
            f.change(Mutation::RevokeFile { id: rid.clone() });
        }
        assert_eq!(
            f.store.resolve_resource(&rid).unwrap_err().code,
            "not_authorized"
        );
        assert!(!f.store.recover_single_file(&rid).unwrap());
        reject_recovery_without_writes(&f.store, &recovery, &moved);
        f.store.scan_all().unwrap();
        assert!(f.store.file_grants().unwrap().is_empty());
        assert!(f.store.needs_file_grant(&moved).unwrap());
        assert_eq!(fs::read(&moved).unwrap(), b"synthetic original document");
    }
}

#[test]
fn stale_resolved_snapshot_cannot_overwrite_mutation_relink_or_restore() {
    for action in ["mutation", "relink", "restore"] {
        let f = Fixture::new();
        let (original, rid, _) = f.linked_file();
        let backup = f.home.join("snapshot.sqlite3");
        f.store.backup_to(&backup).unwrap();
        let moved = f.home.join("moved.txt");
        fs::rename(&original, &moved).unwrap();
        let recovery = recovery_snapshot(&f.store, &rid, &moved);
        match action {
            "mutation" => {
                f.change(Mutation::Favorite {
                    id: rid.clone(),
                    value: false,
                });
            }
            "relink" => {
                let selected = f.home.join("explicit-selected.txt");
                fs::write(&selected, b"explicit user selected target").unwrap();
                let selection =
                    crate::capture_import_selections(&[selected.to_string_lossy().into_owned()])
                        .remove(0);
                f.store.relink_selected(&rid, &selection, false).unwrap();
            }
            _ => {
                f.store.restore_from(&backup).unwrap();
            }
        }
        reject_recovery_without_writes(&f.store, &recovery, &moved);
        assert_eq!(fs::read(&moved).unwrap(), b"synthetic original document");
    }
}

#[test]
fn recovery_requires_the_original_bookmark_even_if_versions_have_not_changed() {
    let f = Fixture::new();
    let (original, rid, _) = f.linked_file();
    let moved = f.home.join("moved.txt");
    fs::rename(&original, &moved).unwrap();
    let recovery = recovery_snapshot(&f.store, &rid, &moved);
    let other = f.home.join("different.txt");
    fs::write(&other, b"not authorized").unwrap();
    let changed = crate::macos_bookmark::create(&other).unwrap();
    f.store
        .connection()
        .unwrap()
        .execute(
            "UPDATE file_locators SET bookmark=?1 WHERE resource_id=?2",
            params![changed, rid],
        )
        .unwrap();
    reject_recovery_without_writes(&f.store, &recovery, &moved);
    assert!(f.store.needs_file_grant(&other).unwrap());
}

#[test]
fn explicit_relink_replaces_locator_identity_and_does_not_preserve_old_object_grant() {
    let f = Fixture::new();
    let (original, rid, workspaces) = f.linked_file();
    let replacement = f.home.join("selected.txt");
    fs::write(&replacement, b"new explicitly selected document").unwrap();
    let selection =
        crate::capture_import_selections(&[replacement.to_string_lossy().into_owned()]).remove(0);
    f.store.relink_selected(&rid, &selection, false).unwrap();
    f.assert_reference(&rid, &replacement, &workspaces);
    assert_locator_points_to(&f.store, &rid, &replacement);
    assert!(f.store.needs_file_grant(&original).unwrap());
    let renamed = f.home.join("selected renamed.txt");
    fs::rename(&replacement, &renamed).unwrap();
    f.store.scan_all().unwrap();
    f.assert_reference(&rid, &renamed, &workspaces);
    assert_locator_points_to(&f.store, &rid, &renamed);
    assert_eq!(count(&f.store, "file_locators"), 1);
    assert_eq!(fs::read(original).unwrap(), b"synthetic original document");
}

#[test]
fn merge_keeps_surviving_uuid_with_target_locator_and_cascades_duplicate_locator() {
    let f = Fixture::new();
    let (original, rid, mut workspaces) = f.linked_file();
    let target = f.home.join("target.txt");
    let duplicate = f.import(&target, b"target object");
    let other_workspace = f
        .change(Mutation::CreateWorkspace {
            name: "目标资料".into(),
            resource_ids: Some(vec![duplicate.clone()]),
        })
        .id
        .unwrap();
    workspaces.push(other_workspace);
    assert_eq!(count(&f.store, "file_locators"), 2);
    let selection =
        crate::capture_import_selections(&[target.to_string_lossy().into_owned()]).remove(0);
    f.store.relink_selected(&rid, &selection, true).unwrap();
    assert_eq!(count(&f.store, "resources"), 1);
    assert_eq!(count(&f.store, "file_locators"), 1);
    assert_eq!(
        f.store
            .connection()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM file_locators WHERE resource_id=?",
                [&duplicate],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
    f.assert_reference(&rid, &target, &workspaces);
    assert_locator_points_to(&f.store, &rid, &target);
    let moved = f.home.join("target moved.txt");
    fs::rename(&target, &moved).unwrap();
    f.store.scan_all().unwrap();
    f.assert_reference(&rid, &moved, &workspaces);
    assert_locator_points_to(&f.store, &rid, &moved);
    assert!(f.store.needs_file_grant(&original).unwrap());
}

#[test]
fn scan_backfills_missing_single_file_locator_without_capturing_every_directory_file() {
    let f = Fixture::new();
    let root = f.home.join("managed");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("one.txt"), b"managed one").unwrap();
    fs::write(root.join("two.txt"), b"managed two").unwrap();
    f.store.grant_root(&root).unwrap();
    f.store.scan_all().unwrap();
    assert_eq!(count(&f.store, "resources"), 2);
    assert_eq!(count(&f.store, "file_locators"), 0);
    let single = f.home.join("single.txt");
    let rid = f.import(&single, b"individual grant");
    f.store
        .connection()
        .unwrap()
        .execute("DELETE FROM file_locators WHERE resource_id=?", [&rid])
        .unwrap();
    assert_eq!(count(&f.store, "file_locators"), 0);
    f.store.scan_all().unwrap();
    assert_eq!(count(&f.store, "file_locators"), 1);
    assert_locator_points_to(&f.store, &rid, &single);
    assert_eq!(count(&f.store, "resources"), 3);
}

#[test]
fn broken_bookmark_reports_error_after_unrelated_root_finishes_and_status_changes_once() {
    let f = Fixture::new();
    let (original, rid, workspaces) = f.linked_file();
    let moved = f.home.join("moved.txt");
    fs::rename(&original, &moved).unwrap();
    f.store
        .connection()
        .unwrap()
        .execute(
            "UPDATE file_locators SET bookmark=?1 WHERE resource_id=?2",
            params![b"invalid bookmark".as_slice(), rid],
        )
        .unwrap();
    let root = f.home.join("unrelated");
    fs::create_dir(&root).unwrap();
    let added = root.join("new.txt");
    fs::write(&added, b"independent root fixture").unwrap();
    let root_id = f.store.grant_root(&root).unwrap();
    assert_eq!(
        f.store.scan_all().unwrap_err().code,
        "bookmark_resolve_failed"
    );
    let root_state = f
        .store
        .roots()
        .unwrap()
        .into_iter()
        .find(|root| root.id == root_id)
        .unwrap();
    assert_eq!(root_state.scan_state, "completed");
    assert_eq!(root_state.scanned, 1);
    let db = f.store.connection().unwrap();
    let status: String = db
        .query_row("SELECT status FROM resources WHERE id=?", [&rid], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(status, "missing");
    let old = Store::resource_db(&db, &rid, false).unwrap();
    assert!(old.favorite);
    assert_eq!(old.workspace_ids.len(), workspaces.len());
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM resources WHERE location=?",
            [added.to_string_lossy().as_ref()],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
    let revision = crate::store::version(&db, "revision").unwrap();
    drop(db);
    // 单独刷新授权文件，排除Catalog扫描自身递增revision的影响。
    let snapshot = logical_snapshot(&f.store);
    assert_eq!(
        f.store.refresh_file_grants().unwrap_err().code,
        "bookmark_resolve_failed"
    );
    assert_eq!(
        crate::store::version(&f.store.connection().unwrap(), "revision").unwrap(),
        revision
    );
    assert!(snapshot == logical_snapshot(&f.store));
    f.change(Mutation::RevokeFile { id: rid });
    f.store.scan_all().unwrap();
    assert_eq!(fs::read(&moved).unwrap(), b"synthetic original document");
    assert_eq!(fs::read(&added).unwrap(), b"independent root fixture");
}

#[test]
fn recovery_identity_guard_cannot_be_bypassed_by_omitting_expected_identity() {
    let f = Fixture::new();
    let (original, rid, _) = f.linked_file();
    let other = f.home.join("other-granted.txt");
    f.import(&other, b"different authorized object");
    let recovery = recovery_snapshot(&f.store, &rid, &other);
    let snapshot = logical_snapshot(&f.store);
    let error = f
        .store
        .import_one_checked(&other, None, false, None, Some(&recovery))
        .unwrap_err();
    assert_eq!(error.code, "identity_unconfirmed");
    assert!(snapshot == logical_snapshot(&f.store));
    assert_locator_points_to(&f.store, &rid, &original);
}

#[test]
fn delayed_bookmark_failure_is_discarded_after_relink_same_path_repair_or_revocation() {
    for action in ["relink", "same-path-repair", "revoke"] {
        let f = Fixture::new();
        let (original, rid, _) = f.linked_file();
        let previous = f.store.file_grants().unwrap().remove(0);
        let moved = f.home.join("moved.txt");
        fs::rename(&original, &moved).unwrap();
        f.store
            .connection()
            .unwrap()
            .execute(
                "UPDATE file_locators SET bookmark=?1 WHERE resource_id=?2",
                params![b"invalid bookmark".as_slice(), rid],
            )
            .unwrap();
        let delayed_error = f.store.resolve_resource(&rid).unwrap_err();
        assert_eq!(delayed_error.code, "bookmark_resolve_failed");
        match action {
            "revoke" => {
                f.change(Mutation::RevokeFile { id: rid.clone() });
            }
            _ => {
                let selected = if action == "same-path-repair" {
                    // 保留旧location，单独覆盖重核原路径已经恢复成功的分支。
                    fs::rename(&moved, &original).unwrap();
                    &original
                } else {
                    &moved
                };
                let selection =
                    crate::capture_import_selections(&[selected.to_string_lossy().into_owned()])
                        .remove(0);
                f.store.relink_selected(&rid, &selection, false).unwrap();
            }
        }
        let snapshot = logical_snapshot(&f.store);
        assert!(f
            .store
            .apply_file_refresh_failure(&previous, delayed_error)
            .unwrap()
            .is_none());
        assert!(
            snapshot == logical_snapshot(&f.store),
            "obsolete failure changed the repaired or revoked state"
        );
        let current = if action == "same-path-repair" {
            &original
        } else {
            &moved
        };
        assert_eq!(fs::read(current).unwrap(), b"synthetic original document");
    }
}

#[test]
fn missing_locator_creation_failure_keeps_available_original_and_clears_after_backfill() {
    let f = Fixture::new();
    let (original, rid, workspaces) = f.linked_file();
    let previous = f.store.file_grants().unwrap().remove(0);
    f.store
        .connection()
        .unwrap()
        .execute("DELETE FROM file_locators WHERE resource_id=?", [&rid])
        .unwrap();
    // 确定性注入可选定位信息创建失败，原件本身依然存在且身份、授权有效。
    let error = AppError::new(
        "bookmark_create_failed",
        "无法创建文件定位信息",
        "File location information could not be created.",
    );
    let snapshot = logical_snapshot(&f.store);
    let reported = f
        .store
        .apply_file_refresh_failure(&previous, error.clone())
        .unwrap()
        .expect("a still-missing locator must report its creation failure");
    assert_eq!(reported.code, "bookmark_create_failed");
    assert!(snapshot == logical_snapshot(&f.store));
    f.assert_reference(&rid, &original, &workspaces);
    assert_eq!(count(&f.store, "file_locators"), 0);

    f.store.scan_all().unwrap();
    assert_locator_points_to(&f.store, &rid, &original);
    let repaired = logical_snapshot(&f.store);
    assert!(f
        .store
        .apply_file_refresh_failure(&previous, error)
        .unwrap()
        .is_none());
    assert!(repaired == logical_snapshot(&f.store));
    assert_eq!(fs::read(original).unwrap(), b"synthetic original document");
}
