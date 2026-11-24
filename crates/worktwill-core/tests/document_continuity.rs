#![cfg(windows)]
use rusqlite::Connection;
use std::{fs, path::PathBuf};
use worktwill_core::{model::*, Store};
struct Fixture {
    _dir: tempfile::TempDir,
    path: PathBuf,
    store: Store,
    rid: String,
    root: String,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("continuity-service-")
            .tempdir_in(std::env::var_os("WORKTWILL_TEST_ROOT").unwrap())
            .unwrap();
        let docs = dir.path().join("documents");
        fs::create_dir(&docs).unwrap();
        let path = docs.join("contract.txt");
        fs::write(&path, b"old").unwrap();
        let store = Store::open(dir.path().join("app/db.sqlite3")).unwrap();
        let rid = store
            .import_files(&[path.to_string_lossy().into()], None, true)
            .unwrap()
            .items[0]
            .resource_id
            .clone()
            .unwrap();
        let root = store.grant_root(&docs).unwrap();
        store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation: Mutation::Favorite {
                    id: rid.clone(),
                    value: true,
                },
            })
            .unwrap();
        store.set_document_tracking(&root, true).unwrap();
        store.establish_document_anchor(&rid).unwrap();
        let opened = store.prepare_open(&rid).unwrap();
        store.record_open_checked(opened).unwrap();
        Self {
            _dir: dir,
            path,
            store,
            rid,
            root,
        }
    }
    fn replace(&self, scan: bool, same_birth: bool) {
        self.replace_with_transfer(scan, same_birth, true);
    }
    fn replace_with_transfer(&self, scan: bool, same_birth: bool, transfer: bool) {
        let before = self.store.document_tracking_status().unwrap();
        let original_identity = tracking_identity(&self.path);
        let temp = self.path.with_file_name("save.tmp");
        fs::write(&temp, b"new version").unwrap();
        if same_birth {
            use std::os::windows::{ffi::OsStrExt, fs::MetadataExt};
            use windows_sys::Win32::{Foundation::CloseHandle, Storage::FileSystem::*};
            let wide: Vec<_> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
            let creation = self.path.metadata().unwrap().creation_time();
            unsafe {
                let h = CreateFileW(
                    wide.as_ptr(),
                    FILE_WRITE_ATTRIBUTES,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                );
                let t = windows_sys::Win32::Foundation::FILETIME {
                    dwLowDateTime: creation as u32,
                    dwHighDateTime: (creation >> 32) as u32,
                };
                assert_ne!(SetFileTime(h, &t, std::ptr::null(), std::ptr::null()), 0);
                CloseHandle(h);
            }
        }
        if scan {
            self.store.scan_all().unwrap();
        }
        let replacement_identity = tracking_identity(&temp);
        use std::os::windows::ffi::OsStrExt;
        let a: Vec<_> = self.path.as_os_str().encode_wide().chain(Some(0)).collect();
        let b: Vec<_> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            assert_ne!(
                windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
                    a.as_ptr(),
                    b.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null(),
                    std::ptr::null()
                ),
                0
            );
        }
        let current = tracking_identity(&self.path);
        eprintln!(
            "REPLACEFILE_NATIVE same_volume={} same_file_id={} same_creation={} anchor_transferred={} temp_had_anchor={} current_has_anchor={}",
            original_identity.0 == current.0,
            original_identity.1 == current.1,
            original_identity.3 == current.3,
            original_identity.2.is_some() && original_identity.2 == current.2,
            replacement_identity.2.is_some(),
            current.2.is_some(),
        );
        // ReplaceFileW 不保证跨环境转移 Object ID。仅测试夹具模拟确实转移的情形；
        // 丢失锚点的拒绝行为由独立测试覆盖，产品代码不得按路径补标记。
        if transfer && current.2.is_none() {
            set_fixture_anchor(&self.path, original_identity.2.unwrap());
        }
        if !transfer && current.2.is_some() {
            remove_fixture_anchor(&self.path);
        }
        let after = self.store.document_tracking_status().unwrap();
        eprintln!("REPLACE_TRACKING before={before:?} after={after:?}");
    }
    fn list(&self, mode: &str) -> ResourcePage {
        self.store
            .resources(ResourceQuery {
                workspace_id: None,
                kind: "file".into(),
                mode: mode.into(),
                offset: 0,
            })
            .unwrap()
    }
}
// 仅读取本测试创建的文件身份，区分 ReplaceFileW 未转移锚点与续接失败。
fn tracking_identity(path: &std::path::Path) -> (u64, [u8; 16], Option<[u8; 16]>, i64) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::*,
        Storage::FileSystem::*,
        System::{Ioctl::*, IO::DeviceIoControl},
    };
    let wide: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let handle = CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        );
        assert_ne!(handle, INVALID_HANDLE_VALUE);
        let mut id: FILE_ID_INFO = std::mem::zeroed();
        let mut basic: FILE_BASIC_INFO = std::mem::zeroed();
        assert_ne!(
            GetFileInformationByHandleEx(
                handle,
                FileIdInfo,
                &mut id as *mut _ as *mut _,
                std::mem::size_of::<FILE_ID_INFO>() as u32
            ),
            0
        );
        assert_ne!(
            GetFileInformationByHandleEx(
                handle,
                FileBasicInfo,
                &mut basic as *mut _ as *mut _,
                std::mem::size_of::<FILE_BASIC_INFO>() as u32
            ),
            0
        );
        let mut object: FILE_OBJECTID_BUFFER = std::mem::zeroed();
        let mut returned = 0;
        let ok = DeviceIoControl(
            handle,
            FSCTL_GET_OBJECT_ID,
            std::ptr::null(),
            0,
            &mut object as *mut _ as *mut _,
            std::mem::size_of::<FILE_OBJECTID_BUFFER>() as u32,
            &mut returned,
            std::ptr::null_mut(),
        );
        let code = if ok == 0 { GetLastError() } else { 0 };
        CloseHandle(handle);
        assert!(
            ok != 0 || matches!(code, 2 | 1168 | 4312),
            "unexpected Object ID read error {code}"
        );
        (
            id.VolumeSerialNumber,
            id.FileId.Identifier,
            (ok != 0 && returned >= 16).then_some(object.ObjectId),
            basic.CreationTime,
        )
    }
}

