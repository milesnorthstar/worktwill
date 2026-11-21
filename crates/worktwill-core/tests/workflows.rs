use std::{fs, path::PathBuf, sync::Arc};
use worktwill_core::{model::*, Store};

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    db: PathBuf,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        let parent = std::env::var_os("WORKTWILL_TEST_ROOT")
            .expect("Set WORKTWILL_TEST_ROOT to a confirmed disposable fixture directory");
        let directory = tempfile::Builder::new()
            .prefix("core-")
            .tempdir_in(parent)
            .unwrap();
        let root = directory.path().join("documents");
        fs::create_dir(&root).unwrap();
        let db = directory.path().join("app/data.sqlite3");
        let store = Store::open(&db).unwrap();
        Self {
            _directory: directory,
            root,
            db,
            store,
        }
    }
    fn seed(&self) -> (String, String) {
        fs::write(self.root.join("合作合同_张三.txt"), b"fixture document").unwrap();
        let root = self.store.grant_root(&self.root).unwrap();
        self.store.scan_all().unwrap();
        let rid = self.page().items[0].id.clone();
        (root, rid)
    }
    fn page(&self) -> ResourcePage {
        self.store
            .resources(ResourceQuery {
                workspace_id: None,
                kind: "all".into(),
                mode: "modified".into(),
                offset: 0,
            })
            .unwrap()
    }
    fn workspace(&self, name: &str) -> String {
        mutation(
            &self.store,
            Mutation::CreateWorkspace {
                name: name.into(),
                resource_ids: None,
            },
        )
        .unwrap()
        .id
        .unwrap()
    }
}
fn mutation(store: &Store, change: Mutation) -> Result<MutationResponse> {
    store.mutate(MutationRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        mutation: change,
    })
}
fn find(store: &Store, q: &str) -> Vec<Resource> {
    store
        .search(SearchRequest {
            request_id: 1,
            query: q.into(),
            workspace_id: None,
            kind: "all".into(),
        })
        .unwrap()
        .page
        .items
}
fn in_workspace(store: &Store, w: &str) -> Vec<Resource> {
    store
        .resources(ResourceQuery {
            workspace_id: Some(w.into()),
            kind: "all".into(),
            mode: "modified".into(),
            offset: 0,
        })
        .unwrap()
        .items
}

#[test]
fn native_file_reference_survives_rename_and_database_restart() {
    let fixture = Fixture::new();
    let (_, rid) = fixture.seed();
    let workspace = fixture.workspace("客户 A");
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: workspace.clone(),
            resource_ids: vec![rid.clone()],
        },
    )
    .unwrap();
    let original = fixture.root.join("合作合同_张三.txt");
    let renamed = fixture.root.join("改名后的合同.txt");
    let before = fs::read(&original).unwrap();
    fs::rename(original, &renamed).unwrap();
    fixture.store.scan_all().unwrap();
    let reopened = Store::open(&fixture.db).unwrap();
    let resources = in_workspace(&reopened, &workspace);
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].id, rid);
    assert_eq!(
        reopened.resolve_resource(&rid).unwrap().0,
        renamed.canonicalize().unwrap().to_string_lossy()
    );
    assert_eq!(fs::read(renamed).unwrap(), before);
}

#[test]
fn old_directory_membership_cannot_authorize_moved_resource_after_new_root_revoked() {
    let fixture = Fixture::new();
    let (_, rid) = fixture.seed();
    let workspace = fixture.workspace("移动边界");
    let observed = fixture
        .store
        .overview()
        .unwrap()
        .activities
        .into_iter()
        .find(|event| event.resource_id.as_deref() == Some(&rid))
        .unwrap()
        .at;
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: workspace.clone(),
            resource_ids: vec![rid.clone()],
        },
    )
    .unwrap();
    // 故意不重扫旧目录，模拟暂停目录或通知延迟留下的历史归属。
    let destination = fixture.root.with_file_name("other-documents");
    fs::create_dir(&destination).unwrap();
    fs::rename(
        fixture.root.join("合作合同_张三.txt"),
        destination.join("合作合同_张三.txt"),
    )
    .unwrap();
    let new_root = fixture.store.grant_root(&destination).unwrap();
    fixture.store.scan_root(&new_root).unwrap();
    assert_eq!(find(&fixture.store, "张三")[0].id, rid);
    mutation(&fixture.store, Mutation::RevokeRoot { id: new_root }).unwrap();
    assert!(find(&fixture.store, "张三").is_empty());
    assert!(fixture.page().items.is_empty());
    let retained = in_workspace(&fixture.store, &workspace);
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].status, "permission_denied");
    assert!(retained[0].location.is_empty());
    assert!(!fixture
        .store
        .overview()
        .unwrap()
        .activities
        .iter()
        .any(|event| event.resource_id.as_deref() == Some(&rid)));
    assert!(fixture.store.resolve_resource(&rid).is_err());
    let day = (observed / 86_400_000.).floor() * 86_400_000.;
    let page = fixture
        .store
        .timeline(worktwill_core::timeline::TimelineRequest {
            from: Some(day),
            to: Some(day + 86_400_000.),
            month_from: day,
            month_to: day + 86_400_000.,
            day_boundaries: vec![day, day + 86_400_000.],
            filter: "discovered".into(),
            basis: "observed".into(),
            workspace_id: None,
            offset: 0,
            cursor: None,
        })
        .unwrap();
    assert!(page.events.is_empty());
    assert!(page.calendar.is_empty());
}

