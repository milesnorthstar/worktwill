//! 普通 Bookmark 只提供候选位置；身份、授权和持久化决策由领域层负责。

use crate::model::{AppError, Result};
use objc2::{rc::autoreleasepool, runtime::Bool};
use objc2_foundation::{
    NSData, NSString, NSURLBookmarkCreationOptions, NSURLBookmarkResolutionOptions, NSURL,
};
use std::path::{Path, PathBuf};

fn create_error() -> AppError {
    AppError::new(
        "bookmark_create_failed",
        "无法创建文件定位信息",
        "File location information could not be created.",
    )
}

fn resolve_error() -> AppError {
    AppError::new(
        "bookmark_resolve_failed",
        "无法恢复文件位置，请重新指向",
        "The file location could not be restored. Relink the file.",
    )
}

fn local_path(url: &NSURL) -> Result<PathBuf> {
    // isFileURL 也可能包含远端主机；不将网络 URL 转成看似本地的路径。
    let remote_host = url.host().is_some_and(|host| {
        let host = host.to_string();
        !host.is_empty() && !host.eq_ignore_ascii_case("localhost")
    });
    if !url.isFileURL() || remote_host {
        return Err(resolve_error());
    }
    let path = url.path().ok_or_else(resolve_error)?.to_string();
    if path.contains('\0') || !Path::new(&path).is_absolute() {
        return Err(resolve_error());
    }
    Ok(PathBuf::from(path))
}

pub(crate) fn create(path: &Path) -> Result<Vec<u8>> {
    let path = path
        .to_str()
        .filter(|text| Path::new(text).is_absolute() && !text.contains('\0'))
        .ok_or_else(|| {
            AppError::new(
                "invalid_input",
                "文件定位需要有效的本地绝对路径",
                "File location requires a valid local absolute path.",
            )
        })?;
    autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath(&NSString::from_str(path));
        // 不创建 security-scoped 权限；不把原生值或 NSError 原文带出适配器。
        let data = url
            .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
                NSURLBookmarkCreationOptions::empty(),
                None,
                None,
            )
            .map_err(|_| create_error())?;
        if data.is_empty() {
            return Err(create_error());
        }
        Ok(data.to_vec())
    })
}

