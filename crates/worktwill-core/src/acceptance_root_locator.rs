//! 真实合成文件的目录定位验收；调用 Core/文件 API，不代表 GUI 或真实办公软件验收。
#![cfg(target_os = "macos")]

use crate::{model::*, Store};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Barrier,
    },
    thread,
    time::{Duration, Instant},
};
use unicode_normalization::UnicodeNormalization;

const FILES: usize = 10_000;
const NEW_NAME: &str = "合同_new_while_paused.txt";

#[derive(Clone)]
struct FileRow {
    location: PathBuf,
    name: String,
    identity: String,
    modified_at: f64,
    size: f64,
    status: String,
}
type Snapshot = BTreeMap<String, FileRow>;

fn normalized(value: &str) -> String {
    value.nfc().flat_map(char::to_lowercase).collect()
}

fn write_report(output: &Path, report: &serde_json::Value) {
    fs::write(output, serde_json::to_vec_pretty(report).unwrap()).unwrap();
}

fn percentile(sorted: &[f64], fraction: f64) -> Option<f64> {
    sorted
        .get(((sorted.len() as f64 * fraction).ceil() as usize).saturating_sub(1))
        .copied()
}

fn timed_phase(
    store: &Store,
    name: &str,
    forbid_new: bool,
    operation: impl FnOnce() -> Result<()>,
) -> serde_json::Value {
    let stop = AtomicBool::new(false);
    let barrier = Barrier::new(2);
    let started = Instant::now();
    let (result, wall_seconds, mut timings, samples, empty, partial, errors) =
        thread::scope(|scope| {
            let reader = scope.spawn(|| {
                barrier.wait();
                let mut timings = Vec::new();
                let mut samples = 0;
                let mut empty = 0;
                let mut partial = 0;
                let mut errors = BTreeMap::<String, usize>::new();
                while !stop.load(Ordering::SeqCst) {
                    let query_started = Instant::now();
                    let response = store.search(SearchRequest {
                        request_id: samples,
                        query: "合同".into(),
                        workspace_id: None,
                        kind: "file".into(),
                    });
                    let elapsed = query_started.elapsed().as_secs_f64() * 1000.;
                    samples += 1;
                    if timings.len() < 10_000 {
                        timings.push(elapsed);
                    }
                    match response {
                        Ok(response) => {
                            let items = response.page.items;
                            // 迁移中的授权投影可短暂为空/部分；不能用“始终全量”掩盖真实中间状态。
                            assert!(items.len() <= 30);
                            assert_eq!(
                                items
                                    .iter()
                                    .map(|item| &item.id)
                                    .collect::<BTreeSet<_>>()
                                    .len(),
                                items.len()
                            );
                            assert!(items
                                .iter()
                                .all(|item| item.kind == "file" && item.name.starts_with("合同_")));
                            if forbid_new {
                                assert!(items.iter().all(|item| item.name != NEW_NAME));
                            }
                            if items.is_empty() {
                                empty += 1;
                            } else if items.len() < 30 {
                                partial += 1;
                            }
                        }
                        Err(error) => *errors.entry(error.code).or_default() += 1,
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                (timings, samples, empty, partial, errors)
            });
            barrier.wait();
            // 主操作失败也先停止读线程；避免 scope 在 panic 展开时永远等待读循环。
            let result = catch_unwind(AssertUnwindSafe(operation));
            let wall_seconds = started.elapsed().as_secs_f64();
            stop.store(true, Ordering::SeqCst);
            let (timings, samples, empty, partial, errors) = reader.join().unwrap();
            (
                result,
                wall_seconds,
                timings,
                samples,
                empty,
                partial,
                errors,
            )
        });
    let outcome = match result {
        Ok(result) => result,
        Err(panic) => resume_unwind(panic),
    };
    timings.sort_by(f64::total_cmp);
    serde_json::json!({
        "phase":name,"wall_seconds":wall_seconds,"operation_error":outcome.err(),
        "concurrent_query_samples_total":samples,"timing_samples_retained":timings.len(),
        "timing_scope":"queries_started_before_stop_including_in_flight_tail; first_up_to_10000_retained",
        "empty_query_samples":empty,"partial_nonempty_query_samples":partial,"query_errors":errors,
        "query_p50_ms":percentile(&timings,0.5),"query_p95_ms":percentile(&timings,0.95),"query_p99_ms":percentile(&timings,0.99),
        "search_reference_p95_ms":60,"query_p95_exceeds_reference":percentile(&timings,0.95).map(|value|value>60.),
    })
}

fn append_phase(output: &Path, report: &mut serde_json::Value, phase: serde_json::Value) {
    report["phases"].as_array_mut().unwrap().push(phase);
    write_report(output, report);
    let phase = report["phases"].as_array().unwrap().last().unwrap();
    assert!(phase["operation_error"].is_null(), "phase failed: {phase}");
    assert_eq!(
        phase["query_errors"].as_object().unwrap().len(),
        0,
        "query failures: {phase}"
    );
}

fn snapshot(store: &Store) -> Snapshot {
    store.connection().unwrap().prepare("SELECT id,location,name,identity,modified_at,size,status FROM resources WHERE kind='file' ORDER BY id").unwrap()
        .query_map([],|row|Ok((row.get(0)?,FileRow{location:PathBuf::from(row.get::<_,String>(1)?),name:row.get(2)?,identity:row.get(3)?,modified_at:row.get(4)?,size:row.get(5)?,status:row.get(6)?})))
        .unwrap().collect::<rusqlite::Result<_>>().unwrap()
}

fn verify_known_paths(
    current: &Snapshot,
    original: &Snapshot,
    original_root: &Path,
    current_root: &Path,
    sampled_root: Option<(&BTreeSet<String>, &Path)>,
) {
    // 直接核对持久行，不先调用 resolve，避免按需恢复掩盖批次尾部未完成的问题。
    for (id, previous) in original {
        let actual = current.get(id).expect("original UUID must remain");
        let destination = sampled_root
            .filter(|(ids, _)| ids.contains(id))
            .map_or(current_root, |(_, path)| path);
        assert_eq!(
            actual.location,
            destination.join(previous.location.strip_prefix(original_root).unwrap()),
            "{id}"
        );
        assert_eq!(actual.name, previous.name);
        assert_eq!(actual.identity, previous.identity);
        assert_eq!(actual.modified_at, previous.modified_at);
        assert_eq!(actual.size, previous.size);
        assert_eq!(actual.status, "available");
    }
}

fn verify_counts(
    store: &Store,
    resources: usize,
    root_id: &str,
    root_path: &Path,
    paused: bool,
    aliases: &[&Path],
) -> serde_json::Value {
    let db = store.connection().unwrap();
    let count = |table: &str| {
        db.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap()
    };
    assert_eq!(count("resources"), resources);
    assert_eq!(count("file_roots"), resources);
    assert_eq!(count("search_documents"), resources);
    assert_eq!(count("search_tri"), resources);
    assert_eq!(count("search_short"), resources);
    assert_eq!(count("file_grants"), 0);
    assert_eq!(count("file_locators"), 0);
    assert_eq!(count("root_locators"), 1);
    let discovered:(usize,usize)=db.query_row("SELECT count(*),count(DISTINCT resource_id) FROM activities WHERE kind='file_discovered'",[],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
    assert_eq!(discovered, (resources, resources));
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM activities WHERE kind='file_modified'",
            [],
            |row| row.get::<_, usize>(0)
        )
        .unwrap(),
        0
    );
    let projection = db.prepare("SELECT r.id,r.kind,r.name,r.location,r.modified_at,d.kind,d.name,d.location,d.modified_at FROM resources r JOIN search_documents d ON d.entity_id=r.id ORDER BY r.id").unwrap()
        .query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,f64>(4)?,row.get::<_,String>(5)?,row.get::<_,String>(6)?,row.get::<_,String>(7)?,row.get::<_,f64>(8)?)))
        .unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
    assert_eq!(projection.len(), resources);
    for (
        id,
        kind,
        name,
        location,
        modified,
        index_kind,
        index_name,
        index_location,
        index_modified,
    ) in projection
    {
        assert_eq!(kind, index_kind, "{id}");
        assert_eq!(normalized(&name), index_name, "{id}");
        assert_eq!(normalized(&location), index_location, "{id}");
        assert_eq!(modified, index_modified, "{id}");
    }
    let persisted_aliases: BTreeSet<String> = db
        .prepare("SELECT path FROM root_aliases WHERE root_id=? ORDER BY path")
        .unwrap()
        .query_map([root_id], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let expected_aliases: BTreeSet<String> = aliases
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(persisted_aliases, expected_aliases);
    assert!(db
        .query_row(
            "SELECT valid=1 AND length(bookmark)>0 FROM root_locators WHERE root_id=?",
            [root_id],
            |row| row.get::<_, bool>(0)
        )
        .unwrap());
    let root_identity: String = db
        .query_row(
            "SELECT identity FROM root_locators WHERE root_id=?",
            [root_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        crate::platform::identity(root_path, &root_path.metadata().unwrap()).unwrap(),
        root_identity
    );
    let roots = store.roots().unwrap();
    assert_eq!(roots.len(), 1);
    let root = &roots[0];
    assert_eq!(root.id, root_id);
    assert_eq!(root.path, root_path.to_string_lossy());
    assert!(root.active);
    assert_eq!(root.paused, paused);
    assert_eq!(root.location_status, "available");
    if !paused {
        assert_eq!(root.scan_state, "completed");
        assert_eq!(root.scanned as usize, resources);
        assert_eq!(root.failures, 0);
    }
    serde_json::json!({"resources":resources,"root_memberships":resources,"search_documents":resources,"fts_tri":resources,"fts_short":resources,"discovered":discovered.0,"distinct_discovered_resources":discovered.1,"file_modified_events":0,"single_file_grants":0,"single_file_locators":0,"root_id":root_id,"root_identity":root_identity,"root_path":root_path,"paused":paused,"root_location_status":root.location_status,"aliases":persisted_aliases,"verified_raw_projection_rows":resources})
}

fn pause(store: &Store, root_id: &str, value: bool) -> Result<()> {
    store.mutate(MutationRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        mutation: Mutation::SetPaused {
            id: root_id.into(),
            value,
        },
    })?;
    Ok(())
}

