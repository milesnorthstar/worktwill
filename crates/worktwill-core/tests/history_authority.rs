#![cfg(target_os = "macos")]

use rusqlite::{params, Connection};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use worktwill_core::{
    capture_import_selections,
    model::*,
    timeline::{TimelinePage, TimelineRequest},
    RelinkReview, Store,
};

struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
    outside: PathBuf,
    database: PathBuf,
    store: Store,
}

impl Fixture {
    fn new() -> Self {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("history-authority-")
            .tempdir_in(approved)
            .unwrap();
        let root = directory.path().join("managed");
        let outside = directory.path().join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        let database = directory.path().join("app/catalog.sqlite3");
        let store = Store::open(&database).unwrap();
        Self {
            directory,
            root,
            outside,
            database,
            store,
        }
    }

    fn write(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        // 样本仅在本测试独占TempDir新建，不借用原件或旧夹具。
        assert!(!path.exists());
        fs::write(path, b"synthetic metadata authority fixture").unwrap();
    }

    fn import(&self, path: &Path) -> String {
        let result = self
            .store
            .import_files(&[path.to_string_lossy().into_owned()], None, true)
            .unwrap();
        assert_eq!(result.items[0].status, "added");
        result.items[0].resource_id.clone().unwrap()
    }

    fn change(&self, mutation: Mutation) {
        self.store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation,
            })
            .unwrap();
    }

    fn open(&self, rid: &str) -> Activity {
        let previous = self
            .store
            .overview()
            .unwrap()
            .activities
            .into_iter()
            .map(|activity| activity.id)
            .collect::<Vec<_>>();
        self.store.resolve_resource(rid).unwrap();
        self.store.record_open(rid).unwrap();
        self.store
            .overview()
            .unwrap()
            .activities
            .into_iter()
            .find(|activity| {
                activity.kind == "opened"
                    && activity.resource_id.as_deref() == Some(rid)
                    && !previous.contains(&activity.id)
            })
            .unwrap()
    }

    fn select(&self, rid: &str, path: &Path) -> RelinkReview {
        let initial = self.store.review_relink(rid).unwrap();
        let selection = capture_import_selections(&[path.to_string_lossy().into_owned()]).remove(0);
        self.store
            .review_relink_selection(&initial, &selection)
            .unwrap()
    }

    fn assert_opened(&self, expected: &[&Activity]) {
        let overview = self.store.overview().unwrap();
        let mut actual_ids = overview
            .activities
            .iter()
            .filter(|activity| activity.kind == "opened")
            .map(|activity| activity.id.clone())
            .collect::<Vec<_>>();
        let mut expected_ids = expected
            .iter()
            .map(|activity| activity.id.clone())
            .collect::<Vec<_>>();
        actual_ids.sort();
        expected_ids.sort();
        assert_eq!(
            actual_ids, expected_ids,
            "overview crossed history authority"
        );
        let timeline = self.opened_timeline();
        let mut timeline_ids = timeline
            .events
            .iter()
            .map(|entry| entry.activity.id.clone())
            .collect::<Vec<_>>();
        timeline_ids.sort();
        assert_eq!(
            timeline_ids, expected_ids,
            "timeline crossed history authority"
        );
        assert_eq!(
            timeline
                .calendar
                .iter()
                .map(|point| point.count)
                .sum::<u32>(),
            expected.len() as u32,
            "calendar exposed a hidden historical observation"
        );
        assert!(!timeline.has_more);
        for expected in expected {
            assert_eq!(
                overview
                    .activities
                    .iter()
                    .find(|activity| activity.id == expected.id)
                    .unwrap()
                    .name,
                expected.name,
                "overview renamed a historical observation"
            );
            assert_eq!(
                timeline
                    .events
                    .iter()
                    .find(|entry| entry.activity.id == expected.id)
                    .unwrap()
                    .activity
                    .name,
                expected.name,
                "timeline renamed a historical observation"
            );
        }
    }

    fn opened_timeline(&self) -> TimelinePage {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            * 1000.;
        let day = (now / 86_400_000.).floor() * 86_400_000.;
        // 前后各留一天，避免夹具跨午夜；只筛opened，使日历不能借其他事件掩盖泄漏。
        self.store
            .timeline(TimelineRequest {
                from: None,
                to: None,
                month_from: day - 86_400_000.,
                month_to: day + 172_800_000.,
                day_boundaries: vec![
                    day - 86_400_000.,
                    day,
                    day + 86_400_000.,
                    day + 172_800_000.,
                ],
                filter: "opened".into(),
                basis: "observed".into(),
                workspace_id: None,
                offset: 0,
                cursor: None,
            })
            .unwrap()
    }

    fn assert_current_placeholder(&self, activity: &Activity, rid: &str) {
        let timeline = self.opened_timeline();
        let observed = timeline
            .events
            .iter()
            .find(|entry| entry.activity.id == activity.id)
            .unwrap();
        let current = observed.resource.as_ref().unwrap();
        assert_eq!(observed.activity.name, activity.name);
        assert_eq!(current.id, rid);
        assert_eq!(current.status, "permission_denied");
        assert_eq!(current.name, "未授权的文件");
        assert!(current.location.is_empty());
        assert_eq!(current.modified_at, 0.);
        assert_eq!(current.size, 0.);
        assert!(!current.favorite);
    }

    fn assert_name_hidden(&self, name: &str) {
        assert!(self
            .store
            .overview()
            .unwrap()
            .activities
            .iter()
            .all(|activity| activity.name != name));
    }

    fn readonly_db(&self) -> Connection {
        Connection::open_with_flags(&self.database, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap()
    }

    fn identity(&self, rid: &str) -> String {
        self.readonly_db()
            .query_row("SELECT identity FROM resources WHERE id=?", [rid], |row| {
                row.get(0)
            })
            .unwrap()
    }

    fn root_identity(&self, root_id: &str) -> String {
        self.readonly_db()
            .query_row(
                "SELECT identity FROM root_locators WHERE root_id=?",
                [root_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn assert_origin(&self, activity: &Activity, observed_id: &str, identity: &str) {
        let origin: (Option<String>, Option<String>) = self
            .readonly_db()
            .query_row(
                "SELECT observed_resource_id,identity FROM activity_origins WHERE activity_id=?",
                [&activity.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(origin, (Some(observed_id.into()), Some(identity.into())));
    }

    fn assert_root_scope(
        &self,
        activity: &Activity,
        root_id: &str,
        identity: &str,
        relative: &str,
    ) {
        let scopes: Vec<(String, Option<String>, String)> = self.readonly_db().prepare(
            "SELECT root_id,root_identity,relative_path FROM activity_root_scopes WHERE activity_id=? ORDER BY root_id"
        ).unwrap().query_map([&activity.id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert_eq!(
            scopes,
            vec![(root_id.into(), Some(identity.into()), relative.into())]
        );
    }

    fn assert_relink_audit(
        &self,
        source: &str,
        merged: Option<&str>,
        previous: &str,
        selected: &str,
    ) {
        let count: u32 = self.readonly_db().query_row(
            "SELECT count(*) FROM resource_relinks WHERE source_resource_id=?1 AND merged_resource_id IS ?2 AND previous_identity=?3 AND selected_identity=?4",
            params![source, merged, previous, selected], |row| row.get(0),
        ).unwrap();
        assert_eq!(
            count, 1,
            "relink audit lost or rewrote an identity boundary"
        );
    }

    fn reopen(&mut self) {
        self.store = Store::open(&self.database).unwrap();
    }
}

#[test]
fn new_file_grant_cannot_revive_revoked_original_history_after_anonymous_relink() {
    let mut f = Fixture::new();
    let original = f.outside.join("private-original-personnel.txt");
    f.write(&original);
    let rid = f.import(&original);
    let old_identity = f.identity(&rid);
    let old = f.open(&rid);
    f.assert_opened(&[&old]);
    f.change(Mutation::RevokeFile { id: rid.clone() });
    f.assert_opened(&[]);
    f.assert_name_hidden(&old.name);
    let anonymous = f.store.review_relink(&rid).unwrap();
    assert!(!anonymous.source.authorized);
    assert_ne!(anonymous.source.name, old.name);
    let target = f.outside.join("new-public-project.txt");
    f.write(&target);
    let selection = capture_import_selections(&[target.to_string_lossy().into_owned()]).remove(0);
    let reviewed = f
        .store
        .review_relink_selection(&anonymous, &selection)
        .unwrap();
    assert!(reviewed.target.is_none());
    f.store.relink_reviewed(&reviewed, false).unwrap();
    assert_eq!(f.store.file_grants().unwrap()[0].id, rid);
    f.assert_opened(&[]);
    f.assert_name_hidden(&old.name);
    let selected_identity = f.identity(&rid);
    let fresh = f.open(&rid);
    assert_eq!(fresh.name, "new-public-project.txt");
    f.assert_opened(&[&fresh]);
    f.reopen();
    f.assert_opened(&[&fresh]);
    f.assert_name_hidden(&old.name);
    f.assert_origin(&old, &rid, &old_identity);
    f.assert_origin(&fresh, &rid, &selected_identity);
    f.assert_relink_audit(&rid, None, &old_identity, &selected_identity);
}

#[test]
fn old_root_history_uses_its_original_anchor_after_resource_relinks_outside() {
    let mut f = Fixture::new();
    let original = f.root.join("private-original-personnel.txt");
    f.write(&original);
    let root_id = f.store.grant_root(&f.root).unwrap();
    let root_identity = f.root_identity(&root_id);
    let rid = f.import(&original);
    assert!(f.store.file_grants().unwrap().is_empty());
    let old = f.open(&rid);
    let target = f.outside.join("new-public-project.txt");
    f.write(&target);
    let review = f.select(&rid, &target);
    f.store.relink_reviewed(&review, false).unwrap();
    f.assert_opened(&[&old]);
    f.change(Mutation::RevokeFile { id: rid.clone() });
    // A的历史scope仍授权旧名称；不能因此暴露当前B的名称/位置或给B启用打开。
    f.assert_opened(&[&old]);
    f.assert_current_placeholder(&old, &rid);
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap_err().code,
        "not_authorized"
    );
    f.reopen();
    f.assert_opened(&[&old]);
    f.assert_current_placeholder(&old, &rid);
    let selected_again = f.select(&rid, &target);
    f.store.relink_reviewed(&selected_again, false).unwrap();
    f.change(Mutation::RevokeRoot {
        id: root_id.clone(),
    });
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap().0,
        target.to_string_lossy()
    );
    f.assert_opened(&[]);
    f.assert_name_hidden(&old.name);
    assert_eq!(f.store.grant_root(&f.root).unwrap(), root_id);
    f.assert_opened(&[&old]);
    f.reopen();
    f.assert_opened(&[&old]);
    f.change(Mutation::RevokeRoot {
        id: root_id.clone(),
    });
    let archived = f.directory.path().join("original-root-kept-aside");
    fs::rename(&f.root, &archived).unwrap();
    fs::create_dir(&f.root).unwrap();
    f.write(&f.root.join("replacement-root-public.txt"));
    // 同路径重新授权会复用管理位置记录；旧历史必须检查物理anchor，而非只比root_id。
    assert_eq!(f.store.grant_root(&f.root).unwrap(), root_id);
    f.store.scan_all().unwrap();
    f.assert_opened(&[]);
    f.assert_name_hidden(&old.name);
    f.reopen();
    f.assert_opened(&[]);
    assert_ne!(f.root_identity(&root_id), root_identity);
    f.assert_root_scope(
        &old,
        &root_id,
        &root_identity,
        "private-original-personnel.txt",
    );
}

#[test]
fn moved_root_preserves_history_scope_but_current_relative_exclusions_hide_it() {
    let mut f = Fixture::new();
    let original = f.root.join("staff/private-original-personnel.txt");
    f.write(&original);
    let root_id = f.store.grant_root(&f.root).unwrap();
    let root_identity = f.root_identity(&root_id);
    let rid = f.import(&original);
    let old = f.open(&rid);
    let target = f.outside.join("new-public-project.txt");
    f.write(&target);
    let reviewed = f.select(&rid, &target);
    f.store.relink_reviewed(&reviewed, false).unwrap();
    let moved = f.directory.path().join("root-renamed-and-moved");
    fs::rename(&f.root, &moved).unwrap();
    f.store.scan_all().unwrap();
    assert_eq!(f.store.roots().unwrap()[0].path, moved.to_string_lossy());
    f.assert_opened(&[&old]);
    f.change(Mutation::Exclude {
        root_id: root_id.clone(),
        relative_path: "staff".into(),
    });
    // 当前资源已在根外，故排除判定必须使用事件原始相对路径。
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap().0,
        target.to_string_lossy()
    );
    f.assert_opened(&[]);
    f.assert_name_hidden(&old.name);
    f.change(Mutation::Include {
        root_id: root_id.clone(),
        relative_path: "staff".into(),
    });
    f.assert_opened(&[&old]);
    f.reopen();
    f.assert_opened(&[&old]);
    f.change(Mutation::Exclude {
        root_id: root_id.clone(),
        relative_path: "staff/private-original-personnel.txt".into(),
    });
    f.assert_opened(&[]);
    f.assert_root_scope(
        &old,
        &root_id,
        &root_identity,
        "staff/private-original-personnel.txt",
    );
}

#[test]
fn merge_does_not_launder_revoked_older_target_identity_history_through_new_grant() {
    let mut f = Fixture::new();
    let oldest = f.outside.join("revoked-older-target-personnel.txt");
    f.write(&oldest);
    let target_id = f.import(&oldest);
    let oldest_identity = f.identity(&target_id);
    let hidden_oldest = f.open(&target_id);
    f.change(Mutation::RevokeFile {
        id: target_id.clone(),
    });
    let selected = f.outside.join("selected-current-target.txt");
    f.write(&selected);
    let reviewed = f.select(&target_id, &selected);
    f.store.relink_reviewed(&reviewed, false).unwrap();
    let selected_identity = f.identity(&target_id);
    let visible_target = f.open(&target_id);
    f.assert_opened(&[&visible_target]);
    let source = f.outside.join("independent-source.txt");
    f.write(&source);
    let source_id = f.import(&source);
    let source_identity = f.identity(&source_id);
    let hidden_source = f.open(&source_id);
    f.assert_opened(&[&visible_target, &hidden_source]);
    let reviewed = f.select(&source_id, &selected);
    assert_eq!(reviewed.target.as_ref().unwrap().resource_id, target_id);
    f.store.relink_reviewed(&reviewed, true).unwrap();
    f.assert_opened(&[&visible_target]);
    f.assert_name_hidden(&hidden_oldest.name);
    f.assert_name_hidden(&hidden_source.name);
    assert!(f.store.resolve_resource(&target_id).is_err());
    assert_eq!(f.store.file_grants().unwrap().len(), 1);
    assert_eq!(f.store.file_grants().unwrap()[0].id, source_id);
    let fresh = f.open(&source_id);
    f.assert_opened(&[&visible_target, &fresh]);
    f.reopen();
    f.assert_opened(&[&visible_target, &fresh]);
    f.assert_name_hidden(&hidden_oldest.name);
    f.assert_name_hidden(&hidden_source.name);
    f.assert_origin(&hidden_oldest, &target_id, &oldest_identity);
    f.assert_origin(&visible_target, &target_id, &selected_identity);
    f.assert_origin(&hidden_source, &source_id, &source_identity);
    f.assert_origin(&fresh, &source_id, &selected_identity);
    f.assert_relink_audit(&target_id, None, &oldest_identity, &selected_identity);
    f.assert_relink_audit(
        &source_id,
        Some(&target_id),
        &source_identity,
        &selected_identity,
    );
}

#[test]
fn known_move_revocation_stays_effective_after_anonymous_relink_and_restore() {
    let mut f = Fixture::new();
    let original = f.root.join("known-moved-private-personnel.txt");
    f.write(&original);
    let old_root = f.store.grant_root(&f.root).unwrap();
    let old_anchor = f.root_identity(&old_root);
    let rid = f.import(&original);
    let old_identity = f.identity(&rid);
    let old = f.open(&rid);
    let destination = f.directory.path().join("new-root");
    fs::create_dir(&destination).unwrap();
    let moved = destination.join("known-moved-private-personnel.txt");
    fs::rename(&original, &moved).unwrap();
    let new_root = f.store.grant_root(&destination).unwrap();
    f.store.scan_root(&new_root).unwrap();
    assert_eq!(f.identity(&rid), old_identity);
    f.assert_opened(&[&old]);
    f.change(Mutation::RevokeRoot { id: new_root });
    f.assert_opened(&[]);
    f.assert_name_hidden(&old.name);
    let anonymous = f.store.review_relink(&rid).unwrap();
    assert!(!anonymous.source.authorized);
    assert_ne!(anonymous.source.name, old.name);
    let target = f.outside.join("selected-after-known-revocation.txt");
    f.write(&target);
    let selected = capture_import_selections(&[target.to_string_lossy().into_owned()]).remove(0);
    let review = f
        .store
        .review_relink_selection(&anonymous, &selected)
        .unwrap();
    assert!(review.target.is_none());
    f.store.relink_reviewed(&review, false).unwrap();
    let selected_identity = f.identity(&rid);
    assert_ne!(old_identity, selected_identity);
    let assert_retired = |fixture: &Fixture| {
        fixture.assert_origin(&old, &rid, &old_identity);
        fixture.assert_root_scope(
            &old,
            &old_root,
            &old_anchor,
            "known-moved-private-personnel.txt",
        );
        let db = fixture.readonly_db();
        let retired: bool = db.query_row(
            "SELECT retired_location_valid FROM activity_root_scopes WHERE activity_id=?1 AND root_id=?2",
            params![old.id, old_root], |row| row.get(0),
        ).unwrap();
        assert!(
            !retired,
            "known movement was forgotten when the identity retired"
        );
        let still_current: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM resources WHERE kind='file' AND identity=?)",
                [&old_identity],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!still_current);
    };
    f.assert_opened(&[]);
    f.assert_name_hidden(&old.name);
    assert_retired(&f);
    let repaired = f
        .store
        .overview()
        .unwrap()
        .activities
        .into_iter()
        .find(|activity| {
            activity.kind == "relinked" && activity.resource_id.as_deref() == Some(rid.as_str())
        })
        .unwrap();
    assert_eq!(repaired.name, "selected-after-known-revocation.txt");
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap().0,
        target.to_string_lossy()
    );
    let fresh = f.open(&rid);
    f.assert_opened(&[&fresh]);
    f.reopen();
    f.assert_opened(&[&fresh]);
    f.assert_name_hidden(&old.name);
    assert_retired(&f);
    let backup = f.directory.path().join("known-move-history-backup.sqlite3");
    f.store.backup_to(&backup).unwrap();
    f.store.restore_from(&backup).unwrap();
    f.assert_opened(&[]);
    assert!(f.store.resolve_resource(&rid).is_err());
    // 把旧根也重新授权，确保不是恢复默认撤权掩盖了已退休scope的错误复活。
    assert_eq!(f.store.grant_root(&f.root).unwrap(), old_root);
    f.assert_opened(&[]);
    f.assert_name_hidden(&old.name);
    let authorized = f
        .store
        .import_files(&[target.to_string_lossy().into_owned()], None, true)
        .unwrap();
    assert_eq!(
        authorized.items[0].resource_id.as_deref(),
        Some(rid.as_str())
    );
    assert_eq!(authorized.items[0].status, "already_present");
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap().0,
        target.to_string_lossy()
    );
    f.assert_opened(&[&fresh]);
    f.assert_name_hidden(&old.name);
    assert_retired(&f);
    f.assert_origin(&fresh, &rid, &selected_identity);
    f.assert_relink_audit(&rid, None, &old_identity, &selected_identity);
    assert!(f
        .store
        .overview()
        .unwrap()
        .activities
        .iter()
        .any(|activity| activity.id == repaired.id));
    f.reopen();
    f.assert_opened(&[&fresh]);
    f.assert_name_hidden(&old.name);
    assert_retired(&f);
}

