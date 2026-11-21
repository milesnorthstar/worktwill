use crate::{model::*, Store};
#[cfg(target_os = "macos")]
use rusqlite::params;
use rusqlite::Connection;
#[cfg(target_os = "macos")]
use rusqlite::OptionalExtension;
use std::path::Path;

pub(crate) fn is_locator_error(error: &AppError) -> bool {
    matches!(
        error.code.as_str(),
        "bookmark_create_failed" | "bookmark_resolve_failed"
    )
}

// 仅域内恢复可以构造；Bookmark本身从不作为授权或资源身份。
#[cfg(target_os = "macos")]
pub(crate) struct FileRecovery {
    pub resource_id: String,
    pub identity: String,
    pub location: String,
    pub bookmark: Vec<u8>,
    pub refreshed_bookmark: Vec<u8>,
    pub permission_version: u32,
    pub revision: u32,
}
pub(crate) trait RecoveryGuard {
    fn identity(&self) -> &str;
    fn validate(&self, db: &Connection) -> Result<()>;
    #[cfg(target_os = "macos")]
    fn updated_bookmark(&self) -> Option<&[u8]> {
        None
    }
}
#[cfg(target_os = "macos")]
impl RecoveryGuard for FileRecovery {
    fn identity(&self) -> &str {
        &self.identity
    }
    fn validate(&self, db: &Connection) -> Result<()> {
        FileRecovery::validate(self, db)
    }
    #[cfg(target_os = "macos")]
    fn updated_bookmark(&self) -> Option<&[u8]> {
        Some(&self.refreshed_bookmark)
    }
}
#[cfg(target_os = "macos")]
impl FileRecovery {
    pub(crate) fn validate(&self, db: &Connection) -> Result<()> {
        let valid:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM resources r JOIN file_grants g ON g.resource_id=r.id JOIN file_locators l ON l.resource_id=r.id WHERE r.id=?1 AND r.kind='file' AND r.identity=?2 AND r.location=?3 AND g.active=1 AND l.identity=?2 AND l.bookmark=?4) AND (SELECT value FROM settings WHERE key='permission_version')=?5 AND (SELECT value FROM settings WHERE key='revision')=?6",params![self.resource_id,self.identity,self.location,self.bookmark,self.permission_version,self.revision],|r|r.get(0))?;
        if !valid {
            return Err(AppError::new(
                "permission_changed",
                "文件或授权已变化，请重新尝试",
                "The file or its authorization has changed. Try again.",
            ));
        }
        Ok(())
    }
}

impl Store {
    pub(crate) fn capture_import_locator(
        &self,
        path: &Path,
        identity: &str,
        grant_selected: bool,
        recovery: Option<&dyn RecoveryGuard>,
    ) -> Result<Option<Vec<u8>>> {
        #[cfg(target_os = "macos")]
        {
            if let Some(bookmark) = recovery.and_then(|guard| guard.updated_bookmark()) {
                return Ok(Some(bookmark.to_vec()));
            }
            let needed = if grant_selected {
                true
            } else {
                let _guard = self.gate.read().map_err(|_| {
                    AppError::new(
                        "internal_error",
                        "状态锁不可用",
                        "App state is unavailable.",
                    )
                })?;
                self.connection()?.query_row("SELECT EXISTS(SELECT 1 FROM resources r JOIN file_grants g ON g.resource_id=r.id WHERE r.identity=?1 AND g.active=1 AND NOT EXISTS(SELECT 1 FROM file_locators l WHERE l.resource_id=r.id AND l.identity=r.identity))",[identity],|r|r.get::<_,bool>(0))?
            };
            if needed {
                crate::macos_bookmark::create(path).map(Some)
            } else {
                Ok(None)
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (path, identity, grant_selected, recovery);
            Ok(None)
        }
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn recover_single_file(&self, rid: &str) -> Result<bool> {
        let recovery = {
            let _guard = self.gate.read().map_err(|_| {
                AppError::new(
                    "internal_error",
                    "状态锁不可用",
                    "App state is unavailable.",
                )
            })?;
            let mut connection = self.connection()?;
            let db = connection.transaction()?;
            let mut recovery=db.query_row("SELECT r.identity,r.location,l.bookmark FROM resources r JOIN file_grants g ON g.resource_id=r.id JOIN file_locators l ON l.resource_id=r.id AND l.identity=r.identity WHERE r.id=? AND r.kind='file' AND g.active=1",[rid],|row|Ok(FileRecovery{resource_id:rid.into(),identity:row.get(0)?,location:row.get(1)?,bookmark:row.get(2)?,refreshed_bookmark:Vec::new(),permission_version:0,revision:0})).optional()?;
            if let Some(recovery) = &mut recovery {
                recovery.permission_version = db.query_row(
                    "SELECT value FROM settings WHERE key='permission_version'",
                    [],
                    |r| r.get(0),
                )?;
                recovery.revision =
                    db.query_row("SELECT value FROM settings WHERE key='revision'", [], |r| {
                        r.get(0)
                    })?;
            }
            recovery
        };
        let Some(mut recovery) = recovery else {
            return Ok(false);
        };
        // Foundation解析与创建都在SQLite写锁和Store gate之外；提交时复核快照及对象。
        let (candidate, stale) = crate::macos_bookmark::resolve(&recovery.bookmark)?;
        let candidate = candidate.canonicalize()?;
        let metadata = candidate.metadata()?;
        if !metadata.is_file()
            || crate::platform::identity(&candidate, &metadata)? != recovery.identity
        {
            return Err(AppError::new(
                "identity_unconfirmed",
                "原件身份无法确认，请重新指向",
                "The original file identity could not be confirmed. Relink it.",
            ));
        }
        recovery.refreshed_bookmark = if stale {
            crate::macos_bookmark::create(&candidate)?
        } else {
            recovery.bookmark.clone()
        };
        self.import_one_checked(
            &candidate,
            None,
            false,
            Some(&recovery.identity),
            Some(&recovery),
        )?;
        Ok(true)
    }
}
