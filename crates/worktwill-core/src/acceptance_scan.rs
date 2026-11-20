//! 实际合成文件的正式扫描路径；不代表原生窗口、真实办公目录或空闲资源验收。
use crate::{model::*, Store};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

fn timed_rescan(
    store: &Arc<Store>,
    phase: &str,
    files: usize,
    first_scan: bool,
) -> serde_json::Value {
    let stopping = Arc::new(AtomicBool::new(false));
    let stop = stopping.clone();
    let reader = store.clone();
    let started = Instant::now();
    let queries = thread::spawn(move || {
        let mut elapsed = Vec::new();
        let mut first_result_seconds = None;
        let mut partial_scan_observation = None;
        let mut empty_samples = 0;
        let mut total_samples = 0;
        while !stop.load(Ordering::SeqCst) {
            let before = if first_scan && partial_scan_observation.is_none() {
                Some(reader.roots().unwrap().remove(0))
            } else {
                None
            };
            let start = Instant::now();
            let result = reader
                .search(SearchRequest {
                    request_id: 1,
                    query: "合同".into(),
                    workspace_id: None,
                    kind: "file".into(),
                })
                .unwrap();
            let query_ms = start.elapsed().as_secs_f64() * 1000.;
            if first_scan {
                assert!(result.page.items.len() <= 30);
                assert!(result.page.items.iter().all(|r| r.kind == "file"
                    && r.name.starts_with("合同_")
                    && r.status == "available"));
            } else {
                assert_eq!(result.page.items.len(), 30);
            }
            if result.page.items.is_empty() {
                empty_samples += 1;
            } else if first_result_seconds.is_none() {
                first_result_seconds = Some(started.elapsed().as_secs_f64());
            }
            if !result.page.items.is_empty() {
                if let Some(before) = before.filter(|r| r.scan_state == "running") {
                    let after = reader.roots().unwrap().remove(0);
                    // 查询前后均仍为未完成的同次初扫，排除结束边界的调度空隙。
                    if after.scan_state == "running" && (after.scanned as usize) < files {
                        partial_scan_observation = Some(
                            serde_json::json!({"seconds":started.elapsed().as_secs_f64(),"scanned_before":before.scanned,"scanned_after":after.scanned,"matches":result.page.items.len()}),
                        );
                    }
                }
            }
            total_samples += 1;
            if elapsed.len() < 10_000 {
                elapsed.push(query_ms);
            }
            thread::sleep(Duration::from_millis(5));
        }
        (
            elapsed,
            first_result_seconds,
            partial_scan_observation,
            empty_samples,
            total_samples,
        )
    });
    let result = store.scan_all();
    let wall = started.elapsed().as_secs_f64();
    stopping.store(true, Ordering::SeqCst);
    let (mut queries, first_result_seconds, partial_scan_observation, empty_samples, total_samples) =
        queries.join().unwrap();
    result.unwrap();
    assert!(first_result_seconds.is_some());
    if first_scan {
        assert!(partial_scan_observation.is_some());
    }
    queries.sort_by(f64::total_cmp);
    let percentile = |p: f64| {
        queries
            .get(((queries.len() as f64 * p).ceil() as usize).saturating_sub(1))
            .copied()
    };
    serde_json::json!({"phase":phase,"wall_seconds":wall,"concurrent_query_samples_total":total_samples,"timing_samples_retained":queries.len(),"timing_scope":"first_up_to_10000_queries_started_before_stop_including_in_flight_tail","first_nonempty_result_seconds":first_result_seconds,"partial_scan_observation":partial_scan_observation,"empty_query_samples":empty_samples,"query_p50_ms":percentile(0.5),"query_p95_ms":percentile(0.95),"query_p99_ms":percentile(0.99)})
}

type CatalogSnapshot = BTreeMap<String, (String, String, Option<String>, f64, f64, String)>;

