#![cfg(any(target_os = "macos", windows))]

use rusqlite::{types::Value, Connection};
use std::{fs, path::PathBuf};
use worktwill_core::{capture_import_selections, model::*, ImportSelection, Store};

struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
    database: PathBuf,
    store: Store,
    root_id: String,
    source: String,
    target: String,
    source_workspace: String,
    target_workspace: String,
}

impl Fixture {
    fn new() -> Self {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("relink-review-")
            .tempdir_in(approved)
            .unwrap();
        let root = directory.path().join("documents");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("source.txt"), b"synthetic source").unwrap();
        fs::write(root.join("target.txt"), b"synthetic target").unwrap();
        let database = directory.path().join("app/catalog.sqlite3");
        let store = Store::open(&database).unwrap();
        let root_id = store.grant_root(&root).unwrap();
        store.scan_all().unwrap();
        let resources = store
            .resources(ResourceQuery {
                workspace_id: None,
                kind: "file".into(),
                mode: "modified".into(),
                offset: 0,
            })
            .unwrap()
            .items;
        let source = resources
            .iter()
            .find(|resource| resource.name == "source.txt")
            .unwrap()
            .id
            .clone();
        let target = resources
            .iter()
            .find(|resource| resource.name == "target.txt")
            .unwrap()
            .id
            .clone();
        let source_workspace = change(
            &store,
            Mutation::CreateWorkspace {
                name: "原记录工作区".into(),
                resource_ids: Some(vec![source.clone()]),
            },
        )
        .id
        .unwrap();
        let target_workspace = change(
            &store,
            Mutation::CreateWorkspace {
                name: "目标记录工作区".into(),
                resource_ids: Some(vec![target.clone()]),
            },
        )
        .id
        .unwrap();
        change(
            &store,
            Mutation::Favorite {
                id: target.clone(),
                value: true,
            },
        );
        Self {
            directory,
            root,
            database,
            store,
            root_id,
            source,
            target,
            source_workspace,
            target_workspace,
        }
    }

    fn selection(&self, name: &str) -> ImportSelection {
        capture_import_selections(&[self
            .root
            .join(name)
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned()])
        .remove(0)
    }

    fn snapshot(&self) -> Vec<Vec<Vec<Value>>> {
        let db = Connection::open(&self.database).unwrap();
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

    fn workspace_ids(&self, workspace: &str) -> Vec<String> {
        self.store
            .resources(ResourceQuery {
                workspace_id: Some(workspace.into()),
                kind: "file".into(),
                mode: "modified".into(),
                offset: 0,
            })
            .unwrap()
            .items
            .into_iter()
            .map(|resource| resource.id)
            .collect()
    }
}

fn change(store: &Store, mutation: Mutation) -> MutationResponse {
    store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation,
        })
        .unwrap()
}

#[test]
fn impact_review_and_cancel_are_read_only_and_expose_both_records() {
    let f = Fixture::new();
    let before = f.snapshot();
    let initial = f.store.review_relink(&f.source).unwrap();
    assert_eq!(initial.source.name, "source.txt");
    assert!(!initial.source.favorite);
    assert!(initial.source.authorized);
    assert!(initial.target.is_none());
    assert_eq!(initial.source.workspaces.len(), 1);
    assert_eq!(initial.source.workspaces[0].id, f.source_workspace);
    assert_eq!(initial.source.workspaces[0].name, "原记录工作区");
    let reviewed = f
        .store
        .review_relink_selection(&initial, &f.selection("target.txt"))
        .unwrap();
    let target = reviewed.target.as_ref().unwrap();
    assert_eq!(target.name, "target.txt");
    assert!(target.favorite);
    assert!(target.authorized);
    assert_eq!(target.workspaces.len(), 1);
    assert_eq!(target.workspaces[0].id, f.target_workspace);
    assert_eq!(target.workspaces[0].name, "目标记录工作区");
    assert!(
        before == f.snapshot(),
        "review or simulated cancellation changed persistent data"
    );
    // 用户取消相当于不提交review；两份引用和原文件必须完整保留。
    drop(reviewed);
    assert_eq!(f.workspace_ids(&f.source_workspace), vec![f.source.clone()]);
    assert_eq!(f.workspace_ids(&f.target_workspace), vec![f.target.clone()]);
    assert_eq!(
        fs::read(f.root.join("source.txt")).unwrap(),
        b"synthetic source"
    );
    assert_eq!(
        fs::read(f.root.join("target.txt")).unwrap(),
        b"synthetic target"
    );
}

