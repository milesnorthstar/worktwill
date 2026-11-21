//! 创建可保留的原生验收夹具；仅在显式指定的测试根内写入合成数据。
use rusqlite::params;
use std::{fs, path::PathBuf};
use worktwill_core::{model::*, Store};
fn main() {
    let approved =
        PathBuf::from(std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed test root"));
    let directory = tempfile::Builder::new()
        .prefix("v131-isolated-native-")
        .tempdir_in(approved)
        .unwrap()
        .keep();
    let documents = directory.join("documents");
    fs::create_dir(&documents).unwrap();
    let store = Store::open(directory.join("app/worktwill.sqlite3")).unwrap();
    let root = store.grant_root(&documents).unwrap();
    let targets = [
        ("客户甲", "合作合同_张三.txt"),
        ("客户乙", "合作合同_张三.txt"),
        ("招聘", "招聘计划2026.txt"),
        ("照片", "20261121_001.jpg"),
        ("资料", "季度总结.txt"),
    ];
    let mut files = Vec::new();
    for (folder, name) in targets {
        let folder = documents.join(folder);
        fs::create_dir(&folder).unwrap();
        let path = folder.join(name);
        fs::write(&path, format!("V131_NATIVE_SYNTHETIC\n{name}\n")).unwrap();
        files.push(path);
    }
    let pages = documents.join("分页");
    fs::create_dir(&pages).unwrap();
    for n in 0..105 {
        let path = pages.join(format!("分页验收_{n:03}.txt"));
        fs::write(&path, format!("V131_PAGE_{n:03}\n")).unwrap();
        files.push(path);
    }
    let result = store
        .import_files(
            &files
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            None,
            false,
        )
        .unwrap();
    let change = |mutation| {
        store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation,
            })
            .unwrap()
    };
    let a = change(Mutation::CreateWorkspace {
        name: "合同项目工作区".into(),
        resource_ids: None,
    })
    .id
    .unwrap();
    let b = change(Mutation::CreateWorkspace {
        name: "资料归档工作区".into(),
        resource_ids: None,
    })
    .id
    .unwrap();
    let contract = result.items[0].resource_id.clone().unwrap();
    for wid in [&a, &b] {
        change(Mutation::Link {
            workspace_id: wid.clone(),
            resource_ids: vec![contract.clone()],
        });
    }
    change(Mutation::Favorite {
        id: contract.clone(),
        value: true,
    });
    change(Mutation::SaveWeb {
        id: None,
        name: "合同管理制度".into(),
        url: "https://example.com/contracts".into(),
        workspace_id: Some(a),
    });
    let mut db = rusqlite::Connection::open(directory.join("app/worktwill.sqlite3")).unwrap();
    let tx = db.transaction().unwrap();
    for n in 0..99890 {
        let id = format!("indexed-distractor-{n:06}");
        let name = format!("20261121_{n:06}_资料.txt");
        tx.execute("INSERT INTO resources(id,kind,name,location,modified_at,size,status,favorite,created_at) VALUES(?1,'file',?2,?3,1,1,'missing',0,1)",params![id,name,documents.join("仅索引的合成干扰项").join(&name).to_string_lossy()]).unwrap();
        tx.execute(
            "INSERT INTO file_roots(root_id,resource_id,generation) VALUES(?1,?2,0)",
            params![root, id],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    change(Mutation::RebuildSearch);
    println!(
        "{}",
        serde_json::json!({"data_directory":directory.join("app"),"fixture_root":directory,"physical_files":110,"indexed_files":100000,"synthetic_metadata_distractors":99890,"contract_id":contract,"scope":"isolated synthetic fixture; no user documents or permissions copied"})
    );
}
