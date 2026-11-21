use std::fs;
use worktwill_core::{model::*, Store};
fn fixture() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("v131-")
        .tempdir_in(std::env::var_os("WORKTWILL_TEST_ROOT").expect("approved fixture root"))
        .unwrap()
}
fn write(store: &Store, id: &str, version: u32, text: &str, deleted: bool) -> NoteWrite {
    NoteWrite {
        scope: store.notes_state().unwrap().scope,
        id: id.into(),
        text: text.into(),
        pinned: false,
        expected_version: version,
        deleted,
    }
}
#[test]
fn notes_retry_order_delete_restore_and_catalog_isolation() {
    let dir = fixture();
    let path = dir.path().join("app/db.sqlite3");
    let store = Store::open(&path).unwrap();
    let revision = store.query_revision().unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let first = write(&store, &id, 0, "第一版\n纯文本 <b>", false);
    let saved = store.write_note(first.clone()).unwrap();
    assert_eq!(saved.version, 1);
    assert_eq!(store.write_note(first).unwrap().version, 1);
    assert_eq!(store.notes().unwrap().len(), 1);
    let late = write(&store, &id, 1, "迟到旧稿", false);
    let mut second = write(&store, &id, 1, "已确认新稿", false);
    second.pinned = true;
    store.write_note(second).unwrap();
    assert_eq!(store.write_note(late).unwrap_err().code, "conflict");
    assert_eq!(store.query_revision().unwrap(), revision);
    let backup = dir.path().join("notes-backup.sqlite3");
    store.backup_to(&backup).unwrap();
    let stale_scope = write(&store, &id, 2, "恢复前待写", false);
    store
        .write_note(write(&store, &id, 2, "删除", true))
        .unwrap();
    assert!(store.notes().unwrap().is_empty());
    assert_eq!(
        store
            .write_note(write(&store, &id, 0, "复活", false))
            .unwrap_err()
            .code,
        "conflict"
    );
    store.restore_from(&backup).unwrap();
    assert_eq!(store.write_note(stale_scope).unwrap_err().code, "conflict");
    let notes = store.notes().unwrap();
    assert_eq!(notes[0].text, "已确认新稿");
    assert!(notes[0].pinned);
    drop(store);
    let reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.notes().unwrap()[0].text, "已确认新稿");
}
#[test]
fn v8_migration_preserves_existing_data_and_rolls_back_on_table_conflict() {
    let dir = fixture();
    let path = dir.path().join("app/db.sqlite3");
    let store = Store::open(&path).unwrap();
    store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation: Mutation::CreateWorkspace {
                name: "原工作区".into(),
                resource_ids: None,
            },
        })
        .unwrap();
    drop(store);
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("DROP TABLE scratch_notes; PRAGMA user_version=8; CREATE VIEW scratch_notes AS SELECT 1 AS id;").unwrap();
    assert!(Store::open(&path).is_err());
    assert_eq!(
        db.pragma_query_value::<u32, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        8
    );
    db.execute_batch("DROP VIEW scratch_notes;").unwrap();
    drop(db);
    let store = Store::open(&path).unwrap();
    assert!(store.notes().unwrap().is_empty());
    assert_eq!(store.overview().unwrap().workspaces[0].name, "原工作区");
}
#[test]
fn recommendations_filter_before_pagination_but_preserve_explicit_intent_and_real_open_time() {
    let dir = fixture();
    let root = dir.path().join("documents");
    fs::create_dir(&root).unwrap();
    let store = Store::open(dir.path().join("app/db.sqlite3")).unwrap();
    store.grant_root(&root).unwrap();
    let names = [
        ".DS_Store",
        "Thumbs.db",
        "~$合同.docx",
        ".env",
        "合同.jpg",
        "20261121.mp4",
        "合同.txt",
    ];
    let mut ids = Vec::new();
    for name in names {
        let path = root.join(name);
        fs::write(&path, b"synthetic v131 fixture").unwrap();
        let imported = store
            .import_files(&[path.to_string_lossy().into_owned()], None, false)
            .unwrap();
        ids.push(imported.items[0].resource_id.clone().unwrap());
    }
    let query = || ResourceQuery {
        workspace_id: None,
        kind: "all".into(),
        mode: "modified".into(),
        offset: 0,
    };
    let page = store.resources(query()).unwrap();
    assert_eq!(page.items.len(), 4);
    assert!(!page.has_more);
    assert!(page.items.iter().all(|r| r.opened_at.is_none()));
    let search = store
        .search(SearchRequest {
            request_id: 1,
            query: ".DS_Store".into(),
            workspace_id: None,
            kind: "all".into(),
        })
        .unwrap();
    assert_eq!(search.page.items[0].id, ids[0]);
    store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation: Mutation::Favorite {
                id: ids[0].clone(),
                value: true,
            },
        })
        .unwrap();
    assert_eq!(store.resources(query()).unwrap().items.len(), 5);
    let target = store.prepare_open(&ids[2]).unwrap();
    store.record_open_checked(target).unwrap();
    let recent = store
        .resources(ResourceQuery {
            mode: "recent".into(),
            ..query()
        })
        .unwrap();
    assert_eq!(recent.items.len(), 1);
    assert!(recent.items[0].opened_at.unwrap() > recent.items[0].modified_at - 1000.);
    assert_eq!(store.resources(query()).unwrap().items.len(), 6);
}

