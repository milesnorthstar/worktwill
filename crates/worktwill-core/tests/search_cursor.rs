use std::{collections::HashSet, fs};
use worktwill_core::{model::*, Store};

struct Fixture {
    directory: tempfile::TempDir,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        let parent = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("search-cursor-")
            .tempdir_in(parent)
            .unwrap();
        let store = Store::open(directory.path().join("app/catalog.sqlite3")).unwrap();
        Self { directory, store }
    }
    fn mutate(&self, mutation: Mutation) -> MutationResponse {
        self.store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation,
            })
            .unwrap()
    }
    fn seed_web(&self) {
        for n in 0..225 {
            self.mutate(Mutation::SaveWeb {
                id: None,
                name: format!("Report_{n:03}"),
                url: format!("https://example.com/{n}"),
                workspace_id: None,
            });
        }
    }
    fn request(&self) -> SearchPageRequest {
        SearchPageRequest {
            request: SearchRequest {
                request_id: 1,
                query: "report".into(),
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
    fn next(&self, first: &SearchResponse, mut request: SearchPageRequest) -> SearchPageRequest {
        request.cursor = first.next_cursor.clone();
        request.offset = request.cursor.as_ref().unwrap().next_offset;
        request.request.request_id += 1;
        request
    }
    fn stale(&self, request: SearchPageRequest) {
        assert_eq!(
            self.store.search_page(request).unwrap_err().code,
            "stale_cursor"
        );
    }
}

#[test]
fn complete_cursor_chain_covers_every_result_once_and_normalizes_equivalent_criteria() {
    let f = Fixture::new();
    f.seed_web();
    for sort in ["name", "modified", "relevance"] {
        let mut request = f.request();
        request.sort = sort.into();
        request.path_prefix = Some("HTTPS://EXAMPLE.COM/".into());
        let first = f.store.search_page(request.clone()).unwrap();
        assert_eq!(first.next_cursor.as_ref().unwrap().version, 4);
        if sort == "name" {
            let position = first
                .next_cursor
                .as_ref()
                .unwrap()
                .name_position
                .as_ref()
                .unwrap();
            assert_eq!(position.name, "report_099");
            assert_eq!(position.id, first.page.items.last().unwrap().id);
        } else {
            assert!(first.next_cursor.as_ref().unwrap().name_position.is_none());
            if sort == "relevance" {
                let position = first
                    .next_cursor
                    .as_ref()
                    .unwrap()
                    .rank_position
                    .as_ref()
                    .unwrap();
                assert_eq!(position.rank, 1);
                assert_eq!(position.name, "report_099");
                assert_eq!(position.id, first.page.items.last().unwrap().id);
            } else {
                assert!(first.next_cursor.as_ref().unwrap().rank_position.is_none());
            }
        }
        let mut next = f.next(&first, request);
        next.request.query = "  REPORT  ".into();
        next.path_prefix = Some("https://example.com/".into());
        let second = f.store.search_page(next.clone()).unwrap();
        let third = f.store.search_page(f.next(&second, next)).unwrap();
        assert_eq!(
            (
                first.page.items.len(),
                second.page.items.len(),
                third.page.items.len()
            ),
            (100, 100, 25)
        );
        assert!(first.page.has_more && second.page.has_more);
        assert!(!third.page.has_more && third.next_cursor.is_none());
        assert_eq!(second.request_id, 2);
        assert_eq!(third.request_id, 3);
        let ids: HashSet<_> = first
            .page
            .items
            .iter()
            .chain(&second.page.items)
            .chain(&third.page.items)
            .map(|r| &r.id)
            .collect();
        assert_eq!(ids.len(), 225);
    }
}

#[test]
fn changed_conditions_versions_offsets_and_missing_cursors_are_rejected() {
    let f = Fixture::new();
    f.seed_web();
    let first = f.store.search_page(f.request()).unwrap();
    let next = f.next(&first, f.request());
    let mut altered = Vec::new();
    let mut q = next.clone();
    q.request.query = "report_1".into();
    altered.push(q);
    let mut q = next.clone();
    q.request.kind = "web".into();
    altered.push(q);
    let mut q = next.clone();
    q.request.workspace_id = Some("different".into());
    altered.push(q);
    let mut q = next.clone();
    q.sort = "modified".into();
    altered.push(q);
    let mut q = next.clone();
    q.modified_from = Some(1.);
    altered.push(q);
    let mut q = next.clone();
    q.modified_to = Some(2.);
    altered.push(q);
    let mut q = next.clone();
    q.path_prefix = Some("https://example.com/1".into());
    altered.push(q);
    let mut q = next.clone();
    q.offset = 200;
    altered.push(q);
    let mut q = next.clone();
    q.offset = 0;
    altered.push(q);
    let mut q = next.clone();
    q.cursor = None;
    altered.push(q);
    let mut q = next.clone();
    q.cursor.as_mut().unwrap().version = 2;
    altered.push(q);
    let mut q = next.clone();
    q.cursor.as_mut().unwrap().name_position = None;
    altered.push(q);
    let mut q = next.clone();
    q.cursor
        .as_mut()
        .unwrap()
        .name_position
        .as_mut()
        .unwrap()
        .name = "a".repeat(32769);
    altered.push(q);
    let mut q = next.clone();
    q.cursor
        .as_mut()
        .unwrap()
        .name_position
        .as_mut()
        .unwrap()
        .id
        .clear();
    altered.push(q);
    for update in [0, 1, 2, 3, 4] {
        let mut q = next.clone();
        let cursor = q.cursor.as_mut().unwrap();
        match update {
            0 => cursor.version += 1,
            1 => cursor.session = "other".into(),
            2 => cursor.permission_version += 1,
            3 => cursor.revision += 1,
            _ => {
                cursor.next_offset = 101;
                q.offset = 101;
            }
        }
        altered.push(q);
    }
    for request in altered {
        f.stale(request);
    }
    assert_eq!(f.store.search_page(next).unwrap().page.items.len(), 100);
}

#[test]
fn data_mutation_invalidates_old_page_and_fresh_first_page_remains_available() {
    let f = Fixture::new();
    f.seed_web();
    let first = f.store.search_page(f.request()).unwrap();
    f.mutate(Mutation::Favorite {
        id: first.page.items[0].id.clone(),
        value: true,
    });
    f.stale(f.next(&first, f.request()));
    let fresh = f.store.search_page(f.request()).unwrap();
    assert!(fresh.page.revision > first.page.revision);
    assert!(fresh.page.items[0].favorite);
    assert_eq!(
        f.store
            .search_page(f.next(&fresh, f.request()))
            .unwrap()
            .page
            .items
            .len(),
        100
    );
}

#[test]
fn revocation_invalidates_cursor_and_neither_later_page_nor_fresh_query_leaks_files() {
    let f = Fixture::new();
    let root = f.directory.path().join("documents");
    fs::create_dir(&root).unwrap();
    for n in 0..105 {
        fs::write(
            root.join(format!("Report_{n:03}.txt")),
            b"synthetic fixture",
        )
        .unwrap();
    }
    let root_id = f.store.grant_root(&root).unwrap();
    f.store.scan_all().unwrap();
    let first = f.store.search_page(f.request()).unwrap();
    assert_eq!(first.page.items.len(), 100);
    f.mutate(Mutation::RevokeRoot { id: root_id });
    f.stale(f.next(&first, f.request()));
    let fresh = f.store.search_page(f.request()).unwrap();
    assert!(fresh.page.items.is_empty() && fresh.next_cursor.is_none());
    assert!(fresh.page.permission_version > first.page.permission_version);
    assert!(fs::read(root.join("Report_000.txt")).unwrap() == b"synthetic fixture");
}

#[test]
fn restart_and_repeated_restore_invalidate_cursors_even_with_same_data() {
    let f = Fixture::new();
    f.seed_web();
    let first = f.store.search_page(f.request()).unwrap();
    let backup = f.directory.path().join("snapshot.sqlite3");
    f.store.backup_to(&backup).unwrap();
    let reopened = Store::open(f.directory.path().join("app/catalog.sqlite3")).unwrap();
    assert_eq!(
        reopened
            .search_page(f.next(&first, f.request()))
            .unwrap_err()
            .code,
        "stale_cursor"
    );
    let mut previous = first;
    for _ in 0..2 {
        f.store.restore_from(&backup).unwrap();
        f.stale(f.next(&previous, f.request()));
        let fresh = f.store.search_page(f.request()).unwrap();
        assert_eq!(fresh.page.session, previous.page.session);
        assert!(
            fresh.page.revision > previous.page.revision
                && fresh.page.permission_version > previous.page.permission_version
        );
        assert_eq!(
            f.store
                .search_page(f.next(&fresh, f.request()))
                .unwrap()
                .page
                .items
                .len(),
            100
        );
        previous = fresh;
    }
}

#[test]
fn backend_issued_unicode_position_need_not_be_a_normalization_fixed_point() {
    let f = Fixture::new();
    let mut expected = Vec::new();
    // NFC后再小写不是幂等运算；位置必须原样使用投影键，不能再次规范化后判等。
    for index in 0..105 {
        let response = f.mutate(Mutation::SaveWeb {
            id: None,
            name: "\u{03aa}\u{0301}合同".into(),
            url: format!("https://example.com/unicode/{index}"),
            workspace_id: None,
        });
        expected.push(response.id.unwrap());
    }
    expected.sort();
    for sort in ["name", "relevance"] {
        let mut request = f.request();
        request.sort = sort.into();
        request.request.query = "合同".into();
        let first = f.store.search_page(request.clone()).unwrap();
        let cursor = first.next_cursor.as_ref().unwrap();
        let (name, id) = if sort == "name" {
            let position = cursor.name_position.as_ref().unwrap();
            (&position.name, &position.id)
        } else {
            let position = cursor.rank_position.as_ref().unwrap();
            assert_eq!(position.rank, 2);
            (&position.name, &position.id)
        };
        assert_eq!(name, "\u{03ca}\u{0301}合同");
        assert_eq!(id, &expected[99]);
        request = f.next(&first, request);
        let second = f.store.search_page(request).unwrap();
        let actual: Vec<_> = first
            .page
            .items
            .into_iter()
            .chain(second.page.items)
            .map(|item| item.id)
            .collect();
        assert_eq!(actual, expected);
        assert!(!second.page.has_more);
        assert!(second.next_cursor.is_none());
    }
}

#[test]
fn relevance_cursor_requires_rank_position_and_rejects_old_protocol_and_wrong_slots() {
    let f = Fixture::new();
    f.seed_web();
    let mut request = f.request();
    request.sort = "relevance".into();
    let first = f.store.search_page(request.clone()).unwrap();
    let next = f.next(&first, request);
    for case in 0..8 {
        let mut q = next.clone();
        let cursor = q.cursor.as_mut().unwrap();
        match case {
            0 => cursor.rank_position = None,
            1 => cursor.rank_position.as_mut().unwrap().rank = 4,
            2 => cursor.rank_position.as_mut().unwrap().id.clear(),
            3 => cursor.rank_position.as_mut().unwrap().name = "a".repeat(32769),
            4 => {
                cursor.name_position = Some(SearchNamePosition {
                    name: "report".into(),
                    id: "other".into(),
                })
            }
            5 => cursor.version = 1,
            6 => cursor.version = 2,
            _ => cursor.criteria.sort = "name".into(),
        }
        f.stale(q);
    }
    let mut old = serde_json::to_value(next.cursor.as_ref().unwrap()).unwrap();
    old["version"] = 2.into();
    old.as_object_mut().unwrap().remove("rank_position");
    let mut legacy = next.clone();
    legacy.cursor = Some(serde_json::from_value(old).unwrap());
    f.stale(legacy);
    assert_eq!(f.store.search_page(next).unwrap().page.items.len(), 100);
}

#[test]
fn modified_cursor_requires_its_own_position_and_mtime_change_invalidates_the_chain() {
    let f = Fixture::new();
    f.seed_web();
    let mut request = f.request();
    request.sort = "modified".into();
    let first = f.store.search_page(request.clone()).unwrap();
    let next = f.next(&first, request.clone());
    assert!(next.cursor.as_ref().unwrap().modified_position.is_some());
    for sort in ["name", "relevance"] {
        let mut other = f.request();
        other.sort = sort.into();
        let response = f.store.search_page(other.clone()).unwrap();
        let mut other = f.next(&response, other);
        other.cursor.as_mut().unwrap().modified_position =
            next.cursor.as_ref().unwrap().modified_position.clone();
        f.stale(other);
    }
    for case in 0..10 {
        let mut q = next.clone();
        let cursor = q.cursor.as_mut().unwrap();
        match case {
            0 => cursor.modified_position = None,
            1 => cursor.modified_position.as_mut().unwrap().id.clear(),
            2 => cursor.modified_position.as_mut().unwrap().name = "a".repeat(32769),
            3 => cursor.modified_position.as_mut().unwrap().modified_at = Some(f64::NAN),
            4 => cursor.modified_position.as_mut().unwrap().modified_at = Some(f64::INFINITY),
            5 => {
                cursor.modified_position.as_mut().unwrap().modified_at =
                    Some(8_640_000_000_000_001.)
            }
            6 => {
                cursor.name_position = Some(SearchNamePosition {
                    name: "report".into(),
                    id: "other".into(),
                })
            }
            7 => {
                cursor.rank_position = Some(SearchRankPosition {
                    rank: 1,
                    name: "report".into(),
                    id: "other".into(),
                })
            }
            8 => cursor.version = 3,
            _ => cursor.criteria.sort = "name".into(),
        }
        f.stale(q);
    }
    assert_eq!(
        f.store.search_page(next.clone()).unwrap().page.items.len(),
        100
    );
    let resource = &first.page.items[0];
    f.mutate(Mutation::SaveWeb {
        id: Some(resource.id.clone()),
        name: resource.name.clone(),
        url: resource.location.clone(),
        workspace_id: None,
    });
    f.stale(next);
    let refreshed = f.store.search_page(request).unwrap();
    assert_eq!(refreshed.page.items[0].id, resource.id);
}
