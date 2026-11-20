//! 根目录身份失效时，旧枚举与迟到收尾不能提交或覆盖新扫描事实。

use super::*;
use std::fs;

struct Fixture {
    _directory: tempfile::TempDir,
    home: PathBuf,
    root: PathBuf,
    store: Store,
}

impl Fixture {
    fn new() -> Self {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT")
            .expect("confirmed disposable fixture root is required");
        let directory = tempfile::Builder::new()
            .prefix("catalog-root-boundary-")
            .tempdir_in(approved)
            .unwrap();
        let home = directory.path().canonicalize().unwrap();
        let root = home.join("documents");
        fs::create_dir(&root).unwrap();
        let store = Store::open(home.join("app/catalog.sqlite3")).unwrap();
        Self {
            _directory: directory,
            home,
            root,
            store,
        }
    }

    fn start(&self, root: &str) -> ScanGeneration {
        let db = self.store.connection().unwrap();
        db.execute(
            "UPDATE scan_roots SET generation=generation+1,scan_state='running' WHERE id=?",
            [root],
        )
        .unwrap();
        ScanGeneration {
            sequence: db
                .query_row(
                    "SELECT generation FROM scan_roots WHERE id=?",
                    [root],
                    |r| r.get(0),
                )
                .unwrap(),
            restore_epoch: self.store.scan_epoch.load(Ordering::SeqCst),
        }
    }

    fn root_state(&self, root: &str) -> Root {
        self.store
            .roots()
            .unwrap()
            .into_iter()
            .find(|r| r.id == root)
            .unwrap()
    }

    fn versions(&self) -> (u32, u32) {
        let db = self.store.connection().unwrap();
        (
            crate::store::version(&db, "permission_version").unwrap(),
            crate::store::version(&db, "revision").unwrap(),
        )
    }