#[test]
fn overlapping_root_concurrent_discovery_deduplicates_and_revoke_is_union() {
    let fixture = Fixture::new();
    let (parent, rid) = fixture.seed();
    let child = fixture.root.join("nested");
    fs::create_dir(&child).unwrap();
    fs::rename(
        fixture.root.join("合作合同_张三.txt"),
        child.join("合作合同_张三.txt"),
    )
    .unwrap();
    let child_id = fixture.store.grant_root(&child).unwrap();
    let store = Arc::new(Store::open(&fixture.db).unwrap());
    let first = store.clone();
    let parent_copy = parent.clone();
    let second = store.clone();
    let child_copy = child_id.clone();
    let a = std::thread::spawn(move || first.scan_root(&parent_copy).unwrap());
    let b = std::thread::spawn(move || second.scan_root(&child_copy).unwrap());
    a.join().unwrap();
    b.join().unwrap();
    assert_eq!(find(&store, "张").len(), 1);
    assert_eq!(find(&store, "张")[0].id, rid);
    mutation(&store, Mutation::RevokeRoot { id: parent }).unwrap();
    assert_eq!(find(&store, "张").len(), 1);
    mutation(&store, Mutation::RevokeRoot { id: child_id }).unwrap();
    assert!(find(&store, "张").is_empty());
    assert_eq!(
        store.resolve_resource(&rid).unwrap_err().code,
        "not_authorized"
    );
}

#[test]
fn revoked_resources_are_anonymous_in_workspaces_and_hidden_elsewhere() {
    let fixture = Fixture::new();
    let (root, rid) = fixture.seed();
    let workspace = fixture.workspace("资料");
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: workspace.clone(),
            resource_ids: vec![rid.clone()],
        },
    )
    .unwrap();
    mutation(
        &fixture.store,
        Mutation::Favorite {
            id: rid.clone(),
            value: true,
        },
    )
    .unwrap();
    fixture.store.record_open(&rid).unwrap();
    mutation(&fixture.store, Mutation::RevokeRoot { id: root }).unwrap();
    assert!(fixture.page().items.is_empty());
    assert!(find(&fixture.store, "张").is_empty());
    assert!(!fixture
        .store
        .overview()
        .unwrap()
        .activities
        .iter()
        .any(|a| a.resource_id == Some(rid.clone())));
    let placeholder = &in_workspace(&fixture.store, &workspace)[0];
    assert_eq!(placeholder.status, "permission_denied");
    assert_eq!(placeholder.name, "未授权的文件");
    assert!(placeholder.location.is_empty());
    assert_eq!(placeholder.size, 0.);
}

#[test]
fn same_path_replacement_never_inherits_original_relationships() {
    let fixture = Fixture::new();
    let (_, rid) = fixture.seed();
    let workspace = fixture.workspace("资料");
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: workspace.clone(),
            resource_ids: vec![rid.clone()],
        },
    )
    .unwrap();
    let original = fixture.root.join("合作合同_张三.txt");
    let new_file = fixture.root.join("replacement.txt");
    fs::write(&new_file, b"different object").unwrap();
    fs::rename(new_file, original).unwrap();
    fixture.store.scan_all().unwrap();
    assert_eq!(
        fixture.store.resolve_resource(&rid).unwrap_err().code,
        "identity_unconfirmed"
    );
    assert_eq!(in_workspace(&fixture.store, &workspace)[0].id, rid);
    let matches = find(&fixture.store, "合作合同");
    assert_eq!(matches.len(), 2);
    let new = matches.iter().find(|r| r.id != rid).unwrap();
    assert!(new.workspace_ids.is_empty());
    assert_eq!(new.status, "available");
}

