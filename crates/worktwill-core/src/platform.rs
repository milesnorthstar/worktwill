use crate::model::{AppError, Result};
use std::fs::Metadata;
use std::path::Path;

#[cfg(unix)]
pub fn identity(_path: &Path, metadata: &Metadata) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    // 仅物理身份连续时自动恢复；不以 Bookmark 的同路径解析推断安全保存。
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt as MacMetadataExt;
        Ok(format!(
            "unix:{}:{}:{}:{}",
            metadata.dev(),
            metadata.ino(),
            metadata.st_birthtime(),
            metadata.st_birthtime_nsec()
        ))
    }
    #[cfg(not(target_os = "macos"))]
    Ok(format!("unix:{}:{}", metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
pub fn identity(path: &Path, _metadata: &Metadata) -> Result<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        Storage::FileSystem::*,
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let handle = CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            return Err(AppError::new(
                "resource_unavailable",
                "无法读取文件身份",
                "The file identity could not be read.",
            ));
        }
        let mut info: FILE_ID_INFO = std::mem::zeroed();
        let mut basic: FILE_BASIC_INFO = std::mem::zeroed();
        let ok = GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            &mut info as *mut _ as *mut _,
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        );
        // File ID与创建时间必须来自同一句柄，不能拼接保存前路径采样的旧metadata。
        let basic_ok = GetFileInformationByHandleEx(
            handle,
            FileBasicInfo,
            &mut basic as *mut _ as *mut _,
            std::mem::size_of::<FILE_BASIC_INFO>() as u32,
        );
        CloseHandle(handle);
        if ok == 0 || basic_ok == 0 {
            return Err(AppError::new(
                "identity_unconfirmed",
                "当前文件系统身份尚不可用",
                "File identity is not available on this file system.",
            ));
        }
        Ok(format!(
            "win:{}:{:x?}:{}",
            info.VolumeSerialNumber, info.FileId.Identifier, basic.CreationTime
        ))
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn identity_does_not_combine_current_file_id_with_stale_creation_time() {
        let dir = tempfile::Builder::new()
            .prefix("identity-same-handle-")
            .tempdir_in(std::env::var_os("WORKTWILL_TEST_ROOT").unwrap())
            .unwrap();
        let path = dir.path().join("original.txt");
        std::fs::write(&path, b"original").unwrap();
        let stale = path.metadata().unwrap();
        let original = identity(&path, &stale).unwrap();
        std::fs::rename(&path, dir.path().join("retained.txt")).unwrap();
        std::fs::write(&path, b"replacement").unwrap();
        use std::os::windows::{fs::MetadataExt, io::AsRawHandle};
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        let creation = stale.creation_time() + 10_000_000;
        let time = windows_sys::Win32::Foundation::FILETIME {
            dwLowDateTime: creation as u32,
            dwHighDateTime: (creation >> 32) as u32,
        };
        unsafe {
            assert_ne!(
                windows_sys::Win32::Storage::FileSystem::SetFileTime(
                    file.as_raw_handle(),
                    &time,
                    std::ptr::null(),
                    std::ptr::null()
                ),
                0
            );
        }
        drop(file);
        let current = path.metadata().unwrap();
        assert_ne!(identity(&path, &current).unwrap(), original);
        assert_eq!(
            identity(&path, &stale).unwrap(),
            identity(&path, &current).unwrap()
        );
    }
}

pub fn safe_open(path: &Path) -> Result<()> {
    let ext = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    // 首个闭环只打开明确的资料类型，避免脚本或可执行文件进入默认启动路径。
    const DOCUMENTS: &[&str] = &[
        "doc", "docx", "xls", "xlsx", "ppt", "pptx", "pdf", "txt", "md", "csv", "png", "jpg",
        "jpeg", "gif", "webp", "bmp", "rtf", "odt", "ods", "odp",
    ];
    if !DOCUMENTS.contains(&ext.as_str()) {
        return Err(AppError::new(
            "unsupported_file_type",
            "此类型暂仅支持显示原始位置",
            "For this file type, only showing its original location is supported.",
        ));
    }
    Ok(())
}
