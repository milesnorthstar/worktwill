use std::{
    fs,
    path::{Path, PathBuf},
};
use worktwill_core::{capture_import_selections, model::*, ImportSelection, Store};

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    outside: PathBuf,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        let parent = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("relink-grants-")
            .tempdir_in(parent)
            .unwrap();
        let root = directory.path().join("documents");
        let outside = directory.path().join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        let store = Store::open(directory.path().join("app/data.sqlite3")).unwrap();
        Self {
            _directory: directory,
            root,
            outside,
            store,
        }
    }
    fn file(&self, path: &Path) -> String {
        fs::write(path, b"fixture original").unwrap();
        self.store
            .import_files(&[path.to_string_lossy().into_owned()], None, true)
            .unwrap()
            .items[0]
            .resource_id
            .clone()
            .unwrap()
    }
    fn selected(&self, path: &Path) -> ImportSelection {
        capture_import_selections(&[path.to_string_lossy().into_owned()]).remove(0)
    }
    fn change(&self, mutation: Mutation) {
        self.store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation,
            })
            .unwrap();
    }
}

#[test]
fn selected_relink_repairs_directory_owned_file_outside_without_granting_parent() {
    let f = Fixture::new();
    f.store.grant_root(&f.root).unwrap();
    let original = f.root.join("original.txt");
    let rid = f.file(&original);
    assert!(f.store.file_grants().unwrap().is_empty());
    let target = f.outside.join("moved.txt");
    fs::rename(original, &target).unwrap();
    let sibling = f.outside.join("private.txt");
    fs::write(&sibling, b"not selected").unwrap();
    assert_eq!(
        f.store.relink(&rid, &target).unwrap_err().code,
        "not_authorized"
    );
    let before = target.metadata().unwrap();
    f.store
        .relink_selected(&rid, &f.selected(&target), false)
        .unwrap();
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap().0,
        target.canonicalize().unwrap().to_string_lossy()
    );
    assert_eq!(f.store.file_grants().unwrap()[0].id, rid);
    assert_eq!(f.store.roots().unwrap().len(), 1);
    assert!(f.store.needs_file_grant(&sibling).unwrap());
    assert_eq!(fs::read(&target).unwrap(), b"fixture original");
    assert_eq!(
        before.modified().unwrap(),
        target.metadata().unwrap().modified().unwrap()
    );
}

#[test]
fn active_old_grant_does_not_authorize_an_unrelated_object_but_selection_can() {
    let f = Fixture::new();
    let old = f.outside.join("old.txt");
    let rid = f.file(&old);
    let target = f.outside.join("unrelated.txt");
    fs::write(&target, b"new object").unwrap();
    assert_eq!(
        f.store.relink(&rid, &target).unwrap_err().code,
        "not_authorized"
    );
    assert_eq!(
        f.store
            .relink_confirmed(&rid, &target, true)
            .unwrap_err()
            .code,
        "not_authorized"
    );
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap().0,
        old.canonicalize().unwrap().to_string_lossy()
    );
    f.store
        .relink_selected(&rid, &f.selected(&target), false)
        .unwrap();
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap().0,
        target.canonicalize().unwrap().to_string_lossy()
    );
    assert_eq!(f.store.file_grants().unwrap().len(), 1);
    assert!(f.store.roots().unwrap().is_empty());
}

#[test]
fn same_physical_object_keeps_its_existing_single_file_grant_after_move() {
    let f = Fixture::new();
    let original = f.outside.join("old.txt");
    let rid = f.file(&original);
    let moved = f.outside.join("moved.txt");
    fs::rename(original, &moved).unwrap();
    f.store.relink(&rid, &moved).unwrap();
    assert_eq!(f.store.file_grants().unwrap()[0].id, rid);
    assert!(f.store.resolve_resource(&rid).is_ok());
}

#[test]
fn old_grant_is_deactivated_when_relinking_to_different_directory_authorized_object() {
    let f = Fixture::new();
    let rid = f.file(&f.outside.join("old.txt"));
    let root_id = f.store.grant_root(&f.root).unwrap();
    let target = f.root.join("different.txt");
    fs::write(&target, b"different object").unwrap();
    f.store.relink_confirmed(&rid, &target, true).unwrap();
    assert!(f.store.resolve_resource(&rid).is_ok());
    assert!(f.store.file_grants().unwrap().is_empty());
    f.change(Mutation::RevokeRoot { id: root_id });
    assert!(f.store.resolve_resource(&rid).is_err());
}