fn remove_fixture_anchor(path: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::*,
        Storage::FileSystem::*,
        System::{Ioctl::*, IO::DeviceIoControl},
    };
    let wide: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let handle = CreateFileW(
            wide.as_ptr(),
            FILE_WRITE_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        );
        assert_ne!(handle, INVALID_HANDLE_VALUE);
        let mut returned = 0;
        let ok = DeviceIoControl(
            handle,
            FSCTL_DELETE_OBJECT_ID,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        );
        let code = if ok == 0 { GetLastError() } else { 0 };
        CloseHandle(handle);
        assert_ne!(
            ok, 0,
            "fixture anchor removal failed (Windows error {code})"
        );
    }
    assert_eq!(tracking_identity(path).2, None);
}

fn set_fixture_anchor(path: &std::path::Path, anchor: [u8; 16]) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::*,
        Storage::FileSystem::*,
        System::{Ioctl::*, IO::DeviceIoControl},
    };
    let wide: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        // 显式设置测试锚点时申请写数据权限，不只使用修改时间等属性的权限。
        let handle = CreateFileW(
            wide.as_ptr(),
            FILE_WRITE_DATA | FILE_WRITE_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        );
        assert_ne!(handle, INVALID_HANDLE_VALUE);
        let mut object: FILE_OBJECTID_BUFFER = std::mem::zeroed();
        object.ObjectId = anchor;
        let mut returned = 0;
        let ok = DeviceIoControl(
            handle,
            FSCTL_SET_OBJECT_ID,
            &mut object as *mut _ as *mut _,
            std::mem::size_of::<FILE_OBJECTID_BUFFER>() as u32,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        );
        let code = if ok == 0 { GetLastError() } else { 0 };
        CloseHandle(handle);
        assert_ne!(
            ok, 0,
            "fixture anchor transfer failed (Windows error {code})"
        );
    }
    assert_eq!(tracking_identity(path).2, Some(anchor));
}

