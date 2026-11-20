use std::{fs, path::PathBuf};
use worktwill_core::{capture_import_selections, Store};

struct Fixture {
    _directory: tempfile::TempDir,
    documents: PathBuf,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        let parent = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("drop-identity-")
            .tempdir_in(parent)
            .unwrap();
        let documents = directory.path().join("documents");
        fs::create_dir(&documents).unwrap();
        let store = Store::open(directory.path().join("app/data.sqlite3")).unwrap();
        Self {
            _directory: directory,
            documents,
            store,
        }
    }
}

#[test]
fn captured_object_cannot_be_replaced_at_same_path_before_authorization() {
    let fixture = Fixture::new();
    let path = fixture.documents.join("contract.txt");
    fs::write(&path, b"original fixture").unwrap();
    let paths = vec![path.to_string_lossy().into_owned()];
    let selections = capture_import_selections(&paths);
    assert!(selections[0].identity.is_some());
    assert_eq!(
        fixture
            .store
            .import_files_checked(&selections, None, false)
            .unwrap()
            .items[0]
            .status,
        "not_authorized"
    );
    let replacement = fixture.documents.join("replacement.txt");
    fs::write(&replacement, b"another object").unwrap();
    fs::remove_file(&path).unwrap();
    fs::rename(replacement, &path).unwrap();
    let output = fixture
        .store
        .import_files_checked(&selections, None, true)
        .unwrap();
    assert_eq!(output.items[0].status, "failed");
    assert!(output.items[0].message.as_deref().unwrap().contains("替换"));
    assert!(output.items[0]
        .message_en
        .as_deref()
        .unwrap()
        .contains("replaced"));
    let payload = serde_json::to_value(&output).unwrap();
    assert_eq!(
        payload["items"][0]["message_en"],
        "The dropped file has been replaced. Drop it again."
    );
    assert!(fixture.store.file_grants().unwrap().is_empty());
    assert_eq!(fs::read(&path).unwrap(), b"another object");
}

#[test]
fn capture_failure_cannot_become_an_implicit_grant_when_file_appears_later() {
    let fixture = Fixture::new();
    let path = fixture.documents.join("not-present.txt");
    let selection = capture_import_selections(&[path.to_string_lossy().into_owned()]);
    assert!(selection[0].capture_error.is_some());
    let capture_error = selection[0].capture_error.as_ref().unwrap();
    fs::write(&path, b"later object").unwrap();
    let output = fixture
        .store
        .import_files_checked(&selection, None, true)
        .unwrap();
    assert_eq!(output.items[0].status, "failed");
    assert_eq!(
        output.items[0].message.as_ref(),
        Some(&capture_error.message)
    );
    assert_eq!(
        output.items[0].message_en.as_ref(),
        Some(&capture_error.message_en)
    );
    assert!(!capture_error.message_en.is_empty());
    assert!(fixture.store.file_grants().unwrap().is_empty());
}

#[test]
fn unchanged_capture_has_no_source_writes_and_checked_import_remains_idempotent() {
    let fixture = Fixture::new();
    let path = fixture.documents.join("stable.txt");
    fs::write(&path, b"stable fixture").unwrap();
    let before = path.metadata().unwrap();
    let selections = capture_import_selections(&[path.to_string_lossy().into_owned()]);
    let first = fixture
        .store
        .import_files_checked(&selections, None, true)
        .unwrap();
    let again = fixture
        .store
        .import_files_checked(&selections, None, true)
        .unwrap();
    assert_eq!(first.items[0].status, "added");
    assert_eq!(again.items[0].status, "already_present");
    assert_eq!(first.items[0].resource_id, again.items[0].resource_id);
    let after = path.metadata().unwrap();
    assert_eq!(fs::read(path).unwrap(), b"stable fixture");
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    assert_eq!(before.len(), after.len());
}

#[test]
fn mixed_checked_import_keeps_unchanged_success_and_rejects_replaced_selection() {
    let fixture = Fixture::new();
    let a = fixture.documents.join("a.txt");
    let b = fixture.documents.join("b.txt");
    fs::write(&a, b"a").unwrap();
    fs::write(&b, b"b").unwrap();
    let selections = capture_import_selections(&[
        a.to_string_lossy().into_owned(),
        b.to_string_lossy().into_owned(),
    ]);
    let replacement = fixture.documents.join("replace.txt");
    fs::write(&replacement, b"new b").unwrap();
    fs::remove_file(&b).unwrap();
    fs::rename(replacement, b).unwrap();
    let output = fixture
        .store
        .import_files_checked(&selections, None, true)
        .unwrap();
    assert_eq!(output.items[0].status, "added");
    assert_eq!(output.items[1].status, "failed");
    assert_eq!(fixture.store.file_grants().unwrap().len(), 1);
}