#[test]
fn source_or_target_impact_changes_require_a_fresh_review() {
    for scenario in [
        "source_favorite",
        "target_favorite",
        "source_workspace_link",
        "target_workspace_link",
        "source_workspace_unlink",
        "target_workspace_unlink",
        "source_workspace_rename",
        "target_workspace_rename",
    ] {
        let f = Fixture::new();
        let initial = f.store.review_relink(&f.source).unwrap();
        let reviewed = f
            .store
            .review_relink_selection(&initial, &f.selection("target.txt"))
            .unwrap();
        let is_source = scenario.starts_with("source");
        let rid = if is_source { &f.source } else { &f.target };
        let workspace = if is_source {
            &f.source_workspace
        } else {
            &f.target_workspace
        };
        if scenario.ends_with("favorite") {
            change(
                &f.store,
                Mutation::Favorite {
                    id: rid.clone(),
                    value: is_source,
                },
            );
        } else if scenario.ends_with("workspace_link") {
            change(
                &f.store,
                Mutation::CreateWorkspace {
                    name: "确认后新增关联".into(),
                    resource_ids: Some(vec![rid.clone()]),
                },
            );
        } else if scenario.ends_with("workspace_unlink") {
            change(
                &f.store,
                Mutation::Unlink {
                    workspace_id: workspace.clone(),
                    resource_ids: vec![rid.clone()],
                },
            );
        } else {
            change(
                &f.store,
                Mutation::RenameWorkspace {
                    id: workspace.clone(),
                    name: "确认后改名".into(),
                },
            );
        }
        let before = f.snapshot();
        let error = f.store.relink_reviewed(&reviewed, true).unwrap_err();
        assert_eq!(error.code, "permission_changed", "scenario: {scenario}");
        assert!(
            before == f.snapshot(),
            "stale review mutated data: {scenario}"
        );
    }
}

#[test]
fn source_changed_after_first_confirmation_is_rejected_before_target_review() {
    let f = Fixture::new();
    let initial = f.store.review_relink(&f.source).unwrap();
    change(
        &f.store,
        Mutation::Favorite {
            id: f.source.clone(),
            value: true,
        },
    );
    let before = f.snapshot();
    let result = f
        .store
        .review_relink_selection(&initial, &f.selection("target.txt"));
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("changed source was accepted for target review"),
    };
    assert_eq!(error.code, "permission_changed");
    assert!(before == f.snapshot());
}

#[test]
fn revocation_restore_or_relink_after_review_cannot_commit_stale_consent() {
    for scenario in ["revoke", "restore", "source_relink", "target_relink"] {
        let f = Fixture::new();
        let backup = f.directory.path().join("before-review.sqlite3");
        f.store.backup_to(&backup).unwrap();
        let initial = f.store.review_relink(&f.source).unwrap();
        let reviewed = f
            .store
            .review_relink_selection(&initial, &f.selection("target.txt"))
            .unwrap();
        match scenario {
            "revoke" => {
                change(
                    &f.store,
                    Mutation::RevokeRoot {
                        id: f.root_id.clone(),
                    },
                );
            }
            "restore" => {
                f.store.restore_from(&backup).unwrap();
            }
            _ => {
                let replacement = f.root.join("new-relink-object.txt");
                fs::write(&replacement, b"different selected object after review").unwrap();
                let rid = if scenario == "source_relink" {
                    &f.source
                } else {
                    &f.target
                };
                f.store.relink(rid, &replacement).unwrap();
            }
        }
        let before = f.snapshot();
        assert_eq!(
            f.store.relink_reviewed(&reviewed, true).unwrap_err().code,
            "permission_changed",
            "scenario: {scenario}"
        );
        assert!(
            before == f.snapshot(),
            "stale review mutated data: {scenario}"
        );
    }
}

#[test]
fn replacing_candidate_after_review_never_grants_or_merges_the_new_object() {
    let f = Fixture::new();
    let initial = f.store.review_relink(&f.source).unwrap();
    let reviewed = f
        .store
        .review_relink_selection(&initial, &f.selection("target.txt"))
        .unwrap();
    let replacement = f.root.join("candidate-replacement.tmp");
    fs::write(&replacement, b"unreviewed different physical candidate").unwrap();
    fs::rename(&replacement, f.root.join("target.txt")).unwrap();
    let before = f.snapshot();
    let error = f.store.relink_reviewed(&reviewed, true).unwrap_err();
    assert!(matches!(
        error.code.as_str(),
        "identity_unconfirmed" | "permission_changed"
    ));
    assert!(
        before == f.snapshot(),
        "candidate replacement changed database"
    );
    assert_eq!(f.workspace_ids(&f.source_workspace), vec![f.source.clone()]);
    assert_eq!(f.workspace_ids(&f.target_workspace), vec![f.target.clone()]);
}