#[test]
#[ignore = "explicit 10k real-file directory locator acceptance; requires approved WORKTWILL_TEST_ROOT"]
fn disk_root_locator_acceptance() {
    let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
    let directory = tempfile::Builder::new()
        .prefix("root-locator-acceptance-")
        .tempdir_in(approved)
        .unwrap();
    // 报告位于本测试独有目录；保留合成夹具及失败中间报告供复查，不清空既有目录。
    let home = directory.keep().canonicalize().unwrap();
    let output = home.join("root-locator-report.json");
    let mut report = serde_json::json!({"evidence":"real_synthetic_10000_files_core_and_native_file_apis_not_gui_or_office","profile":if cfg!(debug_assertions){"debug"}else{"release"},"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"sqlite_version":rusqlite::version(),"initial_files":FILES,"fixtures_retained":true,"status":"in_progress","phases":[]});
    write_report(&output, &report);
    println!("ROOT_LOCATOR_REPORT {}", output.display());
    let documents = home.join("documents");
    fs::create_dir(&documents).unwrap();
    let seed_started = Instant::now();
    for folder in 0..100 {
        fs::create_dir(documents.join(format!("部门{folder:03}"))).unwrap();
    }
    for index in 0..FILES {
        fs::write(
            documents.join(format!("部门{:03}/合同_{index:05}.txt", index % 100)),
            format!("synthetic root locator fixture {index}"),
        )
        .unwrap();
    }
    report["file_creation_seconds"] = seed_started.elapsed().as_secs_f64().into();
    let database = home.join("app/catalog.sqlite3");
    let store = Store::open(&database).unwrap();
    let db = store.connection().unwrap();
    let journal: String = db
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    let synchronous: u32 = db
        .pragma_query_value(None, "synchronous", |row| row.get(0))
        .unwrap();
    assert_eq!(journal.to_lowercase(), "wal");
    assert_eq!(synchronous, 2);
    drop(db);
    report["journal_mode"] = journal.into();
    report["synchronous"] = synchronous.into();
    let granted = Instant::now();
    let root_id = store.grant_root(&documents).unwrap();
    report["grant_seconds"] = granted.elapsed().as_secs_f64().into();
    let phase = timed_phase(&store, "initial_scan", true, || store.scan_all());
    append_phase(&output, &mut report, phase);
    let original = snapshot(&store);
    assert_eq!(original.len(), FILES);
    let original_ids: BTreeSet<_> = original.keys().cloned().collect();
    verify_known_paths(&original, &original, &documents, &documents, None);
    report["initial_counts"] = verify_counts(&store, FILES, &root_id, &documents, false, &[]);
    write_report(&output, &report);
    let paused = Instant::now();
    pause(&store, &root_id, true).unwrap();
    report["pause_seconds"] = paused.elapsed().as_secs_f64().into();
    let moved = home.join("renamed-documents");
    let phase = timed_phase(
        &store,
        "paused_move_and_refresh_known_without_enumeration",
        true,
        || {
            fs::rename(&documents, &moved)?;
            fs::write(
                moved.join(NEW_NAME),
                b"new fixture must not be enumerated while paused",
            )?;
            store.refresh_roots()
        },
    );
    append_phase(&output, &mut report, phase);
    let first = snapshot(&store);
    assert_eq!(first.keys().cloned().collect::<BTreeSet<_>>(), original_ids);
    verify_known_paths(&first, &original, &documents, &moved, None);
    assert!(!first.values().any(|file| file.name == NEW_NAME));
    report["paused_refresh_counts"] =
        verify_counts(&store, FILES, &root_id, &moved, true, &[&documents]);
    write_report(&output, &report);
    let previous_session = store.session.clone();
    let reopening = Instant::now();
    drop(store);
    let store = Store::open(&database).unwrap();
    report["reopen_seconds"] = reopening.elapsed().as_secs_f64().into();
    assert_ne!(store.session, previous_session);
    let reopened = snapshot(&store);
    assert_eq!(
        reopened.keys().cloned().collect::<BTreeSet<_>>(),
        original_ids
    );
    verify_known_paths(&reopened, &original, &documents, &moved, None);
    report["reopen_counts"] = verify_counts(&store, FILES, &root_id, &moved, true, &[&documents]);
    write_report(&output, &report);
    let sampled_ids: BTreeSet<_> = [0, 1, 99, 100, 999, 5000, 9999]
        .into_iter()
        .map(|index| original.keys().nth(index).unwrap().clone())
        .collect();
    fs::create_dir_all(home.join("elsewhere/deep")).unwrap();
    let second = home.join("elsewhere/deep/moved-documents");
    let phase = timed_phase(
        &store,
        "reopened_second_move_and_on_demand_resolve_samples",
        true,
        || {
            fs::rename(&moved, &second)?;
            for id in &sampled_ids {
                let expected = second.join(original[id].location.strip_prefix(&documents).unwrap());
                let (resolved, web) = store.resolve_resource(id)?;
                assert!(!web);
                assert_eq!(Path::new(&resolved), expected);
            }
            Ok(())
        },
    );
    append_phase(&output, &mut report, phase);
    let partly_recovered = snapshot(&store);
    assert_eq!(
        partly_recovered.keys().cloned().collect::<BTreeSet<_>>(),
        original_ids
    );
    verify_known_paths(
        &partly_recovered,
        &original,
        &documents,
        &moved,
        Some((&sampled_ids, &second)),
    );
    report["on_demand_counts"] = verify_counts(
        &store,
        FILES,
        &root_id,
        &second,
        true,
        &[&documents, &moved],
    );
    report["on_demand_sampled_original_resources"] = sampled_ids.len().into();
    write_report(&output, &report);
    let phase = timed_phase(
        &store,
        "unpause_scan_collects_new_file_and_converges_all_paths",
        false,
        || {
            pause(&store, &root_id, false)?;
            store.scan_all()
        },
    );
    append_phase(&output, &mut report, phase);
    let final_rows = snapshot(&store);
    assert_eq!(final_rows.len(), FILES + 1);
    verify_known_paths(&final_rows, &original, &documents, &second, None);
    let added: Vec<_> = final_rows
        .iter()
        .filter(|(id, _)| !original_ids.contains(*id))
        .collect();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].1.location, second.join(NEW_NAME));
    assert_eq!(added[0].1.name, NEW_NAME);
    report["final_counts"] = verify_counts(
        &store,
        FILES + 1,
        &root_id,
        &second,
        false,
        &[&documents, &moved],
    );
    let metadata_started = Instant::now();
    for (id, previous) in &original {
        let actual = &final_rows[id];
        let metadata = actual.location.metadata().unwrap();
        assert_eq!(
            crate::platform::identity(&actual.location, &metadata).unwrap(),
            previous.identity
        );
        assert_eq!(metadata.len() as f64, previous.size);
        assert_eq!(crate::catalog::mtime(&metadata), previous.modified_at);
    }
    for id in &sampled_ids {
        let path = &final_rows[id].location;
        let number: usize = path
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix("合同_")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            fs::read(path).unwrap(),
            format!("synthetic root locator fixture {number}").as_bytes()
        );
    }
    assert_eq!(
        fs::read(second.join(NEW_NAME)).unwrap(),
        b"new fixture must not be enumerated while paused"
    );
    report["final_original_file_metadata_verified"] = FILES.into();
    report["final_original_file_bytes_verified_samples"] = sampled_ids.len().into();
    report["final_file_metadata_and_sample_bytes_verification_seconds"] =
        metadata_started.elapsed().as_secs_f64().into();
    let persisted = Instant::now();
    drop(store);
    let store = Store::open(&database).unwrap();
    report["final_reopen_seconds"] = persisted.elapsed().as_secs_f64().into();
    report["final_persisted_counts"] = verify_counts(
        &store,
        FILES + 1,
        &root_id,
        &second,
        false,
        &[&documents, &moved],
    );
    let persisted_rows = snapshot(&store);
    assert_eq!(
        persisted_rows.keys().cloned().collect::<BTreeSet<_>>(),
        final_rows.keys().cloned().collect()
    );
    verify_known_paths(&persisted_rows, &original, &documents, &second, None);
    report["status"] = "correctness_verified".into();
    write_report(&output, &report);
    println!(
        "ROOT_LOCATOR_ACCEPTANCE {}",
        serde_json::to_string(&report).unwrap()
    );
    println!("ROOT_LOCATOR_REPORT {}", output.display());
}