#[test]
fn transfer_replay_does_not_remove_a_later_readded_source() {
    let fixture = Fixture::new();
    let (_, rid) = fixture.seed();
    let source = fixture.workspace("来源");
    let target = fixture.workspace("目标");
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: source.clone(),
            resource_ids: vec![rid.clone()],
        },
    )
    .unwrap();
    let request = MutationRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        mutation: Mutation::Transfer {
            source: source.clone(),
            target: target.clone(),
            resource_ids: vec![rid.clone()],
        },
    };
    fixture.store.mutate(request.clone()).unwrap();
    assert!(in_workspace(&fixture.store, &source).is_empty());
    assert_eq!(in_workspace(&fixture.store, &target).len(), 1);
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: source.clone(),
            resource_ids: vec![rid.clone()],
        },
    )
    .unwrap();
    fixture.store.mutate(request).unwrap();
    assert_eq!(in_workspace(&fixture.store, &source).len(), 1);
}

#[test]
fn failed_transfer_rolls_back_target_and_preserves_all_source_references() {
    let fixture = Fixture::new();
    let (_, rid) = fixture.seed();
    let source = fixture.workspace("来源");
    let target = fixture.workspace("目标");
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: source.clone(),
            resource_ids: vec![rid.clone()],
        },
    )
    .unwrap();
    let result = mutation(
        &fixture.store,
        Mutation::Transfer {
            source: source.clone(),
            target: target.clone(),
            resource_ids: vec![rid, "nonexistent".into()],
        },
    );
    assert!(result.is_err());
    assert_eq!(in_workspace(&fixture.store, &source).len(), 1);
    assert!(in_workspace(&fixture.store, &target).is_empty());
}

#[test]
fn short_cjk_literal_queries_and_workspace_scope_are_correct() {
    let fixture = Fixture::new();
    let (_, rid) = fixture.seed();
    let workspace = fixture.workspace("2026招聘");
    let web = mutation(
        &fixture.store,
        Mutation::SaveWeb {
            workspace_id: None,
            id: None,
            name: "100%_报价 OR \"说明\"".into(),
            url: "https://example.test/岗位?q=2026招聘".into(),
        },
    )
    .unwrap()
    .id
    .unwrap();
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: workspace.clone(),
            resource_ids: vec![rid, web.clone()],
        },
    )
    .unwrap();
    for q in ["张", "张三", "合同"] {
        assert_eq!(find(&fixture.store, q).len(), 1);
    }
    for q in ["100%", "_", "OR", "\"说明\""] {
        assert_eq!(find(&fixture.store, q)[0].id, web);
    }
    let found = fixture
        .store
        .search(SearchRequest {
            request_id: 7,
            query: "张".into(),
            workspace_id: Some(workspace),
            kind: "file".into(),
        })
        .unwrap();
    assert_eq!(found.request_id, 7);
    assert_eq!(found.page.items.len(), 1);
    assert!(mutation(
        &fixture.store,
        Mutation::SaveWeb {
            workspace_id: None,
            id: None,
            name: "unsafe".into(),
            url: "javascript:alert(1)".into()
        }
    )
    .is_err());
}

#[test]
fn index_rebuild_does_not_modify_resources_or_associations() {
    let fixture = Fixture::new();
    let (_, rid) = fixture.seed();
    let w = fixture.workspace("资料");
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: w.clone(),
            resource_ids: vec![rid.clone()],
        },
    )
    .unwrap();
    mutation(&fixture.store, Mutation::RebuildSearch).unwrap();
    assert_eq!(find(&fixture.store, "张")[0].id, rid);
    assert_eq!(in_workspace(&fixture.store, &w).len(), 1);
}

#[test]
fn backup_restore_preserves_uuids_and_requires_reauthorization() {
    let fixture = Fixture::new();
    let (_, rid) = fixture.seed();
    let w = fixture.workspace("资料");
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: w.clone(),
            resource_ids: vec![rid.clone()],
        },
    )
    .unwrap();
    let backup = fixture._directory.path().join("backup.sqlite3");
    fixture.store.backup_to(&backup).unwrap();
    mutation(&fixture.store, Mutation::DeleteWorkspace { id: w.clone() }).unwrap();
    let before_restore = fixture.store.overview().unwrap();
    fixture.store.restore_from(&backup).unwrap();
    let after_restore = fixture.store.overview().unwrap();
    assert!(after_restore.permission_version > before_restore.permission_version);
    assert!(after_restore.revision > before_restore.revision);
    fixture.store.restore_from(&backup).unwrap();
    let restored_again = fixture.store.overview().unwrap();
    assert!(restored_again.permission_version > after_restore.permission_version);
    assert!(restored_again.revision > after_restore.revision);
    assert_eq!(in_workspace(&fixture.store, &w)[0].id, rid);
    assert!(find(&fixture.store, "张").is_empty());
    fixture.store.grant_root(&fixture.root).unwrap();
    fixture.store.scan_all().unwrap();
    assert_eq!(find(&fixture.store, "张")[0].id, rid);
    assert!(fixture.store.backup_to(&backup).is_err());
}

