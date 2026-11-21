//! 目录追踪同意、逐文件锚点和续接事务；与人工重新指向保持独立。
use crate::{
    model::*,
    platform, search,
    store::{bump, id, now},
    windows_snapshot::{snapshot, ObjectSnapshot},
    Store,
};
use rusqlite::{params, OptionalExtension};
use std::path::Path;
use std::sync::atomic::Ordering;

fn conflict() -> AppError {
    AppError::new(
        "continuity_conflict",
        "文档绑定冲突，请确认原件",
        "Document bindings conflict. Confirm the original file.",
    )
}

impl Store {
    pub fn document_tracking_status(&self) -> Result<DocumentTrackingStatus> {
        let _guard = self.gate.read().map_err(|_| conflict())?;
        let db = self.connection()?;
        let roots=db.prepare("SELECT p.root_id,p.enabled FROM document_tracking_policies p JOIN scan_roots s ON s.id=p.root_id WHERE s.active=1")?.query_map([],|r|Ok(DocumentTrackingRoot{root_id:r.get(0)?,enabled:r.get(1)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut files = Vec::new();
        let rows=db.prepare("SELECT r.id,r.name,r.location,r.identity,a.bound_identity,f.error_json FROM resources r LEFT JOIN document_anchors a ON a.resource_id=r.id LEFT JOIN document_tracking_failures f ON f.resource_id=r.id WHERE r.kind='file' AND (r.favorite=1 OR EXISTS(SELECT 1 FROM workspace_resources wr WHERE wr.resource_id=r.id))")?.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        for (rid, name, location, identity, bound, error) in rows {
            if Self::tracking_root(&db, Path::new(&location))?.is_none() {
                continue;
            }
            // 数据库绑定不能证明当前原件仍受保护；冲突或外部替换时必须如实显示。
            let verification = snapshot(Path::new(&location), false).and_then(|current| {
                // 同一物理文件的Object ID也可能被外部工具移除或改写，不能只复验File ID。
                let anchor_matches: bool = db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM document_anchors WHERE resource_id=?1 AND volume=?2 AND anchor=?3 AND bound_identity=?4)",
                    params![rid, current.volume.to_string(), current.anchor.as_ref().map(|a| a.as_slice()), identity],
                    |r| r.get(0),
                )?;
                if current.identity == identity && (bound.is_none() || anchor_matches) {
                    Ok(())
                } else {
                    Err(conflict())
                }
            });
            let error = error
                .and_then(|s| serde_json::from_str(&s).ok())
                .or_else(|| verification.err());
            files.push(DocumentTrackingFile {
                resource_id: rid,
                name,
                established: bound.as_deref() == Some(identity.as_str()) && error.is_none(),
                error,
            });
        }
        Ok(DocumentTrackingStatus {
            supported: true,
            roots,
            files,
        })
    }

    /// 后台显式维护重要资料；失败逐文件保存，不撤回已经提交的关系。
    pub fn maintain_document_anchors(&self) -> Result<()> {
        let pending = {
            let db = self.connection()?;
            let mut pending = Vec::new();
            let rows=db.prepare("SELECT r.id,r.location FROM resources r LEFT JOIN document_anchors a ON a.resource_id=r.id WHERE r.kind='file' AND (r.favorite=1 OR EXISTS(SELECT 1 FROM workspace_resources wr WHERE wr.resource_id=r.id)) AND a.resource_id IS NULL")?.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            for (rid, location) in rows {
                if Self::tracking_root(&db, Path::new(&location))?.is_some() {
                    pending.push(rid);
                }
            }
            pending
        };
        for rid in pending {
            let outcome = self.establish_document_anchor(&rid);
            let _guard = self.gate.write().map_err(|_| conflict())?;
            let db = self.connection()?;
            match outcome {
                Ok(()) => {
                    db.execute(
                        "DELETE FROM document_tracking_failures WHERE resource_id=?",
                        [&rid],
                    )?;
                }
                Err(error) => {
                    db.execute("INSERT INTO document_tracking_failures VALUES(?1,?2) ON CONFLICT(resource_id) DO UPDATE SET error_json=excluded.error_json",params![rid,serde_json::to_string(&error).map_err(|_|conflict())?])?;
                }
            }
        }
        Ok(())
    }
    /// 原生设置入口使用；目录授权与元数据写入同意分开保存。
    pub fn set_document_tracking(&self, root_id: &str, enabled: bool) -> Result<()> {
        let _guard = self.gate.write().map_err(|_| conflict())?;
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        // 关闭是撤销写入同意，不应要求暂停/离线目录可访问。
        if !enabled {
            tx.execute(
                "UPDATE document_tracking_policies SET enabled=0 WHERE root_id=?",
                [root_id],
            )?;
            bump(&tx, true)?;
            tx.commit()?;
            return Ok(());
        }
        let root = Self::roots_db(&tx)?
            .into_iter()
            .find(|r| r.id == root_id && r.active && !r.paused)
            .ok_or_else(conflict)?;
        if !Self::root_current(&tx, root_id)? {
            return Err(conflict());
        }
        let metadata = Path::new(&root.path).metadata()?;
        let identity = platform::identity(Path::new(&root.path), &metadata)?;
        tx.execute("INSERT INTO document_tracking_policies VALUES(?1,?2,?3) ON CONFLICT(root_id) DO UPDATE SET root_identity=excluded.root_identity,enabled=excluded.enabled",params![root_id,identity,enabled])?;
        bump(&tx, true)?;
        tx.commit()?;
        Ok(())
    }

    fn tracking_root(db: &rusqlite::Connection, path: &Path) -> Result<Option<String>> {
        let enabled: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM document_tracking_policies WHERE enabled=1)",
            [],
            |r| r.get(0),
        )?;
        if !enabled {
            return Ok(None);
        }
        for root in Self::roots_db(db)? {
            if !root.active || root.paused || !Self::allowed_by_root(db, &root, path)? {
                continue;
            }
            let expected:Option<String>=db.query_row("SELECT root_identity FROM document_tracking_policies WHERE root_id=? AND enabled=1",[&root.id],|r|r.get(0)).optional()?;
            if let Some(expected) = expected {
                // 离线位置不提供追踪资格，但不能阻止其他目录或撤销同意入口读取。
                let root_path = Path::new(&root.path);
                let Ok(current) = root_path
                    .metadata()
                    .map_err(AppError::from)
                    .and_then(|metadata| platform::identity(root_path, &metadata))
                else {
                    continue;
                };
                if current == expected {
                    return Ok(Some(root.id));
                }
            }
        }
        Ok(None)
    }

    /// 建立结果以Result反馈；创建失败不回滚既有收藏/关系，也不伪造已建立状态。
    pub fn establish_document_anchor(&self, rid: &str) -> Result<()> {
        let _guard = self.gate.write().map_err(|_| conflict())?;
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        let (location,identity,important):(String,String,bool)=tx.query_row("SELECT location,identity,(favorite=1 OR EXISTS(SELECT 1 FROM workspace_resources WHERE resource_id=resources.id)) FROM resources WHERE id=? AND kind='file'",[rid],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        if !important {
            return Err(conflict());
        }
        let path = Path::new(&location);
        let root = Self::tracking_root(&tx, path)?.ok_or_else(conflict)?;
        let before = snapshot(path, false)?;
        if before.identity != identity {
            return Err(conflict());
        }
        let after = snapshot(path, true)?;
        if before.identity != after.identity {
            return Err(conflict());
        }
        let anchor = after.anchor.ok_or_else(conflict)?;
        let duplicates:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM document_anchors WHERE volume=?1 AND anchor=?2 AND resource_id<>?3)",params![after.volume.to_string(),anchor.as_slice(),rid],|r|r.get(0))?;
        if duplicates {
            return Err(conflict());
        }
        tx.execute("INSERT INTO document_anchors(resource_id,root_id,volume,anchor,bound_identity) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(resource_id) DO UPDATE SET root_id=excluded.root_id,volume=excluded.volume,anchor=excluded.anchor,bound_identity=excluded.bound_identity,binding_version=document_anchors.binding_version+1",params![rid,root,after.volume.to_string(),anchor.as_slice(),identity])?;
        tx.execute(
            "DELETE FROM document_tracking_failures WHERE resource_id=?",
            [rid],
        )?;
        // 创建标记是文件系统副作用；写库期间对象或标记变化时撤回数据库成功结果。
        Self::verify_document_snapshot(path, &after)?;
        bump(&tx, false)?;
        tx.commit()?;
        Ok(())
    }

    /// 所有候选入口复用；没有在线事件不否决持久锚点；不创建新标记。
    pub fn reconcile_document(&self, path: &Path) -> Result<bool> {
        let restore_epoch = self.scan_epoch.load(Ordering::SeqCst);
        let path = path.canonicalize()?;
        // 未启用时不读取追踪元数据，不影响普通扫描范围。
        {
            let db = self.connection()?;
            if Self::tracking_root(&db, &path)?.is_none() {
                return Ok(false);
            }
        }
        let observed = snapshot(&path, false)?;
        let Some(anchor) = observed.anchor else {
            return Ok(false);
        };
        let _guard = self.gate.write().map_err(|_| conflict())?;
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        if restore_epoch != self.scan_epoch.load(Ordering::SeqCst) {
            return Err(conflict());
        }
        let Some(root) = Self::tracking_root(&tx, &path)? else {
            return Ok(false);
        };
        let candidates=tx.prepare("SELECT resource_id,bound_identity,binding_version FROM document_anchors WHERE volume=?1 AND anchor=?2 AND root_id=?3")?.query_map(params![observed.volume.to_string(),anchor.as_slice(),root],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        if candidates.is_empty() {
            return Ok(false);
        }
        if candidates.len() != 1 {
            return Err(conflict());
        }
        let (rid, old, version) = &candidates[0];
        let actual: String =
            tx.query_row("SELECT identity FROM resources WHERE id=?", [rid], |r| {
                r.get(0)
            })?;
        if &actual != old {
            return Err(conflict());
        }
        if observed.identity == actual {
            return Ok(false);
        }
        // 创建时间只做完整复验，竞争发现另用卷+File ID；即便纯扫描记录也不合并。
        // 完整身份以冒号连接创建时间；有界范围可复用identity索引，避免每次保存全表扫描。
        let competing: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM resources WHERE identity>=?1 AND identity<?2 AND id<>?3)",
            params![
                format!("{}:", observed.object_key),
                format!("{};", observed.object_key),
                rid
            ],
            |r| r.get(0),
        )?;
        if competing {
            return Err(conflict());
        }
        Self::verify_document_snapshot(&path, &observed)?;
        crate::activity_origin::retire(&tx, rid)?;
        let removed = tx.execute(
            "UPDATE file_grants SET active=0 WHERE resource_id=? AND active=1",
            [rid],
        )?;
        tx.execute("UPDATE resources SET identity=?1,location=?2,name=?3,size=?4,modified_at=?5,status='available' WHERE id=?6 AND identity=?7",params![observed.identity,path.to_string_lossy(),path.file_name().and_then(|s|s.to_str()).ok_or_else(conflict)?,observed.size as f64,observed.modified_ms as f64,rid,old])?;
        let changed=tx.execute("UPDATE document_anchors SET bound_identity=?1,binding_version=binding_version+1 WHERE resource_id=?2 AND bound_identity=?3 AND binding_version=?4",params![observed.identity,rid,old,version])?;
        if changed != 1 {
            return Err(conflict());
        }
        tx.execute(
            "INSERT INTO resource_binding_history VALUES(?1,?2,?3,?4,?5,?6,?7,1)",
            params![
                id(),
                rid,
                old,
                observed.identity,
                root,
                anchor.as_slice(),
                now()
            ],
        )?;
        let aid = id();
        tx.execute("INSERT INTO activities(id,kind,name,resource_id,at,source,file_modified_at) SELECT ?1,'file_modified',name,id,?2,'filesystem',modified_at FROM resources WHERE id=?3",params![aid,now(),rid])?;
        crate::activity_origin::capture(&tx, &aid, rid)?;
        tx.execute("DELETE FROM file_roots WHERE resource_id=?", [rid])?;
        for scope in Self::roots_db(&tx)? {
            if Self::allowed_by_root(&tx, &scope, &path)? {
                tx.execute(
                    "INSERT INTO file_roots SELECT id,?2,generation FROM scan_roots WHERE id=?1",
                    params![scope.id, rid],
                )?;
            }
        }
        search::project(
            &tx,
            rid,
            "file",
            path.file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(conflict)?,
            &path.to_string_lossy(),
            Some(observed.modified_ms as f64),
        )?;
        Self::verify_document_snapshot(&path, &observed)?;
        bump(&tx, removed > 0)?;
        tx.commit()?;
        Ok(true)
    }
    fn verify_document_snapshot(path: &Path, expected: &ObjectSnapshot) -> Result<()> {
        if &snapshot(path, false)? != expected {
            return Err(conflict());
        }
        Ok(())
    }
}
