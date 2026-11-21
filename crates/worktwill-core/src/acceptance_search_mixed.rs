//! 混合实体的合成磁盘查询验收；不读取真实文档，不代表实扫或 UI 可见时延。
use crate::{model::*, search, Store};
use rusqlite::params;
use std::{collections::HashSet, fs, time::Instant};
use unicode_normalization::UnicodeNormalization;

#[derive(Clone)]
struct Record {
    id: String,
    kind: &'static str,
    name: String,
    location: String,
    authorized: bool,
    in_workspace: bool,
}

struct Case {
    query: &'static str,
    kind: &'static str,
    workspace: bool,
}

// Oracle 从全量原始记录独立规范化和排序，不复用 SQL/FTS/生产候选算法。
fn normalized(value: &str) -> String {
    value.nfc().flat_map(char::to_lowercase).collect()
}

fn oracle(records: &[Record], case: &Case) -> (Vec<String>, bool, [usize; 3]) {
    let query = normalized(case.query);
    let mut matches: Vec<_> = records
        .iter()
        .filter(|record| record.authorized)
        .filter(|record| case.kind == "all" || record.kind == case.kind)
        .filter(|record| !case.workspace || record.in_workspace)
        .filter_map(|record| {
            let name = normalized(&record.name);
            let location = normalized(&record.location);
            if !name.contains(&query) && !location.contains(&query) {
                return None;
            }
            let rank = if name == query {
                0
            } else if name.starts_with(&query) {
                1
            } else if name.contains(&query) {
                2
            } else {
                3
            };
            Some(((rank, name, record.id.as_str()), record))
        })
        .collect();
    matches.sort_by(|a, b| a.0.cmp(&b.0));
    let has_more = matches.len() > 30;
    let mut selected = HashSet::new();
    if case.kind == "all" && has_more {
        for kind in ["workspace", "file", "web"] {
            selected.extend(
                matches
                    .iter()
                    .filter(|(_, record)| record.kind == kind)
                    .take(10)
                    .map(|(_, record)| record.id.as_str()),
            );
        }
    }
    for (_, record) in &matches {
        if selected.len() == 30 {
            break;
        }
        selected.insert(record.id.as_str());
    }
    let mut counts = [0; 3];
    let ids = matches
        .into_iter()
        .filter(|(_, record)| selected.contains(record.id.as_str()))
        .map(|(_, record)| {
            counts[match record.kind {
                "workspace" => 0,
                "file" => 1,
                _ => 2,
            }] += 1;
            record.id.clone()
        })
        .collect();
    (ids, has_more, counts)
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    sorted[((sorted.len() as f64 * fraction).ceil() as usize).saturating_sub(1)]
}