#[test]
fn rejected_backup_keeps_live_database_usable() {
    let fixture = Fixture::new();
    fixture.seed();
    let w = fixture.workspace("保留我");
    let invalid = fixture._directory.path().join("invalid.sqlite3");
    fs::write(&invalid, b"invalid backup").unwrap();
    assert!(fixture.store.restore_from(&invalid).is_err());
    assert!(fixture
        .store
        .overview()
        .unwrap()
        .workspaces
        .iter()
        .any(|item| item.id == w));
}

#[test]
fn exclusions_and_open_gate_enforce_real_path_boundaries() {
    let fixture = Fixture::new();
    let (root, rid) = fixture.seed();
    let hidden = fixture.root.join("private");
    fs::create_dir(&hidden).unwrap();
    fs::write(hidden.join("秘密.txt"), b"private").unwrap();
    fixture.store.scan_all().unwrap();
    assert_eq!(find(&fixture.store, "秘密").len(), 1);
    mutation(
        &fixture.store,
        Mutation::Exclude {
            root_id: root.clone(),
            relative_path: "private".into(),
        },
    )
    .unwrap();
    assert!(find(&fixture.store, "秘密").is_empty());
    assert!(mutation(
        &fixture.store,
        Mutation::Exclude {
            root_id: root,
            relative_path: "../outside".into()
        }
    )
    .is_err());
    assert!(fixture.store.check_open_type("danger.exe").is_err());
    assert!(fixture.store.check_open_type("script.command").is_err());
    assert!(fixture.store.check_open_type("contract.docx").is_ok());
    assert!(fixture.store.resolve_resource(&rid).is_ok());
}

#[cfg(unix)]
#[test]
fn symbolic_link_does_not_expand_authorized_scan() {
    let fixture = Fixture::new();
    fixture.seed();
    let outside = fixture._directory.path().join("outside.txt");
    fs::write(&outside, b"not authorized").unwrap();
    std::os::unix::fs::symlink(outside, fixture.root.join("外部链接.txt")).unwrap();
    fixture.store.scan_all().unwrap();
    assert!(find(&fixture.store, "外部链接").is_empty());
}

#[test]
fn search_applies_authorization_and_workspace_scope_before_result_limit() {
    let fixture = Fixture::new();
    let denied = fixture.root.join("denied");
    let allowed = fixture.root.join("allowed");
    fs::create_dir(&denied).unwrap();
    fs::create_dir(&allowed).unwrap();
    for index in 0..2100 {
        let parent = if index < 2060 { &denied } else { &allowed };
        fs::write(
            parent.join(format!("合同_{index:04}.txt")),
            b"acceptance fixture",
        )
        .unwrap();
    }
    let denied_root = fixture.store.grant_root(&denied).unwrap();
    fixture.store.grant_root(&allowed).unwrap();
    fixture.store.scan_all().unwrap();
    mutation(&fixture.store, Mutation::RevokeRoot { id: denied_root }).unwrap();
    let results = find(&fixture.store, "合同");
    assert_eq!(results.len(), 30);
    assert_eq!(results[0].name, "合同_2060.txt");
    assert_eq!(results[29].name, "合同_2089.txt");
    let wid = fixture.workspace("限额后段资料");
    let all = fixture.page().items;
    let last = all.iter().find(|r| r.name == "合同_2099.txt").unwrap();
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: wid.clone(),
            resource_ids: vec![last.id.clone()],
        },
    )
    .unwrap();
    let scoped = fixture
        .store
        .search(SearchRequest {
            request_id: 1,
            query: "合同".into(),
            workspace_id: Some(wid),
            kind: "file".into(),
        })
        .unwrap();
    assert_eq!(scoped.page.items.len(), 1);
    assert_eq!(scoped.page.items[0].id, last.id);
}

#[test]
fn workspace_web_save_is_atomic_and_operation_replay_does_not_duplicate() {
    let fixture = Fixture::new();
    let wid = fixture.workspace("网页与文件");
    let request = MutationRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        mutation: Mutation::SaveWeb {
            id: None,
            name: "招聘后台".into(),
            url: "https://example.com/jobs".into(),
            workspace_id: Some(wid.clone()),
        },
    };
    let result = fixture.store.mutate(request.clone()).unwrap();
    let repeated = fixture.store.mutate(request).unwrap();
    assert_eq!(result.id, repeated.id);
    let items = in_workspace(&fixture.store, &wid);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].location, "https://example.com/jobs");
    assert_eq!(items[0].id, result.id.unwrap());
    let activity_count = fixture.store.overview().unwrap().activities.len();
    let error = mutation(
        &fixture.store,
        Mutation::SaveWeb {
            id: None,
            name: "不能留下的孤立网页".into(),
            url: "https://example.com/orphan".into(),
            workspace_id: Some("missing-workspace".into()),
        },
    );
    assert!(error.is_err());
    assert!(find(&fixture.store, "孤立网页").is_empty());
    assert_eq!(
        fixture.store.overview().unwrap().activities.len(),
        activity_count
    );
}