    fn counts(&self) -> (u32, u32, u32) {
        self.store
            .connection()
            .unwrap()
            .query_row(
                "SELECT (SELECT count(*) FROM resources),(SELECT count(*) FROM search_documents),(SELECT count(*) FROM activities)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap()
    }
}

fn discovered(path: &Path) -> Discovered {
    let path = path.canonicalize().unwrap();
    let metadata = fs::symlink_metadata(&path).unwrap();
    Discovered {
        name: path.file_name().unwrap().to_str().unwrap().into(),
        identity: platform::identity(&path, &metadata).unwrap(),
        size: metadata.len() as f64,
        modified: mtime(&metadata),
        path,
    }
}

fn page_request(cursor: Option<SearchCursor>) -> SearchPageRequest {
    SearchPageRequest {
        request: SearchRequest {
            request_id: 1,
            query: "stable-page-".into(),
            workspace_id: None,
            kind: "file".into(),
        },
        offset: cursor.as_ref().map_or(0, |c| c.next_offset),
        cursor,
        sort: "name".into(),
        modified_from: None,
        modified_to: None,
        path_prefix: None,
    }
}

#[test]
fn unchanged_scan_preserves_cursor_but_real_changes_invalidate_it() {
    let f = Fixture::new();
    for i in 0..103 {
        fs::write(f.root.join(format!("stable-page-{i:03}.txt")), b"fixture").unwrap();
    }
    let root = f.store.grant_root(&f.root).unwrap();
    f.store.scan_root(&root).unwrap();
    let first = f.store.search_page(page_request(None)).unwrap();
    let versions = f.versions();
    f.store.scan_root(&root).unwrap();
    assert_eq!(f.versions(), versions, "scan progress is not a data change");
    let second = f
        .store
        .search_page(page_request(first.next_cursor.clone()))
        .unwrap();
    assert_eq!(first.page.items.len(), 100);
    assert_eq!(second.page.items.len(), 3);
    assert_eq!(second.page.items[2].name, "stable-page-102.txt");
    assert!(!second.page.has_more);
    fs::write(f.root.join("stable-page-102.txt"), b"changed fixture size").unwrap();
    f.store.scan_root(&root).unwrap();
    assert_eq!(
        f.store
            .search_page(page_request(first.next_cursor))
            .unwrap_err()
            .code,
        "stale_cursor"
    );
    let first = f.store.search_page(page_request(None)).unwrap();
    fs::remove_file(f.root.join("stable-page-101.txt")).unwrap();
    f.store.scan_root(&root).unwrap();
    assert_eq!(
        f.store
            .search_page(page_request(first.next_cursor))
            .unwrap_err()
            .code,
        "stale_cursor"
    );
}

#[test]
fn same_root_hardlink_keeps_available_path_but_missing_original_can_move() {
    let f = Fixture::new();
    let original = f.root.join("original.txt");
    let alias = f.root.join("alias.txt");
    fs::write(&original, b"same physical file").unwrap();
    let root = f.store.grant_root(&f.root).unwrap();
    f.store.scan_root(&root).unwrap();
    let rid: String = f
        .store
        .connection()
        .unwrap()
        .query_row("SELECT id FROM resources", [], |r| r.get(0))
        .unwrap();
    fs::hard_link(&original, &alias).unwrap();
    let versions = f.versions();
    let generation = f.start(&root);
    f.store
        .commit_batch(&root, generation, &mut vec![discovered(&alias)], 1, &mut 0)
        .unwrap();
    let resource = Store::resource_db(&f.store.connection().unwrap(), &rid, false).unwrap();
    assert_eq!(Path::new(&resource.location), original);
    assert_eq!(resource.name, "original.txt");
    assert_eq!(f.versions(), versions);
    fs::remove_file(&original).unwrap();
    f.store.scan_root(&root).unwrap();
    let resource = Store::resource_db(&f.store.connection().unwrap(), &rid, false).unwrap();
    assert_eq!(Path::new(&resource.location), alias);
    assert_eq!(resource.name, "alias.txt");
    assert_eq!(f.counts().0, 1);
    assert!(f.versions().1 > versions.1);
}

#[test]
fn excluded_retained_path_cannot_hide_alias_replacement_or_keep_excluded_location() {
    let f = Fixture::new();
    let excluded = f.root.join("excluded");
    fs::create_dir(&excluded).unwrap();
    let original = excluded.join("original.txt");
    let alias = f.root.join("alias.txt");
    fs::write(&original, b"retained identity").unwrap();
    fs::write(&alias, b"different original identity").unwrap();
    let root = f.store.grant_root(&f.root).unwrap();
    f.store.scan_root(&root).unwrap();
    let resource_at = |path: &Path| -> String {
        f.store
            .connection()
            .unwrap()
            .query_row(
                "SELECT id FROM resources WHERE location=?",
                [path.to_string_lossy()],
                |r| r.get(0),
            )
            .unwrap()
    };
    let original_id = resource_at(&original);
    let replaced_id = resource_at(&alias);
    fs::remove_file(&alias).unwrap();
    fs::hard_link(&original, &alias).unwrap();
    f.store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation: Mutation::Exclude {
                root_id: root.clone(),
                relative_path: "excluded".into(),
            },
        })
        .unwrap();
    f.store.scan_root(&root).unwrap();
    let db = f.store.connection().unwrap();
    let current = Store::resource_db(&db, &original_id, false).unwrap();
    assert_eq!(Path::new(&current.location), alias);
    assert_eq!(current.status, "available");
    assert_eq!(
        Store::resource_db(&db, &replaced_id, false).unwrap().status,
        "identity_unconfirmed"
    );
}

#[test]
fn moved_root_rejects_captured_batch_and_finishes_without_false_completion() {
    let f = Fixture::new();
    let root = f.store.grant_root(&f.root).unwrap();
    let path = f.root.join("pending.txt");
    fs::write(&path, b"captured before directory move").unwrap();
    let generation = f.start(&root);
    let mut batch = vec![discovered(&path)];
    let counts = f.counts();
    let versions = f.versions();

    fs::rename(&f.root, f.home.join("moved-documents")).unwrap();
    assert!(!valid(&f.store, &f.store.connection().unwrap(), &root, generation).unwrap());
    assert!(!f
        .store
        .commit_batch(&root, generation, &mut batch, 1, &mut 0)
        .unwrap());
    assert_eq!(
        f.counts(),
        counts,
        "rejected batches must not write resources or events"
    );
    assert_eq!(
        f.versions(),
        versions,
        "batch rejection itself must not commit"
    );

    f.store.finish_invalid_scan(&root, generation).unwrap();
    let current = f.root_state(&root);
    assert_eq!(current.scan_state, "partial_failure");
    assert_eq!(current.location_status, "unavailable");
    assert_eq!(current.scanned, 0);
    let after = f.versions();
    assert!(after.0 > versions.0 && after.1 > versions.1);
    assert_eq!(f.counts(), counts);
}