pub(crate) fn resolve(bookmark: &[u8]) -> Result<(PathBuf, bool)> {
    if bookmark.is_empty() {
        return Err(resolve_error());
    }
    autoreleasepool(|_| {
        let data = NSData::with_bytes(bookmark);
        let mut stale = Bool::NO;
        // stale 指针仅在同步调用内借出，Objective-C 对象在池内转换成 Rust 值。
        let url = unsafe {
            NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(
                &data,
                NSURLBookmarkResolutionOptions::WithoutUI
                    | NSURLBookmarkResolutionOptions::WithoutMounting,
                None,
                &mut stale,
            )
        }
        .map_err(|_| resolve_error())?;
        Ok((local_path(&url)?, stale.as_bool()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};

    fn fixture() -> tempfile::TempDir {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT")
            .expect("confirmed disposable fixture root is required");
        tempfile::Builder::new()
            .prefix("macos-bookmark-")
            .tempdir_in(approved)
            .unwrap()
    }

    fn physical_identity(path: &Path) -> String {
        crate::platform::identity(path, &path.metadata().unwrap()).unwrap()
    }

    fn assert_points_to(bookmark: &[u8], expected: &Path) {
        let (resolved, _) = resolve(bookmark).unwrap();
        assert!(resolved.canonicalize().unwrap() == expected.canonicalize().unwrap());
    }

    fn rejected<T>(result: Result<T>) -> AppError {
        match result {
            Err(error) => error,
            // 不通过 unwrap_err 的 Debug 输出意外成功的路径或 Bookmark。
            Ok(_) => panic!("expected the adapter to reject this input"),
        }
    }

    #[test]
    fn bookmark_resolves_after_rename_and_same_volume_move() {
        let directory = fixture();
        let original = directory.path().join("合同 original.txt");
        fs::write(&original, b"synthetic bookmark fixture").unwrap();
        let identity = physical_identity(&original);
        let bookmark = create(&original).unwrap();
        assert_points_to(&bookmark, &original);

        let renamed = directory.path().join("合同 renamed.txt");
        fs::rename(&original, &renamed).unwrap();
        assert_points_to(&bookmark, &renamed);
        assert!(physical_identity(&renamed) == identity);

        let destination = directory.path().join("moved");
        fs::create_dir(&destination).unwrap();
        let moved = destination.join("合同 renamed.txt");
        fs::rename(&renamed, &moved).unwrap();
        assert_points_to(&bookmark, &moved);
        assert!(physical_identity(&moved) == identity);
    }

    #[test]
    fn directory_bookmark_resolves_rename_move_and_parent_rename() {
        let directory = fixture();
        let parent = directory.path().join("original-parent");
        fs::create_dir(&parent).unwrap();
        let original = parent.join("资料 root");
        fs::create_dir(&original).unwrap();
        fs::write(original.join("合同.txt"), b"directory fixture").unwrap();
        let identity = physical_identity(&original);
        let child_identity = physical_identity(&original.join("合同.txt"));
        let bookmark = create(&original).unwrap();
        assert_points_to(&bookmark, &original);

        let renamed = parent.join("资料 renamed");
        fs::rename(&original, &renamed).unwrap();
        assert_points_to(&bookmark, &renamed);
        assert!(physical_identity(&renamed) == identity);

        let destination = directory.path().join("destination");
        fs::create_dir(&destination).unwrap();
        let moved = destination.join("资料 renamed");
        fs::rename(&renamed, &moved).unwrap();
        assert_points_to(&bookmark, &moved);
        assert!(physical_identity(&moved) == identity);

        let renamed_parent = directory.path().join("renamed-destination");
        fs::rename(&destination, &renamed_parent).unwrap();
        let final_path = renamed_parent.join("资料 renamed");
        let (candidate, _) = resolve(&bookmark).unwrap();
        assert!(candidate.canonicalize().unwrap() == final_path.canonicalize().unwrap());
        assert!(candidate.is_dir());
        assert!(physical_identity(&candidate) == identity);
        assert!(physical_identity(&candidate.join("合同.txt")) == child_identity);
        assert!(fs::read(candidate.join("合同.txt")).unwrap() == b"directory fixture");
    }

    #[test]
    fn directory_copy_does_not_receive_the_original_bookmark_identity() {
        let directory = fixture();
        let original = directory.path().join("original");
        let copy = directory.path().join("copy");
        fs::create_dir(&original).unwrap();
        fs::write(original.join("合同.txt"), b"same contents").unwrap();
        let identity = physical_identity(&original);
        let child_identity = physical_identity(&original.join("合同.txt"));
        let bookmark = create(&original).unwrap();

        fs::create_dir(&copy).unwrap();
        fs::copy(original.join("合同.txt"), copy.join("合同.txt")).unwrap();
        assert!(physical_identity(&copy) != identity);
        assert!(physical_identity(&copy.join("合同.txt")) != child_identity);
        assert_points_to(&bookmark, &original);
        let (candidate, _) = resolve(&bookmark).unwrap();
        assert!(physical_identity(&candidate) == identity);
        assert!(candidate.canonicalize().unwrap() != copy.canonicalize().unwrap());
    }

    #[test]
    fn directory_replacement_candidate_requires_the_original_physical_identity() {
        let directory = fixture();
        let original = directory.path().join("managed");
        fs::create_dir(&original).unwrap();
        fs::write(original.join("合同.txt"), b"original directory").unwrap();
        let identity = physical_identity(&original);
        let bookmark = create(&original).unwrap();

        let retained = directory.path().join("retained-original");
        fs::rename(&original, &retained).unwrap();
        fs::create_dir(&original).unwrap();
        fs::write(original.join("合同.txt"), b"replacement directory").unwrap();
        assert!(physical_identity(&original) != identity);

        let (candidate, _) = resolve(&bookmark).unwrap();
        let candidate = candidate.canonicalize().unwrap();
        // API若回退到旧路径，领域层必须拒绝新目录；候选路径本身不是身份证据。
        if candidate == original.canonicalize().unwrap() {
            assert!(physical_identity(&candidate) != identity);
        } else {
            assert!(candidate == retained.canonicalize().unwrap());
            assert!(physical_identity(&candidate) == identity);
            assert!(fs::read(candidate.join("合同.txt")).unwrap() == b"original directory");
        }
    }

    #[test]
    fn bookmark_resolves_in_a_fresh_process() {
        const CHILD_DIRECTORY: &str = "WORKTWILL_BOOKMARK_CHILD_DIRECTORY";
        if let Some(directory) = std::env::var_os(CHILD_DIRECTORY) {
            // 子进程只读取父测试独占 TempDir 中的样本，不创建或修改文件。
            let approved = PathBuf::from(
                std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root"),
            )
            .canonicalize()
            .unwrap();
            let directory = PathBuf::from(directory).canonicalize().unwrap();
            assert!(directory != approved && directory.starts_with(&approved));
            let bookmark = fs::read(directory.join("locator.data")).unwrap();
            assert_points_to(&bookmark, &directory.join("renamed.txt"));
            return;
        }

        let directory = fixture();
        let original = directory.path().join("original.txt");
        let renamed = directory.path().join("renamed.txt");
        fs::write(&original, b"fresh process fixture").unwrap();
        fs::write(
            directory.path().join("locator.data"),
            create(&original).unwrap(),
        )
        .unwrap();
        fs::rename(&original, &renamed).unwrap();

        let test_name = std::thread::current()
            .name()
            .expect("test harness thread name")
            .to_owned();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &test_name, "--test-threads=1"])
            .env(CHILD_DIRECTORY, directory.path())
            .output()
            .unwrap();
        assert!(
            output.status.success() && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "fresh-process bookmark check failed"
        );
    }

    #[test]
    fn same_name_recreation_does_not_supply_identity_evidence() {
        let directory = fixture();
        let original = directory.path().join("same-name.txt");
        fs::write(&original, b"original physical object").unwrap();
        let identity = physical_identity(&original);
        let bookmark = create(&original).unwrap();
        // 保留旧对象的句柄，避免删除后 inode 立即复用影响负例。
        let original_handle = fs::File::open(&original).unwrap();
        fs::remove_file(&original).unwrap();
        fs::write(&original, b"different physical object").unwrap();
        assert!(physical_identity(&original) != identity);

        match resolve(&bookmark) {
            Ok((candidate, _)) => {
                // 解析成功仍只是候选；调用者必须拒绝它继承旧对象的身份和关系。
                assert!(candidate.canonicalize().unwrap() == original.canonicalize().unwrap());
                assert!(physical_identity(&candidate) != identity);
            }
            Err(error) => assert_eq!(error.code, "bookmark_resolve_failed"),
        }
        drop(original_handle);
    }

    #[test]
    fn invalid_data_and_nonlocal_urls_return_short_bilingual_errors() {
        for data in [b"".as_slice(), b"not a bookmark".as_slice()] {
            let error = rejected(resolve(data));
            assert_eq!(error.code, "bookmark_resolve_failed");
            assert!(!error.message.is_empty() && !error.message_en.is_empty());
        }
        assert_eq!(
            rejected(create(Path::new("relative.txt"))).code,
            "invalid_input"
        );
        autoreleasepool(|_| {
            for value in ["https://example.com/file", "file://example.com/file"] {
                let url = NSURL::URLWithString(&NSString::from_str(value)).unwrap();
                assert_eq!(rejected(local_path(&url)).code, "bookmark_resolve_failed");
            }
        });
    }
}
