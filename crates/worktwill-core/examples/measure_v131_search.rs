use std::{path::PathBuf, time::Instant};
use worktwill_core::{model::*, Store};
fn main() {
    let root = PathBuf::from(std::env::var_os("WORKTWILL_TEST_ROOT").expect("approved root"))
        .canonicalize()
        .unwrap();
    let path = PathBuf::from(std::env::args().nth(1).expect("synthetic database path"))
        .canonicalize()
        .unwrap();
    assert!(path.starts_with(root));
    let store = Store::open(path).unwrap();
    for query in [
        "20261121_001.jpg",
        "招聘计划2026.txt",
        "季度总结",
        "合同项目工作区",
        "合同管理制度",
    ] {
        let start = Instant::now();
        let result = store
            .search(SearchRequest {
                request_id: 1,
                query: query.into(),
                workspace_id: None,
                kind: "all".into(),
            })
            .unwrap();
        println!(
            "{}",
            serde_json::json!({"query":query,"ms":start.elapsed().as_secs_f64()*1000.,"hits":result.page.items.len()})
        );
    }
}
