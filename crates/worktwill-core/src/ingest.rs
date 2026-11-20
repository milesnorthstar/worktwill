use crate::{
    model::*,
    platform, search,
    store::{bump, event, id, now},
    Store,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ImportItem {
    pub path: String,
    pub status: String,
    pub resource_id: Option<String>,
    pub message: Option<String>,
    pub message_en: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ImportResult {
    pub items: Vec<ImportItem>,
}
/// 仅供原生选择/拖入会话保存的对象快照，不由前端路径参数构造授权。
#[derive(Debug, Clone)]
pub struct ImportSelection {
    pub path: String,
    pub identity: Option<String>,
    pub capture_error: Option<AppError>,
}

pub fn capture_import_selections(paths: &[String]) -> Vec<ImportSelection> {
    paths
        .iter()
        .map(|path| {
            let captured = (|| -> Result<String> {
                let canonical = Path::new(path).canonicalize()?;
                let metadata = canonical.metadata()?;
                if !metadata.is_file() {
                    return Err(AppError::new(
                        "invalid_input",
                        "请拖入文件；文件夹请使用添加管理位置",
                        "Drop files here. For folders, use Add location.",
                    ));
                }
                platform::identity(&canonical, &metadata)
            })();
            match captured {
                Ok(identity) => ImportSelection {
                    path: path.clone(),
                    identity: Some(identity),
                    capture_error: None,
                },
                Err(error) => ImportSelection {
                    path: path.clone(),
                    identity: None,
                    capture_error: Some(error),
                },
            }
        })
        .collect()
}

impl Store {
    pub(crate) fn file_grants_db(db: &rusqlite::Connection) -> Result<Vec<Resource>> {
        let ids = db
            .prepare("SELECT resource_id FROM file_grants WHERE active=1 ORDER BY resource_id")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids.iter()
            .map(|id| Self::resource_db(db, id, false))
            .collect()
    }
    pub fn file_grants(&self) -> Result<Vec<Resource>> {
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut connection = self.connection()?;
        let db = connection.transaction()?;
        Self::file_grants_db(&db)
    }
    pub(crate) fn refresh_file_grants(&self) -> Result<()> {
        let mut locator_error = None;
        for file in self.file_grants()? {
            let refreshed = self.resolve_resource(&file.id).and_then(|(location, _)| {
                self.import_one(Path::new(&location), None, false)
                    .map(|_| ())
            });
            if let Err(error) = refreshed {
                if let Some(error) = self.apply_file_refresh_failure(&file, error)? {
                    locator_error.get_or_insert(error);
                }
            }
        }
        locator_error.map_or(Ok(()), Err)
    }

    pub(crate) fn apply_file_refresh_failure(
        &self,
        file: &Resource,
        error: AppError,
    ) -> Result<Option<AppError>> {
        if matches!(error.code.as_str(), "not_authorized" | "permission_changed") {
            return Ok(None);
        }
        if !matches!(
            error.code.as_str(),
            "resource_unavailable"
                | "identity_unconfirmed"
                | "bookmark_resolve_failed"
                | "bookmark_create_failed"
                | "root_unavailable"
        ) {
            return Err(error);
        }
        let _guard = self.gate.write().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        let current:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM resources r JOIN file_grants g ON g.resource_id=r.id WHERE r.id=?1 AND r.location=?2 AND g.active=1)",params![file.id,file.location],|r|r.get(0))?;
        if !current {
            return Ok(None);
        }
        // 原错误与状态一起复核；修复/撤权后的旧定位失败也不能继续对用户报告。
        match Self::resolve_resource_db(&tx, &file.id) {
            Ok(_) => {
                let needs_locator:bool=tx.query_row("SELECT NOT EXISTS(SELECT 1 FROM file_locators l JOIN resources r ON r.id=l.resource_id AND r.identity=l.identity WHERE r.id=?)",[&file.id],|r|r.get(0))?;
                return Ok(
                    (error.code == "bookmark_create_failed" && needs_locator).then_some(error)
                );
            }
            Err(current)
                if matches!(
                    current.code.as_str(),
                    "not_authorized" | "permission_changed"
                ) =>
            {
                return Ok(None)
            }
            Err(current)
                if matches!(
                    current.code.as_str(),
                    "resource_unavailable" | "identity_unconfirmed"
                ) => {}
            Err(current) => return Err(current),
        }
        let status = if Path::new(&file.location).exists() {
            "identity_unconfirmed"
        } else {
            "missing"
        };
        let changed = tx.execute(
            "UPDATE resources SET status=?1 WHERE id=?2 AND status<>?1",
            params![status, file.id],
        )?;
        if changed > 0 {
            bump(&tx, false)?;
        }
        tx.commit()?;
        Ok(crate::root_locator::is_root_error(&error).then_some(error))
    }
    pub fn needs_file_grant(&self, path: &Path) -> Result<bool> {
        let path = path.canonicalize()?;
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let db = self.connection()?;
        if Self::path_allowed(&db, &path)? {
            return Ok(false);
        }
        let identity = platform::identity(&path, &path.metadata()?)?;
        Ok(!db.query_row("SELECT EXISTS(SELECT 1 FROM resources r JOIN file_grants f ON f.resource_id=r.id WHERE r.identity=? AND f.active=1)",[identity],|r|r.get::<_,bool>(0))?)
    }
    // 调用者必须由原生拖放事件/选择器取得路径；逐项授权决定不得变成通用前端路径写接口。
    pub fn import_files(
        &self,
        paths: &[String],
        workspace: Option<&str>,
        grant_selected: bool,
    ) -> Result<ImportResult> {
        if paths.len() > 1000 {
            return Err(AppError::new(
                "invalid_input",
                "每次最多添加 1000 项",
                "Add up to 1,000 items at a time.",
            ));
        }
        let mut items = Vec::new();
        for path in paths {
            let result = self.import_one(Path::new(path), workspace, grant_selected);
            items.push(match result {
                Ok((rid, existing)) => ImportItem {
                    path: path.clone(),
                    status: if existing { "already_present" } else { "added" }.into(),
                    resource_id: Some(rid),
                    message: None,
                    message_en: None,
                },
                Err(error) => ImportItem {
                    path: path.clone(),
                    status: if error.code == "not_authorized" {
                        "not_authorized"
                    } else {
                        "failed"
                    }
                    .into(),
                    resource_id: None,
                    message: Some(error.message),
                    message_en: Some(error.message_en),
                },
            });
        }
        Ok(ImportResult { items })
    }
    pub fn import_files_checked(
        &self,
        selections: &[ImportSelection],
        workspace: Option<&str>,
        grant_selected: bool,
    ) -> Result<ImportResult> {
        if selections.len() > 1000 {
            return Err(AppError::new(
                "invalid_input",
                "每次最多添加 1000 项",
                "Add up to 1,000 items at a time.",
            ));
        }
        let items = selections
            .iter()
            .map(|selection| {
                let result = if let Some(error) = &selection.capture_error {
                    Err(error.clone())
                } else if let Some(identity) = &selection.identity {
                    self.import_one_checked(
                        Path::new(&selection.path),
                        workspace,
                        grant_selected,
                        Some(identity),
                        None,
                    )
                } else {
                    Err(AppError::new(
                        "identity_unconfirmed",
                        "无法确认拖入文件身份，请重新拖入",
                        "The dropped file identity could not be confirmed. Drop it again.",
                    ))
                };
                match result {
                    Ok((rid, existing)) => ImportItem {
                        path: selection.path.clone(),
                        status: if existing { "already_present" } else { "added" }.into(),
                        resource_id: Some(rid),
                        message: None,
                        message_en: None,
                    },
                    Err(error) => ImportItem {
                        path: selection.path.clone(),
                        status: if error.code == "not_authorized" {
                            "not_authorized"
                        } else {
                            "failed"
                        }
                        .into(),
                        resource_id: None,
                        message: Some(error.message),
                        message_en: Some(error.message_en),
                    },
                }
            })
            .collect();
        Ok(ImportResult { items })
    }

    pub(crate) fn import_one(
        &self,
        path: &Path,
        workspace: Option<&str>,
        grant_selected: bool,
    ) -> Result<(String, bool)> {
        self.import_one_checked(path, workspace, grant_selected, None, None)
    }

    pub(crate) fn import_one_checked(
        &self,
        path: &Path,
        workspace: Option<&str>,
        grant_selected: bool,
        expected_identity: Option<&str>,
        recovery: Option<&dyn crate::file_locator::RecoveryGuard>,
    ) -> Result<(String, bool)> {
        let path = path.canonicalize()?;
        let meta = path.metadata()?;
        if !meta.is_file() {
            return Err(AppError::new(
                "invalid_input",
                "请拖入文件；文件夹请使用添加管理位置",
                "Drop files here. For folders, use Add location.",
            ));
        }
        let identity = platform::identity(&path, &meta)?;
        if expected_identity.is_some_and(|expected| expected != identity) {
            return Err(AppError::new(
                "identity_unconfirmed",
                "拖入文件已被替换，请重新拖入",
                "The dropped file has been replaced. Drop it again.",
            ));
        }
        if recovery.is_some_and(|recovery| recovery.identity() != identity) {
            return Err(AppError::new(
                "identity_unconfirmed",
                "定位候选与原件身份不符",
                "The located candidate does not match the original file identity.",
            ));
        }
        let name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
            AppError::new(
                "invalid_input",
                "文件名称编码不受支持",
                "The filename encoding is not supported.",
            )
        })?;
        let locator = self.capture_import_locator(&path, &identity, grant_selected, recovery)?;
        let _guard = self.gate.write().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        if let Some(recovery) = recovery {
            recovery.validate(&tx)?;
        }
        if let Some(wid) = workspace {
            if !tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?)",
                [wid],
                |r| r.get::<_, bool>(0),
            )? {
                return Err(AppError::new(
                    "conflict",
                    "工作区已不存在",
                    "The workspace no longer exists.",
                ));
            }
        }
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM resources WHERE identity=?",
                [&identity],
                |r| r.get(0),
            )
            .optional()?;
        let previous: Option<(f64, f64)> = if let Some(rid) = &existing {
            Some(tx.query_row(
                "SELECT modified_at,size FROM resources WHERE id=?",
                [rid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?)
        } else {
            None
        };
        let rid = existing.clone().unwrap_or_else(id);
        let granted: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM file_grants WHERE resource_id=? AND active=1)",
            [&rid],
            |r| r.get(0),
        )?;
        let scoped = Self::path_allowed(&tx, &path)?;
        if !scoped && !granted && !grant_selected {
            return Err(AppError::new(
                "not_authorized",
                "未授权此文件",
                "This file is not authorized.",
            ));
        }
        let current_meta = path.metadata()?;
        if platform::identity(&path, &current_meta)? != identity {
            return Err(AppError::new(
                "identity_unconfirmed",
                "文件已变化，请重新拖入",
                "The file has changed. Drop it again.",
            ));
        }
        // 等待写锁期间可能再次保存，投影和活动必须采用提交时重新读取的元数据。
        let meta = current_meta;
        tx.execute(
            "UPDATE resources SET status='identity_unconfirmed' WHERE location=?1 AND id<>?2",
            params![path.to_string_lossy(), rid],
        )?;
        tx.execute("INSERT INTO resources(id,kind,name,location,identity,modified_at,size,created_at) VALUES(?1,'file',?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET name=excluded.name,location=excluded.location,modified_at=excluded.modified_at,size=excluded.size,status='available'",params![rid,name,path.to_string_lossy(),identity,crate::catalog::mtime(&meta),meta.len() as f64,now()])?;
        if !scoped && !granted && grant_selected {
            tx.execute("INSERT INTO file_grants VALUES(?1,1) ON CONFLICT(resource_id) DO UPDATE SET active=1",[&rid])?;
        }
        for root in Self::roots_db(&tx)? {
            if !Self::allowed_by_root(&tx, &root, &path)? {
                continue;
            }
            // 当前文件已在此事务复核；进行中的旧枚举收尾不能把通知发现的文件判为失联。
            tx.execute("INSERT INTO file_roots SELECT ?1,?2,generation FROM scan_roots WHERE id=?1 ON CONFLICT(root_id,resource_id) DO UPDATE SET generation=excluded.generation",params![root.id,rid])?;
        }
        search::project(
            &tx,
            &rid,
            "file",
            name,
            &path.to_string_lossy(),
            Some(crate::catalog::mtime(&meta)),
        )?;
        if let Some(bookmark) = locator {
            tx.execute("INSERT INTO file_locators VALUES(?1,?2,?3) ON CONFLICT(resource_id) DO UPDATE SET identity=excluded.identity,bookmark=excluded.bookmark",params![rid,identity,bookmark])?;
        }
        if previous.is_some_and(|(modified, size)| {
            modified != crate::catalog::mtime(&meta) || size != meta.len() as f64
        }) {
            let aid = id();
            tx.execute("INSERT INTO activities(id,kind,name,resource_id,at,source,file_modified_at) VALUES(?1,'file_modified',?5,?2,?3,'filesystem',?4)",params![aid,rid,now(),crate::catalog::mtime(&meta),name])?;
            crate::activity_origin::capture(&tx, &aid, &rid)?;
        }
        if existing.is_none() {
            let aid = id();
            tx.execute("INSERT INTO activities(id,kind,name,resource_id,at,source,file_modified_at) VALUES(?1,'file_discovered',?5,?2,?3,'filesystem',?4)",params![aid,rid,now(),crate::catalog::mtime(&meta),name])?;
            crate::activity_origin::capture(&tx, &aid, &rid)?;
        }
        let mut duplicate = existing.is_some();
        if let Some(wid) = workspace {
            let count = tx.execute(
                "INSERT OR IGNORE INTO workspace_resources VALUES(?1,?2,?3)",
                params![wid, rid, now()],
            )?;
            duplicate = count == 0;
            if count > 0 {
                event(&tx, "linked", "", Some(&rid))?;
            }
        }
        bump(&tx, grant_selected && !scoped && !granted)?;
        tx.commit()?;
        Ok((rid, duplicate))
    }
}
