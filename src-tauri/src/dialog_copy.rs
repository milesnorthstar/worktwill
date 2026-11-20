use tauri_plugin_dialog::MessageDialogButtons;
use worktwill_core::model::UiLanguage;
use worktwill_core::{RelinkImpact, RelinkReview, RelinkWorkspace};

pub fn text(language: UiLanguage, zh: &'static str, en: &'static str) -> &'static str {
    match language {
        UiLanguage::Zh => zh,
        UiLanguage::En => en,
    }
}

pub enum Confirmation {
    GrantFiles,
    Relink,
    Merge,
    Restore,
}

pub struct DialogCopy {
    pub title: &'static str,
    pub body: &'static str,
    pub confirm: &'static str,
    pub cancel: &'static str,
}
impl DialogCopy {
    pub fn buttons(&self) -> MessageDialogButtons {
        MessageDialogButtons::OkCancelCustom(self.confirm.into(), self.cancel.into())
    }
}

pub fn confirmation(kind: Confirmation, language: UiLanguage) -> DialogCopy {
    let (title, body, confirm) = match kind {
        Confirmation::GrantFiles => (
            text(language, "授权所选文件", "Authorize selected files"),
            text(language, "仅授权这次选中的文件，不扫描它们所在的目录。加入工作区不会移动或修改原文件。", "Authorize only these selected files. Their folders will not be scanned. Adding them to a workspace does not move or modify the originals."),
            text(language, "授权文件", "Authorize files"),
        ),
        Confirmation::Relink => (
            text(language, "重新指向原件", "Relink original"),
            text(language, "重新指向会更新这份资源在所有工作区和收藏中的引用。请在接下来的文件选择器中明确选择原件。若原件已在资料库中，后续将另行确认是否合并引用。", "Relinking updates this resource in every workspace and in favorites. Choose the original in the next file picker. If it already has a separate record, you will be asked to confirm merging the references."),
            text(language, "选择原件", "Choose original"),
        ),
        Confirmation::Merge => (
            text(language, "合并原件引用", "Merge original references"),
            text(language, "所选原件已有独立记录。是否合并两份记录的所有工作区关联与收藏？此操作只合并应用引用，原文件不变。取消将保留现状。", "The selected original already has a separate record. Merge both records' workspace references and favorites? Only app references are merged; the original file stays unchanged. Cancel keeps the current records."),
            text(language, "合并引用", "Merge references"),
        ),
        Confirmation::Restore => (
            text(language, "恢复本地备份", "Restore local backup"),
            text(language, "恢复会替换当前工作区与收藏。恢复前自动备份当前数据；恢复后需重新选择管理位置。源文件不会被改变。", "Restoring replaces the current workspaces and favorites. Current data is backed up automatically first. You must reauthorize locations or individual files afterward. Original files are not changed."),
            text(language, "选择备份", "Choose backup"),
        ),
    };
    DialogCopy {
        title,
        body,
        confirm,
        cancel: text(language, "取消", "Cancel"),
    }
}

