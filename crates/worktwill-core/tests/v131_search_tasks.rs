use rusqlite::params;
use std::{fs, time::Instant};
use worktwill_core::{model::*, Store};
#[test]
fn task_rank_evaluation_with_ten_thousand_distractors() {
    let root = tempfile::Builder::new()
        .prefix("v131-search-tasks-")
        .tempdir_in(std::env::var_os("WORKTWILL_TEST_ROOT").expect("approved root"))
        .unwrap();
    let documents = root.path().join("documents");
    fs::create_dir(&documents).unwrap();
    let path = root.path().join("app/catalog.sqlite3");
    let store = Store::open(&path).unwrap();
    let rid = store.grant_root(&documents).unwrap();
    let names = [
        ("客户甲", "合作合同_张三.txt"),
        ("客户乙", "合作合同_张三.txt"),
        ("招聘", "招聘计划2026.txt"),
        ("照片", "20261121_001.jpg"),
        ("资料", "季度总结.txt"),
    ];
    let mut ids = Vec::new();
    for (folder, name) in names {
        let folder = documents.join(folder);
        fs::create_dir(&folder).unwrap();
        let file = folder.join(name);
        fs::write(&file, b"synthetic search fixture").unwrap();
        let result = store
            .import_files(&[file.to_string_lossy().into_owned()], None, false)
            .unwrap();
        ids.push(result.items[0].resource_id.clone().unwrap());
    }
    let mut db = rusqlite::Connection::open(&path).unwrap();
    let tx = db.transaction().unwrap();
    for n in 0..10000 {
        let id = format!("distractor-{n:05}");
        let name = format!("20261121_{n:05}_资料.txt");
        let location = documents.join("干扰项").join(&name);
        tx.execute("INSERT INTO resources(id,kind,name,location,modified_at,size,status,favorite,created_at) VALUES(?1,'file',?2,?3,1,1,'available',0,1)",params![id,name,location.to_string_lossy()]).unwrap();
        tx.execute(
            "INSERT INTO file_roots(root_id,resource_id,generation) VALUES(?1,?2,0)",
            params![rid, id],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    let change = |mutation| {
        store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation,
            })
            .unwrap()
    };
    change(Mutation::RebuildSearch);
    let workspace = change(Mutation::CreateWorkspace {
        name: "合同项目工作区".into(),
        resource_ids: None,
    })
    .id
    .unwrap();
    let web = change(Mutation::SaveWeb {
        id: None,
        name: "合同管理制度".into(),
        url: "https://example.com/contracts".into(),
        workspace_id: None,
    })
    .id
    .unwrap();
    let cases = [
        ("精确名", "招聘计划2026.txt", vec![ids[2].clone()], 1),
        ("部分名称", "招聘计划", vec![ids[2].clone()], 1),
        ("短中文", "张", vec![ids[0].clone(), ids[1].clone()], 2),
        ("路径", "客户甲", vec![ids[0].clone()], 1),
        (
            "同名不同目录",
            "合作合同_张三.txt",
            vec![ids[0].clone(), ids[1].clone()],
            2,
        ),
        ("数字文件名", "20261121_001.jpg", vec![ids[3].clone()], 1),
        ("网页", "合同管理制度", vec![web], 1),
        ("工作区", "合同项目工作区", vec![workspace], 1),
    ];
    let mut report = Vec::new();
    for (label, query, targets, max_rank) in cases {
        let start = Instant::now();
        let result = store
            .search(SearchRequest {
                request_id: 1,
                query: query.into(),
                workspace_id: None,
                kind: "all".into(),
            })
            .unwrap();
        let elapsed = start.elapsed().as_secs_f64() * 1000.;
        let ranks: Vec<_> = targets
            .iter()
            .map(|id| {
                result
                    .page
                    .items
                    .iter()
                    .position(|r| &r.id == id)
                    .map(|n| n + 1)
            })
            .collect();
        assert!(
            ranks
                .iter()
                .all(|rank| rank.is_some_and(|rank| rank <= max_rank)),
            "{label}: {ranks:?}"
        );
        report.push(serde_json::json!({"task":label,"query":query,"target_ranks":ranks,"backend_ms":elapsed,"query_rewrites":0,"opened":false,"boundary":"Core search only; no user or OS open; physical target files plus 10000 synthetic indexed distractors"}));
    }
    let output = std::path::PathBuf::from(std::env::var_os("WORKTWILL_TEST_ROOT").unwrap())
        .join(format!("v131-search-quality-{}.json", std::process::id()));
    fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("SEARCH_TASK_REPORT {}", output.display());
}