#[test]
fn unknown_observation_identity_cannot_borrow_current_or_retired_root_authority() {
    let mut f = Fixture::new();
    let original = f.root.join("unknown-history-private-personnel.txt");
    f.write(&original);
    let root_id = f.store.grant_root(&f.root).unwrap();
    let root_identity = f.root_identity(&root_id);
    let rid = f.import(&original);
    let old = f.open(&rid);
    f.assert_opened(&[&old]);
    // 仅自有夹具模拟来源缺失；保留observed UUID和原scope，未知身份不能被当作已知无当前记录。
    let mut db = Connection::open(&f.database).unwrap();
    let tx = db.transaction().unwrap();
    tx.execute(
        "UPDATE activity_origins SET identity=NULL WHERE activity_id=?",
        [&old.id],
    )
    .unwrap();
    // 夹具直接改库也必须遵循正式写入的版本约定，使已有月历快照失效。
    tx.execute("UPDATE settings SET value=value+1 WHERE key='revision'", [])
        .unwrap();
    tx.commit().unwrap();
    drop(db);
    assert!(f.store.resolve_resource(&rid).is_ok());
    f.assert_opened(&[]);
    f.assert_root_scope(
        &old,
        &root_id,
        &root_identity,
        "unknown-history-private-personnel.txt",
    );
    let target = f.outside.join("known-new-file-after-unknown-history.txt");
    f.write(&target);
    let reviewed = f.select(&rid, &target);
    f.store.relink_reviewed(&reviewed, false).unwrap();
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap().0,
        target.to_string_lossy()
    );
    f.assert_opened(&[]);
    let repaired = f
        .store
        .overview()
        .unwrap()
        .activities
        .into_iter()
        .find(|activity| {
            activity.kind == "relinked" && activity.resource_id.as_deref() == Some(rid.as_str())
        })
        .unwrap();
    assert_eq!(repaired.name, "known-new-file-after-unknown-history.txt");
    let fresh = f.open(&rid);
    f.assert_opened(&[&fresh]);
    f.reopen();
    f.assert_opened(&[&fresh]);
    let origin: (Option<String>, Option<String>) = f
        .readonly_db()
        .query_row(
            "SELECT observed_resource_id,identity FROM activity_origins WHERE activity_id=?",
            [&old.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(origin, (Some(rid), None));
    f.assert_root_scope(
        &old,
        &root_id,
        &root_identity,
        "unknown-history-private-personnel.txt",
    );
    assert!(f
        .store
        .overview()
        .unwrap()
        .activities
        .iter()
        .any(|activity| activity.id == repaired.id));
}

#[test]
fn excluded_known_location_cannot_regain_public_history_after_anonymous_relink() {
    let mut f = Fixture::new();
    let original = f.root.join("public/excluded-move-private-personnel.txt");
    f.write(&original);
    let root_id = f.store.grant_root(&f.root).unwrap();
    let root_identity = f.root_identity(&root_id);
    let rid = f.import(&original);
    let old_identity = f.identity(&rid);
    let old = f.open(&rid);
    f.assert_opened(&[&old]);
    let private = f.root.join("private");
    fs::create_dir(&private).unwrap();
    f.change(Mutation::Exclude {
        root_id: root_id.clone(),
        relative_path: "private".into(),
    });
    let moved = private.join("excluded-move-private-personnel.txt");
    fs::rename(&original, &moved).unwrap();
    // 显式选择已排除位置中的同一对象，仅授予单文件权限并更新其已知位置。
    let imported = f
        .store
        .import_files(&[moved.to_string_lossy().into_owned()], None, true)
        .unwrap();
    assert_eq!(imported.items[0].resource_id.as_deref(), Some(rid.as_str()));
    assert_eq!(imported.items[0].status, "already_present");
    assert_eq!(f.identity(&rid), old_identity);
    assert_eq!(f.store.file_grants().unwrap()[0].id, rid);
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap().0,
        moved.to_string_lossy()
    );
    f.assert_opened(&[&old]);
    f.change(Mutation::RevokeFile { id: rid.clone() });
    f.assert_opened(&[]);
    f.assert_name_hidden(&old.name);
    assert!(f.store.resolve_resource(&rid).is_err());
    let anonymous = f.store.review_relink(&rid).unwrap();
    assert!(!anonymous.source.authorized);
    assert_ne!(anonymous.source.name, old.name);
    let target = f
        .outside
        .join("public-new-target-after-excluded-location.txt");
    f.write(&target);
    let selection = capture_import_selections(&[target.to_string_lossy().into_owned()]).remove(0);
    let reviewed = f
        .store
        .review_relink_selection(&anonymous, &selection)
        .unwrap();
    assert!(reviewed.target.is_none());
    f.store.relink_reviewed(&reviewed, false).unwrap();
    let selected_identity = f.identity(&rid);
    assert_ne!(old_identity, selected_identity);
    f.assert_opened(&[]);
    f.assert_name_hidden(&old.name);
    let repaired = f
        .store
        .overview()
        .unwrap()
        .activities
        .into_iter()
        .find(|activity| {
            activity.kind == "relinked" && activity.resource_id.as_deref() == Some(rid.as_str())
        })
        .unwrap();
    assert_eq!(
        repaired.name,
        "public-new-target-after-excluded-location.txt"
    );
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap().0,
        target.to_string_lossy()
    );
    let fresh = f.open(&rid);
    f.assert_opened(&[&fresh]);
    f.reopen();
    f.assert_opened(&[&fresh]);
    f.assert_name_hidden(&old.name);
    f.assert_origin(&old, &rid, &old_identity);
    f.assert_origin(&fresh, &rid, &selected_identity);
    f.assert_root_scope(
        &old,
        &root_id,
        &root_identity,
        "public/excluded-move-private-personnel.txt",
    );
    let retired: bool = f
        .readonly_db()
        .query_row(
            "SELECT retired_location_valid FROM activity_root_scopes WHERE activity_id=?1 AND root_id=?2",
            params![old.id, root_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        !retired,
        "the excluded last-known location borrowed its earlier public scope"
    );
    f.assert_relink_audit(&rid, None, &old_identity, &selected_identity);
    assert!(f
        .store
        .overview()
        .unwrap()
        .activities
        .iter()
        .any(|activity| activity.id == repaired.id));
}