#[test]
#[ignore = "explicit mixed acceptance benchmark; requires confirmed WORKTWILL_TEST_ROOT"]
fn disk_search_acceptance_mixed() {
    let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
    let directory = tempfile::Builder::new()
        .prefix("rust-search-mixed-")
        .tempdir_in(&approved)
        .unwrap();
    let root = directory.path().join("documents");
    let denied_root = directory.path().join("unauthorized");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&denied_root).unwrap();
    // 与正式授权入库路径一致；权限外样本仍使用独立目录并显式撤权。
    let root = root.canonicalize().unwrap();
    let denied_root = denied_root.canonicalize().unwrap();
    let store = Store::open(directory.path().join("app/catalog.sqlite3")).unwrap();
    let root_id = store.grant_root(&root).unwrap();
    let denied_id = store.grant_root(&denied_root).unwrap();
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
    let mut records = Vec::with_capacity(100_000);
    let seed_started = Instant::now();
    let (journal, synchronous) = {
        let mut db = store.connection().unwrap();
        let journal: String = db
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        let synchronous: u32 = db
            .pragma_query_value(None, "synchronous", |row| row.get(0))
            .unwrap();
        assert_eq!(journal.to_lowercase(), "wal");
        assert_eq!(synchronous, 2, "SQLite synchronous must remain FULL");
        let tx = db.transaction().unwrap();
        for kind in ["file", "web", "workspace"] {
            let count = match kind {
                "file" => 98_900,
                "web" => 1_000,
                _ => 100,
            };
            for index in 0..count {
                let id = format!("{kind}-{index:06}");
                let authorized = kind != "file" || index < 98_500;
                let rare_count = match kind {
                    "file" => 120,
                    "web" => 3,
                    _ => 2,
                };
                let stem = if index < rare_count {
                    "限额余位_合同"
                } else if index == rare_count {
                    "稀少唯一"
                } else if !authorized {
                    "张"
                } else {
                    patterns[index % patterns.len()]
                };
                let name = if !authorized {
                    "张".into()
                } else {
                    format!(
                        "{stem}_{index:06}{}",
                        if kind == "file" { ".docx" } else { "" }
                    )
                };
                let location = match kind {
                    "file" => if authorized { &root } else { &denied_root }
                        .join(format!("部门{}/{}", index % 37, name))
                        .to_string_lossy()
                        .into_owned(),
                    "web" => format!(
                        "https://example.com/library/{}/{index:06}?topic=张三",
                        index % 37
                    ),
                    _ => String::new(),
                };
                // 工作区零号作为范围测试：只暴露关联子集及范围本身，不把其他工作区混入。
                let in_workspace = (kind == "workspace" && index == 0)
                    || (kind == "file" && index < 500)
                    || (kind == "web" && index < 100);
                if kind == "workspace" {
                    tx.execute(
                        "INSERT INTO workspaces(id,name,created_at) VALUES(?1,?2,0)",
                        params![id, name],
                    )
                    .unwrap();
                } else {
                    tx.execute("INSERT INTO resources(id,kind,name,location,created_at) VALUES(?1,?2,?3,?4,0)", params![id,kind,name,location]).unwrap();
                    if kind == "file" {
                        tx.execute(
                            "INSERT INTO file_roots VALUES(?1,?2,0)",
                            params![if authorized { &root_id } else { &denied_id }, id],
                        )
                        .unwrap();
                    }
                }
                search::project(
                    &tx,
                    &id,
                    kind,
                    &name,
                    &location,
                    (kind != "workspace").then_some(0.),
                )
                .unwrap();
                records.push(Record {
                    id,
                    kind,
                    name,
                    location,
                    authorized,
                    in_workspace,
                });
            }
        }
        for record in records
            .iter()
            .filter(|record| record.kind != "workspace" && record.in_workspace)
        {
            tx.execute(
                "INSERT INTO workspace_resources VALUES('workspace-000000',?1,0)",
                [&record.id],
            )
            .unwrap();
        }
        tx.execute("UPDATE scan_roots SET active=0 WHERE id=?", [&denied_id])
            .unwrap();
        tx.commit().unwrap();
        (journal, synchronous)
    };
    let seed_seconds = seed_started.elapsed().as_secs_f64();
    let queries = [
        "张",
        "张三",
        "合同",
        "合作合同",
        "招聘",
        "2026",
        "report",
        "项目 r",
        "100%",
        "_",
        "\"draft\"",
        "archive",
        "docx",
        "部门17",
        "example.com",
        "限额余位",
        "稀少唯一",
        "不存在的资料",
    ];
    let mut cases: Vec<_> = queries
        .into_iter()
        .map(|query| Case {
            query,
            kind: "all",
            workspace: false,
        })
        .collect();
    cases.extend([
        Case {
            query: "合同",
            kind: "all",
            workspace: true,
        },
        Case {
            query: "限额余位",
            kind: "all",
            workspace: true,
        },
        Case {
            query: "张",
            kind: "file",
            workspace: false,
        },
        Case {
            query: "report",
            kind: "web",
            workspace: false,
        },
    ]);
    let mut timings = Vec::new();
    let mut query_reports = Vec::new();
    let mut failures = Vec::new();
    let mut oracle_checks = 0;
    for case in &cases {
        let (expected, expected_more, expected_counts) = oracle(&records, case);
        let mut per_query = Vec::new();
        for repeat in 0..11 {
            let started = Instant::now();
            let response = store
                .search(SearchRequest {
                    request_id: repeat,
                    query: case.query.into(),
                    workspace_id: case.workspace.then(|| "workspace-000000".into()),
                    kind: case.kind.into(),
                })
                .unwrap();
            let elapsed = started.elapsed().as_secs_f64() * 1000.;
            let actual: Vec<_> = response
                .page
                .items
                .iter()
                .map(|item| item.id.clone())
                .collect();
            if actual != expected || response.page.has_more != expected_more {
                failures.push(serde_json::json!({"query":case.query,"kind":case.kind,"workspace":case.workspace,"repeat":repeat,"expected":expected,"actual":actual,"expected_has_more":expected_more,"actual_has_more":response.page.has_more}));
            }
            oracle_checks += 1;
            if repeat > 0 {
                per_query.push(elapsed);
            }
        }
        per_query.sort_by(f64::total_cmp);
        query_reports.push(serde_json::json!({"query":case.query,"kind":case.kind,"workspace_scoped":case.workspace,"samples":per_query.len(),"expected_counts":{"workspace":expected_counts[0],"file":expected_counts[1],"web":expected_counts[2]},"p50_ms":percentile(&per_query,0.5),"p95_ms":percentile(&per_query,0.95),"p99_ms":percentile(&per_query,0.99)}));
        timings.extend(per_query);
    }
    timings.sort_by(f64::total_cmp);
    let budget_failed = percentile(&timings, 0.95) > 60.
        || query_reports
            .iter()
            .any(|report| report["p95_ms"].as_f64().unwrap() > 60.);
    let report = serde_json::json!({
        "evidence":"synthetic_metadata_on_disk_store_search_not_scan_or_ui_latency",
        "profile":if cfg!(debug_assertions) {"debug"} else {"release"},
        "os":std::env::consts::OS,"arch":std::env::consts::ARCH,
        "resources":100_000,"distribution":{"file":98_900,"web":1_000,"workspace":100,"unauthorized_files":400},
        "sqlite_version":rusqlite::version(),"journal_mode":journal,"synchronous":synchronous,
        "seed_seconds":seed_seconds,"queries":cases.len(),"samples":timings.len(),"oracle_checks":oracle_checks,
        "p50_ms":percentile(&timings,0.5),"p95_ms":percentile(&timings,0.95),"p99_ms":percentile(&timings,0.99),
        "budget_ms":60,"budget_failed":budget_failed,"oracle_failures":failures,"per_query":query_reports,
    });
    let output = directory.path().parent().unwrap().join(format!(
        "rust-search-mixed-report-{}.json",
        uuid::Uuid::new_v4()
    ));
    fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("MIXED_SEARCH_ACCEPTANCE {report}");
    println!("MIXED_SEARCH_REPORT {}", output.display());
    assert!(
        failures.is_empty(),
        "mixed query oracle failed; preserve report for diagnosis"
    );
    assert!(
        !budget_failed,
        "100k mixed native hot query P95 exceeded 60ms; keep acceptance failed"
    );
}
