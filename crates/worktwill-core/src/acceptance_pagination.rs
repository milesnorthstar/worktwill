//! 合成多资源磁盘库分页验收；原始 Vec 独立核对，不代表实扫、原生窗口或冷磁盘。
use crate::{model::*, search, timeline::*, Store};
use rusqlite::params;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    time::Instant,
};
use unicode_normalization::UnicodeNormalization;

#[derive(Clone)]
struct Record {
    id: String,
    kind: &'static str,
    name: String,
    path: String,
    modified: f64,
    authorized: bool,
    workspaces: Vec<String>,
}
struct Event {
    id: String,
    kind: &'static str,
    resource: Option<usize>,
    observed: f64,
    modified: Option<f64>,
}
fn normalized(value: &str) -> String {
    value.nfc().flat_map(char::to_lowercase).collect()
}
fn percentile(values: &[f64], p: f64) -> f64 {
    values[((values.len() as f64 * p).ceil() as usize).saturating_sub(1)]
}
fn timings(mut values: Vec<f64>) -> Value {
    if values.is_empty() {
        return json!({"samples":0});
    }
    values.sort_by(f64::total_cmp);
    json!({"samples":values.len(),"p50_ms":percentile(&values,0.5),"p95_ms":percentile(&values,0.95),"p99_ms":percentile(&values,0.99),"max_ms":values.last()})
}
fn check(
    condition: bool,
    message: Value,
    failures: &mut Vec<Value>,
    failed: &mut usize,
    checks: &mut usize,
) {
    *checks += 1;
    if !condition {
        *failed += 1;
        if failures.len() < 32 {
            failures.push(message);
        }
    }
}
fn search_oracle(records: &[Record], request: &SearchPageRequest) -> Vec<String> {
    let query = normalized(request.request.query.trim());
    let prefix = request.path_prefix.as_deref().map(normalized);
    let mut selected: Vec<_> =
        records
            .iter()
            .filter(|r| r.authorized)
            .filter(|r| request.request.kind == "all" || r.kind == request.request.kind)
            .filter(|r| {
                request.request.workspace_id.as_ref().is_none_or(|w| {
                    r.workspaces.contains(w) || (r.kind == "workspace" && &r.id == w)
                })
            })
            .filter(|r| {
                request
                    .modified_from
                    .is_none_or(|from| r.kind != "workspace" && r.modified >= from)
                    && request
                        .modified_to
                        .is_none_or(|to| r.kind != "workspace" && r.modified < to)
            })
            .filter_map(|r| {
                let name = normalized(&r.name);
                let path = normalized(&r.path);
                if (!name.contains(&query) && !path.contains(&query))
                    || prefix
                        .as_ref()
                        .is_some_and(|p| r.kind == "workspace" || !path.starts_with(p))
                {
                    return None;
                }
                let rank = if request.sort != "relevance" || name == query {
                    0
                } else if name.starts_with(&query) {
                    1
                } else if name.contains(&query) {
                    2
                } else {
                    3
                };
                Some((r, rank, name))
            })
            .collect();
    selected.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then_with(|| {
                if request.sort == "modified" {
                    // SQLite 降序中非资源工作区的 NULL 在文件/网页时间之后，包括零时间。
                    match (a.0.kind == "workspace", b.0.kind == "workspace") {
                        (false, true) => std::cmp::Ordering::Less,
                        (true, false) => std::cmp::Ordering::Greater,
                        _ => b.0.modified.total_cmp(&a.0.modified),
                    }
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.0.id.cmp(&b.0.id))
    });
    selected.into_iter().map(|(r, _, _)| r.id.clone()).collect()
}
fn effective(event: &Event, basis: &str) -> f64 {
    if basis == "file_modified" && event.kind == "file_modified" {
        event
            .modified
            .filter(|value| *value > 0.)
            .unwrap_or(event.observed)
    } else {
        event.observed
    }
}
fn timeline_oracle(
    records: &[Record],
    events: &[Event],
    request: &TimelineRequest,
) -> (Vec<String>, Vec<(f64, u32)>) {
    let mut selected: Vec<_> = events
        .iter()
        .filter(|e| match request.filter.as_str() {
            "all" => matches!(e.kind, "file_modified" | "file_discovered" | "opened"),
            "modified" => e.kind == "file_modified",
            "discovered" => e.kind == "file_discovered",
            "opened" => e.kind == "opened",
            _ => !matches!(e.kind, "file_modified" | "file_discovered" | "opened"),
        })
        .filter(|e| e.resource.is_none_or(|i| records[i].authorized))
        .filter(|e| {
            request.workspace_id.as_ref().is_none_or(|w| {
                e.resource
                    .is_some_and(|i| records[i].workspaces.contains(w))
            })
        })
        .map(|e| (e, effective(e, &request.basis)))
        .collect();
    let calendar = request
        .day_boundaries
        .windows(2)
        .filter_map(|day| {
            let count = selected
                .iter()
                .filter(|(_, time)| *time >= day[0] && *time < day[1])
                .count() as u32;
            (count > 0).then_some((day[0], count))
        })
        .collect();
    selected.retain(|(_, time)| {
        request.from.is_none_or(|from| *time >= from) && request.to.is_none_or(|to| *time < to)
    });
    selected.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| b.0.id.cmp(&a.0.id)));
    (
        selected.into_iter().map(|(e, _)| e.id.clone()).collect(),
        calendar,
    )
}
// 除顺序/覆盖外核对实际展示字段，避免ID正确但来自另一条记录的正文被忽略。
fn resource_matches(actual: &Resource, expected: &Record) -> bool {
    let mut actual_workspaces = actual.workspace_ids.clone();
    actual_workspaces.sort();
    let mut expected_workspaces = expected.workspaces.clone();
    expected_workspaces.sort();
    expected.authorized
        && actual.id == expected.id
        && actual.kind == expected.kind
        && actual.name == expected.name
        && actual.location == expected.path
        && actual.modified_at == expected.modified
        && actual.status == "available"
        && actual.size == 0.
        && !actual.favorite
        && actual_workspaces == expected_workspaces
}
fn search_payload_matches(
    page: &ResourcePage,
    records: &HashMap<&str, &Record>,
    metadata: &Overview,
) -> bool {
    page.session == metadata.session
        && page.permission_version == metadata.permission_version
        && page.revision == metadata.revision
        && page.items.iter().all(|actual| {
            records
                .get(actual.id.as_str())
                .is_some_and(|expected| resource_matches(actual, expected))
        })
}
fn timeline_payload_matches(
    page: &TimelinePage,
    events: &HashMap<&str, &Event>,
    records: &[Record],
    request: &TimelineRequest,
    metadata: &Overview,
) -> bool {
    page.session == metadata.session
        && page.permission_version == metadata.permission_version
        && page.revision == metadata.revision
        && page.events.iter().all(|actual| {
            let Some(expected) = events.get(actual.activity.id.as_str()) else {
                return false;
            };
            let expected_source = if matches!(expected.kind, "file_modified" | "file_discovered") {
                "filesystem"
            } else {
                "app"
            };
            actual.effective_at == effective(expected, &request.basis)
                && actual.activity.kind == expected.kind
                && actual.activity.at == expected.observed
                && actual.activity.file_modified_at == expected.modified
                && actual.activity.source == expected_source
                && match expected.resource {
                    Some(index) => {
                        actual.activity.resource_id.as_deref() == Some(records[index].id.as_str())
                            && actual.activity.name == records[index].name
                            && actual
                                .resource
                                .as_ref()
                                .is_some_and(|resource| resource_matches(resource, &records[index]))
                    }
                    None => {
                        actual.resource.is_none()
                            && actual.activity.resource_id.is_none()
                            && actual.activity.name.is_empty()
                    }
                }
        })
}
fn search_request() -> SearchPageRequest {
    SearchPageRequest {
        request: SearchRequest {
            request_id: 1,
            query: "资料".into(),
            kind: "all".into(),
            workspace_id: None,
        },
        offset: 0,
        sort: std::env::var("WORKTWILL_PAGINATION_SORT").unwrap_or_else(|_| "name".into()),
        cursor: None,
        modified_from: None,
        modified_to: None,
        path_prefix: None,
    }
}
fn timeline_request(boundaries: &[f64]) -> TimelineRequest {
    TimelineRequest {
        from: Some(boundaries[0]),
        to: boundaries.last().copied(),
        month_from: boundaries[0],
        month_to: *boundaries.last().unwrap(),
        day_boundaries: boundaries.to_vec(),
        filter: "all".into(),
        basis: "file_modified".into(),
        workspace_id: None,
        offset: 0,
        cursor: None,
    }
}

