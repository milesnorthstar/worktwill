use worktwill_core::{model::*, SearchCancellation, Store};

struct Fixture {
    _directory: tempfile::TempDir,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("search-cancel-")
            .tempdir_in(approved)
            .unwrap();
        let store = Store::open(directory.path().join("app/catalog.sqlite3")).unwrap();
        store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation: Mutation::SaveWeb {
                    id: None,
                    name: "needle example".into(),
                    url: "https://example.com".into(),
                    workspace_id: None,
                },
            })
            .unwrap();
        Self {
            _directory: directory,
            store,
        }
    }
}
fn quick(id: u32) -> SearchRequest {
    SearchRequest {
        request_id: id,
        query: "needle".into(),
        workspace_id: None,
        kind: "all".into(),
    }
}
fn page(id: u32) -> SearchPageRequest {
    SearchPageRequest {
        request: quick(id),
        offset: 0,
        sort: "relevance".into(),
        cursor: None,
        modified_from: None,
        modified_to: None,
        path_prefix: None,
    }
}

#[test]
fn both_public_query_interfaces_reject_pre_cancelled_tokens_instead_of_empty_results() {
    let fixture = Fixture::new();
    let cancellation = SearchCancellation::default();
    cancellation.cancel();
    assert_eq!(
        fixture
            .store
            .search_with_cancellation(quick(1), &cancellation)
            .unwrap_err()
            .code,
        "search_cancelled"
    );
    assert_eq!(
        fixture
            .store
            .search_page_with_cancellation(page(2), &cancellation)
            .unwrap_err()
            .code,
        "search_cancelled"
    );
    assert_eq!(fixture.store.search(quick(3)).unwrap().page.items.len(), 1);
    assert_eq!(
        fixture.store.search_page(page(4)).unwrap().page.items.len(),
        1
    );
}

#[test]
fn uncancelled_interfaces_preserve_results_and_cancelling_one_token_does_not_poison_later_queries()
{
    let fixture = Fixture::new();
    let cancellation = SearchCancellation::default();
    let cancellable = fixture
        .store
        .search_with_cancellation(quick(1), &cancellation)
        .unwrap();
    let legacy = fixture.store.search(quick(2)).unwrap();
    assert_eq!(cancellable.page.items[0].id, legacy.page.items[0].id);
    let paged = fixture
        .store
        .search_page_with_cancellation(page(3), &cancellation)
        .unwrap();
    let legacy_page = fixture.store.search_page(page(4)).unwrap();
    assert_eq!(paged.page.items[0].id, legacy_page.page.items[0].id);
    cancellation.cancel();
    let independent = SearchCancellation::default();
    assert_eq!(
        fixture
            .store
            .search_with_cancellation(quick(5), &independent)
            .unwrap()
            .page
            .items
            .len(),
        1
    );
    assert_eq!(
        fixture
            .store
            .search_page_with_cancellation(page(6), &independent)
            .unwrap()
            .page
            .items
            .len(),
        1
    );
}
