use std::{collections::HashSet, fs, path::PathBuf};
use worktwill_core::{model::*, Store};

struct Fixture {
    _directory: tempfile::TempDir,
    db: PathBuf,
    root: PathBuf,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        let parent = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("search-filters-")
            .tempdir_in(parent)
            .unwrap();
        let root = directory.path().join("documents");
        fs::create_dir(&root).unwrap();
        let db = directory.path().join("app/data.sqlite3");
        let store = Store::open(&db).unwrap();
        Self {
            _directory: directory,
            db,
            root,
            store,
        }
    }
    fn mutate(&self, mutation: Mutation) -> MutationResponse {
        self.store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation,
            })
            .unwrap()
    }
    fn request(&self) -> SearchPageRequest {
        SearchPageRequest {
            request: SearchRequest {
                request_id: 7,
                query: "资料".into(),
                workspace_id: None,
                kind: "all".into(),
            },
            offset: 0,
            sort: "name".into(),
            cursor: None,
            modified_from: None,
            modified_to: None,
            path_prefix: None,
        }
    }
    fn names(&self, request: SearchPageRequest) -> HashSet<String> {
        self.store
            .search_page(request)
            .unwrap()
            .page
            .items
            .into_iter()
            .map(|item| item.name)
            .collect()
    }
    fn mtime(&self, name: &str, time: f64) {
        let mut db = rusqlite::Connection::open(&self.db).unwrap();
        let tx = db.transaction().unwrap();
        tx.execute(
            "UPDATE resources SET modified_at=?1 WHERE name=?2",
            rusqlite::params![time, name],
        )
        .unwrap();
        tx.execute("UPDATE search_documents SET modified_at=?1 WHERE entity_id IN (SELECT id FROM resources WHERE name=?2)",rusqlite::params![time,name]).unwrap();
        tx.execute("UPDATE settings SET value=value+1 WHERE key='revision'", [])
            .unwrap();
        tx.commit().unwrap();
    }
}

#[test]
fn half_open_modified_range_handles_files_web_and_excludes_workspaces() {
    let fixture = Fixture::new();
    for name in ["资料旧.txt", "资料起点.txt", "资料终点.txt"] {
        fs::write(fixture.root.join(name), b"fixture").unwrap();
    }
    fixture.store.grant_root(&fixture.root).unwrap();
    fixture.store.scan_all().unwrap();
    fixture.mutate(Mutation::CreateWorkspace {
        name: "资料工作区".into(),
        resource_ids: None,
    });
    fixture.mutate(Mutation::SaveWeb {
        id: None,
        name: "资料网页".into(),
        url: "https://example.com/library".into(),
        workspace_id: None,
    });
    fixture.mtime("资料旧.txt", 100.);
    fixture.mtime("资料起点.txt", 200.);
    fixture.mtime("资料终点.txt", 300.);
    fixture.mtime("资料网页", 250.);
    let mut request = fixture.request();
    request.modified_from = Some(200.);
    request.modified_to = Some(300.);
    assert_eq!(
        fixture.names(request.clone()),
        HashSet::from(["资料起点.txt".into(), "资料网页".into()])
    );
    request.request.kind = "file".into();
    assert_eq!(
        fixture.names(request.clone()),
        HashSet::from(["资料起点.txt".into()])
    );
    request.request.kind = "all".into();
    request.modified_from = None;
    assert_eq!(fixture.names(request.clone()).len(), 3);
    request.modified_to = None;
    request.modified_from = Some(300.);
    assert_eq!(
        fixture.names(request),
        HashSet::from(["资料终点.txt".into()])
    );
    assert!(fixture.names(fixture.request()).contains("资料工作区"));
}

#[test]
fn prefix_is_literal_normalized_and_supports_urls_without_filesystem_access() {
    let fixture = Fixture::new();
    for directory in ["Café%_", "CaféXX"] {
        fs::create_dir(fixture.root.join(directory)).unwrap();
        fs::write(
            fixture.root.join(directory).join("资料.txt"),
            b"unchanged source",
        )
        .unwrap();
    }
    fixture.store.grant_root(&fixture.root).unwrap();
    fixture.store.scan_all().unwrap();
    fixture.mutate(Mutation::SaveWeb {
        id: None,
        name: "资料网页".into(),
        url: "https://example.com/a%20b_?q=资料".into(),
        workspace_id: None,
    });
    let mut request = fixture.request();
    // 分解重音和大写前缀应使用现有索引规范化，不把 %/_ 当 LIKE 通配符。
    request.path_prefix = Some(format!("{}/CAFE\u{301}%_", fixture.root.display()));
    let result = fixture
        .store
        .search_page(request.clone())
        .unwrap()
        .page
        .items;
    assert_eq!(result.len(), 1);
    assert!(result[0].location.contains("Café%_"));
    fs::remove_file(&result[0].location).unwrap();
    assert_eq!(
        fixture.store.search_page(request).unwrap().page.items.len(),
        1
    );
    let mut request = fixture.request();
    request.path_prefix = Some("HTTPS://EXAMPLE.COM/a%20b_".into());
    assert_eq!(fixture.names(request), HashSet::from(["资料网页".into()]));
    let mut request = fixture.request();
    request.path_prefix = Some(String::new());
    assert_eq!(
        fixture.store.search_page(request).unwrap().page.items.len(),
        3
    );
}

