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
pub fn identity(path: &Path, metadata: &Metadata) -> Result<String> {
    use std::os::windows::{ffi::OsStrExt, fs::MetadataExt};
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
        let ok = GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            &mut info as *mut _ as *mut _,
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        );
        CloseHandle(handle);
        if ok == 0 {
            return Err(AppError::new(
                "identity_unconfirmed",
                "当前文件系统身份尚不可用",
                "File identity is not available on this file system.",
            ));
        }
        Ok(format!(
            "win:{}:{:x?}:{}",
            info.VolumeSerialNumber,
            info.FileId.Identifier,
            metadata.creation_time()
        ))
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