#[test]
fn unlinking_and_deleting_workspace_preserves_other_references_and_all_source_metadata() {
    let fixture = Fixture::new();
    let (_, rid) = fixture.seed();
    let a = fixture.workspace("用户路径 A");
    let b = fixture.workspace("用户路径 B");
    let path = fixture.root.join("合作合同_张三.txt");
    let content = fs::read(&path).unwrap();
    let before = path.metadata().unwrap();
    for wid in [&a, &b] {
        for _ in 0..2 {
            mutation(
                &fixture.store,
                Mutation::Link {
                    workspace_id: wid.clone(),
                    resource_ids: vec![rid.clone()],
                },
            )
            .unwrap();
        }
        assert_eq!(in_workspace(&fixture.store, wid).len(), 1);
    }
    mutation(
        &fixture.store,
        Mutation::Unlink {
            workspace_id: a.clone(),
            resource_ids: vec![rid.clone()],
        },
    )
    .unwrap();
    mutation(&fixture.store, Mutation::DeleteWorkspace { id: a }).unwrap();
    drop(fixture.store);
    let reopened = Store::open(&fixture.db).unwrap();
    assert_eq!(in_workspace(&reopened, &b)[0].id, rid);
    let after = path.metadata().unwrap();
    assert_eq!(fs::read(&path).unwrap(), content);
    assert_eq!(before.len(), after.len());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
}

#[test]
fn future_schema_is_rejected_without_modification() {
    let fixture = Fixture::new();
    let db = rusqlite::Connection::open(&fixture.db).unwrap();
    db.pragma_update(None, "user_version", 999).unwrap();
    drop(db);
    assert!(
        matches!(Store::open(&fixture.db),Err(AppError{code,..}) if code=="unsupported_data_version")
    );
    let db = rusqlite::Connection::open(&fixture.db).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        999
    );
}

#[test]
fn single_file_import_rejects_without_grant_and_retains_partial_success() {
    let fixture = Fixture::new();
    let (_, authorized) = fixture.seed();
    let outside = fixture.root.with_file_name("outside.txt");
    fs::write(&outside, b"outside fixture").unwrap();
    let wid = fixture.workspace("拖放验收");
    let paths = vec![
        fixture
            .root
            .join("合作合同_张三.txt")
            .to_string_lossy()
            .into_owned(),
        outside.to_string_lossy().into_owned(),
    ];
    let partial = fixture
        .store
        .import_files(&paths, Some(&wid), false)
        .unwrap();
    assert_eq!(partial.items[0].status, "added");
    assert_eq!(partial.items[1].status, "not_authorized");
    assert_eq!(partial.items[0].message_en, None);
    assert_eq!(partial.items[1].message.as_deref(), Some("未授权此文件"));
    assert_eq!(
        partial.items[1].message_en.as_deref(),
        Some("This file is not authorized.")
    );
    assert_eq!(in_workspace(&fixture.store, &wid).len(), 1);
    assert!(fixture.store.needs_file_grant(&outside).unwrap());
    let granted = fixture
        .store
        .import_files(&paths[1..], Some(&wid), true)
        .unwrap();
    let rid = granted.items[0].resource_id.clone().unwrap();
    assert_eq!(granted.items[0].status, "added");
    assert_eq!(fixture.store.file_grants().unwrap().len(), 1);
    let neighbor = outside.with_file_name("not-granted.txt");
    fs::write(&neighbor, b"never indexed").unwrap();
    fs::write(&outside, b"modified granted file").unwrap();
    fixture.store.scan_all().unwrap();
    assert_eq!(find(&fixture.store, "outside")[0].id, rid);
    assert!(find(&fixture.store, "not-granted").is_empty());
    assert!(fixture.store.resolve_resource(&rid).is_ok());
    mutation(&fixture.store, Mutation::RevokeFile { id: rid.clone() }).unwrap();
    assert!(find(&fixture.store, "outside").is_empty());
    assert!(fixture.store.resolve_resource(&rid).is_err());
    let retained = in_workspace(&fixture.store, &wid);
    assert_eq!(retained.len(), 2);
    assert!(retained
        .iter()
        .any(|r| r.id == authorized && r.status == "available"));
    assert!(retained
        .iter()
        .any(|r| r.id == rid && r.status == "permission_denied"));
}

