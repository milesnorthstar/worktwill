//! 文件选择器私有批次边界；不通过IPC接收路径或构成额外授权入口。
use worktwill_core::{
    capture_import_selections, model::Result, ImportResult, ImportSelection, Store,
};

pub(crate) struct PickedFiles {
    selections: Vec<ImportSelection>,
}

impl PickedFiles {
    pub(crate) fn capture(paths: Vec<String>) -> Self {
        // 先固定整批所选对象，逐项提交时不能按路径改授后来替换的对象。
        Self {
            selections: capture_import_selections(&paths),
        }
    }

    pub(crate) fn submit(&self, store: &Store, workspace: Option<&str>) -> Result<ImportResult> {
        store.import_files_checked(&self.selections, workspace, true)
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};
    use worktwill_core::model::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
            let directory = PathBuf::from(approved)
                .canonicalize()
                .unwrap()
                .join(format!("picked-files-domain-{}", uuid::Uuid::new_v4()));
            // 必须是本测试新建的独有目录，已存在则失败，不清空他人样本。
            fs::create_dir(&directory).unwrap();
            Self(directory)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let cleanup = fs::remove_dir_all(&self.0);
            if !std::thread::panicking() {
                cleanup.expect("remove only this test's own directory");
            }
        }
    }

    #[test]
    fn discarded_capture_does_not_create_authorization_or_resources() {
        let directory = TempDir::new();
        let store = Store::open(directory.0.join("app/catalog.sqlite3")).unwrap();
        let file = directory.0.join("cancelled-selection.txt");
        fs::write(&file, b"cancelled selection fixture").unwrap();
        let modified = file.metadata().unwrap().modified().unwrap();
        let before = store.overview().unwrap();
        let selected = PickedFiles::capture(vec![file.to_string_lossy().into_owned()]);
        // 选择器取消在调用方直接返回None；此处也确认丢弃已捕获批次不会提交授权。
        drop(selected);
        let after = store.overview().unwrap();
        assert_eq!(after.permission_version, before.permission_version);
        assert_eq!(after.revision, before.revision);
        assert!(after.file_grants.is_empty());
        assert!(after.roots.is_empty());
        assert!(after.workspaces.is_empty());
        assert!(after.activities.is_empty());
        assert!(store
            .resources(ResourceQuery {
                workspace_id: None,
                kind: "file".into(),
                mode: "modified".into(),
                offset: 0,
            })
            .unwrap()
            .items
            .is_empty());
        assert!(store.needs_file_grant(&file).unwrap());
        assert_eq!(fs::read(&file).unwrap(), b"cancelled selection fixture");
        assert_eq!(file.metadata().unwrap().modified().unwrap(), modified);
    }

    #[test]
    fn captured_batch_rejects_replaced_later_file_without_losing_other_items() {
        let directory = TempDir::new();
        let store = Store::open(directory.0.join("app/catalog.sqlite3")).unwrap();
        let first = directory.0.join("first-selected.txt");
        let second = directory.0.join("second-selected.txt");
        let third = directory.0.join("third-selected.txt");
        fs::write(&first, b"first selected fixture").unwrap();
        fs::write(&second, b"second selected fixture").unwrap();
        fs::write(&third, b"third selected fixture").unwrap();
        let first_modified = first.metadata().unwrap().modified().unwrap();
        let third_modified = third.metadata().unwrap().modified().unwrap();
        let workspace = store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation: Mutation::CreateWorkspace {
                    name: "选择器批量身份验收".into(),
                    resource_ids: None,
                },
            })
            .unwrap()
            .id
            .unwrap();
        let selected = PickedFiles::capture(
            [&first, &second, &third]
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
        );
        let replacement = directory.0.join("unselected-replacement.tmp");
        fs::write(&replacement, b"unselected different physical object").unwrap();
        // 全批捕获后、提交前覆盖后项；无需计时、线程或操作系统选择器模拟。
        fs::rename(&replacement, &second).unwrap();
        let second_modified = second.metadata().unwrap().modified().unwrap();
        let result = selected.submit(&store, Some(&workspace)).unwrap();
        assert_eq!(result.items.len(), 3);
        assert_eq!(result.items[0].status, "added");
        assert_eq!(result.items[1].status, "failed");
        assert!(result.items[1].resource_id.is_none());
        assert_eq!(result.items[2].status, "added");
        assert!(result.items[0].message.is_none());
        assert!(result.items[1].message.is_some() && result.items[1].message_en.is_some());
        assert!(result.items[2].message.is_none());
        let grants = store.file_grants().unwrap();
        assert_eq!(grants.len(), 2);
        assert!(grants
            .iter()
            .all(|resource| resource.location != second.to_string_lossy()));
        assert!(store.needs_file_grant(&second).unwrap());
        assert!(!store.needs_file_grant(&first).unwrap());
        assert!(!store.needs_file_grant(&third).unwrap());
        let resources = store
            .resources(ResourceQuery {
                workspace_id: Some(workspace),
                kind: "file".into(),
                mode: "modified".into(),
                offset: 0,
            })
            .unwrap()
            .items;
        let mut actual = resources
            .iter()
            .map(|resource| resource.id.clone())
            .collect::<Vec<_>>();
        let mut expected = [0, 2]
            .map(|index| result.items[index].resource_id.clone().unwrap())
            .to_vec();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
        assert_eq!(resources.len(), 2);
        assert!(resources
            .iter()
            .all(|resource| resource.status == "available"));
        assert_eq!(
            store
                .resources(ResourceQuery {
                    workspace_id: None,
                    kind: "file".into(),
                    mode: "modified".into(),
                    offset: 0
                })
                .unwrap()
                .items
                .len(),
            2
        );
        assert_eq!(fs::read(&first).unwrap(), b"first selected fixture");
        assert_eq!(
            fs::read(&second).unwrap(),
            b"unselected different physical object"
        );
        assert_eq!(fs::read(&third).unwrap(), b"third selected fixture");
        assert_eq!(
            first.metadata().unwrap().modified().unwrap(),
            first_modified
        );
        assert_eq!(
            second.metadata().unwrap().modified().unwrap(),
            second_modified
        );
        assert_eq!(
            third.metadata().unwrap().modified().unwrap(),
            third_modified
        );
    }
}
