use std::{fs, path::Path};
use worktwill_core::{model::*, Store};

fn mutate(store: &Store, mutation: Mutation) -> MutationResponse {
    store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation,
        })
        .unwrap()
}
fn consistent(db: &Path) {
    let db = rusqlite::Connection::open(db).unwrap();
    let bad:u64=db.query_row("SELECT count(*) FROM resources r LEFT JOIN search_documents d ON d.entity_id=r.id WHERE d.entity_id IS NULL OR d.modified_at IS NOT r.modified_at",[],|r|r.get(0)).unwrap();
    assert_eq!(
        bad, 0,
        "every resource must carry the same nullable time in its projection"
    );
    let bad:u64=db.query_row("SELECT count(*) FROM workspaces w LEFT JOIN search_documents d ON d.entity_id=w.id WHERE d.entity_id IS NULL OR d.modified_at IS NOT NULL",[],|r|r.get(0)).unwrap();
    assert_eq!(
        bad, 0,
        "workspaces never acquire a resource modification time"
    );
}
fn set_time(path: &Path, seconds: u64) {
    let file = fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_times(
        fs::FileTimes::new()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds)),
    )
    .unwrap();
}

#[test]
fn every_formal_write_path_preserves_modified_projection_through_rebuild_and_restore() {
    let parent = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
    let directory = tempfile::Builder::new()
        .prefix("modified-projection-")
        .tempdir_in(parent)
        .unwrap();
    let root = directory.path().join("documents");
    fs::create_dir(&root).unwrap();
    let db = directory.path().join("app/catalog.sqlite3");
    let store = Store::open(&db).unwrap();
    let path = root.join("合同.txt");
    fs::write(&path, b"original fixture").unwrap();
    set_time(&path, 100);
    store.grant_root(&root).unwrap();
    store.scan_all().unwrap();
    consistent(&db);
    let rid = store
        .search(SearchRequest {
            request_id: 1,
            query: "合同".into(),
            workspace_id: None,
            kind: "file".into(),
        })
        .unwrap()
        .page
        .items[0]
        .id
        .clone();
    set_time(&path, 200);
    store.scan_all().unwrap();
    consistent(&db);
    let renamed = root.join("合同改名.txt");
    fs::rename(&path, &renamed).unwrap();
    store.scan_all().unwrap();
    consistent(&db);
    let moved = root.join("sub");
    fs::create_dir(&moved).unwrap();
    let path = moved.join("合同改名.txt");
    fs::rename(&renamed, &path).unwrap();
    store.scan_all().unwrap();
    consistent(&db);
    let workspace = mutate(
        &store,
        Mutation::CreateWorkspace {
            name: "合同工作区".into(),
            resource_ids: Some(vec![rid.clone()]),
        },
    )
    .id
    .unwrap();
    mutate(
        &store,
        Mutation::RenameWorkspace {
            id: workspace,
            name: "合同工作区新名".into(),
        },
    );
    let web = mutate(
        &store,
        Mutation::SaveWeb {
            id: None,
            name: "合同网页".into(),
            url: "https://example.com/a".into(),
            workspace_id: None,
        },
    )
    .id
    .unwrap();
    consistent(&db);
    mutate(
        &store,
        Mutation::SaveWeb {
            id: Some(web),
            name: "合同网页新名".into(),
            url: "https://example.com/b".into(),
            workspace_id: None,
        },
    );
    consistent(&db);
    let external = directory.path().join("外部合同.txt");
    fs::write(&external, b"individually granted").unwrap();
    set_time(&external, 300);
    let single = store
        .import_files(&[external.to_string_lossy().into_owned()], None, true)
        .unwrap()
        .items[0]
        .resource_id
        .clone()
        .unwrap();
    consistent(&db);
    set_time(&external, 400);
    store.scan_all().unwrap();
    consistent(&db);
    // 人工合并使用另一已导入对象，同时保留原UUID与关系；派生时间必须跟随最终事实。
    store.relink_confirmed(&rid, &external, true).unwrap();
    consistent(&db);
    assert_ne!(rid, single);
    mutate(&store, Mutation::RebuildSearch);
    consistent(&db);
    let backup = directory.path().join("backup.sqlite3");
    store.backup_to(&backup).unwrap();
    store.restore_from(&backup).unwrap();
    consistent(&db);
    let content = fs::read(&external).unwrap();
    assert_eq!(content, b"individually granted");
}