#[test]
fn same_path_replacement_retires_old_single_grant_even_with_later_directory_grant() {
    let f = Fixture::new();
    let target = f.root.join("contract.txt");
    // 先选单文件再授权目录，覆盖本轮发现的混合授权顺序。
    let rid = f.file(&target);
    let root_id = f.store.grant_root(&f.root).unwrap();
    assert_eq!(f.store.file_grants().unwrap().len(), 1);
    let retired = f.outside.join("retired.txt");
    fs::rename(&target, &retired).unwrap();
    fs::write(&target, b"replacement content").unwrap();
    f.store.relink(&rid, &target).unwrap();
    assert!(f.store.file_grants().unwrap().is_empty());
    assert!(f.store.resolve_resource(&rid).is_ok());
    f.change(Mutation::RevokeRoot { id: root_id });
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap_err().code,
        "not_authorized"
    );
    assert_eq!(fs::read(&target).unwrap(), b"replacement content");
    assert_eq!(fs::read(&retired).unwrap(), b"fixture original");
}

#[test]
fn merge_unions_active_target_grant_over_inactive_original_grant() {
    let f = Fixture::new();
    let rid = f.file(&f.outside.join("original.txt"));
    f.change(Mutation::RevokeFile { id: rid.clone() });
    let target = f.outside.join("target.txt");
    let other = f.file(&target);
    let selected = f.selected(&target);
    assert_eq!(
        f.store
            .relink_selected(&rid, &selected, false)
            .unwrap_err()
            .code,
        "merge_confirmation_required"
    );
    assert_eq!(f.store.file_grants().unwrap()[0].id, other);
    f.store.relink_confirmed(&rid, &target, true).unwrap();
    let grants = f.store.file_grants().unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].id, rid);
    assert!(f.store.resolve_resource(&rid).is_ok());
    assert!(f.store.resolve_resource(&other).is_err());
}

#[test]
fn selected_merge_rejects_replacement_while_confirmation_is_pending() {
    let f = Fixture::new();
    let old = f.outside.join("original.txt");
    let rid = f.file(&old);
    let target = f.outside.join("target.txt");
    let other = f.file(&target);
    let selection = f.selected(&target);
    assert_eq!(
        f.store
            .relink_selected(&rid, &selection, false)
            .unwrap_err()
            .code,
        "merge_confirmation_required"
    );
    let replacement = f.outside.join("replacement.txt");
    fs::write(&replacement, b"substitution").unwrap();
    fs::rename(replacement, &target).unwrap();
    assert_eq!(
        f.store
            .relink_selected(&rid, &selection, true)
            .unwrap_err()
            .code,
        "identity_unconfirmed"
    );
    assert_eq!(
        f.store.resolve_resource(&rid).unwrap().0,
        old.canonicalize().unwrap().to_string_lossy()
    );
    assert_eq!(f.store.file_grants().unwrap().len(), 2);
    assert_ne!(rid, other);
}

#[test]
fn selected_capture_failure_does_not_grant_a_file_which_appears_later() {
    let f = Fixture::new();
    let rid = f.file(&f.outside.join("original.txt"));
    let missing = f.outside.join("later.txt");
    let selection = f.selected(&missing);
    fs::write(&missing, b"later object").unwrap();
    assert!(f.store.relink_selected(&rid, &selection, false).is_err());
    assert!(f.store.needs_file_grant(&missing).unwrap());
    assert_eq!(f.store.file_grants().unwrap().len(), 1);
}

#[test]
fn merge_does_not_reactivate_revoked_target_or_transfer_unrelated_original_grant() {
    let f = Fixture::new();
    let rid = f.file(&f.outside.join("original.txt"));
    let target = f.root.join("target.txt");
    let other = f.file(&target);
    f.change(Mutation::RevokeFile { id: other.clone() });
    let root_id = f.store.grant_root(&f.root).unwrap();
    f.store.relink_confirmed(&rid, &target, true).unwrap();
    assert!(f.store.resolve_resource(&rid).is_ok());
    assert!(f.store.file_grants().unwrap().is_empty());
    f.change(Mutation::RevokeRoot { id: root_id });
    assert!(f.store.resolve_resource(&rid).is_err());
    assert!(f.store.resolve_resource(&other).is_err());
}