#[test]
fn independent_original_record_controls_named_history_and_empty_name_requires_canonical_authority()
{
    let mut f = Fixture::new();
    let original = f.root.join("public/retained-original-personnel.txt");
    f.write(&original);
    let root_id = f.store.grant_root(&f.root).unwrap();
    let root_identity = f.root_identity(&root_id);
    let canonical_id = f.import(&original);
    let original_identity = f.identity(&canonical_id);
    let old = f.open(&canonical_id);
    let selected = f.outside.join("canonical-current-project.txt");
    f.write(&selected);
    let reviewed = f.select(&canonical_id, &selected);
    f.store.relink_reviewed(&reviewed, false).unwrap();
    f.assert_opened(&[&old]);
    // 原件一直保留在原目录；重新收纳后，其identity由另一个UUID持有。
    assert!(original.is_file());
    let original_id = f.import(&original);
    assert_ne!(original_id, canonical_id);
    assert_eq!(f.identity(&original_id), original_identity);
    f.change(Mutation::RevokeFile {
        id: canonical_id.clone(),
    });
    f.assert_opened(&[&old]);
    f.assert_current_placeholder(&old, &canonical_id);
    assert!(f.store.resolve_resource(&original_id).is_ok());
    f.change(Mutation::RevokeRoot {
        id: root_id.clone(),
    });
    f.assert_opened(&[]);
    let authorize_existing = |fixture: &Fixture, path: &Path, expected_id: &str| {
        let imported = fixture
            .store
            .import_files(&[path.to_string_lossy().into_owned()], None, true)
            .unwrap();
        assert_eq!(imported.items[0].resource_id.as_deref(), Some(expected_id));
        assert_eq!(imported.items[0].status, "already_present");
    };
    authorize_existing(&f, &selected, &canonical_id);
    assert!(f.store.resolve_resource(&canonical_id).is_ok());
    assert!(f.store.resolve_resource(&original_id).is_err());
    // 外层B有权限不能替代内层独立A的权限。
    f.assert_opened(&[]);
    assert_eq!(f.store.grant_root(&f.root).unwrap(), root_id);
    f.assert_opened(&[&old]);
    let alias = f.outside.join("retained-original-alias.txt");
    fs::hard_link(&original, &alias).unwrap();
    // 原路径仍在；同一物理对象的当前记录指向根外别名，才可区分VISIBLE=0和根scope回落。
    authorize_existing(&f, &alias, &original_id);
    assert_eq!(f.identity(&original_id), original_identity);
    assert_eq!(
        f.store.resolve_resource(&original_id).unwrap().0,
        alias.to_string_lossy()
    );
    f.change(Mutation::RevokeFile {
        id: original_id.clone(),
    });
    assert!(original.is_file());
    assert!(f.store.resolve_resource(&original_id).is_err());
    let eligible_scope: bool = f
        .readonly_db()
        .query_row(
            "SELECT hs.retired_location_valid=1 AND s.active=1 AND l.valid=1 AND hs.root_identity=l.identity FROM activity_root_scopes hs JOIN scan_roots s ON s.id=hs.root_id JOIN root_locators l ON l.root_id=s.id WHERE hs.activity_id=?1 AND hs.root_id=?2",
            params![old.id, root_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(eligible_scope);
    // A的独立行存在且不可见，应保留标量0，不能当作无行NULL而借旧scope放行。
    f.assert_opened(&[]);
    authorize_existing(&f, &alias, &original_id);
    f.assert_opened(&[&old]);
    f.change(Mutation::RevokeFile {
        id: canonical_id.clone(),
    });
    f.assert_opened(&[&old]);
    f.assert_current_placeholder(&old, &canonical_id);
    let mut db = Connection::open(&f.database).unwrap();
    let tx = db.transaction().unwrap();
    tx.execute("UPDATE activities SET name='' WHERE id=?", [&old.id])
        .unwrap();
    tx.execute("UPDATE settings SET value=value+1 WHERE key='revision'", [])
        .unwrap();
    tx.commit().unwrap();
    drop(db);
    // 空历史名只能回填当前B名称，因此只有A授权时仍不应显示。
    f.assert_opened(&[]);
    authorize_existing(&f, &selected, &canonical_id);
    let mut fallback = old.clone();
    fallback.name = "canonical-current-project.txt".into();
    f.assert_opened(&[&fallback]);
    f.change(Mutation::RevokeFile {
        id: original_id.clone(),
    });
    // B授权也不能跳过观察A的当前明确拒绝。
    f.assert_opened(&[]);
    authorize_existing(&f, &alias, &original_id);
    f.assert_opened(&[&fallback]);
    f.reopen();
    f.assert_opened(&[&fallback]);
    f.assert_origin(&old, &canonical_id, &original_identity);
    f.assert_root_scope(
        &old,
        &root_id,
        &root_identity,
        "public/retained-original-personnel.txt",
    );
    let stored_name: String = f
        .readonly_db()
        .query_row("SELECT name FROM activities WHERE id=?", [&old.id], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(stored_name.is_empty());
    f.change(Mutation::RevokeFile { id: canonical_id });
    f.assert_opened(&[]);
}

fn recent_resources(fixture: &Fixture) -> Vec<Resource> {
    let page = fixture
        .store
        .resources(ResourceQuery {
            workspace_id: None,
            kind: "all".into(),
            mode: "recent".into(),
            offset: 0,
        })
        .unwrap();
    assert!(!page.has_more);
    page.items
}

#[test]
fn recent_usage_keeps_same_identity_moves_but_not_cross_identity_relink_and_keeps_web_history() {
    let mut f = Fixture::new();
    let original = f.outside.join("recent-original.txt");
    f.write(&original);
    let rid = f.import(&original);
    let original_identity = f.identity(&rid);
    let opened = f.open(&rid);
    let web_id = f
        .store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation: Mutation::SaveWeb {
                id: None,
                name: "Saved web history without file origin".into(),
                url: "https://example.com/recent-history".into(),
                workspace_id: None,
            },
        })
        .unwrap()
        .id
        .unwrap();
    let web_opened = f.open(&web_id);
    let has_web_origin: bool = f
        .readonly_db()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM activity_origins WHERE activity_id=?)",
            [&web_opened.id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!has_web_origin);
    let renamed = f.outside.join("recent-renamed.txt");
    fs::rename(&original, &renamed).unwrap();
    f.store.relink(&rid, &renamed).unwrap();
    let moved = f.root.join("recent-moved.txt");
    fs::rename(&renamed, &moved).unwrap();
    f.store.relink(&rid, &moved).unwrap();
    let recent = recent_resources(&f);
    assert_eq!(recent.len(), 2);
    assert!(recent.iter().any(|resource| resource.id == web_id));
    assert!(recent
        .iter()
        .any(|resource| resource.id == rid && resource.name == "recent-moved.txt"));
    assert_eq!(f.identity(&rid), original_identity);
    let target = f.outside.join("recent-never-opened-target.txt");
    f.write(&target);
    let reviewed = f.select(&rid, &target);
    f.store.relink_reviewed(&reviewed, false).unwrap();
    let recent = recent_resources(&f);
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].id, web_id);
    f.assert_origin(&opened, &rid, &original_identity);
    let fresh = f.open(&rid);
    assert_eq!(fresh.name, "recent-never-opened-target.txt");
    let recent = recent_resources(&f);
    assert_eq!(recent.len(), 2);
    assert!(recent.iter().any(|resource| resource.id == rid));
    assert!(recent.iter().any(|resource| resource.id == web_id));
    f.reopen();
    let recent = recent_resources(&f);
    assert_eq!(recent.len(), 2);
    assert!(recent.iter().any(|resource| resource.id == rid));
    assert!(recent.iter().any(|resource| resource.id == web_id));
}

