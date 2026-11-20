#![cfg(target_os = "macos")]

use std::{fs, path::PathBuf, time::SystemTime};
use worktwill_core::{model::*, timeline::TimelineRequest, Store};

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    database: PathBuf,
    store: Store,
}

impl Fixture {
    fn new() -> Self {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("u8-recovery-review-")
            .tempdir_in(approved)
            .unwrap();
        let root = directory.path().join("documents");
        fs::create_dir(&root).unwrap();
        let database = directory.path().join("app/catalog.sqlite3");
        let store = Store::open(&database).unwrap();
        Self {
            _directory: directory,
            root,
            database,
            store,
        }
    }

    fn mutate(&self, mutation: Mutation) -> MutationResponse {
        self.store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation,
            })
            .unwrap()
    }

    fn resources(&self, workspace: Option<&str>) -> Vec<Resource> {
        self.store
            .resources(ResourceQuery {
                workspace_id: workspace.map(str::to_owned),
                kind: "file".into(),
                mode: "modified".into(),
                offset: 0,
            })
            .unwrap()
            .items
    }

    fn seed(&self) -> (String, String) {
        fs::write(
            self.root.join("original-contract.txt"),
            b"synthetic original",
        )
        .unwrap();
        self.store.grant_root(&self.root).unwrap();
        self.store.scan_all().unwrap();
        let rid = self.resources(None)[0].id.clone();
        let workspace = self
            .mutate(Mutation::CreateWorkspace {
                name: "恢复验收工作区".into(),
                resource_ids: Some(vec![rid.clone()]),
            })
            .id
            .unwrap();
        self.mutate(Mutation::Favorite {
            id: rid.clone(),
            value: true,
        });
        (rid, workspace)
    }

    fn opened_timeline(&self, workspace: &str) -> worktwill_core::timeline::TimelinePage {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            * 1000.;
        let day = (now / 86_400_000.).floor() * 86_400_000.;
        self.store
            .timeline(TimelineRequest {
                from: None,
                to: None,
                month_from: day,
                month_to: day + 86_400_000.,
                day_boundaries: vec![day, day + 86_400_000.],
                filter: "opened".into(),
                basis: "observed".into(),
                workspace_id: Some(workspace.into()),
                offset: 0,
                cursor: None,
            })
            .unwrap()
    }
}

#[test]
fn same_path_replacement_retains_identity_confirmation_reason_in_workspace() {
    let f = Fixture::new();
    let (rid, workspace) = f.seed();
    let original = f.root.join("original-contract.txt");
    let replacement = f.root.join("fresh-physical-object.tmp");
    // 新对象先存在再覆盖，避免删除重建造成inode复用而失去测试前提。
    fs::write(
        &replacement,
        b"synthetic replacement with different identity",
    )
    .unwrap();
    fs::rename(&replacement, &original).unwrap();
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap_err().code,
        "identity_unconfirmed"
    );
    f.store.scan_all().unwrap();
    let retained = f.resources(Some(&workspace));
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].id, rid);
    assert!(retained[0].favorite);
    assert_eq!(f.resources(None).len(), 2);
    // 原路径仍有另一对象，应提示确认身份；不能被扫描收尾覆盖成一般的失联。
    assert_eq!(retained[0].status, "identity_unconfirmed");
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let discoveries: u32 = db
        .query_row(
            "SELECT count(*) FROM activities WHERE kind='file_discovered'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(discoveries, 2);
    let mut ids: Vec<_> = f
        .resources(None)
        .into_iter()
        .map(|resource| resource.id)
        .collect();
    ids.sort();
    for _ in 0..2 {
        f.store.scan_all().unwrap();
        assert_eq!(
            f.resources(Some(&workspace))[0].status,
            "identity_unconfirmed"
        );
        let mut current: Vec<_> = f
            .resources(None)
            .into_iter()
            .map(|resource| resource.id)
            .collect();
        current.sort();
        assert_eq!(current, ids);
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM activities WHERE kind='file_discovered'",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
            discoveries
        );
    }
    fs::remove_file(&original).unwrap();
    f.store.scan_all().unwrap();
    let missing = f.resources(Some(&workspace));
    assert_eq!(missing[0].id, rid);
    assert_eq!(missing[0].status, "missing");
    assert!(missing[0].favorite);
    assert_eq!(f.resources(None).len(), 2);
}