#[test]
fn replacement_root_with_the_same_file_object_still_rejects_old_batch() {
    let f = Fixture::new();
    let root = f.store.grant_root(&f.root).unwrap();
    let path = f.root.join("pending.txt");
    fs::write(&path, b"same file object under a different directory").unwrap();
    let generation = f.start(&root);
    let item = discovered(&path);
    let original_identity = item.identity.clone();
    let counts = f.counts();

    let retained = f.home.join("retained-original-root");
    fs::rename(&f.root, &retained).unwrap();
    fs::create_dir(&f.root).unwrap();
    // 文件身份故意保持相同，单独验证目录授权锚点，而非碰巧被文件复验挡住。
    fs::hard_link(retained.join("pending.txt"), &path).unwrap();
    assert!(platform::identity(&path, &path.metadata().unwrap()).unwrap() == original_identity);
    assert!(!f
        .store
        .commit_batch(&root, generation, &mut vec![item], 1, &mut 0)
        .unwrap());
    assert_eq!(f.counts(), counts);

    f.store.finish_invalid_scan(&root, generation).unwrap();
    let current = f.root_state(&root);
    assert_eq!(current.scan_state, "partial_failure");
    assert_eq!(current.location_status, "unavailable");
    assert!(f.store.needs_file_grant(&path).unwrap());
    assert_eq!(
        fs::read(&path).unwrap(),
        b"same file object under a different directory"
    );
}

#[test]
fn late_invalid_cleanup_cannot_overwrite_new_root_scan_completion() {
    let f = Fixture::new();
    let root = f.store.grant_root(&f.root).unwrap();
    let path = f.root.join("pending.txt");
    fs::write(&path, b"accepted only by the new scan").unwrap();
    let old_generation = f.start(&root);
    let old_item = discovered(&path);

    let moved = f.home.join("new-location");
    fs::rename(&f.root, &moved).unwrap();
    // 明确重选同一物理目录，启动并完成新扫描后再让旧回包到达。
    assert_eq!(f.store.grant_root(&moved).unwrap(), root);
    f.store.scan_root(&root).unwrap();
    let counts = f.counts();
    let versions = f.versions();
    assert_eq!(counts.0, 1);
    assert!(!f
        .store
        .commit_batch(&root, old_generation, &mut vec![old_item], 1, &mut 0)
        .unwrap());
    f.store.finish_invalid_scan(&root, old_generation).unwrap();

    let current = f.root_state(&root);
    assert_eq!(current.scan_state, "completed");
    assert_eq!(current.location_status, "available");
    assert_eq!(current.scanned, 1);
    assert_eq!(f.counts(), counts);
    assert_eq!(f.versions(), versions);
}