#[test]
fn file_timeline_records_first_discovery_once_then_real_changes_not_scan_replays() {
    let fixture = Fixture::new();
    let (_, rid) = fixture.seed();
    let first = fixture.store.overview().unwrap().activities;
    assert_eq!(
        first.iter().filter(|a| a.kind == "file_discovered").count(),
        1
    );
    assert_eq!(first[0].source, "filesystem");
    assert!(first[0].file_modified_at.is_some());
    fixture.store.scan_all().unwrap();
    fixture.store.scan_all().unwrap();
    assert_eq!(
        fixture.store.overview().unwrap().activities.len(),
        first.len()
    );
    fs::write(
        fixture.root.join("合作合同_张三.txt"),
        b"changed contents longer than before",
    )
    .unwrap();
    let reopened = Store::open(&fixture.db).unwrap();
    reopened.scan_all().unwrap();
    reopened.scan_all().unwrap();
    let events = reopened.overview().unwrap().activities;
    assert_eq!(
        events
            .iter()
            .filter(|a| a.kind == "file_modified" && a.resource_id.as_deref() == Some(&rid))
            .count(),
        1
    );
    let child = fixture.root.join("nested");
    fs::create_dir(&child).unwrap();
    fs::rename(
        fixture.root.join("合作合同_张三.txt"),
        child.join("合作合同_张三.txt"),
    )
    .unwrap();
    reopened.grant_root(&child).unwrap();
    reopened.scan_all().unwrap();
    assert_eq!(reopened.overview().unwrap().activities.len(), events.len());
}

#[test]
fn explicit_relink_merge_keeps_original_uuid_and_unions_references_without_touching_source() {
    let fixture = Fixture::new();
    let (_, rid) = fixture.seed();
    let a = fixture.workspace("原记录");
    let b = fixture.workspace("新记录");
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: a.clone(),
            resource_ids: vec![rid.clone()],
        },
    )
    .unwrap();
    let path = fixture.root.join("合作合同_张三.txt");
    let replacement = fixture.root.join("replacement.txt");
    fs::write(&replacement, b"replacement fixture").unwrap();
    fs::rename(&replacement, &path).unwrap();
    fixture.store.scan_all().unwrap();
    let other = find(&fixture.store, "张三")
        .into_iter()
        .find(|r| r.id != rid)
        .unwrap()
        .id;
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: b.clone(),
            resource_ids: vec![other.clone()],
        },
    )
    .unwrap();
    mutation(
        &fixture.store,
        Mutation::Favorite {
            id: other,
            value: true,
        },
    )
    .unwrap();
    assert_eq!(
        fixture.store.relink(&rid, &path).unwrap_err().code,
        "merge_confirmation_required"
    );
    assert_eq!(in_workspace(&fixture.store, &b).len(), 1);
    let before = fs::read(&path).unwrap();
    let mtime = path.metadata().unwrap().modified().unwrap();
    fixture.store.relink_confirmed(&rid, &path, true).unwrap();
    assert_eq!(in_workspace(&fixture.store, &a)[0].id, rid);
    assert_eq!(in_workspace(&fixture.store, &b)[0].id, rid);
    assert!(in_workspace(&fixture.store, &b)[0].favorite);
    assert_eq!(find(&fixture.store, "张三").len(), 1);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(path.metadata().unwrap().modified().unwrap(), mtime);
}

#[test]
fn version_one_backup_migrates_in_staging_without_reactivating_grants() {
    let fixture = Fixture::new();
    let legacy = fixture.root.with_file_name("legacy.sqlite3");
    let db = rusqlite::Connection::open(&legacy).unwrap();
    db.execute_batch(include_str!("../src/schema.sql")).unwrap();
    db.execute(
        "INSERT INTO workspaces VALUES('legacy-workspace','旧版资料',0)",
        [],
    )
    .unwrap();
    drop(db);
    fixture.store.restore_from(&legacy).unwrap();
    assert_eq!(
        fixture.store.overview().unwrap().workspaces[0].name,
        "旧版资料"
    );
    assert!(fixture.store.overview().unwrap().file_grants.is_empty());
    let reopened = Store::open(&fixture.db).unwrap();
    assert_eq!(reopened.overview().unwrap().workspaces.len(), 1);
}

#[test]
fn full_search_paginates_without_duplicates_and_sort_and_type_filters_work() {
    let fixture = Fixture::new();
    for index in 0..115 {
        fs::write(
            fixture.root.join(format!("合同_{index:03}.txt")),
            b"fixture",
        )
        .unwrap();
    }
    fixture.store.grant_root(&fixture.root).unwrap();
    fixture.store.scan_all().unwrap();
    let quick = fixture
        .store
        .search(SearchRequest {
            request_id: 1,
            query: "合同".into(),
            workspace_id: None,
            kind: "all".into(),
        })
        .unwrap();
    assert_eq!(quick.page.items.len(), 30);
    assert!(quick.page.has_more);
    let request = |offset, kind: &str, sort: &str| SearchPageRequest {
        request: SearchRequest {
            request_id: 2,
            query: "合同".into(),
            workspace_id: None,
            kind: kind.into(),
        },
        offset,
        sort: sort.into(),
        cursor: None,
        modified_from: None,
        modified_to: None,
        path_prefix: None,
    };
    let first = fixture
        .store
        .search_page(request(0, "file", "name"))
        .unwrap();
    let mut next = request(100, "file", "name");
    next.cursor = first.next_cursor.clone();
    let second = fixture.store.search_page(next).unwrap();
    assert_eq!(first.page.items.len(), 100);
    assert!(first.page.has_more);
    assert_eq!(second.page.items.len(), 15);
    assert!(!second.page.has_more);
    let ids = first
        .page
        .items
        .iter()
        .chain(second.page.items.iter())
        .map(|r| r.id.clone())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), 115);
    assert!(fixture
        .store
        .search_page(request(0, "web", "modified"))
        .unwrap()
        .page
        .items
        .is_empty());
}