#[test]
fn manual_recovery_does_not_describe_past_open_as_opening_the_new_object() {
    let f = Fixture::new();
    let (rid, workspace) = f.seed();
    f.store.resolve_resource(&rid).unwrap();
    f.store.record_open(&rid).unwrap();
    let previous = f
        .store
        .overview()
        .unwrap()
        .activities
        .into_iter()
        .find(|event| event.kind == "opened" && event.resource_id.as_deref() == Some(&rid))
        .unwrap();
    assert_eq!(previous.name, "original-contract.txt");
    f.store.record_open(&rid).unwrap();
    let legacy = f
        .store
        .overview()
        .unwrap()
        .activities
        .into_iter()
        .find(|event| event.kind == "opened" && event.id != previous.id)
        .unwrap();
    // 仅夹具模拟旧版本未保存观察名的历史，不能为它追造不存在的快照。
    rusqlite::Connection::open(&f.database)
        .unwrap()
        .execute("UPDATE activities SET name='' WHERE id=?", [&legacy.id])
        .unwrap();
    let replacement = f.root.join("selected-new-contract.txt");
    fs::write(&replacement, b"explicitly selected different object").unwrap();
    fs::remove_file(f.root.join("original-contract.txt")).unwrap();
    f.store.scan_all().unwrap();
    assert_eq!(
        f.store.relink(&rid, &replacement).unwrap_err().code,
        "merge_confirmation_required"
    );
    f.store.relink_confirmed(&rid, &replacement, true).unwrap();
    let current = f.resources(Some(&workspace));
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].id, rid);
    assert_eq!(current[0].name, "selected-new-contract.txt");
    assert!(current[0].favorite);
    let timeline = f.opened_timeline(&workspace);
    let historical = timeline
        .events
        .iter()
        .find(|entry| entry.activity.id == previous.id)
        .unwrap();
    assert_eq!(historical.activity.kind, "opened");
    assert_eq!(historical.activity.at, previous.at);
    assert_eq!(
        historical.resource.as_ref().unwrap().name,
        "selected-new-contract.txt"
    );
    // 可以解析到当前资源，但历史事实必须保留当时对象，不能把新对象伪装成曾打开过。
    assert_eq!(historical.activity.name, previous.name);
    let overview = f.store.overview().unwrap();
    assert_eq!(
        overview
            .activities
            .iter()
            .find(|event| event.id == previous.id)
            .unwrap()
            .name,
        previous.name
    );
    assert_eq!(
        timeline
            .events
            .iter()
            .find(|entry| entry.activity.id == legacy.id)
            .unwrap()
            .activity
            .name,
        "selected-new-contract.txt"
    );
    assert_eq!(
        overview
            .activities
            .iter()
            .find(|event| event.id == legacy.id)
            .unwrap()
            .name,
        "selected-new-contract.txt"
    );
    f.store.resolve_resource(&rid).unwrap();
    f.store.record_open(&rid).unwrap();
    let fresh = f
        .store
        .overview()
        .unwrap()
        .activities
        .into_iter()
        .find(|event| event.kind == "opened" && event.id != previous.id && event.id != legacy.id)
        .unwrap();
    assert_eq!(fresh.name, "selected-new-contract.txt");
    let timeline = f.opened_timeline(&workspace);
    assert_eq!(
        timeline
            .events
            .iter()
            .find(|entry| entry.activity.id == fresh.id)
            .unwrap()
            .activity
            .name,
        fresh.name
    );
    assert_eq!(
        timeline
            .events
            .iter()
            .find(|entry| entry.activity.id == previous.id)
            .unwrap()
            .activity
            .name,
        previous.name
    );
    let root_id = f.store.overview().unwrap().roots[0].id.clone();
    f.mutate(Mutation::RevokeRoot { id: root_id });
    let hidden = f.store.overview().unwrap();
    assert!(hidden
        .activities
        .iter()
        .all(|event| event.resource_id.as_deref() != Some(rid.as_str())));
    assert!(hidden
        .activities
        .iter()
        .all(|event| event.name != previous.name && event.name != fresh.name));
    assert!(f.opened_timeline(&workspace).events.is_empty());
}
