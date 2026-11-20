//! 名称有序读取仍须先完成授权、范围、日期和字面前缀过滤。
use crate::{model::*, search, Store};
use rusqlite::params;
use std::{fs, path::PathBuf};

struct Fixture {
    _directory: tempfile::TempDir,
    store: Store,
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("complete-name-plan-")
            .tempdir_in(approved)
            .unwrap();
        let root = directory.path().join("docs");
        fs::create_dir(&root).unwrap();
        let store = Store::open(directory.path().join("app/catalog.sqlite3")).unwrap();
        Self {
            _directory: directory,
            store,
            root,
        }
    }
    fn request() -> SearchPageRequest {
        SearchPageRequest {
            request: SearchRequest {
                request_id: 1,
                query: "合同".into(),
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
}

#[test]
fn high_match_name_pages_filter_before_limit_and_use_ordered_index() {
    let fixture = Fixture::new();
    let mut expected = Vec::new();
    {
        let mut db = fixture.store.connection().unwrap();
        let tx = db.transaction().unwrap();
        tx.execute(
            "INSERT INTO workspaces(id,name,created_at) VALUES('w','合同范围',0)",
            [],
        )
        .unwrap();
        search::project(&tx, "w", "workspace", "合同范围", "", None).unwrap();
        for index in 0..2100 {
            let id = format!("web-{index:04}");
            let name = format!("合同_{:04}", index / 7);
            let location = format!(
                "https://example.com/{}/{index:04}%_",
                if index % 5 == 0 { "other" } else { "library" }
            );
            let modified = index % 4;
            tx.execute("INSERT INTO resources(id,kind,name,location,modified_at,created_at) VALUES(?1,'web',?2,?3,?4,0)",params![id,name,location,modified]).unwrap();
            search::project(&tx, &id, "web", &name, &location, Some(f64::from(modified))).unwrap();
            if index % 3 == 0 {
                tx.execute("INSERT INTO workspace_resources VALUES('w',?1,0)", [&id])
                    .unwrap();
            }
            if index % 3 == 0 && (1..3).contains(&modified) && index % 5 != 0 {
                expected.push((name, id));
            }
        }
        // 精确名称诱饵位于所有合法前缀之前，但没有任何文件授权。
        for index in 0..20 {
            let id = format!("denied-{index:04}");
            let path = fixture
                .root
                .join(format!("{index}.txt"))
                .to_string_lossy()
                .into_owned();
            tx.execute("INSERT INTO resources(id,kind,name,location,modified_at,created_at) VALUES(?1,'file','合同',?2,1,0)",params![id,path]).unwrap();
            search::project(&tx, &id, "file", "合同", &path, Some(1.)).unwrap();
            tx.execute("INSERT INTO workspace_resources VALUES('w',?1,0)", [id])
                .unwrap();
        }
        tx.commit().unwrap();
    }
    expected.sort();
    let expected: Vec<_> = expected.into_iter().map(|(_, id)| id).collect();
    assert!(expected.len() > 200);
    let mut request = Fixture::request();
    request.request.kind = "web".into();
    request.request.workspace_id = Some("w".into());
    request.modified_from = Some(1.);
    request.modified_to = Some(3.);
    request.path_prefix = Some("HTTPS://EXAMPLE.COM/LIBRARY/".into());
    let criteria = search::page_criteria(&request).unwrap();
    let plan = search::explain_page(&fixture.store.connection().unwrap(), &criteria, 0).unwrap();
    assert!(
        plan.iter()
            .any(|line| line.contains("search_documents_kind_name")),
        "{plan:?}"
    );
    assert!(
        !plan
            .iter()
            .any(|line| line.contains("TEMP B-TREE") || line.contains("VIRTUAL TABLE")),
        "{plan:?}"
    );
    let mut actual = Vec::new();
    loop {
        let response = fixture.store.search_page(request.clone()).unwrap();
        let end = (request.offset as usize + 100).min(expected.len());
        assert_eq!(
            response
                .page
                .items
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>(),
            expected[request.offset as usize..end]
        );
        assert_eq!(response.page.has_more, end < expected.len());
        actual.extend(response.page.items.into_iter().map(|item| item.id));
        if !response.page.has_more {
            break;
        }
        let cursor = response.next_cursor.unwrap();
        assert_eq!(
            cursor.name_position.as_ref().unwrap().id,
            actual.last().unwrap().as_str()
        );
        let seek_plan = search::explain_page_after(
            &fixture.store.connection().unwrap(),
            &criteria,
            cursor.next_offset,
            cursor.name_position.as_ref(),
        )
        .unwrap();
        assert!(
            seek_plan.iter().any(|line| line
                .contains("SEARCH d USING INDEX search_documents_kind_name")
                && line.contains("(name,entity_id)>")),
            "{seek_plan:?}"
        );
        assert!(
            !seek_plan.iter().any(|line| line.contains("TEMP B-TREE")),
            "{seek_plan:?}"
        );
        request.offset = cursor.next_offset;
        request.cursor = Some(cursor);
        request.request.request_id += 1;
    }
    assert_eq!(actual, expected);
    let mut denied = Fixture::request();
    denied.request.kind = "file".into();
    assert!(fixture
        .store
        .search_page(denied)
        .unwrap()
        .page
        .items
        .is_empty());
    let plan = search::explain_page(
        &fixture.store.connection().unwrap(),
        &search::page_criteria(&Fixture::request()).unwrap(),
        0,
    )
    .unwrap();
    assert!(
        plan.iter()
            .any(|line| line.contains("search_documents_name")),
        "{plan:?}"
    );
    assert!(
        !plan.iter().any(|line| line.contains("TEMP B-TREE")),
        "{plan:?}"
    );
}

#[test]
fn low_match_fts_keeps_all_sort_orders_and_workspace_null_time() {
    let fixture = Fixture::new();
    {
        let mut db = fixture.store.connection().unwrap();
        let tx = db.transaction().unwrap();
        for (id, name, time, path) in [
            ("a", "合同", 0, "https://example.com/a"),
            ("b", "合同项目", 10, "https://example.com/b"),
            ("c", "其他合同", 10, "https://example.com/c"),
            ("d", "另一个标题", 0, "https://example.com/合同"),
        ] {
            tx.execute("INSERT INTO resources(id,kind,name,location,modified_at,created_at) VALUES(?1,'web',?2,?3,?4,0)",params![id,name,path,time]).unwrap();
            search::project(&tx, id, "web", name, path, Some(f64::from(time))).unwrap();
        }
        tx.execute(
            "INSERT INTO workspaces(id,name,created_at) VALUES('w','合同',0)",
            [],
        )
        .unwrap();
        search::project(&tx, "w", "workspace", "合同", "", None).unwrap();
        tx.commit().unwrap();
    }
    for (sort, expected) in [
        ("name", vec!["c", "d", "a", "w", "b"]),
        ("relevance", vec!["a", "w", "b", "c", "d"]),
        ("modified", vec!["c", "b", "d", "a", "w"]),
    ] {
        let mut request = Fixture::request();
        request.sort = sort.into();
        let plan = search::explain_page(
            &fixture.store.connection().unwrap(),
            &search::page_criteria(&request).unwrap(),
            0,
        )
        .unwrap();
        assert!(
            plan.iter().any(|line| line.contains("VIRTUAL TABLE")),
            "{plan:?}"
        );
        let response = fixture.store.search_page(request).unwrap();
        assert_eq!(
            response
                .page
                .items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{sort}"
        );
        assert!(!response.page.has_more);
        assert!(response.next_cursor.is_none());
    }
}

#[test]
fn high_match_relevance_and_modified_traverse_all_ranks_times_nulls_and_filters() {
    use unicode_normalization::UnicodeNormalization;
    let f = Fixture::new();
    // 原始记录独立排序，不复用生产rank或候选选择器。
    struct RawRecord {
        id: String,
        kind: String,
        name: String,
        path: String,
        modified: Option<f64>,
        scope: bool,
        authorized: bool,
    }
    let mut records: Vec<RawRecord> = Vec::new();
    {
        let mut db = f.store.connection().unwrap();
        let tx = db.transaction().unwrap();
        tx.execute(
            "INSERT INTO workspaces(id,name,created_at) VALUES('scope','合同范围',0)",
            [],
        )
        .unwrap();
        search::project(&tx, "scope", "workspace", "合同范围", "", None).unwrap();
        records.push(RawRecord {
            id: "scope".into(),
            kind: "workspace".into(),
            name: "合同范围".into(),
            path: "".into(),
            modified: None,
            scope: true,
            authorized: true,
        });
        for (rank, count) in [(0, 250), (1, 1200), (2, 900), (3, 1000)] {
            for index in 0..count {
                let id = format!("rank-{rank}-{index:04}");
                let kind = if rank == 0 && index < 135 {
                    "workspace"
                } else {
                    "web"
                };
                let name = match rank {
                    0 => "合同".into(),
                    1 => format!("合同_{:04}", index / 7),
                    2 => format!("项目合同_{:04}", index / 7),
                    _ => format!("参考_{:04}", index / 7),
                };
                let path = if kind == "workspace" {
                    String::new()
                } else {
                    format!(
                        "https://example.com/{}/合同/{index:04}%_",
                        if index % 5 == 0 { "other" } else { "library" }
                    )
                };
                let modified = if kind == "workspace" {
                    None
                } else {
                    Some((index % 5) as f64 - 1.)
                };
                let scope = kind != "workspace" && index % 3 == 0;
                if kind == "workspace" {
                    tx.execute(
                        "INSERT INTO workspaces(id,name,created_at) VALUES(?1,?2,0)",
                        params![id, name],
                    )
                    .unwrap();
                } else {
                    tx.execute("INSERT INTO resources(id,kind,name,location,modified_at,created_at) VALUES(?1,'web',?2,?3,?4,0)",params![id,name,path,modified]).unwrap();
                    if scope {
                        tx.execute(
                            "INSERT INTO workspace_resources VALUES('scope',?1,0)",
                            [&id],
                        )
                        .unwrap();
                    }
                }
                search::project(&tx, &id, kind, &name, &path, modified).unwrap();
                records.push(RawRecord {
                    id,
                    kind: kind.into(),
                    name,
                    path,
                    modified,
                    scope,
                    authorized: true,
                });
            }
        }
        for index in 0..40 {
            let id = format!("denied-{index:04}");
            let path = f
                .root
                .join(format!("{index}.txt"))
                .to_string_lossy()
                .into_owned();
            tx.execute("INSERT INTO resources(id,kind,name,location,modified_at,created_at) VALUES(?1,'file','合同',?2,1,0)",params![id,path]).unwrap();
            search::project(&tx, &id, "file", "合同", &path, Some(1.)).unwrap();
            tx.execute(
                "INSERT INTO workspace_resources VALUES('scope',?1,0)",
                [&id],
            )
            .unwrap();
            records.push(RawRecord {
                id,
                kind: "file".into(),
                name: "合同".into(),
                path,
                modified: Some(1.),
                scope: true,
                authorized: false,
            });
        }
        tx.commit().unwrap();
    }
    let normalize = |value: &str| value.nfc().flat_map(char::to_lowercase).collect::<String>();
    for sort in ["relevance", "modified"] {
        let mut modified_ranges = std::collections::BTreeSet::new();
        for case in 0..8 {
            let mut request = Fixture::request();
            request.sort = sort.into();
            match case {
                1 => request.request.workspace_id = Some("scope".into()),
                2 => {
                    request.modified_from = Some(1.);
                    request.modified_to = Some(3.);
                }
                3 => request.path_prefix = Some("HTTPS://EXAMPLE.COM/LIBRARY/".into()),
                4 => request.request.kind = "workspace".into(),
                5 => request.request.kind = "file".into(),
                6 => request.request.kind = "web".into(),
                7 => {
                    request.request.workspace_id = Some("scope".into());
                    request.modified_from = Some(1.);
                    request.modified_to = Some(3.);
                    request.path_prefix = Some("https://example.com/library/".into());
                }
                _ => {}
            }
            let mut expected = records
                .iter()
                .filter(|r| {
                    r.authorized
                        && (request.request.kind == "all" || r.kind == request.request.kind)
                        && (request.request.workspace_id.is_none() || r.scope)
                        && request
                            .modified_from
                            .is_none_or(|from| r.modified.is_some_and(|mtime| mtime >= from))
                        && request
                            .modified_to
                            .is_none_or(|to| r.modified.is_some_and(|mtime| mtime < to))
                        && request.path_prefix.as_ref().is_none_or(|prefix| {
                            r.kind != "workspace"
                                && normalize(&r.path).starts_with(&normalize(prefix))
                        })
                })
                .map(|r| {
                    let name = normalize(&r.name);
                    let rank = if name == "合同" {
                        0
                    } else if name.starts_with("合同") {
                        1
                    } else if name.contains("合同") {
                        2
                    } else {
                        3
                    };
                    (rank, name, r.id.clone(), r.modified)
                })
                .collect::<Vec<_>>();
            expected.sort_by(|a, b| {
                if sort == "modified" {
                    b.3.partial_cmp(&a.3)
                        .unwrap()
                        .then_with(|| (&a.1, &a.2).cmp(&(&b.1, &b.2)))
                } else {
                    (&a.0, &a.1, &a.2).cmp(&(&b.0, &b.1, &b.2))
                }
            });
            let expected: Vec<_> = expected.into_iter().map(|(_, _, id, _)| id).collect();
            let mut actual = Vec::new();
            loop {
                let response = f.store.search_page(request.clone()).unwrap();
                let end = (request.offset as usize + 100).min(expected.len());
                assert_eq!(
                    response
                        .page
                        .items
                        .iter()
                        .map(|item| item.id.clone())
                        .collect::<Vec<_>>(),
                    expected[request.offset as usize..end],
                    "case={case}"
                );
                actual.extend(response.page.items.into_iter().map(|item| item.id));
                assert_eq!(response.page.has_more, end < expected.len());
                let Some(cursor) = response.next_cursor else {
                    break;
                };
                if sort == "relevance" {
                    let position = cursor.rank_position.as_ref().unwrap();
                    assert_eq!(position.id, expected[end - 1]);
                    assert!(position.rank <= 3);
                    assert!(cursor.modified_position.is_none());
                } else {
                    let position = cursor.modified_position.as_ref().unwrap();
                    let raw = records.iter().find(|r| r.id == expected[end - 1]).unwrap();
                    assert_eq!(position.id, raw.id);
                    assert_eq!(position.modified_at, raw.modified);
                    assert!(cursor.rank_position.is_none());
                }
                assert!(cursor.name_position.is_none());
                request.offset = cursor.next_offset;
                request.cursor = Some(cursor);
                let plan = search::explain_page_positions(
                    &f.store.connection().unwrap(),
                    &search::page_criteria(&request).unwrap(),
                    request.offset,
                    None,
                    request.cursor.as_ref().unwrap().rank_position.as_ref(),
                    request.cursor.as_ref().unwrap().modified_position.as_ref(),
                )
                .unwrap();
                if sort == "modified" {
                    for (stage, condition) in [
                        ("Equal", "modified_at=?"),
                        ("Earlier", "modified_at<?"),
                        ("Null", "modified_at=?"),
                    ] {
                        let lines: Vec<_> = plan
                            .iter()
                            .filter(|line| {
                                line.starts_with(&format!(
                                    "modified={stage}: SEARCH d USING INDEX search_documents"
                                ))
                            })
                            .collect();
                        if !lines.is_empty() {
                            assert!(
                                lines.iter().all(|line| line.contains(condition)),
                                "{plan:?}"
                            );
                            if stage == "Equal"
                                || (stage == "Null"
                                    && request
                                        .cursor
                                        .as_ref()
                                        .unwrap()
                                        .modified_position
                                        .as_ref()
                                        .unwrap()
                                        .modified_at
                                        .is_none())
                            {
                                assert!(
                                    lines.iter().all(|line| line.contains("(name,entity_id)>")),
                                    "{plan:?}"
                                );
                            }
                            modified_ranges.insert(stage);
                        }
                    }
                }
                assert!(
                    !plan
                        .iter()
                        .any(|line| line.contains("TEMP B-TREE") || line.contains("VIRTUAL TABLE")),
                    "{plan:?}"
                );
            }
            assert_eq!(actual, expected);
        }
        if sort == "modified" {
            assert_eq!(
                modified_ranges,
                std::collections::BTreeSet::from(["Equal", "Earlier", "Null"])
            );
        }
    }
}

#[test]
fn nul_query_is_rejected_by_quick_and_full_search_before_rank_selection() {
    let f = Fixture::new();
    let mut request = Fixture::request();
    request.sort = "relevance".into();
    request.request.query = "a\0".into();
    assert_eq!(
        f.store.search(request.request.clone()).unwrap_err().code,
        "invalid_input"
    );
    assert_eq!(
        f.store.search_page(request).unwrap_err().code,
        "invalid_input"
    );
}

#[test]
fn low_match_modified_seeks_equal_negative_zero_and_multiple_null_pages() {
    let f = Fixture::new();
    let mut raw = Vec::new();
    {
        let mut db = f.store.connection().unwrap();
        let tx = db.transaction().unwrap();
        for index in 0..380 {
            let id = format!("item-{index:04}");
            let name = format!("合同_{:03}", index / 9);
            let time = if index < 180 {
                Some(i64::from(index % 3) - 1)
            } else {
                None
            };
            let kind = if time.is_some() { "web" } else { "workspace" };
            if let Some(time) = time {
                tx.execute("INSERT INTO resources(id,kind,name,location,modified_at,created_at) VALUES(?1,'web',?2,'https://example.com/',?3,0)",params![id,name,time]).unwrap();
            } else {
                tx.execute(
                    "INSERT INTO workspaces(id,name,created_at) VALUES(?1,?2,0)",
                    params![id, name],
                )
                .unwrap();
            }
            search::project(
                &tx,
                &id,
                kind,
                &name,
                if time.is_some() {
                    "https://example.com/"
                } else {
                    ""
                },
                time.map(|t| t as f64),
            )
            .unwrap();
            raw.push((time, name, id));
        }
        tx.commit().unwrap();
    }
    raw.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| (&a.1, &a.2).cmp(&(&b.1, &b.2))));
    let expected: Vec<_> = raw.iter().map(|r| r.2.clone()).collect();
    let mut request = Fixture::request();
    request.sort = "modified".into();
    let plan = search::explain_page(
        &f.store.connection().unwrap(),
        &search::page_criteria(&request).unwrap(),
        0,
    )
    .unwrap();
    assert!(
        plan.iter().any(|line| line.contains("VIRTUAL TABLE")),
        "{plan:?}"
    );
    let mut actual = Vec::new();
    loop {
        let response = f.store.search_page(request.clone()).unwrap();
        actual.extend(response.page.items.iter().map(|r| r.id.clone()));
        let Some(cursor) = response.next_cursor else {
            break;
        };
        let position = cursor.modified_position.as_ref().unwrap();
        let raw = raw.iter().find(|r| r.2 == position.id).unwrap();
        assert_eq!(position.modified_at, raw.0.map(|t| t as f64));
        request.offset = cursor.next_offset;
        request.cursor = Some(cursor);
    }
    assert_eq!(actual, expected);
}
