//! 仅在确认的临时根内生成原生搜索验收库；元数据合成，不声称实扫文件。
use rusqlite::{params, Connection};
use std::{fs, path::PathBuf};
use worktwill_core::{model::*, Store};

fn main() {
    let root = PathBuf::from(std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed root"))
        .canonicalize()
        .unwrap();
    let directory = root.join(format!("native-search-100k-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    let documents = directory.join("documents");
    fs::create_dir(&documents).unwrap();
    let database = directory.join("worktwill.sqlite3");
    let store = Store::open(&database).unwrap();
    let root_id = store.grant_root(&documents).unwrap();
    let mut db = Connection::open(&database).unwrap();
    let tx = db.transaction().unwrap();
    let patterns = [
        "合作合同_张三",
        "招聘计划_李四",
        "年度总结_2026",
        "项目 Report",
        "notes_100%_done",
        "预算_财务",
        "archive_alpha",
        "报价 draft",
    ];
    for index in 0..100_000 {
        let id = format!("synthetic-{index:06}");
        let name = format!("{}_{index:06}.docx", patterns[index % patterns.len()]);
        let location = documents
            .join(format!("部门{}/{}", index % 37, name))
            .to_string_lossy()
            .into_owned();
        tx.execute(
            "INSERT INTO resources(id,kind,name,location,created_at) VALUES(?1,'file',?2,?3,0)",
            params![id, name, location],
        )
        .unwrap();
        tx.execute(
            "INSERT INTO file_roots VALUES(?1,?2,0)",
            params![root_id, id],
        )
        .unwrap();
    }
    // 暂停只阻止实扫，不撤回查询授权，避免空夹具目录覆盖合成状态。
    tx.execute("UPDATE scan_roots SET paused=1 WHERE id=?", [&root_id])
        .unwrap();
    tx.commit().unwrap();
    store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation: Mutation::RebuildSearch,
        })
        .unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM search_documents", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        100_000
    );
    println!(
        "{}",
        serde_json::json!({"directory":directory,"resources":100_000,"synthetic_metadata":true,"real_files":false})
    );
}