#[test]
#[ignore = "explicit multi-resource deep pagination acceptance; requires confirmed WORKTWILL_TEST_ROOT"]
fn disk_pagination_acceptance() {
    // 查询迭代仅重测搜索；未执行的时间线必须在报告中明确标记。
    let search_only = std::env::var_os("WORKTWILL_PAGINATION_SEARCH_ONLY").is_some();
    let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
    let directory = tempfile::Builder::new()
        .prefix("rust-pagination-")
        .tempdir_in(approved)
        .unwrap();
    let root = directory.path().join("documents");
    let denied = directory.path().join("unauthorized");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&denied).unwrap();
    let database = directory.path().join("app/catalog.sqlite3");
    let store = Store::open(&database).unwrap();
    let root_id = store.grant_root(&root).unwrap();
    let denied_id = store.grant_root(&denied).unwrap();
    // America/New_York 2026-11：首日25小时；直接给定独立日界，不调用生产时间表达式。
    let mut boundaries = vec![1_793_505_600_000.];
    for day in 0..30 {
        boundaries.push(boundaries.last().unwrap() + if day == 0 { 25. } else { 24. } * 3_600_000.);
    }
    let mut records = Vec::with_capacity(100_000);
    let mut events = Vec::with_capacity(100_010);
    let seed = Instant::now();
    let (journal, synchronous, bytes_before_events, bytes_after_events) = {
        let mut db = store.connection().unwrap();
        let journal: String = db
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap();
        let synchronous: u32 = db
            .pragma_query_value(None, "synchronous", |r| r.get(0))
            .unwrap();
        assert_eq!(journal.to_lowercase(), "wal");
        assert_eq!(synchronous, 2);
        let tx = db.transaction().unwrap();
        // 工作区先建立，文件/网页可在同一事务挂到两个相交范围。
        for index in 0..100 {
            tx.execute(
                "INSERT INTO workspaces(id,name,created_at) VALUES(?1,?2,0)",
                params![format!("workspace-{index:06}"), format!("资料_{index:05}")],
            )
            .unwrap();
        }
        for (kind, count) in [("file", 98_900), ("web", 1_000), ("workspace", 100)] {
            for index in 0..count {
                let id = format!("{kind}-{index:06}");
                let authorized = kind != "file" || index < 98_500;
                let name = if !authorized {
                    "资料".into()
                } else if kind == "file" && index == 0 {
                    "唯一测试".into()
                } else {
                    format!(
                        "资料_{:05}{}",
                        index / 300,
                        if kind == "file" { ".docx" } else { "" }
                    )
                };
                let path = if kind == "file" {
                    if authorized { &root } else { &denied }
                        .join(format!("部门{}/record-{index:06}/{}", index % 37, name))
                        .to_string_lossy()
                        .into_owned()
                } else if kind == "web" {
                    format!("https://example.com/library/{index:06}")
                } else {
                    String::new()
                };
                let modified = if kind == "workspace" {
                    0.
                } else {
                    boundaries[index % 30] + ((index / 30) % 4) as f64 * 60_000.
                };
                let mut workspaces = Vec::new();
                if kind != "workspace" {
                    // 合成元数据明确提供合成identity，不能把NULL未知来源当作可验证的历史。
                    let fixture_identity =
                        (kind == "file").then(|| format!("synthetic-pagination:{index:06}"));
                    tx.execute("INSERT INTO resources(id,kind,name,location,modified_at,created_at,identity) VALUES(?1,?2,?3,?4,?5,0,?6)",params![id,kind,name,path,modified,fixture_identity]).unwrap();
                    if kind == "file" {
                        tx.execute(
                            "INSERT INTO file_roots VALUES(?1,?2,0)",
                            params![if authorized { &root_id } else { &denied_id }, id],
                        )
                        .unwrap();
                    }
                    for (w, divisor) in [("workspace-000000", 101), ("workspace-000001", 97)] {
                        if index % divisor == 0 || !authorized {
                            tx.execute(
                                "INSERT INTO workspace_resources VALUES(?1,?2,0)",
                                params![w, id],
                            )
                            .unwrap();
                            workspaces.push(w.into());
                        }
                    }
                } else {
                    tx.execute(
                        "UPDATE workspaces SET name=?1 WHERE id=?2",
                        params![name, id],
                    )
                    .unwrap();
                }
                search::project(
                    &tx,
                    &id,
                    kind,
                    &name,
                    &path,
                    (kind != "workspace").then_some(modified),
                )
                .unwrap();
                records.push(Record {
                    id,
                    kind,
                    name,
                    path,
                    modified,
                    authorized,
                    workspaces,
                });
            }
        }
        tx.execute("UPDATE scan_roots SET active=0 WHERE id=?", [&denied_id])
            .unwrap();
        tx.commit().unwrap();
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        let before = database.metadata().unwrap().len();
        for index in 0..100_000 {
            let resource = index % 99_900;
            let record = &records[resource];
            let kind = if index % 1000 == 0 {
                "workspace_created"
            } else if record.kind == "web" || index % 10 == 1 {
                "opened"
            } else if index % 10 == 0 {
                "file_discovered"
            } else if index % 50 == 2 {
                "linked"
            } else {
                "file_modified"
            };
            let observed = boundaries[index % 30] + ((index / 30) % 60) as f64 * 60_000.;
            events.push(Event {
                id: format!("event-{index:06}"),
                kind,
                resource: (kind != "workspace_created").then_some(resource),
                observed,
                modified: matches!(kind, "file_modified" | "file_discovered")
                    .then_some(observed - 120_000.),
            });
        }
        for (index, time) in [
            boundaries[0] - 1.,
            boundaries[0],
            boundaries[1] - 1.,
            boundaries[1],
            boundaries[30] - 1.,
            boundaries[30],
            boundaries[1],
        ]
        .into_iter()
        .enumerate()
        {
            events.push(Event {
                id: format!("boundary-{index:02}"),
                kind: "file_modified",
                resource: Some(0),
                observed: time + 60_000.,
                modified: Some(time),
            });
        }
        let tx = db.transaction().unwrap();
        for event in &events {
            tx.execute("INSERT INTO activities(id,kind,name,resource_id,at,source,file_modified_at) VALUES(?1,?2,'',?3,?4,?5,?6)",params![event.id,event.kind,event.resource.map(|i|records[i].id.as_str()),event.observed,if matches!(event.kind,"file_modified"|"file_discovered"){"filesystem"}else{"app"},event.modified]).unwrap();
        }
        // 该批是显式旧格式合成事件；全部写入后绑定迁移基准，正式读取不放宽来源校验。
        crate::activity_origin::bind_legacy(&tx).unwrap();
        tx.commit().unwrap();
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        (
            journal,
            synchronous,
            before,
            database.metadata().unwrap().len(),
        )
    };
    let seed_seconds = seed.elapsed().as_secs_f64();
    let metadata = store.overview().unwrap();
    let record_lookup: HashMap<_, _> = records
        .iter()
        .map(|record| (record.id.as_str(), record))
        .collect();
    let event_lookup: HashMap<_, _> = events
        .iter()
        .map(|event| (event.id.as_str(), event))
        .collect();
    let mut failures = Vec::new();
    let mut failed = 0;
    let mut checks = 0;
    let mut reports = Vec::new();
    // API失败与缺页也先保全报告，不能让验收器自己的unwrap掩盖产品失败。
    macro_rules! read {
        ($call:expr,$phase:literal,$offset:expr,$stop:expr) => {
            match $call {
                Ok(response)=>response,
                Err(error)=>{
                    check(false,json!({"phase":$phase,"offset":$offset,"error":error}),&mut failures,&mut failed,&mut checks);
                    $stop
                }
            }
        };
    }

    if std::env::var_os("WORKTWILL_PROFILE_COMPLETE_SEARCH").is_some() {
        // 两种排序仅测代表首页；不能冒充它们的完整深页性能。
        for sort in ["relevance", "modified"] {
            for case in 0..9 {
                let mut request = search_request();
                request.sort = sort.into();
                match case {
                    0 => {}
                    1 => request.request.workspace_id = Some("workspace-000000".into()),
                    2 => request.request.kind = "file".into(),
                    3 => {
                        request.modified_from = Some(boundaries[14]);
                        request.modified_to = Some(boundaries[15]);
                    }
                    4 => {
                        request.path_prefix =
                            Some(root.join("部门17").to_string_lossy().into_owned() + "/")
                    }
                    5 => request.request.query = "唯一".into(),
                    6 => request.request.query = "record".into(),
                    7 => request.request.query = "部门".into(),
                    _ => request.request.kind = "workspace".into(),
                }
                let expected = search_oracle(&records, &request);
                let mut samples = Vec::new();
                for repeat in 0..31 {
                    let started = Instant::now();
                    let response = read!(
                        store.search_page(request.clone()),
                        "complete_search_profile_error",
                        0,
                        break
                    );
                    let elapsed = started.elapsed().as_secs_f64() * 1000.;
                    let actual: Vec<_> = response
                        .page
                        .items
                        .iter()
                        .map(|item| item.id.clone())
                        .collect();
                    check(
                        actual == expected.iter().take(100).cloned().collect::<Vec<_>>()
                            && search_payload_matches(&response.page, &record_lookup, &metadata)
                            && response.page.has_more == (expected.len() > 100),
                        json!({"phase":"complete_search_profile","sort":sort,"case":case,"repeat":repeat}),
                        &mut failures,
                        &mut failed,
                        &mut checks,
                    );
                    if repeat > 0 {
                        samples.push(elapsed);
                    }
                }
                let plan = search::explain_page(
                    &store.connection().unwrap(),
                    &search::page_criteria(&request).unwrap(),
                    0,
                )
                .unwrap();
                reports.push(json!({"sort":sort,"case":case,"matching":expected.len(),"timing":timings(samples),"native_select_plan":plan}));
            }
        }
        let report = json!({"evidence":"synthetic_100k_complete_search_first_pages_only_not_native_or_deep_chain","sqlite_version":rusqlite::version(),"entities":records.len(),"oracle_checks":checks,"oracle_failure_count":failed,"failures":failures,"existing_hot_budget_ms":60,"per_case":reports});
        let output = directory.path().parent().unwrap().join(format!(
            "rust-complete-search-profile-{}.json",
            uuid::Uuid::new_v4()
        ));
        fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        println!("COMPLETE_SEARCH_PROFILE_REPORT {}", output.display());
        assert_eq!(failed, 0, "preserve profile report");
        return;
    }

    if std::env::var_os("WORKTWILL_PAGINATION_PROFILE_TIMELINE").is_some() {
        let base = timeline_request(&boundaries);
        let (expected, calendar) = timeline_oracle(&records, &events, &base);
        let mut real_pages = BTreeMap::new();
        let mut next = base.clone();
        loop {
            real_pages.insert(next.offset, next.clone());
            let page = store.timeline(next.clone()).unwrap();
            let Some(cursor) = page.next_cursor else {
                break;
            };
            next.offset = cursor.next_offset;
            next.cursor = Some(cursor);
        }
        for offset in [0, 100, 10_000, 97_500] {
            let request = real_pages.get(&offset).unwrap().clone();
            for repeat in 0..4 {
                let (page, segments) = store.profile_timeline(&request).unwrap();
                let ids: Vec<_> = page
                    .events
                    .iter()
                    .map(|event| event.activity.id.clone())
                    .collect();
                let points: Vec<_> = page
                    .calendar
                    .iter()
                    .map(|point| (point.at, point.count))
                    .collect();
                let end = (offset as usize + 100).min(expected.len());
                check(
                    ids == expected[offset as usize..end]
                        && points == calendar
                        && timeline_payload_matches(
                            &page,
                            &event_lookup,
                            &records,
                            &request,
                            &metadata,
                        )
                        && page.has_more == (end < expected.len()),
                    json!({"phase":"timeline_profile","offset":offset,"repeat":repeat}),
                    &mut failures,
                    &mut failed,
                    &mut checks,
                );
                reports.push(json!({"repeat":repeat,"segments":segments}));
            }
        }
        let report = json!({"evidence":"synthetic_multi_resource_diagnostic_shared_sql_not_production_api_sla","sqlite_version":rusqlite::version(),"entities":records.len(),"events":events.len(),"oracle_checks":checks,"oracle_failure_count":failed,"failures":failures,"samples":reports,"first_sample_per_offset_is_not_warmup_excluded":true});
        let output = directory.path().parent().unwrap().join(format!(
            "rust-timeline-profile-{}.json",
            uuid::Uuid::new_v4()
        ));
        fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        println!("TIMELINE_PROFILE_REPORT {}", output.display());
        assert_eq!(failed, 0, "preserve diagnostic report");
        return;
    }

    let mut full_search = search_request();
    let expected_search = search_oracle(&records, &full_search);
    let mut search_pages = BTreeMap::new();
    let mut actual_search = Vec::new();
    let mut search_chain_ms = Vec::new();
    loop {
        let offset = full_search.offset as usize;
        search_pages.insert(full_search.offset, full_search.clone());
        let started = Instant::now();
        let response = read!(
            store.search_page(full_search.clone()),
            "search_chain_error",
            full_search.offset,
            break
        );
        search_chain_ms.push(started.elapsed().as_secs_f64() * 1000.);
        let end = (offset + 100).min(expected_search.len());
        let expected = &expected_search[offset.min(end)..end];
        let actual: Vec<_> = response
            .page
            .items
            .iter()
            .map(|item| item.id.clone())
            .collect();
        check(
            actual == expected
                && search_payload_matches(&response.page, &record_lookup, &metadata)
                && response.page.has_more == (end < expected_search.len())
                && response.request_id == full_search.request.request_id,
            json!({"phase":"search_chain","offset":offset,"actual":actual,"expected":expected,"has_more":response.page.has_more}),
            &mut failures,
            &mut failed,
            &mut checks,
        );
        actual_search.extend(actual);
        if search_pages.len().is_multiple_of(100) {
            println!("PAGINATION_PROGRESS search pages={}", search_pages.len());
        }
        if !response.page.has_more {
            break;
        }
        let Some(cursor) = response.next_cursor else {
            check(
                false,
                json!({"phase":"search_cursor_missing","offset":offset}),
                &mut failures,
                &mut failed,
                &mut checks,
            );
            break;
        };
        if cursor.next_offset != full_search.offset + 100 {
            check(
                false,
                json!({"phase":"search_cursor_offset","offset":offset}),
                &mut failures,
                &mut failed,
                &mut checks,
            );
            break;
        }
        full_search.offset = cursor.next_offset;
        full_search.cursor = Some(cursor);
        full_search.request.request_id += 1;
        if search_pages.len() > 1001 {
            check(
                false,
                json!({"phase":"search_chain_unbounded"}),
                &mut failures,
                &mut failed,
                &mut checks,
            );
            break;
        }
    }
    check(
        actual_search == expected_search
            && actual_search.iter().collect::<HashSet<_>>().len() == actual_search.len(),
        json!({"phase":"search_full_coverage","actual_count":actual_search.len(),"expected_count":expected_search.len()}),
        &mut failures,
        &mut failed,
        &mut checks,
    );
    reports.push(json!({"case":"search_full_chain","pages":search_pages.len(),"results":actual_search.len(),"timing":timings(search_chain_ms)}));
    println!(
        "PAGINATION_PROGRESS search chain pages={} results={}",
        search_pages.len(),
        actual_search.len()
    );
    let base_timeline = timeline_request(&boundaries);
    let (expected_events, expected_calendar) = timeline_oracle(&records, &events, &base_timeline);
    let mut timeline_offsets = Vec::new();
    let mut timeline_requests = BTreeMap::new();
    let mut actual_events = Vec::new();
    let mut timeline_chain_ms = Vec::new();
    let mut request = base_timeline.clone();
    if !search_only {
        loop {
            let offset = request.offset as usize;
            timeline_offsets.push(request.offset);
            timeline_requests.insert(request.offset, request.clone());
            let started = Instant::now();
            let response = read!(
                store.timeline(request.clone()),
                "timeline_chain_error",
                request.offset,
                break
            );
            timeline_chain_ms.push(started.elapsed().as_secs_f64() * 1000.);
            let end = (offset + 100).min(expected_events.len());
            let expected = &expected_events[offset.min(end)..end];
            let actual: Vec<_> = response
                .events
                .iter()
                .map(|entry| entry.activity.id.clone())
                .collect();
            let calendar: Vec<_> = response
                .calendar
                .iter()
                .map(|point| (point.at, point.count))
                .collect();
            check(
                actual == expected
                    && timeline_payload_matches(
                        &response,
                        &event_lookup,
                        &records,
                        &request,
                        &metadata,
                    )
                    && calendar == expected_calendar
                    && response.has_more == (end < expected_events.len()),
                json!({"phase":"timeline_chain","offset":offset,"actual":actual,"expected":expected,"calendar":calendar,"expected_calendar":expected_calendar}),
                &mut failures,
                &mut failed,
                &mut checks,
            );
            actual_events.extend(actual);
            if timeline_offsets.len().is_multiple_of(100) {
                println!(
                    "PAGINATION_PROGRESS timeline pages={}",
                    timeline_offsets.len()
                );
            }
            if !response.has_more {
                break;
            }
            let Some(cursor) = response.next_cursor else {
                check(
                    false,
                    json!({"phase":"timeline_cursor_missing","offset":offset}),
                    &mut failures,
                    &mut failed,
                    &mut checks,
                );
                break;
            };
            if cursor.next_offset != request.offset + 100 {
                check(
                    false,
                    json!({"phase":"timeline_cursor_offset","offset":offset}),
                    &mut failures,
                    &mut failed,
                    &mut checks,
                );
                break;
            }
            request.offset = cursor.next_offset;
            request.cursor = Some(cursor);
            if timeline_offsets.len() > 1001 {
                check(
                    false,
                    json!({"phase":"timeline_chain_unbounded"}),
                    &mut failures,
                    &mut failed,
                    &mut checks,
                );
                break;
            }
        }
        check(
            actual_events == expected_events
                && actual_events.iter().collect::<HashSet<_>>().len() == actual_events.len(),
            json!({"phase":"timeline_full_coverage","actual_count":actual_events.len(),"expected_count":expected_events.len()}),
            &mut failures,
            &mut failed,
            &mut checks,
        );
        reports.push(json!({"case":"timeline_first_api_call_cold_calendar","ms":timeline_chain_ms.first(),"not_os_cache_cold":true}));
        reports.push(json!({"case":"timeline_full_chain","pages":timeline_offsets.len(),"events":actual_events.len(),"calendar_points":expected_calendar.len(),"timing":timings(timeline_chain_ms)}));
        println!(
            "PAGINATION_PROGRESS timeline chain pages={} events={}",
            timeline_offsets.len(),
            actual_events.len()
        );
    }
    // 每种筛选只核对首页；整链正确性由上面的全量遍历保证，不把其余筛选冒充千页遍历。
    for index in 0..7 {
        let mut request = search_request();
        match index {
            0 => request.sort = "relevance".into(),
            1 => request.sort = "modified".into(),
            2 => request.request.workspace_id = Some("workspace-000000".into()),
            3 => {
                request.modified_from = Some(boundaries[14]);
                request.modified_to = Some(boundaries[15]);
            }
            4 => {
                request.path_prefix = Some(root.join("部门17").to_string_lossy().into_owned() + "/")
            }
            5 => {
                request.request.kind = "web".into();
                request.path_prefix = Some("HTTPS://EXAMPLE.COM/LIBRARY/0000".into());
            }
            _ => request.request.query = "唯一".into(),
        }
        let expected = search_oracle(&records, &request);
        let response = read!(
            store.search_page(request),
            "search_filter_error",
            0,
            continue
        );
        let actual: Vec<_> = response.page.items.iter().map(|r| r.id.clone()).collect();
        check(
            actual == expected.iter().take(100).cloned().collect::<Vec<_>>()
                && search_payload_matches(&response.page, &record_lookup, &metadata)
                && response.page.has_more == (expected.len() > 100),
            json!({"phase":"search_filter","case":index,"actual":actual,"expected":expected.iter().take(100).collect::<Vec<_>>()}),
            &mut failures,
            &mut failed,
            &mut checks,
        );
    }
    if !search_only {
        for index in 0..7 {
            let mut request = base_timeline.clone();
            match index {
                0 => request.basis = "observed".into(),
                1 => {
                    request.from = Some(boundaries[0]);
                    request.to = Some(boundaries[1]);
                }
                2 => request.workspace_id = Some("workspace-000000".into()),
                3 => request.workspace_id = Some("workspace-000001".into()),
                4 => request.filter = "discovered".into(),
                5 => request.filter = "system".into(),
                _ => {
                    request.filter = "modified".into();
                    request.basis = "observed".into();
                    request.from = Some(boundaries[14]);
                    request.to = Some(boundaries[15]);
                }
            }
            let (expected, calendar) = timeline_oracle(&records, &events, &request);
            let response = read!(
                store.timeline(request.clone()),
                "timeline_filter_error",
                request.offset,
                continue
            );
            let actual: Vec<_> = response
                .events
                .iter()
                .map(|r| r.activity.id.clone())
                .collect();
            let actual_calendar: Vec<_> =
                response.calendar.iter().map(|p| (p.at, p.count)).collect();
            check(
                actual == expected.iter().take(100).cloned().collect::<Vec<_>>()
                    && timeline_payload_matches(
                        &response,
                        &event_lookup,
                        &records,
                        &request,
                        &metadata,
                    )
                    && actual_calendar == calendar
                    && response.has_more == (expected.len() > 100),
                json!({"phase":"timeline_filter","case":index,"actual":actual,"expected":expected.iter().take(100).collect::<Vec<_>>(),"calendar":actual_calendar,"expected_calendar":calendar}),
                &mut failures,
                &mut failed,
                &mut checks,
            );
        }
    }
    let search_plan = search::explain_page(
        &store.connection().unwrap(),
        &search::page_criteria(&search_request()).unwrap(),
        0,
    )
    .unwrap();
    reports.push(json!({"case":"native_search_select_plan","plan":search_plan,"bounded_fts_routing_before_select":true}));
    for offset in [0, 100, 10_000, *search_pages.last_key_value().unwrap().0] {
        let Some(request) = search_pages.get(&offset) else {
            check(
                false,
                json!({"phase":"search_depth_missing","offset":offset}),
                &mut failures,
                &mut failed,
                &mut checks,
            );
            continue;
        };
        let plan = search::explain_page_positions(
            &store.connection().unwrap(),
            &search::page_criteria(request).unwrap(),
            offset,
            request
                .cursor
                .as_ref()
                .and_then(|cursor| cursor.name_position.as_ref()),
            request
                .cursor
                .as_ref()
                .and_then(|cursor| cursor.rank_position.as_ref()),
            request
                .cursor
                .as_ref()
                .and_then(|cursor| cursor.modified_position.as_ref()),
        )
        .unwrap();
        reports.push(json!({"case":"native_search_depth_select_plan","offset":offset,"plan":plan}));
        let end = (offset as usize + 100).min(expected_search.len());
        let expected = &expected_search[offset as usize..end];
        let mut values = Vec::new();
        let mut bytes = 0;
        for repeat in 0..31 {
            let started = Instant::now();
            let response = read!(
                store.search_page(request.clone()),
                "search_depth_error",
                request.offset,
                break
            );
            let elapsed = started.elapsed().as_secs_f64() * 1000.;
            let actual: Vec<_> = response.page.items.iter().map(|r| r.id.clone()).collect();
            check(
                actual == expected
                    && search_payload_matches(&response.page, &record_lookup, &metadata)
                    && response.page.has_more == (end < expected_search.len()),
                json!({"phase":"search_depth","offset":offset,"repeat":repeat}),
                &mut failures,
                &mut failed,
                &mut checks,
            );
            bytes = serde_json::to_vec(&response).unwrap().len();
            if repeat > 0 {
                values.push(elapsed);
            }
        }
        reports.push(json!({"case":"search_depth","offset":offset,"returned":expected.len(),"response_bytes":bytes,"timing":timings(values)}));
    }
    if !search_only {
        for offset in [0, 100, 10_000, *timeline_offsets.last().unwrap()] {
            if offset as usize >= expected_events.len() {
                check(
                    false,
                    json!({"phase":"timeline_depth_outside_oracle","offset":offset}),
                    &mut failures,
                    &mut failed,
                    &mut checks,
                );
                continue;
            }
            let Some(request) = timeline_requests.get(&offset) else {
                check(
                    false,
                    json!({"phase":"timeline_depth_missing","offset":offset}),
                    &mut failures,
                    &mut failed,
                    &mut checks,
                );
                continue;
            };
            let end = (offset as usize + 100).min(expected_events.len());
            let expected = &expected_events[offset as usize..end];
            let mut values = Vec::new();
            let mut bytes = 0;
            for repeat in 0..31 {
                let started = Instant::now();
                let response = read!(
                    store.timeline(request.clone()),
                    "timeline_depth_error",
                    request.offset,
                    break
                );
                let elapsed = started.elapsed().as_secs_f64() * 1000.;
                let actual: Vec<_> = response
                    .events
                    .iter()
                    .map(|r| r.activity.id.clone())
                    .collect();
                let calendar: Vec<_> = response.calendar.iter().map(|p| (p.at, p.count)).collect();
                check(
                    actual == expected
                        && timeline_payload_matches(
                            &response,
                            &event_lookup,
                            &records,
                            request,
                            &metadata,
                        )
                        && calendar == expected_calendar
                        && response.has_more == (end < expected_events.len()),
                    json!({"phase":"timeline_depth","offset":offset,"repeat":repeat}),
                    &mut failures,
                    &mut failed,
                    &mut checks,
                );
                bytes = serde_json::to_vec(&response).unwrap().len();
                if repeat > 0 {
                    values.push(elapsed);
                }
            }
            reports.push(json!({"case":"timeline_depth","offset":offset,"returned":expected.len(),"response_bytes":bytes,"timing":timings(values)}));
            let (_, diagnostic) = store.profile_timeline(request).unwrap();
            reports.push(json!({"case":"timeline_native_shared_sql_diagnostic","offset":offset,"diagnostic":diagnostic,"uncached_calendar_diagnostic_not_api_timing":true}));
        }
    }
    // 模拟当前schema库补建三个既有派生索引；不混作v3到v4新增列的迁移。
    store.connection().unwrap().execute_batch("DROP INDEX search_documents_kind_name; DROP INDEX activities_file_time; DROP INDEX resources_location;").unwrap();
    let bootstrap = (|| -> Result<f64> {
        let started = Instant::now();
        let reopened = Store::open(&database)?;
        let elapsed = started.elapsed().as_secs_f64() * 1000.;
        let restored_metadata = reopened.overview()?;
        check(
            restored_metadata.permission_version == metadata.permission_version
                && restored_metadata.revision == metadata.revision,
            json!({"phase":"index_bootstrap_versions"}),
            &mut failures,
            &mut failed,
            &mut checks,
        );
        let search = reopened.search_page(search_request())?;
        let actual: Vec<_> = search
            .page
            .items
            .iter()
            .map(|item| item.id.clone())
            .collect();
        check(
            actual
                == expected_search
                    .iter()
                    .take(100)
                    .cloned()
                    .collect::<Vec<_>>()
                && search_payload_matches(&search.page, &record_lookup, &restored_metadata),
            json!({"phase":"index_bootstrap_search"}),
            &mut failures,
            &mut failed,
            &mut checks,
        );
        if !search_only {
            let timeline = reopened.timeline(base_timeline.clone())?;
            let actual: Vec<_> = timeline
                .events
                .iter()
                .map(|entry| entry.activity.id.clone())
                .collect();
            let calendar: Vec<_> = timeline
                .calendar
                .iter()
                .map(|point| (point.at, point.count))
                .collect();
            check(
                actual
                    == expected_events
                        .iter()
                        .take(100)
                        .cloned()
                        .collect::<Vec<_>>()
                    && calendar == expected_calendar
                    && timeline_payload_matches(
                        &timeline,
                        &event_lookup,
                        &records,
                        &base_timeline,
                        &restored_metadata,
                    ),
                json!({"phase":"index_bootstrap_timeline"}),
                &mut failures,
                &mut failed,
                &mut checks,
            );
        }
        Ok(elapsed)
    })();
    match bootstrap {
        Ok(elapsed)=>reports.push(json!({"case":"populated_schema_index_bootstrap","schema_version":crate::migrations::VERSION,"open_ms":elapsed,"dropped_derived_indexes":3,"os_cache_cold":false})),
        Err(error)=>check(false,json!({"phase":"index_bootstrap_error","error":error}),&mut failures,&mut failed,&mut checks),
    }
    let migration = (|| -> Result<f64> {
        let fixture = directory.path().join("structural-v3.sqlite3");
        store.backup_to(&fixture)?;
        // 规模夹具移除v3之后的列、索引、定位与历史表；历史链真实性另由迁移单测证明。
        let db = rusqlite::Connection::open(&fixture)?;
        db.execute_batch("BEGIN IMMEDIATE; DROP TABLE activity_root_scopes; DROP TABLE activity_origins; DROP TABLE resource_relinks; DROP INDEX activities_resource_kind_time; DROP INDEX search_documents_modified; DROP INDEX search_documents_kind_modified; ALTER TABLE search_documents DROP COLUMN modified_at; DROP TABLE root_aliases; DROP TABLE root_locators; DROP TABLE file_locators; PRAGMA user_version=3; COMMIT;")?;
        drop(db);
        let started = Instant::now();
        let migrated = Store::open(&fixture)?;
        let elapsed = started.elapsed().as_secs_f64() * 1000.;
        let db = migrated.connection()?;
        let version: u32 = db.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let incorrect:usize=db.query_row("SELECT count(*) FROM search_documents d LEFT JOIN resources r ON r.id=d.entity_id WHERE (d.kind='workspace' AND d.modified_at IS NOT NULL) OR (d.kind<>'workspace' AND d.modified_at IS NOT r.modified_at)",[],|row|row.get(0))?;
        let restored_metadata = migrated.overview()?;
        let search = migrated.search_page(search_request())?;
        check(
            version == crate::migrations::VERSION
                && incorrect == 0
                && restored_metadata.permission_version == metadata.permission_version
                && restored_metadata.revision == metadata.revision
                && search
                    .page
                    .items
                    .iter()
                    .map(|r| r.id.clone())
                    .collect::<Vec<_>>()
                    == expected_search
                        .iter()
                        .take(100)
                        .cloned()
                        .collect::<Vec<_>>()
                && search_payload_matches(&search.page, &record_lookup, &restored_metadata),
            json!({"phase":"structural_v3_migration_projection_and_search"}),
            &mut failures,
            &mut failed,
            &mut checks,
        );
        Ok(elapsed)
    })();
    match migration {
        Ok(elapsed)=>reports.push(json!({"case":"structural_v3_to_current_migration","open_ms":elapsed,"fixture_created_by_dropping_post_v3_columns_indexes_locator_and_history_tables":true,"schema_version":crate::migrations::VERSION,"os_cache_cold":false})),
        Err(error)=>check(false,json!({"phase":"structural_v3_migration_error","error":error}),&mut failures,&mut failed,&mut checks),
    }
    let cache_stats = store.calendar_cache_stats();
    reports.push(json!({"case":"timeline_calendar_cache_stats","entries":cache_stats.0,"hits":cache_stats.1,"capacity":4}));
    let report = json!({"evidence":"synthetic_multi_resource_disk_pagination_not_scan_native_ui_or_cold_disk","timeline_executed":!search_only,"search_sort":search_request().sort,"profile":if cfg!(debug_assertions){"debug"}else{"release"},"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"sqlite_version":rusqlite::version(),"journal_mode":journal,"synchronous":synchronous,"entities":records.len(),"distribution":{"files":98_900,"web":1000,"workspaces":100,"unauthorized_files":400},"events":events.len(),"distinct_synthetic_file_paths":records.iter().filter(|r|r.kind=="file").map(|r|r.path.as_str()).collect::<HashSet<_>>().len(),"event_resources":events.iter().filter_map(|e|e.resource).collect::<HashSet<_>>().len(),"day_lengths_hours":boundaries.windows(2).map(|d|(d[1]-d[0])/3_600_000.).collect::<Vec<_>>(),"seed_seconds":seed_seconds,"checkpointed_bytes_before_events":bytes_before_events,"checkpointed_bytes_after_events":bytes_after_events,"event_growth_bytes":bytes_after_events-bytes_before_events,"oracle_checks":checks,"oracle_failure_count":failed,"oracle_failures_first_32":failures,"per_case":reports,"existing_search_hot_budget_ms":60,"search_depth_exceeds_existing_60ms_budget":reports.iter().filter(|case|case["case"]=="search_depth").any(|case|case["timing"]["p95_ms"].as_f64().is_some_and(|value|value>60.)),"performance_status":"reported_separately_from_correctness_no_budget_relaxed"});
    let output = directory.path().parent().unwrap().join(format!(
        "rust-pagination-report-{}.json",
        uuid::Uuid::new_v4()
    ));
    fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("PAGINATION_ACCEPTANCE {report}");
    println!("PAGINATION_REPORT {}", output.display());
    assert_eq!(
        failed, 0,
        "independent pagination oracle failed; preserve report"
    );
}
