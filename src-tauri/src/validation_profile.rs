//! 显式原生验收配置；生产默认仍使用系统应用数据目录。
use std::{
    io,
    path::{Path, PathBuf},
};

pub fn database(default: impl FnOnce() -> io::Result<PathBuf>) -> io::Result<PathBuf> {
    match std::env::var_os("WORKTWILL_VALIDATION_DATA_DIR") {
        None => default().map(|dir| dir.join("worktwill.sqlite3")),
        Some(directory) => {
            let root = std::env::var_os("WORKTWILL_TEST_ROOT")
                .ok_or_else(|| io::Error::other("Validation requires WORKTWILL_TEST_ROOT"))?;
            checked_directory(Path::new(&root), Path::new(&directory))
                .map(|dir| dir.join("worktwill.sqlite3"))
        }
    }
}

fn checked_directory(root: &Path, directory: &Path) -> io::Result<PathBuf> {
    if !root.is_absolute() || !directory.is_absolute() {
        return Err(io::Error::other("Validation paths must be absolute"));
    }
    let root = root.canonicalize()?;
    let directory = directory.canonicalize()?;
    if directory == root || !directory.starts_with(&root) || !directory.is_dir() {
        return Err(io::Error::other(
            "Validation data must be an existing child directory of the confirmed test root",
        ));
    }
    Ok(directory)
}

/// 只在显式隔离验收配置下记录计数，不写查询、名称或路径。
pub fn search_execution(request: u32, elapsed_ms: f64, counts: (usize, usize)) {
    use std::io::Write;
    let Some(directory) = std::env::var_os("WORKTWILL_VALIDATION_DATA_DIR") else {
        return;
    };
    let Some(root) = std::env::var_os("WORKTWILL_TEST_ROOT") else {
        return;
    };
    let Ok(directory) = checked_directory(Path::new(&root), Path::new(&directory)) else {
        return;
    };
    if let Ok(mut log) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("search-execution.jsonl"))
    {
        let _ = writeln!(
            log,
            "{}",
            serde_json::json!({"request_id":request,"elapsed_ms":elapsed_ms,"vm_progress_ticks":counts.0,"vm_interrupts":counts.1})
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_root_relative_and_outside_paths() {
        let root = PathBuf::from(std::env::var_os("WORKTWILL_TEST_ROOT").unwrap());
        assert!(checked_directory(&root, &root).is_err());
        assert!(checked_directory(&root, Path::new("relative")).is_err());
        assert!(checked_directory(&root, root.parent().unwrap()).is_err());
        assert_eq!(
            checked_directory(root.parent().unwrap(), &root).unwrap(),
            root.canonicalize().unwrap()
        );
    }
}