#[test]
fn workspace_reorder_survives_restart_and_rejects_partial_or_duplicate_lists() {
    let fixture = Fixture::new();
    let a = fixture.workspace("A");
    let b = fixture.workspace("B");
    mutation(
        &fixture.store,
        Mutation::ReorderWorkspaces {
            ids: vec![a.clone(), b.clone()],
        },
    )
    .unwrap();
    let reopened = Store::open(&fixture.db).unwrap();
    assert_eq!(
        reopened
            .overview()
            .unwrap()
            .workspaces
            .iter()
            .map(|w| w.id.clone())
            .collect::<Vec<_>>(),
        vec![a.clone(), b]
    );
    assert!(mutation(
        &reopened,
        Mutation::ReorderWorkspaces {
            ids: vec![a.clone(), a]
        }
    )
    .is_err());
    assert!(mutation(&reopened, Mutation::ReorderWorkspaces { ids: vec![] }).is_err());
}

#[test]
fn including_excluded_folder_restores_visibility_without_changing_identity() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.root.join("private")).unwrap();
    fs::write(fixture.root.join("private/report.txt"), b"report").unwrap();
    let root = fixture.store.grant_root(&fixture.root).unwrap();
    fixture.store.scan_all().unwrap();
    let original = fixture.page().items[0].id.clone();
    let wid = fixture.workspace("review");
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: wid.clone(),
            resource_ids: vec![original.clone()],
        },
    )
    .unwrap();
    mutation(
        &fixture.store,
        Mutation::Exclude {
            root_id: root.clone(),
            relative_path: "private".into(),
        },
    )
    .unwrap();
    assert!(find(&fixture.store, "report").is_empty());
    assert_eq!(
        in_workspace(&fixture.store, &wid)[0].status,
        "permission_denied"
    );
    mutation(
        &fixture.store,
        Mutation::Include {
            root_id: root,
            relative_path: "private".into(),
        },
    )
    .unwrap();
    fixture.store.scan_all().unwrap();
    assert_eq!(find(&fixture.store, "report")[0].id, original);
    assert_eq!(in_workspace(&fixture.store, &wid)[0].status, "available");
    assert_eq!(
        fs::read(fixture.root.join("private/report.txt")).unwrap(),
        b"report"
    );
}

#[test]
fn delete_web_clears_all_references_and_search_but_rejects_file_deletion() {
    let fixture = Fixture::new();
    let (_, file) = fixture.seed();
    let a = fixture.workspace("a");
    let b = fixture.workspace("b");
    let web = mutation(
        &fixture.store,
        Mutation::SaveWeb {
            id: None,
            name: "public link".into(),
            url: "https://example.com/".into(),
            workspace_id: Some(a.clone()),
        },
    )
    .unwrap()
    .id
    .unwrap();
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: b.clone(),
            resource_ids: vec![web.clone()],
        },
    )
    .unwrap();
    mutation(
        &fixture.store,
        Mutation::Favorite {
            id: web.clone(),
            value: true,
        },
    )
    .unwrap();
    let request = MutationRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        mutation: Mutation::DeleteWeb { id: web },
    };
    fixture.store.mutate(request.clone()).unwrap();
    fixture.store.mutate(request).unwrap();
    assert!(in_workspace(&fixture.store, &a).is_empty());
    assert!(in_workspace(&fixture.store, &b).is_empty());
    assert!(find(&fixture.store, "public link").is_empty());
    assert!(mutation(&fixture.store, Mutation::DeleteWeb { id: file.clone() }).is_err());
    assert!(fixture.store.resolve_resource(&file).is_ok());
}