#[test]
fn old_artifact_events_and_calendar_use_the_same_filter_before_paging() {
    let dir = fixture();
    let root = dir.path().join("documents");
    fs::create_dir(&root).unwrap();
    let store = Store::open(dir.path().join("app/db.sqlite3")).unwrap();
    let root_id = store.grant_root(&root).unwrap();
    let mut artifact_id = String::new();
    for n in 0..105 {
        let folder = root.join(format!("folder-{n:03}"));
        fs::create_dir(&folder).unwrap();
        let path = folder.join(".DS_Store");
        fs::write(&path, b"synthetic metadata").unwrap();
        let result = store
            .import_files(&[path.to_string_lossy().into_owned()], None, false)
            .unwrap();
        if n == 0 {
            artifact_id = result.items[0].resource_id.clone().unwrap();
        }
    }
    let file = root.join("正常资料.txt");
    fs::write(&file, b"ordinary fixture").unwrap();
    store
        .import_files(&[file.to_string_lossy().into_owned()], None, false)
        .unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    let from = now - 86400000.;
    let to = now + 86400000.;
    let request = || worktwill_core::timeline::TimelineRequest {
        from: None,
        to: None,
        month_from: from,
        month_to: to,
        day_boundaries: vec![from, to],
        filter: "discovered".into(),
        basis: "observed".into(),
        workspace_id: None,
        offset: 0,
        cursor: None,
    };
    let first = store.timeline(request()).unwrap();
    assert_eq!(first.events.len(), 1);
    assert_eq!(first.calendar.iter().map(|p| p.count).sum::<u32>(), 1);
    assert!(!first.has_more);
    let change = |mutation| {
        store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation,
            })
            .unwrap()
    };
    change(Mutation::Favorite {
        id: artifact_id,
        value: true,
    });
    let explicit = store.timeline(request()).unwrap();
    assert_eq!(explicit.events.len(), 2);
    assert_eq!(explicit.calendar.iter().map(|p| p.count).sum::<u32>(), 2);
    change(Mutation::RevokeRoot { id: root_id });
    let revoked = store.timeline(request()).unwrap();
    assert!(revoked.events.is_empty());
    assert!(revoked.calendar.is_empty());
}
#[test]
fn malformed_current_note_backup_is_rejected_before_replacing_saved_notes() {
    let dir = fixture();
    let path = dir.path().join("app/db.sqlite3");
    let store = Store::open(&path).unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    store
        .write_note(write(&store, &id, 0, "保留当前便签", false))
        .unwrap();
    let bad = dir.path().join("invalid.sqlite3");
    store.backup_to(&bad).unwrap();
    let db = rusqlite::Connection::open(&bad).unwrap();
    db.execute("DROP TABLE scratch_notes", []).unwrap();
    drop(db);
    assert!(store.restore_from(&bad).is_err());
    assert_eq!(store.notes().unwrap()[0].text, "保留当前便签");
}

#[test]
fn known_build_artifacts_are_quiet_but_explicit_links_and_search_keep_them() {
    let dir = fixture();
    let root = dir.path().join("documents");
    fs::create_dir(&root).unwrap();
    let store = Store::open(dir.path().join("app/db.sqlite3")).unwrap();
    store.grant_root(&root).unwrap();
    let paths = [
        "target/debug/.fingerprint/lib-objc2",
        ".next-core-local/server/pages-manifest.json",
        ".env",
        "target/业务资料.txt",
        "debug/会议纪要.txt",
        "媒体/照片.jpg",
    ];
    let mut ids = Vec::new();
    for relative in paths {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"synthetic build quality fixture").unwrap();
        let result = store
            .import_files(&[path.to_string_lossy().into_owned()], None, false)
            .unwrap();
        ids.push(result.items[0].resource_id.clone().unwrap());
    }
    let query = || ResourceQuery {
        workspace_id: None,
        kind: "all".into(),
        mode: "modified".into(),
        offset: 0,
    };
    assert_eq!(store.resources(query()).unwrap().items.len(), 4);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    let request = || worktwill_core::timeline::TimelineRequest {
        from: None,
        to: None,
        month_from: now - 86400000.,
        month_to: now + 86400000.,
        day_boundaries: vec![now - 86400000., now + 86400000.],
        filter: "discovered".into(),
        basis: "observed".into(),
        workspace_id: None,
        offset: 0,
        cursor: None,
    };
    let events = store.timeline(request()).unwrap();
    assert_eq!(events.events.len(), 4);
    assert_eq!(events.calendar.iter().map(|p| p.count).sum::<u32>(), 4);
    let searched = store
        .search(SearchRequest {
            request_id: 1,
            query: "lib-objc2".into(),
            workspace_id: None,
            kind: "file".into(),
        })
        .unwrap();
    assert_eq!(searched.page.items[0].id, ids[0]);
    let workspace = store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation: Mutation::CreateWorkspace {
                name: "明确保留产物".into(),
                resource_ids: Some(vec![ids[0].clone()]),
            },
        })
        .unwrap();
    assert!(workspace.id.is_some());
    assert_eq!(store.resources(query()).unwrap().items.len(), 5);
    let events = store.timeline(request()).unwrap();
    assert_eq!(events.events.len(), 5);
    assert_eq!(events.calendar.iter().map(|p| p.count).sum::<u32>(), 5);
}
