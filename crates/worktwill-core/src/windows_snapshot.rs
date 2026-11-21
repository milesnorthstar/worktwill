//! 同一文件句柄上的物理身份及可选NTFS锚点；读取绝不隐式创建标记。
use crate::model::*;
use std::{os::windows::ffi::OsStrExt, path::Path};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::*,
    System::{Ioctl::*, IO::DeviceIoControl},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectSnapshot {
    pub identity: String,
    pub object_key: String,
    pub volume: u64,
    pub anchor: Option<[u8; 16]>,
    pub size: u64,
    pub modified_ms: i64,
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn error() -> AppError {
    AppError::new(
        "identity_unconfirmed",
        &format!(
            "无法确认文件身份或追踪标记（系统错误{}）",
            unsafe { GetLastError() }
        ),
        "The file identity or tracking marker could not be confirmed.",
    )
}

pub(crate) fn snapshot(path: &Path, create_anchor: bool) -> Result<ObjectSnapshot> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let handle = Handle(CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        ));
        if handle.0 == INVALID_HANDLE_VALUE {
            return Err(error());
        }
        let mut id: FILE_ID_INFO = std::mem::zeroed();
        let mut basic: FILE_BASIC_INFO = std::mem::zeroed();
        let mut standard: FILE_STANDARD_INFO = std::mem::zeroed();
        for (class, buffer, length) in [
            (
                FileIdInfo,
                &mut id as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<FILE_ID_INFO>(),
            ),
            (
                FileBasicInfo,
                &mut basic as *mut _ as *mut _,
                std::mem::size_of::<FILE_BASIC_INFO>(),
            ),
            (
                FileStandardInfo,
                &mut standard as *mut _ as *mut _,
                std::mem::size_of::<FILE_STANDARD_INFO>(),
            ),
        ] {
            if GetFileInformationByHandleEx(handle.0, class, buffer, length as u32) == 0 {
                return Err(error());
            }
        }
        if basic.FileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0
            || standard.NumberOfLinks != 1
        {
            return Err(error());
        }
        let mut filesystem = [0u16; 32];
        if GetVolumeInformationByHandleW(
            handle.0,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            filesystem.as_mut_ptr(),
            filesystem.len() as u32,
        ) == 0
        {
            return Err(error());
        }
        let name = String::from_utf16_lossy(
            &filesystem[..filesystem
                .iter()
                .position(|v| *v == 0)
                .unwrap_or(filesystem.len())],
        );
        if name != "NTFS" {
            return Err(AppError::new(
                "tracking_unsupported",
                "文档追踪仅支持NTFS",
                "Document tracking requires NTFS.",
            ));
        }
        let mut object: FILE_OBJECTID_BUFFER = std::mem::zeroed();
        let mut returned = 0;
        let control = if create_anchor {
            FSCTL_CREATE_OR_GET_OBJECT_ID
        } else {
            FSCTL_GET_OBJECT_ID
        };
        let ok = DeviceIoControl(
            handle.0,
            control,
            std::ptr::null(),
            0,
            &mut object as *mut _ as *mut _,
            std::mem::size_of::<FILE_OBJECTID_BUFFER>() as u32,
            &mut returned,
            std::ptr::null_mut(),
        );
        let anchor = if ok != 0 && returned >= 16 {
            Some(object.ObjectId)
        } else {
            let code = GetLastError();
            // 已成功打开对象；NTFS无锚点可能返回FILE_NOT_FOUND/NOT_FOUND/OBJECT_NOT_FOUND。
            // 访问拒绝等其他错误不能伪装成未启用。
            if create_anchor || !matches!(code, 2 | 1168 | 4312) {
                return Err(error());
            }
            None
        };
        let object_key = format!("win:{}:{:x?}", id.VolumeSerialNumber, id.FileId.Identifier);
        Ok(ObjectSnapshot {
            identity: format!("{}:{}", object_key, basic.CreationTime),
            object_key,
            volume: id.VolumeSerialNumber,
            anchor,
            size: standard.EndOfFile as u64,
            modified_ms: basic.LastWriteTime / 10_000 - 11_644_473_600_000,
        })
    }
}