fn single_line(value: &str) -> String {
    // 文件和工作区可含换行；确认中始终作为单条名称展示，避免混成其他提示行。
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn impact_details(
    name: &str,
    authorized: bool,
    workspaces: &[RelinkWorkspace],
    favorite: bool,
    language: UiLanguage,
) -> String {
    let name = if authorized {
        single_line(name)
    } else {
        text(language, "未授权的文件", "Unauthorized file").into()
    };
    let mut lines = vec![
        format!("{}{}", text(language, "资料：", "File: "), name),
        format!(
            "{}{}",
            text(language, "受影响工作区：", "Affected workspaces: "),
            workspaces.len()
        ),
    ];
    if workspaces.is_empty() {
        lines.push(text(language, "（未加入工作区）", "(Not in a workspace)").into());
    } else {
        lines.extend(
            workspaces
                .iter()
                .map(|workspace| format!("• {}", single_line(&workspace.name))),
        );
    }
    lines.push(
        if favorite {
            text(
                language,
                "已收藏，收藏将保留",
                "Favorited; the favorite will be kept",
            )
        } else {
            text(language, "未收藏", "Not favorited")
        }
        .into(),
    );
    lines.join("\n")
}

fn impact_text(impact: &RelinkImpact, language: UiLanguage) -> String {
    impact_details(
        &impact.name,
        impact.authorized,
        &impact.workspaces,
        impact.favorite,
        language,
    )
}

pub fn relink_body(review: &RelinkReview, language: UiLanguage) -> String {
    format!(
        "{}\n\n{}",
        impact_text(&review.source, language),
        confirmation(Confirmation::Relink, language).body
    )
}

pub fn merge_body(review: &RelinkReview, language: UiLanguage) -> String {
    let selected = review
        .selected_path()
        .map(|path| single_line(&path.to_string_lossy()))
        .unwrap_or_default();
    let target = review
        .target
        .as_ref()
        .map(|impact| impact_text(impact, language))
        .unwrap_or_default();
    format!(
        "{}{}\n\n{}\n{}\n\n{}\n{}\n\n{}",
        text(language, "所选原件：", "Selected original: "),
        selected,
        text(language, "正在修复的记录", "Record being repaired"),
        impact_text(&review.source, language),
        text(
            language,
            "所选原件的已有记录",
            "Existing record for the selected file"
        ),
        target,
        confirmation(Confirmation::Merge, language).body
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relink_impact_lists_every_workspace_and_favorite_in_both_languages() {
        let workspaces = vec![
            RelinkWorkspace {
                id: "a".into(),
                name: "客户 A".into(),
            },
            RelinkWorkspace {
                id: "b".into(),
                name: "合同\n进度".into(),
            },
        ];
        for language in [UiLanguage::Zh, UiLanguage::En] {
            let body = impact_details("原件\n合同.docx", true, &workspaces, true, language);
            assert!(body.contains("原件 合同.docx"));
            assert!(body.contains("• 客户 A\n• 合同 进度"));
            assert!(body.contains("2"));
            assert!(body.contains(text(
                language,
                "已收藏，收藏将保留",
                "Favorited; the favorite will be kept"
            )));
            assert!(!body.contains("\na\n") && !body.contains("\nb\n"));
        }
    }

    #[test]
    fn unauthorized_impact_hides_cached_filename_but_preserves_named_relationships() {
        for language in [UiLanguage::Zh, UiLanguage::En] {
            let body = impact_details("private-employee-name.docx", false, &[], false, language);
            assert!(!body.contains("private-employee-name"));
            assert!(body.contains(text(language, "未授权的文件", "Unauthorized file")));
            assert!(body.contains(text(language, "（未加入工作区）", "(Not in a workspace)")));
        }
    }

    #[test]
    fn real_relink_reviews_feed_named_impacts_and_selected_path_into_bilingual_dialogs() {
        use std::{fs, path::PathBuf};
        use worktwill_core::{capture_import_selections, model::*, Store};

        // 原生crate无tempfile依赖；只创建独有目录，清理范围由本测试持有的句柄限定。
        struct TempDir(PathBuf);
        impl Drop for TempDir {
            fn drop(&mut self) {
                let cleanup = fs::remove_dir_all(&self.0);
                if !std::thread::panicking() {
                    cleanup.expect("remove only this test's own temporary directory");
                }
            }
        }
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = PathBuf::from(approved)
            .canonicalize()
            .unwrap()
            .join(format!("dialog-relink-review-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let directory = TempDir(directory);
        let source_path = directory.0.join("source-original.txt");
        let target_path = directory.0.join("target-original.txt");
        fs::write(&source_path, b"synthetic source original").unwrap();
        fs::write(&target_path, b"synthetic target original").unwrap();
        let store = Store::open(directory.0.join("app/catalog.sqlite3")).unwrap();
        let imported = store
            .import_files(
                &[
                    source_path.to_string_lossy().into_owned(),
                    target_path.to_string_lossy().into_owned(),
                ],
                None,
                true,
            )
            .unwrap();
        assert_eq!(imported.items.len(), 2);
        assert!(imported.items.iter().all(|item| item.status == "added"));
        let source_id = imported.items[0].resource_id.clone().unwrap();
        let target_id = imported.items[1].resource_id.clone().unwrap();
        let mutate = |mutation| {
            store
                .mutate(MutationRequest {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    mutation,
                })
                .unwrap()
        };
        let source_workspace = mutate(Mutation::CreateWorkspace {
            name: "原记录_源工作区".into(),
            resource_ids: Some(vec![source_id.clone()]),
        })
        .id
        .unwrap();
        let target_workspace = mutate(Mutation::CreateWorkspace {
            name: "目标记录_已有工作区".into(),
            resource_ids: Some(vec![target_id.clone()]),
        })
        .id
        .unwrap();
        mutate(Mutation::Favorite {
            id: target_id.clone(),
            value: true,
        });
        let first = store.review_relink(&source_id).unwrap();
        assert_eq!(first.source.resource_id, source_id);
        assert!(!first.source.favorite);
        assert!(first.target.is_none() && first.selected_path().is_none());
        let selections = capture_import_selections(&[target_path.to_string_lossy().into_owned()]);
        let merge = store
            .review_relink_selection(&first, &selections[0])
            .unwrap();
        assert_eq!(merge.source.resource_id, source_id);
        assert_eq!(merge.target.as_ref().unwrap().resource_id, target_id);
        assert!(merge.target.as_ref().unwrap().favorite);
        let selected_path = target_path.canonicalize().unwrap();
        assert_eq!(merge.selected_path(), Some(selected_path.as_path()));
        for language in [UiLanguage::Zh, UiLanguage::En] {
            let first_body = relink_body(&first, language);
            assert!(first_body.contains("source-original.txt"));
            assert!(first_body.contains("• 原记录_源工作区"));
            assert!(!first_body.contains("目标记录_已有工作区"));
            assert!(!first_body.contains("target-original.txt"));
            assert!(first_body.contains(text(
                language,
                "受影响工作区：1",
                "Affected workspaces: 1"
            )));
            assert!(first_body.contains(text(language, "未收藏", "Not favorited")));
            assert!(!first_body.contains(text(
                language,
                "已收藏，收藏将保留",
                "Favorited; the favorite will be kept"
            )));

            let merge_body = merge_body(&merge, language);
            assert!(merge_body.contains(&format!(
                "{}{}",
                text(language, "所选原件：", "Selected original: "),
                selected_path.display()
            )));
            assert!(merge_body.contains(text(language, "正在修复的记录", "Record being repaired")));
            assert!(merge_body.contains(text(
                language,
                "所选原件的已有记录",
                "Existing record for the selected file"
            )));
            assert!(merge_body.contains("source-original.txt"));
            assert!(merge_body.contains("target-original.txt"));
            assert!(merge_body.contains("• 原记录_源工作区"));
            assert!(merge_body.contains("• 目标记录_已有工作区"));
            assert_eq!(
                merge_body
                    .matches(text(language, "受影响工作区：1", "Affected workspaces: 1"))
                    .count(),
                2
            );
            assert!(merge_body.contains(text(language, "未收藏", "Not favorited")));
            assert!(merge_body.contains(text(
                language,
                "已收藏，收藏将保留",
                "Favorited; the favorite will be kept"
            )));
            for id in [&source_id, &target_id, &source_workspace, &target_workspace] {
                assert!(!first_body.contains(id.as_str()));
                assert!(!merge_body.contains(id.as_str()));
            }
        }
        // review与文案生成没有执行合并，也不能改写两份真实源文件。
        for (workspace, resource_id, favorite) in [
            (&source_workspace, &source_id, false),
            (&target_workspace, &target_id, true),
        ] {
            let resources = store
                .resources(ResourceQuery {
                    workspace_id: Some(workspace.clone()),
                    kind: "file".into(),
                    mode: "modified".into(),
                    offset: 0,
                })
                .unwrap();
            assert_eq!(resources.items.len(), 1);
            assert_eq!(resources.items[0].id, *resource_id);
            assert_eq!(resources.items[0].favorite, favorite);
        }
        assert_eq!(fs::read(source_path).unwrap(), b"synthetic source original");
        assert_eq!(fs::read(target_path).unwrap(), b"synthetic target original");
    }
}
