use std::fs;
use worktwill_core::{model::*, Store};

fn change(store: &Store, mutation: Mutation) -> MutationResponse {
    store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation,
        })
        .unwrap()
}
fn search(store: &Store, workspace: Option<String>, kind: &str) -> ResourcePage {
    store
        .search(SearchRequest {
            request_id: 1,
            query: "预算".into(),
            workspace_id: workspace,
            kind: kind.into(),
        })
        .unwrap()
        .page
}
fn fixture() -> (tempfile::TempDir, Store, std::path::PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix("search-groups-")
        .tempdir_in(std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root"))
        .unwrap();
    let root = dir.path().join("documents");
    fs::create_dir(&root).unwrap();
    let store = Store::open(dir.path().join("app/db.sqlite3")).unwrap();
    (dir, store, root)
}
#[test]
fn many_files_do_not_hide_matching_workspaces_and_websites() {
    let (_dir, store, root) = fixture();
    for i in 0..80 {
        fs::write(root.join(format!("预算_A{i:03}.txt")), b"fixture").unwrap();
    }
    store.grant_root(&root).unwrap();
    store.scan_all().unwrap();
    for i in 0..15 {
        change(
            &store,
            Mutation::CreateWorkspace {
                name: format!("预算_Z{i:03}"),
                resource_ids: None,
            },
        );
        change(
            &store,
            Mutation::SaveWeb {
                id: None,
                name: format!("预算_Z{i:03}网站"),
                url: format!("https://example.com/{i}"),
                workspace_id: None,
            },
        );
    }
    let page = search(&store, None, "all");
    assert_eq!(page.items.len(), 30);
    assert!(page.has_more);
    for kind in ["workspace", "file", "web"] {
        assert_eq!(page.items.iter().filter(|r| r.kind == kind).count(), 10);
    }
    assert_eq!(
        page.items
            .iter()
            .filter(|r| r.kind == "file")
            .map(|r| r.name.clone())
            .collect::<Vec<_>>(),
        (0..10)
            .map(|i| format!("预算_A{i:03}.txt"))
            .collect::<Vec<_>>()
    );
    let only_files = search(&store, None, "file");
    assert_eq!(only_files.items.len(), 30);
    assert!(only_files.items.iter().all(|r| r.kind == "file"));
}
#[test]
fn rare_groups_return_unused_capacity_to_relevance_and_keep_scope() {
    let (_dir, store, root) = fixture();
    for i in 0..80 {
        fs::write(root.join(format!("预算_A{i:03}.txt")), b"fixture").unwrap();
    }
    store.grant_root(&root).unwrap();
    store.scan_all().unwrap();
    let ws = change(
        &store,
        Mutation::CreateWorkspace {
            name: "预算_Z工作区".into(),
            resource_ids: None,
        },
    )
    .id
    .unwrap();
    let file_ids = search(&store, None, "file")
        .items
        .into_iter()
        .map(|r| r.id)
        .collect();
    change(
        &store,
        Mutation::Link {
            workspace_id: ws.clone(),
            resource_ids: file_ids,
        },
    );
    let web = change(
        &store,
        Mutation::SaveWeb {
            id: None,
            name: "预算_Z网站".into(),
            url: "https://example.com/".into(),
            workspace_id: None,
        },
    )
    .id
    .unwrap();
    let global = search(&store, None, "all");
    assert_eq!(global.items.len(), 30);
    assert_eq!(global.items.iter().filter(|r| r.kind == "file").count(), 28);
    assert!(global.items.iter().any(|r| r.id == web));
    assert!(global.items.iter().any(|r| r.id == ws));
    let scoped = search(&store, Some(ws.clone()), "all");
    assert_eq!(scoped.items.len(), 30);
    assert!(!scoped.items.iter().any(|r| r.id == web));
    assert!(scoped.items.iter().any(|r| r.id == ws));
    assert!(scoped
        .items
        .iter()
        .filter(|r| r.kind == "file")
        .all(|r| r.workspace_ids.contains(&ws)));
}
#[test]
fn reservations_never_restore_revoked_files_or_shrink_single_type_results() {
    let (_dir, store, root) = fixture();
    for i in 0..45 {
        fs::write(root.join(format!("预算_A{i:03}.txt")), b"fixture").unwrap();
    }
    let rid = store.grant_root(&root).unwrap();
    store.scan_all().unwrap();
    let files = search(&store, None, "all");
    assert_eq!(files.items.len(), 30);
    assert!(files.has_more);
    assert_eq!(files.items[29].name, "预算_A029.txt");
    change(&store, Mutation::RevokeRoot { id: rid });
    for i in 0..40 {
        change(
            &store,
            Mutation::SaveWeb {
                id: None,
                name: format!("预算_Z{i:03}"),
                url: format!("https://example.com/{i}"),
                workspace_id: None,
            },
        );
    }
    let page = search(&store, None, "all");
    assert_eq!(page.items.len(), 30);
    assert!(page.has_more);
    assert!(page.items.iter().all(|r| r.kind == "web"));
}