fn verify_phase(store: &Store, files: usize, modified_id: Option<&str>) -> CatalogSnapshot {
    let db = store.connection().unwrap();
    let snapshot: CatalogSnapshot = db.prepare("SELECT id,location,name,identity,modified_at,size,status FROM resources WHERE kind='file'").unwrap().query_map([], |r| Ok((r.get(0)?,(r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    assert_eq!(snapshot.len(), files);
    let mismatched:usize=db.query_row("SELECT count(*) FROM resources r LEFT JOIN search_documents d ON d.entity_id=r.id WHERE d.entity_id IS NULL OR d.modified_at IS NOT r.modified_at",[],|r|r.get(0)).unwrap();
    assert_eq!(
        mismatched, 0,
        "each scan phase must preserve the derived modification time"
    );
    assert!(snapshot
        .values()
        .all(|v| v.2.is_some() && v.5 == "available"));
    let discovered: (usize, usize) = db.query_row("SELECT count(*),count(DISTINCT resource_id) FROM activities WHERE kind='file_discovered'", [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(discovered, (files, files));
    let modified: Vec<String> = db
        .prepare("SELECT resource_id FROM activities WHERE kind='file_modified'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(
        modified,
        modified_id
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    );
    let root = store.roots().unwrap().remove(0);
    assert_eq!(root.scan_state, "completed");
    assert_eq!(root.scanned as usize, files);
    assert_eq!(root.failures, 0);
    snapshot
}

#[test]
#[ignore = "explicit real-file scan benchmark; requires confirmed WORKTWILL_TEST_ROOT"]
fn disk_scan_acceptance() {
    let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
    // 大规模验收显式选择，避免日常检查意外创建十万个文件。
    let files = match std::env::var("WORKTWILL_SCAN_FILES").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("10000") => 10_000,
        Ok("100000") => 100_000,
        _ => panic!("WORKTWILL_SCAN_FILES must be 10000 or 100000"),
    };
    let folders = files / 100;
    let directory = tempfile::Builder::new()
        .prefix("real-scan-")
        .tempdir_in(approved)
        .unwrap();
    let docs = directory.path().join("documents");
    fs::create_dir(&docs).unwrap();
    let seed = Instant::now();
    for folder in 0..folders {
        fs::create_dir(docs.join(format!("部门{folder:03}"))).unwrap();
    }
    for n in 0..files {
        fs::write(
            docs.join(format!("部门{:03}/合同_{n:05}.txt", n % folders)),
            format!("synthetic fixture {n}"),
        )
        .unwrap();
    }
    let seed_seconds = seed.elapsed().as_secs_f64();
    let store = Arc::new(Store::open(directory.path().join("app/catalog.sqlite3")).unwrap());
    store.grant_root(&docs).unwrap();
    let initial = timed_rescan(&store, "initial_with_concurrent_search", files, true);
    let baseline = verify_phase(&store, files, None);
    let paths: HashSet<_> = baseline.values().map(|v| v.0.as_str()).collect();
    assert_eq!(paths.len(), files);
    for n in 0..files {
        assert!(paths.contains(
            docs.join(format!("部门{:03}/合同_{n:05}.txt", n % folders))
                .to_str()
                .unwrap()
        ));
    }
    let target = store
        .search(SearchRequest {
            request_id: 1,
            query: "合同_00000".into(),
            workspace_id: None,
            kind: "file".into(),
        })
        .unwrap()
        .page
        .items
        .remove(0);
    let mutate = |mutation| {
        store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation,
            })
            .unwrap()
    };
    let workspace = mutate(Mutation::CreateWorkspace {
        name: "扫描验收".into(),
        resource_ids: None,
    })
    .id
    .unwrap();
    mutate(Mutation::Link {
        workspace_id: workspace.clone(),
        resource_ids: vec![target.id.clone()],
    });
    mutate(Mutation::Favorite {
        id: target.id.clone(),
        value: true,
    });
    let before_bytes = fs::metadata(&store.path).unwrap().len();
    let unchanged = timed_rescan(&store, "unchanged_with_concurrent_search", files, false);
    assert_eq!(verify_phase(&store, files, None), baseline);
    let after_bytes = fs::metadata(&store.path).unwrap().len();
    fs::write(
        &target.location,
        b"changed synthetic fixture with distinct size",
    )
    .unwrap();
    let changed = timed_rescan(&store, "one_modified_with_concurrent_search", files, false);
    let mut expected = baseline;
    let target_metadata = fs::metadata(&target.location).unwrap();
    let expected_target = expected.get_mut(&target.id).unwrap();
    expected_target.3 = crate::catalog::mtime(&target_metadata);
    expected_target.4 = target_metadata.len() as f64;
    assert_eq!(verify_phase(&store, files, Some(&target.id)), expected);
    let db = store.connection().unwrap();
    let current = Store::resource_db(&db, &target.id, false).unwrap();
    assert!(current.favorite && current.workspace_ids.contains(&workspace));
    assert_eq!(current.status, "available");
    assert_eq!(current.location, target.location);
    assert_eq!(
        fs::read(&target.location).unwrap(),
        b"changed synthetic fixture with distinct size"
    );
    let report = serde_json::json!({"evidence":"real_synthetic_files_formal_store_scan_not_native_ui_or_idle_resource_usage","profile":if cfg!(debug_assertions){"debug"}else{"release"},"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"files":files,"directories":folders,"seed_seconds":seed_seconds,"initial_scan_seconds":initial["wall_seconds"],"db_bytes_before_repeat":before_bytes,"db_bytes_after_repeat":after_bytes,"resource_count":files,"discovery_events":files,"modified_events":1,"every_phase_counts_and_target_event_verified":true,"all_resource_mappings_and_metadata_verified":true,"target_reference_favorite_and_bytes_preserved":true,"results":[initial,unchanged,changed]});
    let output = directory
        .path()
        .parent()
        .unwrap()
        .join(format!("rust-scan-report-{}.json", uuid::Uuid::new_v4()));
    fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("SCAN_ACCEPTANCE {report}");
    println!("SCAN_REPORT {}", output.display());
}
