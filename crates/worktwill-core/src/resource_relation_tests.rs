//! 反向覆盖索引是派生结构；旧v3补建不能改变关系、正文或业务版本。
use crate::{model::*, Store};

#[test]
fn existing_v3_reverse_membership_index_preserves_shared_resource_and_versions() {
    let approved = std::env::var_os("WORKTWILL_TEST_ROOT").unwrap();
    let directory = tempfile::Builder::new()
        .prefix("relation-index-")
        .tempdir_in(approved)
        .unwrap();
    let path = directory.path().join("app.sqlite3");
    let store = Store::open(&path).unwrap();
    let mutate = |mutation| {
        store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation,
            })
            .unwrap()
            .id
            .unwrap()
    };
    let resource = mutate(Mutation::SaveWeb {
        id: None,
        name: "共享网页".into(),
        url: "https://example.com/shared".into(),
        workspace_id: None,
    });
    for index in 0..20 {
        let workspace = mutate(Mutation::CreateWorkspace {
            name: format!("工作区{index}"),
            resource_ids: Some(vec![resource.clone()]),
        });
        assert!(!workspace.is_empty());
    }
    let before = store.overview().unwrap();
    let db = store.connection().unwrap();
    let original = Store::resource_db(&db, &resource, false).unwrap();
    assert_eq!(original.workspace_ids.len(), 20);
    db.execute("DROP INDEX workspace_resource_memberships", [])
        .unwrap();
    let plan = |db: &rusqlite::Connection| {
        db.prepare(
            "EXPLAIN QUERY PLAN SELECT workspace_id FROM workspace_resources WHERE resource_id=?",
        )
        .unwrap()
        .query_map([&resource], |row| row.get::<_, String>(3))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
    };
    assert!(plan(&db)
        .iter()
        .any(|line| line.contains("SCAN workspace_resources")));
    drop(db);
    let reopened = Store::open(&path).unwrap();
    let db = reopened.connection().unwrap();
    let after_plan = plan(&db);
    assert!(after_plan.iter().any(|line|line.contains("SEARCH workspace_resources USING COVERING INDEX workspace_resource_memberships (resource_id=?)")),"{after_plan:?}");
    let rebuilt = Store::resource_db(&db, &resource, false).unwrap();
    assert_eq!(
        serde_json::to_value(original).unwrap(),
        serde_json::to_value(rebuilt).unwrap()
    );
    let after = reopened.overview().unwrap();
    assert_eq!(
        (before.permission_version, before.revision),
        (after.permission_version, after.revision)
    );
    let integrity: String = db
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
}