#[test]
fn merge_into_unopened_object_does_not_inherit_source_recent_usage() {
    let mut f = Fixture::new();
    let original = f.outside.join("opened-before-merge.txt");
    f.write(&original);
    let source_id = f.import(&original);
    let opened = f.open(&source_id);
    let original_identity = f.identity(&source_id);
    let target = f.outside.join("unopened-merge-target.txt");
    f.write(&target);
    let target_id = f.import(&target);
    assert_eq!(recent_resources(&f)[0].id, source_id);
    let reviewed = f.select(&source_id, &target);
    assert_eq!(reviewed.target.as_ref().unwrap().resource_id, target_id);
    f.store.relink_reviewed(&reviewed, true).unwrap();
    assert!(recent_resources(&f).is_empty());
    assert!(f.store.resolve_resource(&target_id).is_err());
    f.assert_origin(&opened, &source_id, &original_identity);
    f.reopen();
    assert!(recent_resources(&f).is_empty());
    let fresh = f.open(&source_id);
    assert_eq!(fresh.name, "unopened-merge-target.txt");
    let recent = recent_resources(&f);
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].id, source_id);
}

#[test]
fn recent_merge_sort_uses_target_actual_open_time_instead_of_later_source_open() {
    let mut f = Fixture::new();
    let target = f.outside.join("earlier-opened-target.txt");
    f.write(&target);
    let target_id = f.import(&target);
    let target_identity = f.identity(&target_id);
    let target_opened = f.open(&target_id);
    let middle = f.outside.join("middle-opened-independent.txt");
    f.write(&middle);
    let middle_id = f.import(&middle);
    let middle_opened = f.open(&middle_id);
    let original = f.outside.join("later-opened-source.txt");
    f.write(&original);
    let source_id = f.import(&original);
    let source_identity = f.identity(&source_id);
    let source_opened = f.open(&source_id);
    let mut db = Connection::open(&f.database).unwrap();
    let tx = db.transaction().unwrap();
    // 为三个真实Core打开固定不同时间，避免依赖机器速度及毫秒碰撞来验证排序。
    for (activity, at) in [
        (&target_opened, target_opened.at - 3000.),
        (&middle_opened, target_opened.at - 2000.),
        (&source_opened, target_opened.at - 1000.),
    ] {
        tx.execute(
            "UPDATE activities SET at=?1 WHERE id=?2",
            params![at, activity.id],
        )
        .unwrap();
    }
    tx.execute("UPDATE settings SET value=value+1 WHERE key='revision'", [])
        .unwrap();
    tx.commit().unwrap();
    drop(db);
    let ids = recent_resources(&f)
        .into_iter()
        .map(|resource| resource.id)
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![source_id.clone(), middle_id.clone(), target_id.clone()]
    );
    let reviewed = f.select(&source_id, &target);
    f.store.relink_reviewed(&reviewed, true).unwrap();
    let recent = recent_resources(&f);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].id, middle_id);
    assert_eq!(recent[1].id, source_id);
    assert_eq!(recent[1].name, "earlier-opened-target.txt");
    f.assert_origin(&source_opened, &source_id, &source_identity);
    f.assert_origin(&target_opened, &target_id, &target_identity);
    f.reopen();
    let ids = recent_resources(&f)
        .into_iter()
        .map(|resource| resource.id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![middle_id.clone(), source_id.clone()]);
    f.open(&source_id);
    let ids = recent_resources(&f)
        .into_iter()
        .map(|resource| resource.id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![source_id, middle_id]);
}
