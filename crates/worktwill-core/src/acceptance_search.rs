//! 产品 Rust 查询路径的磁盘基准，样本元数据是合成的，不代表文件扫描或界面延迟。
use crate::{model::*, search, Store};
use rusqlite::params;
use std::{fs, time::Instant};

#[test]
#[ignore = "explicit acceptance benchmark; requires confirmed WORKTWILL_TEST_ROOT"]
fn disk_search_acceptance() {
    let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
    let directory = tempfile::Builder::new()
        .prefix("rust-search-")
        .tempdir_in(approved)
        .unwrap();
    let root = directory.path().join("documents");
    fs::create_dir(&root).unwrap();
    let store = Store::open(directory.path().join("app/catalog.sqlite3")).unwrap();
    let root_id = store.grant_root(&root).unwrap();
    let patterns = [
        "合作合同_张三",
        "招聘计划_李四",
        "年度总结_2026",
        "项目 Report",
        "notes_100%_done",
        "预算_财务",
        "archive_alpha",
        "报价 \"draft\"",
    ];
    let queries = [
        "张",
        "张三",
        "合作合同",
        "合同",
        "招聘",
        "2026",
        "report",
        "项目 r",
        "100%",
        "_",
        "\"draft\"",
        "archive",
        "docx",
        "documents",
        "不存在的资料",
    ];
    let mut records: Vec<(String, String, String)> = Vec::new();
    let mut reports = Vec::new();
    let mut budget_failed = false;
    let sizes = if std::env::var_os("WORKTWILL_SEARCH_DIAGNOSTIC").is_some() {
        vec![100_000]
    } else {
        vec![10_000, 50_000, 100_000, 500_000]
    };
    for size in sizes {
        let seed_started = Instant::now();
        {
            let mut db = store.connection().unwrap();
            let tx = db.transaction().unwrap();
            for index in records.len()..size {
                let id = format!("file-{index:06}");
                let name = format!("{}_{index:06}.docx", patterns[index % patterns.len()]);
                let location = root
                    .join(format!("部门{}/{}", index % 37, name))
                    .to_string_lossy()
                    .into_owned();
                tx.execute("INSERT INTO resources(id,kind,name,location,created_at) VALUES(?1,'file',?2,?3,0)",params![id,name,location]).unwrap();
                tx.execute(
                    "INSERT INTO file_roots VALUES(?1,?2,0)",
                    params![root_id, id],
                )
                .unwrap();
                search::project(&tx, &id, "file", &name, &location, Some(0.)).unwrap();
                records.push((id, name, location));
            }
            tx.commit().unwrap();
        }
        let seed_seconds = seed_started.elapsed().as_secs_f64();
        let mut timings = Vec::new();
        let mut oracle_checks = 0;
        let mut query_timings = Vec::new();
        let mut expected_by_query = Vec::new();
        for query in queries {
            let q = query.to_lowercase();
            let mut expected: Vec<_> = records
                .iter()
                .filter(|(_, name, path)| {
                    name.to_lowercase().contains(&q) || path.to_lowercase().contains(&q)
                })
                .collect();
            expected.sort_by_cached_key(|(id, name, _)| {
                let name = name.to_lowercase();
                let rank = if name == q {
                    0
                } else if name.starts_with(&q) {
                    1
                } else if name.contains(&q) {
                    2
                } else {
                    3
                };
                (rank, name, id.clone())
            });
            let expected_ids: Vec<_> = expected.iter().take(30).map(|r| r.0.as_str()).collect();
            expected_by_query.push(
                expected_ids
                    .iter()
                    .map(|id| (*id).to_owned())
                    .collect::<Vec<_>>(),
            );
            let mut per_query = Vec::new();
            for repeat in 0..11 {
                let started = Instant::now();
                let response = store
                    .search(SearchRequest {
                        request_id: repeat,
                        query: query.into(),
                        workspace_id: None,
                        kind: "all".into(),
                    })
                    .unwrap();
                let elapsed = started.elapsed().as_secs_f64() * 1000.;
                assert_eq!(
                    response
                        .page
                        .items
                        .iter()
                        .map(|r| r.id.as_str())
                        .collect::<Vec<_>>(),
                    expected_ids,
                    "{size}: {query}"
                );
                oracle_checks += 1;
                if repeat > 0 {
                    per_query.push(elapsed);
                }
            }
            per_query.sort_by(f64::total_cmp);
            query_timings.push(serde_json::json!({"query":query,"p95_ms":per_query[9]}));
            timings.extend(per_query.iter().copied());
        }
        timings.sort_by(f64::total_cmp);
        let percentile =
            |p: f64| timings[((timings.len() as f64 * p).ceil() as usize).saturating_sub(1)];
        let p95 = percentile(0.95);
        if size == 100_000 && p95 > 60. {
            budget_failed = true;
        }
        let report = serde_json::json!({"resources":size,"seed_seconds":seed_seconds,"samples":timings.len(),"oracle_checks":oracle_checks,"p50_ms":percentile(0.5),"p95_ms":p95,"p99_ms":percentile(0.99),"sqlite_version":rusqlite::version(),"per_query":query_timings});
        println!("WORKTWILL_SEARCH_ACCEPTANCE {report}");
        reports.push(report);
        if size == 100_000 {
            use std::sync::{
                atomic::{AtomicBool, Ordering},
                Barrier,
            };
            let stop = AtomicBool::new(false);
            let barrier = Barrier::new(2);
            let mut concurrent = Vec::new();
            let writes = std::thread::scope(|scope| {
                let writer = scope.spawn(|| {
                    barrier.wait();
                    let mut writes = 0;
                    for index in 0..500 {
                        if stop.load(Ordering::SeqCst) {
                            break;
                        }
                        store
                            .mutate(MutationRequest {
                                operation_id: uuid::Uuid::new_v4().to_string(),
                                mutation: Mutation::Favorite {
                                    id: "file-000000".into(),
                                    value: index % 2 == 0,
                                },
                            })
                            .unwrap();
                        writes += 1;
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                    writes
                });
                barrier.wait();
                for repeat in 0..10 {
                    for (index, query) in queries.iter().enumerate() {
                        let started = Instant::now();
                        let response = store
                            .search(SearchRequest {
                                request_id: repeat,
                                query: (*query).into(),
                                workspace_id: None,
                                kind: "all".into(),
                            })
                            .unwrap();
                        concurrent.push(started.elapsed().as_secs_f64() * 1000.);
                        assert_eq!(
                            response
                                .page
                                .items
                                .iter()
                                .map(|r| r.id.as_str())
                                .collect::<Vec<_>>(),
                            expected_by_query[index]
                                .iter()
                                .map(String::as_str)
                                .collect::<Vec<_>>()
                        );
                    }
                }
                stop.store(true, Ordering::SeqCst);
                writer.join().unwrap()
            });
            assert!(writes > 0);
            concurrent.sort_by(f64::total_cmp);
            let percentile = |p: f64| {
                concurrent[((concurrent.len() as f64 * p).ceil() as usize).saturating_sub(1)]
            };
            if percentile(0.95) > 60. {
                budget_failed = true;
            }
            let report = serde_json::json!({"resources":size,"mode":"concurrent_favorite_writer","committed_writes":writes,"samples":concurrent.len(),"oracle_checks":concurrent.len(),"p50_ms":percentile(0.5),"p95_ms":percentile(0.95),"p99_ms":percentile(0.99)});
            println!("WORKTWILL_SEARCH_ACCEPTANCE {report}");
            reports.push(report);
        }
    }
    let output = directory
        .path()
        .parent()
        .unwrap()
        .join(format!("rust-search-report-{}.json", uuid::Uuid::new_v4()));
    fs::write(&output, serde_json::to_vec_pretty(&reports).unwrap()).unwrap();
    println!("WORKTWILL_SEARCH_REPORT {}", output.display());
    assert!(
        !budget_failed,
        "100k native hot query P95 exceeded 60ms; keep acceptance failed"
    );
}