#[test]
fn replacement_without_transferred_anchor_cannot_inherit_binding() {
    let f = Fixture::new();
    let before = tracking_identity(&f.path);
    f.replace_with_transfer(false, false, false);
    let after = tracking_identity(&f.path);
    assert_ne!(before.1, after.1);
    assert_eq!(after.2, None);
    assert!(!f.store.reconcile_document(&f.path).unwrap());
    let status = f.store.document_tracking_status().unwrap();
    assert!(!status.files[0].established);
    assert_eq!(
        status.files[0].error.as_ref().unwrap().code,
        "continuity_conflict"
    );
    assert_eq!(f.list("favorites").items[0].id, f.rid);
    assert_eq!(f.store.file_grants().unwrap().len(), 1);
    let db = Connection::open(f._dir.path().join("app/db.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM resource_binding_history", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
}

#[test]
fn removed_object_id_must_not_report_established_for_same_physical_file() {
    let f = Fixture::new();
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        Storage::FileSystem::*,
        System::{Ioctl::FSCTL_DELETE_OBJECT_ID, IO::DeviceIoControl},
    };
    let wide: Vec<_> = f.path.as_os_str().encode_wide().chain(Some(0)).collect();
    // 仅对本测试新建的TempDir文件移除标记，模拟外部工具改变追踪元数据。
    unsafe {
        let h = CreateFileW(
            wide.as_ptr(),
            FILE_WRITE_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );
        let mut returned = 0;
        let result = DeviceIoControl(
            h,
            FSCTL_DELETE_OBJECT_ID,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        );
        CloseHandle(h);
        assert_ne!(result, 0);
    }
    assert!(f.store.resolve_resource(&f.rid).is_ok());
    let status = f.store.document_tracking_status().unwrap();
    assert!(!status.files[0].established);
    assert_eq!(
        status.files[0].error.as_ref().unwrap().code,
        "continuity_conflict"
    );
    assert_eq!(f.list("favorites").items[0].id, f.rid);
}

#[test]
fn service_continuation_keeps_recent_and_retires_mixed_grant() {
    let f = Fixture::new();
    f.replace(false, false);
    assert!(f.store.reconcile_document(&f.path).unwrap());
    assert_eq!(f.list("recent").items[0].id, f.rid);
    assert_eq!(f.list("favorites").items[0].id, f.rid);
    assert!(f.store.file_grants().unwrap().is_empty());
    let db = Connection::open(f._dir.path().join("app/db.sqlite3")).unwrap();
    let history: i64 = db
        .query_row("SELECT count(*) FROM resource_binding_history", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(history, 1);
    for _ in 0..2 {
        f.replace(false, false);
        assert!(f.store.reconcile_document(&f.path).unwrap());
    }
    assert_eq!(f.list("recent").items.len(), 1);
    f.store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation: Mutation::RevokeRoot { id: f.root.clone() },
        })
        .unwrap();
    assert!(f.list("recent").items.is_empty());
    assert!(f.list("favorites").items.is_empty());
    assert!(f.store.resolve_resource(&f.rid).is_err());
}
#[test]
fn concurrent_candidates_commit_one_binding_and_preserve_original_observation() {
    let f = Fixture::new();
    let db = Connection::open(f._dir.path().join("app/db.sqlite3")).unwrap();
    let original: String = db.query_row(
        "SELECT identity FROM activity_origins o JOIN activities a ON a.id=o.activity_id WHERE a.kind='opened'",
        [], |r| r.get(0),
    ).unwrap();
    f.replace(false, false);
    let start = std::sync::Barrier::new(4);
    let results = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    start.wait();
                    f.store.reconcile_document(&f.path).unwrap()
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.into_iter().filter(|changed| *changed).count(), 1);
    assert_eq!(
        db.query_row("SELECT count(*) FROM resource_binding_history", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT binding_version FROM document_anchors", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(db.query_row("SELECT identity FROM activity_origins o JOIN activities a ON a.id=o.activity_id WHERE a.kind='opened'", [], |r| r.get::<_, String>(0)).unwrap(), original);
    assert!(f.store.file_grants().unwrap().is_empty());
    assert_eq!(f.list("recent").items[0].id, f.rid);
    assert_eq!(f.list("favorites").items[0].id, f.rid);
}

#[test]
fn current_access_cannot_revive_excluded_historical_observation() {
    let f = Fixture::new();
    f.replace(false, false);
    assert!(f.store.reconcile_document(&f.path).unwrap());
    let nested = f.path.parent().unwrap().join("current");
    fs::create_dir(&nested).unwrap();
    let moved = nested.join("contract.txt");
    fs::rename(&f.path, &moved).unwrap();
    f.store.scan_all().unwrap();
    assert_eq!(f.list("recent").items[0].id, f.rid);
    f.store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation: Mutation::Exclude {
                root_id: f.root.clone(),
                relative_path: "contract.txt".into(),
            },
        })
        .unwrap();
    // 当前路径仍在授权内，原打开活动的旧路径已被排除；连续性链不能提供历史授权。
    assert!(f.store.resolve_resource(&f.rid).is_ok());
    assert_eq!(f.list("favorites").items[0].id, f.rid);
    assert!(f.list("recent").items.is_empty());
}