#[test]
fn candidate_added_to_library_after_review_requires_new_merge_impact_confirmation() {
    let f = Fixture::new();
    let fresh = f.root.join("not-yet-indexed.txt");
    fs::write(
        &fresh,
        b"same physical selection later independently indexed",
    )
    .unwrap();
    let initial = f.store.review_relink(&f.source).unwrap();
    let reviewed = f
        .store
        .review_relink_selection(&initial, &f.selection("not-yet-indexed.txt"))
        .unwrap();
    assert!(reviewed.target.is_none());
    let added = f
        .store
        .import_files(
            &[fresh.canonicalize().unwrap().to_string_lossy().into_owned()],
            Some(&f.target_workspace),
            true,
        )
        .unwrap();
    let new_id = added.items[0].resource_id.clone().unwrap();
    for merge in [false, true] {
        let before = f.snapshot();
        assert_eq!(
            f.store.relink_reviewed(&reviewed, merge).unwrap_err().code,
            "permission_changed"
        );
        assert!(before == f.snapshot());
    }
    assert!(f.workspace_ids(&f.target_workspace).contains(&new_id));
    assert_eq!(f.workspace_ids(&f.source_workspace), vec![f.source.clone()]);
}

#[test]
fn unchanged_review_merges_all_workspaces_and_favorites_only_after_explicit_merge() {
    let f = Fixture::new();
    let shared = change(
        &f.store,
        Mutation::CreateWorkspace {
            name: "共同关联".into(),
            resource_ids: Some(vec![f.source.clone(), f.target.clone()]),
        },
    )
    .id
    .unwrap();
    let initial = f.store.review_relink(&f.source).unwrap();
    let reviewed = f
        .store
        .review_relink_selection(&initial, &f.selection("target.txt"))
        .unwrap();
    assert_eq!(reviewed.source.workspaces.len(), 2);
    assert_eq!(reviewed.target.as_ref().unwrap().workspaces.len(), 2);
    // 无关打开只增加revision，不改变确认所覆盖的身份、授权及引用影响。
    f.store.record_open(&f.target).unwrap();
    let before = f.snapshot();
    assert_eq!(
        f.store.relink_reviewed(&reviewed, false).unwrap_err().code,
        "merge_confirmation_required"
    );
    assert!(before == f.snapshot());
    let source_bytes = fs::read(f.root.join("source.txt")).unwrap();
    let target_bytes = fs::read(f.root.join("target.txt")).unwrap();
    f.store.relink_reviewed(&reviewed, true).unwrap();
    for workspace in [&f.source_workspace, &f.target_workspace, &shared] {
        assert_eq!(f.workspace_ids(workspace), vec![f.source.clone()]);
    }
    let current = f
        .store
        .resources(ResourceQuery {
            workspace_id: None,
            kind: "file".into(),
            mode: "modified".into(),
            offset: 0,
        })
        .unwrap()
        .items;
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].id, f.source);
    assert_eq!(current[0].name, "target.txt");
    assert!(current[0].favorite);
    assert_eq!(current[0].workspace_ids.len(), 3);
    assert_eq!(
        f.store.resolve_resource(&f.source).unwrap().0,
        f.root
            .join("target.txt")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
    );
    assert_eq!(fs::read(f.root.join("source.txt")).unwrap(), source_bytes);
    assert_eq!(fs::read(f.root.join("target.txt")).unwrap(), target_bytes);
    // 同一review不能再次吞并或写入第二次relinked事件。
    let committed = f.snapshot();
    assert_eq!(
        f.store.relink_reviewed(&reviewed, true).unwrap_err().code,
        "permission_changed"
    );
    assert!(committed == f.snapshot());
}

#[test]
fn same_identity_no_op_relink_review_cannot_write_a_second_event_on_replay() {
    let f = Fixture::new();
    let initial = f.store.review_relink(&f.source).unwrap();
    let reviewed = f
        .store
        .review_relink_selection(&initial, &f.selection("source.txt"))
        .unwrap();
    assert!(reviewed.target.is_none());
    let versions = f.store.overview().unwrap();
    let db = Connection::open(&f.database).unwrap();
    let before: u32 = db
        .query_row(
            "SELECT count(*) FROM activities WHERE resource_id=? AND kind='relinked'",
            [&f.source],
            |row| row.get(0),
        )
        .unwrap();
    let original = fs::read(f.root.join("source.txt")).unwrap();
    f.store.relink_reviewed(&reviewed, false).unwrap();
    assert_eq!(
        f.store.overview().unwrap().permission_version,
        versions.permission_version
    );
    assert_eq!(f.workspace_ids(&f.source_workspace), vec![f.source.clone()]);
    assert_eq!(f.workspace_ids(&f.target_workspace), vec![f.target.clone()]);
    assert_eq!(
        f.store.resolve_resource(&f.source).unwrap().0,
        f.root
            .join("source.txt")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM activities WHERE resource_id=? AND kind='relinked'",
            [&f.source],
            |row| row.get::<_, u32>(0),
        )
        .unwrap(),
        before + 1
    );
    // 同路径同身份不改变影响或权限，仍须以本次提交记录拒绝旧确认重放。
    let committed = f.snapshot();
    assert_eq!(
        f.store.relink_reviewed(&reviewed, false).unwrap_err().code,
        "permission_changed"
    );
    assert!(committed == f.snapshot());
    assert_eq!(fs::read(f.root.join("source.txt")).unwrap(), original);
}
