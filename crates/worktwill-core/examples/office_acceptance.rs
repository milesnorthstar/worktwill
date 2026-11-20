//! 在已确认的测试目录中观察真实 Office 保存；不打开或写入办公源文件。
use std::{fs, path::PathBuf};
use worktwill_core::{model::*, Store};

fn change(store: &Store, mutation: Mutation) -> MutationResponse {
    store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation,
        })
        .expect("fixture mutation")
}
fn main() {
    let arguments: Vec<_> = std::env::args().collect();
    let action = arguments.get(1).expect("init|check");
    let approved =
        PathBuf::from(std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root"))
            .canonicalize()
            .unwrap();
    let directory = PathBuf::from(arguments.get(2).expect("existing fixture directory"))
        .canonicalize()
        .unwrap();
    assert!(
        directory != approved && directory.starts_with(&approved),
        "fixture must be inside confirmed root"
    );
    let store = Store::open(directory.join("app/data.sqlite3")).unwrap();
    let state_file = directory.join("app/office-state.json");
    if action == "init" {
        assert!(
            !state_file.exists(),
            "do not overwrite an acceptance session"
        );
        store.grant_root(&directory.join("documents")).unwrap();
        store.scan_all().unwrap();
        let file = store
            .resources(ResourceQuery {
                workspace_id: None,
                kind: "file".into(),
                mode: "modified".into(),
                offset: 0,
            })
            .unwrap()
            .items
            .into_iter()
            .find(|r| r.name.ends_with(".docx"))
            .expect("fixture docx");
        let mut workspaces = Vec::new();
        for name in ["Office 保存验收 A", "Office 保存验收 B"] {
            let wid = change(
                &store,
                Mutation::CreateWorkspace {
                    name: name.into(),
                    resource_ids: None,
                },
            )
            .id
            .unwrap();
            change(
                &store,
                Mutation::Link {
                    workspace_id: wid.clone(),
                    resource_ids: vec![file.id.clone()],
                },
            );
            workspaces.push(wid);
        }
        change(
            &store,
            Mutation::Favorite {
                id: file.id.clone(),
                value: true,
            },
        );
        let state =
            serde_json::json!({"id":file.id,"location":file.location,"workspaces":workspaces});
        fs::write(&state_file, serde_json::to_vec_pretty(&state).unwrap()).unwrap();
        println!("{}", state);
    } else {
        assert_eq!(action, "check");
        let state: serde_json::Value =
            serde_json::from_slice(&fs::read(state_file).unwrap()).unwrap();
        store.scan_all().unwrap();
        let rid = state["id"].as_str().unwrap();
        let mut relations = Vec::new();
        for wid in state["workspaces"].as_array().unwrap() {
            relations.push(
                store
                    .resources(ResourceQuery {
                        workspace_id: Some(wid.as_str().unwrap().into()),
                        kind: "all".into(),
                        mode: "modified".into(),
                        offset: 0,
                    })
                    .unwrap()
                    .items,
            );
        }
        let resolution = store.resolve_resource(rid);
        let stable = resolution.is_ok()
            && relations.iter().all(|items| {
                items.len() == 1
                    && items[0].id == rid
                    && items[0].favorite
                    && items[0].status == "available"
            });
        println!(
            "{}",
            serde_json::json!({"stable_without_manual_repair":stable,"resolution":resolution,"workspace_resources":relations,"catalog":store.resources(ResourceQuery {workspace_id:None,kind:"all".into(),mode:"modified".into(),offset:0}).unwrap()})
        );
        if !stable {
            std::process::exit(1);
        }
    }
}
