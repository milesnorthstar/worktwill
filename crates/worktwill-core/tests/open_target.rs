#![cfg(target_os = "macos")]

use rusqlite::Connection;
use std::{fs, path::PathBuf};
use worktwill_core::{model::*, ResolvedOpen, Store};

struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
    root_id: String,
    database: PathBuf,
    store: Store,
}

impl Fixture {
    fn new() -> Self {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("open-target-")
            .tempdir_in(approved)
            .unwrap();
        let root = directory.path().join("documents");
        fs::create_dir(&root).unwrap();
        let database = directory.path().join("app/catalog.sqlite3");
        let store = Store::open(&database).unwrap();
        let root_id = store.grant_root(&root).unwrap();
        Self {
            directory,
            root,
            root_id,
            database,
            store,
        }
    }

    fn write(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        assert!(!path.exists());
        fs::write(&path, b"synthetic open target fixture").unwrap();
        path
    }

    fn file(&self, name: &str) -> (PathBuf, String) {
        let path = self.write(name);
        let imported = self
            .store
            .import_files(&[path.to_string_lossy().into_owned()], None, true)
            .unwrap();
        let rid = imported.items[0].resource_id.clone().unwrap();
        (path, rid)
    }

    fn change(&self, mutation: Mutation) -> MutationResponse {
        self.store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation,
            })
            .unwrap()
    }

    fn db(&self) -> Connection {
        Connection::open_with_flags(&self.database, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap()
    }

    fn opened_count(&self) -> u32 {
        self.db()
            .query_row(
                "SELECT count(*) FROM activities WHERE kind='opened'",
                [],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn assert_changed(&self, target: ResolvedOpen) {
        let before = self.opened_count();
        let error = self.store.record_open_checked(target).unwrap_err();
        assert_eq!(error.code, "open_record_changed");
        assert!(error.message.contains("系统已接受打开请求"));
        assert!(error.message.contains("未记录"));
        assert!(error.message_en.contains("system accepted"));
        assert!(error.message_en.contains("not recorded"));
        assert_eq!(
            self.opened_count(),
            before,
            "a stale target wrote an opened activity"
        );
    }
}

#[test]
fn preparation_is_read_only_and_success_records_the_prepared_file_origin() {
    let f = Fixture::new();
    let (path, rid) = f.file("normal-open.txt");
    let target = f.store.prepare_open(&rid).unwrap();
    assert_eq!(target.location(), path.to_string_lossy());
    assert!(!target.is_web());
    assert_eq!(f.opened_count(), 0);
    f.store.record_open_checked(target).unwrap();
    assert_eq!(f.opened_count(), 1);
    let (name, observed_id, identity_matches): (String, String, bool) = f.db().query_row(
        "SELECT a.name,o.observed_resource_id,o.identity=r.identity FROM activities a JOIN activity_origins o ON o.activity_id=a.id JOIN resources r ON r.id=a.resource_id WHERE a.kind='opened'",
        [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    assert_eq!(name, "normal-open.txt");
    assert_eq!(observed_id, rid);
    assert!(identity_matches);
    let (_, unsupported) = f.file("do-not-run.command");
    let error = f.store.prepare_open(&unsupported).err().unwrap();
    assert_eq!(error.code, "unsupported_file_type");
    assert_eq!(f.opened_count(), 1);
}

#[test]
fn unrelated_scan_favorite_workspace_and_other_open_do_not_invalidate_target() {
    let f = Fixture::new();
    let (_, rid) = f.file("selected.txt");
    let (_, other) = f.file("other.txt");
    let target = f.store.prepare_open(&rid).unwrap();
    f.store.scan_all().unwrap();
    f.change(Mutation::Favorite {
        id: rid.clone(),
        value: true,
    });
    f.change(Mutation::CreateWorkspace {
        name: "Unrelated workspace".into(),
        resource_ids: None,
    });
    f.store.record_open(&other).unwrap();
    f.store.record_open_checked(target).unwrap();
    assert_eq!(f.opened_count(), 2);
    let count: u32 = f
        .db()
        .query_row(
            "SELECT count(*) FROM activities WHERE kind='opened' AND resource_id=?",
            [&rid],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn different_identity_relink_cannot_receive_an_earlier_open_acceptance() {
    let f = Fixture::new();
    let (_, rid) = f.file("opened-original.txt");
    let target = f.store.prepare_open(&rid).unwrap();
    let replacement = f.write("different-selected-original.txt");
    f.store.relink(&rid, &replacement).unwrap();
    f.assert_changed(target);
    let fresh = f.store.prepare_open(&rid).unwrap();
    f.store.record_open_checked(fresh).unwrap();
    let name: String = f
        .db()
        .query_row(
            "SELECT name FROM activities WHERE kind='opened'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(name, "different-selected-original.txt");
}

#[test]
fn merging_rejects_both_changed_source_and_removed_target_snapshots() {
    let f = Fixture::new();
    let (_, source) = f.file("merge-source.txt");
    let (target_path, target_id) = f.file("merge-target.txt");
    let source_snapshot = f.store.prepare_open(&source).unwrap();
    let target_snapshot = f.store.prepare_open(&target_id).unwrap();
    f.store
        .relink_confirmed(&source, &target_path, true)
        .unwrap();
    f.assert_changed(source_snapshot);
    f.assert_changed(target_snapshot);
    assert_eq!(f.opened_count(), 0);
}

#[test]
fn revoked_or_revoked_then_regranted_authority_cannot_commit_an_old_target() {
    for regrant in [false, true] {
        let f = Fixture::new();
        let (_, rid) = f.file("permission-changed.txt");
        let target = f.store.prepare_open(&rid).unwrap();
        f.change(Mutation::RevokeRoot {
            id: f.root_id.clone(),
        });
        if regrant {
            assert_eq!(f.store.grant_root(&f.root).unwrap(), f.root_id);
        }
        f.assert_changed(target);
    }
}

#[test]
fn restore_then_reauthorization_cannot_reuse_a_pre_restore_target() {
    let f = Fixture::new();
    let (_, rid) = f.file("restore-open.txt");
    let target = f.store.prepare_open(&rid).unwrap();
    let backup = f.directory.path().join("before-open.sqlite3");
    f.store.backup_to(&backup).unwrap();
    f.store.restore_from(&backup).unwrap();
    f.store.grant_root(&f.root).unwrap();
    f.assert_changed(target);
    f.store
        .record_open_checked(f.store.prepare_open(&rid).unwrap())
        .unwrap();
    assert_eq!(f.opened_count(), 1);
}

#[test]
fn same_identity_new_path_requires_a_new_prepared_location() {
    let f = Fixture::new();
    let (path, rid) = f.file("before-move.txt");
    let target = f.store.prepare_open(&rid).unwrap();
    let moved = f.root.join("after-move.txt");
    fs::rename(&path, &moved).unwrap();
    f.store.relink(&rid, &moved).unwrap();
    f.assert_changed(target);
    let current = f.store.prepare_open(&rid).unwrap();
    assert_eq!(current.location(), moved.to_string_lossy());
    f.store.record_open_checked(current).unwrap();
    assert_eq!(f.opened_count(), 1);
}

#[test]
fn unobserved_same_path_physical_replacement_is_rejected_before_recording() {
    let f = Fixture::new();
    let (path, rid) = f.file("physical-original.txt");
    let target = f.store.prepare_open(&rid).unwrap();
    let replacement = f.write("new-physical-object.tmp");
    fs::rename(&replacement, &path).unwrap();
    f.assert_changed(target);
    assert_eq!(f.opened_count(), 0);
}

#[test]
fn changed_web_url_is_rejected_but_title_only_change_keeps_snapshot_observation() {
    let f = Fixture::new();
    let rid = f
        .change(Mutation::SaveWeb {
            id: None,
            name: "First page".into(),
            url: "https://example.com/first".into(),
            workspace_id: None,
        })
        .id
        .unwrap();
    let first = f.store.prepare_open(&rid).unwrap();
    assert!(first.is_web());
    assert_eq!(first.location(), "https://example.com/first");
    f.change(Mutation::SaveWeb {
        id: Some(rid.clone()),
        name: "Second page".into(),
        url: "https://example.com/second".into(),
        workspace_id: None,
    });
    f.assert_changed(first);
    let second = f.store.prepare_open(&rid).unwrap();
    f.change(Mutation::SaveWeb {
        id: Some(rid),
        name: "Title edited while opening".into(),
        url: "https://example.com/second".into(),
        workspace_id: None,
    });
    f.store.record_open_checked(second).unwrap();
    let (name, origin_count): (String, u32) = f.db().query_row(
        "SELECT a.name,(SELECT count(*) FROM activity_origins o WHERE o.activity_id=a.id) FROM activities a WHERE a.kind='opened'",
        [], |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap();
    assert_eq!(name, "Second page");
    assert_eq!(origin_count, 0);
}

#[test]
fn target_is_bound_to_its_store_session() {
    let f = Fixture::new();
    let (_, rid) = f.file("session-bound.txt");
    let target = f.store.prepare_open(&rid).unwrap();
    let reopened = Store::open(&f.database).unwrap();
    let error = reopened.record_open_checked(target).unwrap_err();
    assert_eq!(error.code, "open_record_changed");
    assert_eq!(f.opened_count(), 0);
}