#[test]
fn competing_scan_record_rejected_for_both_creation_time_cases() {
    for same in [false, true] {
        let f = Fixture::new();
        f.replace(true, same);
        assert_eq!(
            f.store.reconcile_document(&f.path).unwrap_err().code,
            "continuity_conflict"
        );
        assert_eq!(f.list("favorites").items[0].id, f.rid);
        assert_eq!(f.store.file_grants().unwrap().len(), 1);
        let status = f.store.document_tracking_status().unwrap();
        let tracked = status
            .files
            .iter()
            .find(|file| file.resource_id == f.rid)
            .unwrap();
        assert!(!tracked.established);
        assert_eq!(tracked.error.as_ref().unwrap().code, "continuity_conflict");
    }
}

#[test]
fn every_entry_preserves_both_uuids_when_replacement_source_was_catalogued() {
    for entry in ["scan", "notification", "import", "open"] {
        let f = Fixture::new();
        f.replace(true, false);
        let db = Connection::open(f._dir.path().join("app/db.sqlite3")).unwrap();
        let before: Vec<(String, String)> = db
            .prepare("SELECT id,identity FROM resources ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(before.len(), 2);
        match entry {
            "scan" => {
                // 混合授权先复验单文件，冲突应显式拒绝，不能期望扫描成功掩盖错误。
                assert_eq!(f.store.scan_all().unwrap_err().code, "continuity_conflict");
            }
            "notification" => {
                assert_eq!(
                    f.store.refresh_observed_file(&f.path).unwrap_err().code,
                    "continuity_conflict"
                );
            }
            "import" => {
                let result = f
                    .store
                    .import_files(&[f.path.to_string_lossy().into()], None, false)
                    .unwrap();
                assert!(result.items[0].resource_id.is_none());
            }
            "open" => {
                assert!(f.store.prepare_open(&f.rid).is_err());
            }
            _ => unreachable!(),
        }
        let after: Vec<(String, String)> = db
            .prepare("SELECT id,identity FROM resources ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(after, before, "{entry} changed a conflicting binding");
        assert_eq!(
            db.query_row("SELECT count(*) FROM resource_binding_history", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
            0
        );
        assert_eq!(f.list("favorites").items[0].id, f.rid);
    }
}

#[test]
fn same_name_recreation_without_anchor_cannot_inherit_existing_relations() {
    let f = Fixture::new();
    fs::rename(&f.path, f._dir.path().join("retained-original.txt")).unwrap();
    fs::write(&f.path, b"independent recreated document").unwrap();
    f.store.scan_all().unwrap();
    assert!(f.store.prepare_open(&f.rid).is_err());
    let current = f
        .list("modified")
        .items
        .into_iter()
        .find(|item| item.id != f.rid)
        .unwrap();
    assert!(!current.favorite);
    assert_ne!(current.id, f.rid);
    let db = Connection::open(f._dir.path().join("app/db.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM resource_binding_history", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    assert_eq!(f.list("favorites").items[0].id, f.rid);
}

#[test]
fn conflicting_single_grant_does_not_starve_unrelated_directory_scan() {
    let f = Fixture::new();
    let other = f._dir.path().join("other-documents");
    fs::create_dir(&other).unwrap();
    f.store.grant_root(&other).unwrap();
    f.replace(true, false);
    fs::write(other.join("unrelated.txt"), b"must still be discovered").unwrap();
    assert_eq!(f.store.scan_all().unwrap_err().code, "continuity_conflict");
    let db = Connection::open(f._dir.path().join("app/db.sqlite3")).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM resources WHERE name='unrelated.txt' AND status='available'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM resource_binding_history", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    assert_eq!(f.list("favorites").items[0].id, f.rid);
}

#[test]
fn failed_commit_rolls_back_binding_grant_and_history_then_retries() {
    let f = Fixture::new();
    f.replace(false, false);
    let db = Connection::open(f._dir.path().join("app/db.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_lineage BEFORE INSERT ON resource_binding_history BEGIN SELECT RAISE(ABORT,'fixture rollback'); END;").unwrap();
    assert!(f.store.reconcile_document(&f.path).is_err());
    assert_eq!(f.store.file_grants().unwrap().len(), 1);
    assert_eq!(
        db.query_row("SELECT count(*) FROM resource_binding_history", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    db.execute_batch("DROP TRIGGER reject_lineage;").unwrap();
    assert!(f.store.reconcile_document(&f.path).unwrap());
    assert!(f.store.file_grants().unwrap().is_empty());
    assert_eq!(f.list("recent").items.len(), 1);
}

#[test]
fn disabling_consent_succeeds_while_root_is_paused_and_unavailable() {
    let f = Fixture::new();
    f.store
        .mutate(MutationRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            mutation: Mutation::SetPaused {
                id: f.root.clone(),
                value: true,
            },
        })
        .unwrap();
    fs::rename(
        f.path.parent().unwrap(),
        f._dir.path().join("offline-documents"),
    )
    .unwrap();
    f.store.set_document_tracking(&f.root, false).unwrap();
    let db = Connection::open(f._dir.path().join("app/db.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT enabled FROM document_tracking_policies", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM document_anchors", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn unavailable_enabled_root_does_not_hide_consent_controls() {
    let f = Fixture::new();
    fs::rename(
        f.path.parent().unwrap(),
        f._dir.path().join("offline-documents"),
    )
    .unwrap();
    let status = f.store.document_tracking_status().unwrap();
    assert!(status
        .roots
        .iter()
        .any(|root| root.root_id == f.root && root.enabled));
    assert!(status.files.is_empty());
    f.store.set_document_tracking(&f.root, false).unwrap();
    assert!(f
        .store
        .document_tracking_status()
        .unwrap()
        .roots
        .iter()
        .all(|root| !root.enabled));
}

#[test]
fn disabled_tracking_does_not_reconcile_and_restore_does_not_reactivate_consent() {
    let f = Fixture::new();
    let backup = f._dir.path().join("backup.sqlite3");
    f.store.backup_to(&backup).unwrap();
    f.store.set_document_tracking(&f.root, false).unwrap();
    f.replace(false, false);
    assert!(!f.store.reconcile_document(&f.path).unwrap());
    f.store.restore_from(&backup).unwrap();
    let db = Connection::open(f._dir.path().join("app/db.sqlite3")).unwrap();
    let enabled: i64 = db
        .query_row("SELECT enabled FROM document_tracking_policies", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(enabled, 0);
    assert!(!f.store.reconcile_document(&f.path).unwrap());
}

#[test]
fn maintenance_reports_failure_without_losing_favorite_and_retries_after_reopen() {
    let f = Fixture::new();
    let db = Connection::open(f._dir.path().join("app/db.sqlite3")).unwrap();
    db.execute("DELETE FROM document_anchors", []).unwrap();
    fs::remove_file(&f.path).unwrap();
    f.store.maintain_document_anchors().unwrap();
    let status = f.store.document_tracking_status().unwrap();
    assert_eq!(status.files.len(), 1);
    assert!(!status.files[0].established);
    assert!(status.files[0].error.is_some());
    assert_eq!(f.list("favorites").items[0].id, f.rid);
    // 文件重建不能继承旧物理身份；重启重试仍须明确失败。
    fs::write(&f.path, b"another document").unwrap();
    drop(f.store);
    let reopened = Store::open(f._dir.path().join("app/db.sqlite3")).unwrap();
    reopened.maintain_document_anchors().unwrap();
    assert!(!reopened.document_tracking_status().unwrap().files[0].established);
    let anchors: i64 = db
        .query_row("SELECT count(*) FROM document_anchors", [], |r| r.get(0))
        .unwrap();
    assert_eq!(anchors, 0);
}

#[test]
fn durable_anchor_recovers_save_while_app_is_closed() {
    let f = Fixture::new();
    f.replace(false, false);
    let database = f._dir.path().join("app/db.sqlite3");
    drop(f.store);
    let reopened = Store::open(database).unwrap();
    assert!(reopened.resolve_resource(&f.rid).is_ok());
    let status = reopened.document_tracking_status().unwrap();
    assert!(status.files[0].established);
    assert!(reopened.file_grants().unwrap().is_empty());
}

#[test]
fn copies_stay_independent_and_move_within_tracked_root_keeps_binding() {
    let f = Fixture::new();
    let copy = f.path.with_file_name("copy.txt");
    fs::copy(&f.path, &copy).unwrap();
    f.store.scan_all().unwrap();
    assert_eq!(f.list("modified").items.len(), 2);
    let copy_resource = f
        .list("modified")
        .items
        .into_iter()
        .find(|r| r.name == "copy.txt")
        .unwrap();
    assert_ne!(copy_resource.id, f.rid);
    assert!(!copy_resource.favorite);
    let moved = f.path.with_file_name("renamed.txt");
    fs::rename(&f.path, &moved).unwrap();
    f.store.scan_all().unwrap();
    let favorite = &f.list("favorites").items[0];
    assert_eq!(favorite.id, f.rid);
    assert_eq!(favorite.name, "renamed.txt");
    assert!(f.store.resolve_resource(&f.rid).is_ok());
}