#[test]
fn scope_permission_date_and_path_are_applied_before_limit_and_paging() {
    let fixture = Fixture::new();
    let selected = fixture.root.join("selected");
    let allowed = selected.join("allowed");
    let denied = selected.join("denied");
    let other = fixture.root.join("other");
    for folder in [&allowed, &denied, &other] {
        fs::create_dir_all(folder).unwrap();
    }
    for index in 0..115 {
        fs::write(allowed.join(format!("资料_z_{index:03}.txt")), b"kept").unwrap();
    }
    for index in 0..110 {
        fs::write(denied.join(format!("资料_a_{index:03}.txt")), b"denied").unwrap();
    }
    for index in 0..8 {
        fs::write(other.join(format!("资料_b_{index:03}.txt")), b"wrong path").unwrap();
    }
    for name in ["资料_c_old.txt", "资料_d_unlinked.txt"] {
        fs::write(allowed.join(name), b"filtered").unwrap();
    }
    fixture.store.grant_root(&allowed).unwrap();
    let denied_root = fixture.store.grant_root(&denied).unwrap();
    fixture.store.grant_root(&other).unwrap();
    fixture.store.scan_all().unwrap();
    let workspace = fixture
        .mutate(Mutation::CreateWorkspace {
            name: "资料工作区".into(),
            resource_ids: None,
        })
        .id
        .unwrap();
    let all = rusqlite::Connection::open(&fixture.db).unwrap();
    all.execute_batch("BEGIN; UPDATE resources SET modified_at=200; UPDATE search_documents SET modified_at=200 WHERE kind<>'workspace'; UPDATE settings SET value=value+1 WHERE key='revision'; COMMIT;").unwrap();
    let ids = all
        .prepare("SELECT id FROM resources WHERE name<>'资料_d_unlinked.txt'")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    fixture.mutate(Mutation::Link {
        workspace_id: workspace.clone(),
        resource_ids: ids,
    });
    fixture.mutate(Mutation::RevokeRoot { id: denied_root });
    fixture.mtime("资料_c_old.txt", 100.);
    let mut request = fixture.request();
    request.request.workspace_id = Some(workspace);
    request.request.kind = "file".into();
    request.modified_from = Some(200.);
    request.modified_to = Some(300.);
    request.path_prefix = Some(selected.to_string_lossy().into_owned());
    let response = fixture.store.search_page(request.clone()).unwrap();
    request.cursor = response.next_cursor;
    let first = response.page;
    request.offset = 100;
    let second = fixture.store.search_page(request).unwrap().page;
    assert_eq!(first.items.len(), 100);
    assert!(first.has_more);
    assert_eq!(second.items.len(), 15);
    assert!(!second.has_more);
    let ids: HashSet<_> = first
        .items
        .iter()
        .chain(&second.items)
        .map(|item| &item.id)
        .collect();
    assert_eq!(ids.len(), 115);
    assert!(first
        .items
        .iter()
        .chain(&second.items)
        .all(|item| item.name.starts_with("资料_z_")));
    assert!(fs::read(allowed.join("资料_z_000.txt")).unwrap() == b"kept");
}

#[test]
fn invalid_ranges_prefixes_and_legacy_requests_have_explicit_behavior() {
    let fixture = Fixture::new();
    for (from, to) in [
        (Some(f64::NAN), None),
        (None, Some(f64::INFINITY)),
        (Some(300.), Some(200.)),
        (Some(200.), Some(200.)),
        (Some(8_640_000_000_000_001.), None),
    ] {
        let mut request = fixture.request();
        request.modified_from = from;
        request.modified_to = to;
        assert_eq!(
            fixture.store.search_page(request).unwrap_err().code,
            "invalid_input"
        );
    }
    for prefix in ["bad\0path".into(), "a".repeat(8193)] {
        let mut request = fixture.request();
        request.path_prefix = Some(prefix);
        assert_eq!(
            fixture.store.search_page(request).unwrap_err().code,
            "invalid_input"
        );
    }
    let legacy: SearchPageRequest = serde_json::from_str(r#"{"request":{"request_id":1,"query":"资料","workspace_id":null,"kind":"all"},"offset":0,"sort":"name"}"#).unwrap();
    assert!(
        legacy.modified_from.is_none()
            && legacy.modified_to.is_none()
            && legacy.path_prefix.is_none()
    );
    assert!(fixture
        .store
        .search_page(legacy)
        .unwrap()
        .page
        .items
        .is_empty());
}