#[test]
fn restored_root_generation_aba_does_not_revive_old_batch_or_cleanup() {
    let f = Fixture::new();
    let root = f.store.grant_root(&f.root).unwrap();
    f.store
        .connection()
        .unwrap()
        .execute(
            "UPDATE scan_roots SET generation=8,scan_state='idle' WHERE id=?",
            [&root],
        )
        .unwrap();
    let backup = f.home.join("before-old-scan.sqlite3");
    f.store.backup_to(&backup).unwrap();
    f.store
        .connection()
        .unwrap()
        .execute("UPDATE scan_roots SET generation=9 WHERE id=?", [&root])
        .unwrap();
    let generation = f.start(&root);
    assert_eq!(generation.sequence, 10);
    let path = f.root.join("old-pending.txt");
    fs::write(&path, b"old scan must never import this").unwrap();
    let item = discovered(&path);

    f.store.restore_from(&backup).unwrap();
    assert_eq!(f.store.grant_root(&f.root).unwrap(), root);
    let db = f.store.connection().unwrap();
    let current_generation: i64 = db
        .query_row(
            "SELECT generation FROM scan_roots WHERE id=?",
            [&root],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(current_generation, generation.sequence);
    assert!(Store::root_current(&db, &root).unwrap());
    assert_ne!(
        f.store.scan_epoch.load(Ordering::SeqCst),
        generation.restore_epoch
    );
    // 模拟恢复后的新扫描已进入running；旧epoch的收尾不能将它改成失败。
    db.execute(
        "UPDATE scan_roots SET scan_state='running' WHERE id=?",
        [&root],
    )
    .unwrap();
    drop(db);
    let versions = f.versions();
    let counts = f.counts();
    assert!(!valid(&f.store, &f.store.connection().unwrap(), &root, generation).unwrap());
    assert!(!f
        .store
        .commit_batch(&root, generation, &mut vec![item], 1, &mut 0)
        .unwrap());
    f.store.finish_invalid_scan(&root, generation).unwrap();
    assert_eq!(f.root_state(&root).scan_state, "running");
    assert_eq!(f.counts(), counts);
    assert_eq!(f.versions(), versions);
}

#[test]
fn invalid_overlapping_root_cannot_protect_a_missing_resource() {
    let f = Fixture::new();
    let child = f.root.join("nested");
    fs::create_dir(&child).unwrap();
    let path = child.join("contract.txt");
    fs::write(&path, b"overlapping root fixture").unwrap();
    let parent_root = f.store.grant_root(&f.root).unwrap();
    let child_root = f.store.grant_root(&child).unwrap();
    f.store.scan_root(&parent_root).unwrap();
    f.store.scan_root(&child_root).unwrap();
    let rid: String = f
        .store
        .connection()
        .unwrap()
        .query_row(
            "SELECT id FROM resources WHERE location=?",
            [path.to_string_lossy().as_ref()],
            |r| r.get(0),
        )
        .unwrap();

    let moved_child = f.root.join("moved-nested");
    fs::rename(&child, &moved_child).unwrap();
    fs::create_dir(&child).unwrap();
    fs::remove_file(moved_child.join("contract.txt")).unwrap();
    f.store.verify_root_locations().unwrap();
    assert_eq!(f.root_state(&child_root).location_status, "unavailable");
    let db = f.store.connection().unwrap();
    // 保留同代归属，确保测试验证invalid过滤，而不是偶然靠generation不同通过。
    db.execute("UPDATE file_roots SET generation=(SELECT generation FROM scan_roots WHERE id=?1) WHERE root_id=?1 AND resource_id=?2", params![child_root,rid]).unwrap();
    drop(db);

    f.store.scan_root(&parent_root).unwrap();
    let status: String = f
        .store
        .connection()
        .unwrap()
        .query_row("SELECT status FROM resources WHERE id=?", [&rid], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "missing");
    assert_eq!(f.root_state(&parent_root).scan_state, "completed");
    assert_eq!(f.root_state(&child_root).location_status, "unavailable");
}

#[test]
fn excluded_directory_does_not_override_independent_file_grant_status() {
    let f = Fixture::new();
    let private = f.root.join("private");
    fs::create_dir(&private).unwrap();
    let path = private.join("contract.txt");
    fs::write(&path, b"independently authorized original").unwrap();

    // 先独立授权，再让目录扫描建立重叠归属，确保排除后仅file grant仍覆盖。
    let imported = f
        .store
        .import_files(&[path.to_string_lossy().into_owned()], None, true)
        .unwrap();
    assert_eq!(imported.items[0].status, "added");
    let rid = imported.items[0].resource_id.clone().unwrap();
    let root = f.store.grant_root(&f.root).unwrap();
    f.store.scan_all().unwrap();
    f.store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation: Mutation::Exclude {
                root_id: root.clone(),
                relative_path: "private".into(),
            },
        })
        .unwrap();
    f.store.scan_all().unwrap();

    let status = || {
        f.store
            .connection()
            .unwrap()
            .query_row("SELECT status FROM resources WHERE id=?", [&rid], |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
    };
    assert_eq!(status(), "available");
    let (location, web) = f.store.resolve_resource(&rid).unwrap();
    assert!(!web);
    assert!(Path::new(&location).canonicalize().unwrap() == path.canonicalize().unwrap());
    f.store.check_open_type(&location).unwrap();
    assert_eq!(
        fs::read(&location).unwrap(),
        b"independently authorized original"
    );

    fs::remove_file(&path).unwrap();
    // 原生定位失败允许显式返回；状态仍须由单文件复验落为missing并保留授权。
    if let Err(error) = f.store.scan_all() {
        assert!(crate::file_locator::is_locator_error(&error));
    }
    assert_eq!(status(), "missing");
    assert!(f.store.resolve_resource(&rid).is_err());
    let granted: bool = f
        .store
        .connection()
        .unwrap()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM file_grants WHERE resource_id=? AND active=1)",
            [&rid],
            |r| r.get(0),
        )
        .unwrap();
    assert!(granted);
    assert_eq!(f.counts().0, 1);
    assert_eq!(f.root_state(&root).scan_state, "completed");
}