#[test]
fn create_workspace_with_five_selected_files_is_atomic_idempotent_and_persistent() {
    let fixture = Fixture::new();
    let mut originals = Vec::new();
    for index in 0..5 {
        let folder = fixture.root.join(format!("folder-{index}"));
        fs::create_dir(&folder).unwrap();
        let path = folder.join("same-name.txt");
        fs::write(&path, format!("original-{index}")).unwrap();
        let metadata = path.metadata().unwrap();
        originals.push((
            path.clone(),
            fs::read(&path).unwrap(),
            metadata.len(),
            metadata.modified().unwrap(),
        ));
    }
    fixture.store.grant_root(&fixture.root).unwrap();
    fixture.store.scan_all().unwrap();
    let ids: Vec<_> = fixture
        .page()
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect();
    assert_eq!(ids.len(), 5);
    let existing = fixture.workspace("原工作区");
    mutation(
        &fixture.store,
        Mutation::Link {
            workspace_id: existing.clone(),
            resource_ids: ids.clone(),
        },
    )
    .unwrap();
    mutation(
        &fixture.store,
        Mutation::Favorite {
            id: ids[0].clone(),
            value: true,
        },
    )
    .unwrap();
    let request = MutationRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        mutation: Mutation::CreateWorkspace {
            name: "新客户".into(),
            resource_ids: Some([ids.clone(), vec![ids[0].clone()]].concat()),
        },
    };
    let created = fixture.store.mutate(request.clone()).unwrap();
    assert_eq!(created.changed, 6);
    let wid = created.id.unwrap();
    let repeated = fixture.store.mutate(request).unwrap();
    assert_eq!(repeated.id.as_deref(), Some(wid.as_str()));
    assert_eq!(repeated.changed, 6);
    let reopened = Store::open(&fixture.db).unwrap();
    let mut actual: Vec<_> = in_workspace(&reopened, &wid)
        .iter()
        .map(|item| item.id.clone())
        .collect();
    let mut expected = ids.clone();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    assert_eq!(in_workspace(&reopened, &existing).len(), 5);
    assert!(
        in_workspace(&reopened, &wid)
            .iter()
            .find(|item| item.id == ids[0])
            .unwrap()
            .favorite
    );
    let db = rusqlite::Connection::open(&fixture.db).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM activities WHERE kind='workspace_created' AND name='新客户'",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM resources", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        5
    );
    for (path, bytes, size, modified) in originals {
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let metadata = path.metadata().unwrap();
        assert_eq!(metadata.len(), size);
        assert_eq!(metadata.modified().unwrap(), modified);
    }
}

#[test]
fn create_workspace_rolls_back_references_projection_events_and_receipt_on_failure() {
    let fixture = Fixture::new();
    let (root, rid) = fixture.seed();
    let web = mutation(
        &fixture.store,
        Mutation::SaveWeb {
            id: None,
            name: "网页".into(),
            url: "https://example.com/".into(),
            workspace_id: None,
        },
    )
    .unwrap()
    .id
    .unwrap();
    mutation(&fixture.store, Mutation::RevokeRoot { id: root }).unwrap();
    let before = fixture.store.overview().unwrap();
    let request = MutationRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        mutation: Mutation::CreateWorkspace {
            name: "不得留下的工作区".into(),
            resource_ids: Some(vec![web.clone(), rid.clone()]),
        },
    };
    assert_eq!(
        fixture.store.mutate(request.clone()).unwrap_err().code,
        "not_authorized"
    );
    let after = fixture.store.overview().unwrap();
    assert_eq!(after.workspaces.len(), before.workspaces.len());
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.activities.len(), before.activities.len());
    assert!(find(&fixture.store, "不得留下的工作区").is_empty());
    let db = rusqlite::Connection::open(&fixture.db).unwrap();
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM workspace_resources", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM operations WHERE id=?",
            [&request.operation_id],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    for ids in [vec![web; 1001], vec![uuid::Uuid::new_v4().to_string()]] {
        assert!(mutation(
            &fixture.store,
            Mutation::CreateWorkspace {
                name: "也不能留下".into(),
                resource_ids: Some(ids)
            }
        )
        .is_err());
        assert_eq!(
            fixture.store.overview().unwrap().workspaces.len(),
            before.workspaces.len()
        );
    }
    fixture.store.grant_root(&fixture.root).unwrap();
    fixture.store.scan_all().unwrap();
    let created = fixture.store.mutate(request).unwrap().id.unwrap();
    assert_eq!(in_workspace(&fixture.store, &created).len(), 2);
}

#[test]
fn ordinary_create_workspace_keeps_existing_serialized_operation_receipt() {
    let fixture = Fixture::new();
    let payload = r#"{"type":"create_workspace","name":"旧格式创建"}"#;
    let change: Mutation = serde_json::from_str(payload).unwrap();
    assert_eq!(serde_json::to_string(&change).unwrap(), payload);
    let request = MutationRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        mutation: change,
    };
    let first = fixture.store.mutate(request.clone()).unwrap();
    let reopened = Store::open(&fixture.db).unwrap();
    let repeated = reopened.mutate(request).unwrap();
    assert_eq!(first.id, repeated.id);
    assert_eq!(reopened.overview().unwrap().workspaces.len(), 1);
}
