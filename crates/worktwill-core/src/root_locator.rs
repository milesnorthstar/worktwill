use crate::{
    file_locator::RecoveryGuard,
    model::*,
    platform,
    store::{bump, version},
    Store,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

#[derive(Clone)]
struct Anchor {
    root_id: String,
    path: String,
    identity: String,
    bookmark: Option<Vec<u8>>,
    valid: bool,
    generation: i64,
    permission: u32,
    revision: u32,
}

fn changed() -> AppError {
    AppError::new(
        "permission_changed",
        "管理位置或授权已变化，请重新尝试",
        "The managed location or its authorization has changed. Try again.",
    )
}

pub(crate) fn unavailable() -> AppError {
    AppError::new(
        "root_unavailable",
        "管理位置暂不可用，请连接原目录或重新选择管理位置",
        "The managed location is unavailable. Reconnect the original folder or select it again.",
    )
}

pub(crate) fn is_root_error(error: &AppError) -> bool {
    error.code == "root_unavailable" || crate::file_locator::is_locator_error(error)
}

// 同路径不能证明身份；拒绝根路径本身或祖先被链接替换的情况。
fn matches_directory(path: &Path, identity: &str) -> bool {
    path.canonicalize().ok().as_deref() == Some(path)
        && std::fs::symlink_metadata(path)
            .ok()
            .filter(|meta| meta.is_dir() && !meta.file_type().is_symlink())
            .and_then(|meta| platform::identity(path, &meta).ok())
            .as_deref()
            == Some(identity)
}

impl Anchor {
    fn current_scope(&self, db: &Connection) -> Result<bool> {
        Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM scan_roots s JOIN root_locators l ON l.root_id=s.id WHERE s.id=?1 AND s.path=?2 AND s.active=1 AND s.generation=?3 AND l.identity=?4 AND l.bookmark IS ?5 AND l.valid=?6) AND (SELECT value FROM settings WHERE key='permission_version')=?7",params![self.root_id,self.path,self.generation,self.identity,self.bookmark,self.valid,self.permission],|r|r.get(0))?)
    }
    fn current(&self, db: &Connection) -> Result<bool> {
        Ok(self.current_scope(db)? && version(db, "revision")? == self.revision)
    }
}

struct RootRecovery {
    anchor: Anchor,
    resource_id: String,
    identity: String,
    location: String,
    candidate: PathBuf,
}

impl RecoveryGuard for RootRecovery {
    fn identity(&self) -> &str {
        &self.identity
    }
    fn validate(&self, db: &Connection) -> Result<()> {
        // 无关索引写入只改变全局revision；目录/权限/资源快照未变时仍可安全恢复。
        if !self.anchor.current_scope(db)? {
            return Err(changed());
        }
        let same: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM resources r JOIN file_roots f ON f.resource_id=r.id WHERE r.id=?1 AND r.kind='file' AND r.identity=?2 AND r.location=?3 AND f.root_id=?4)",params![self.resource_id,self.identity,self.location,self.anchor.root_id],|r|r.get(0))?;
        let root = Store::roots_db(db)?
            .into_iter()
            .find(|root| root.id == self.anchor.root_id)
            .ok_or_else(changed)?;
        if !same || !Store::allowed_by_root(db, &root, &self.candidate)? {
            return Err(changed());
        }
        Ok(())
    }
}

impl Store {
    fn anchors_db(db: &Connection) -> Result<Vec<Anchor>> {
        let permission = version(db, "permission_version")?;
        let revision = version(db, "revision")?;
        Ok(db.prepare("SELECT s.id,s.path,l.identity,l.bookmark,l.valid,s.generation FROM scan_roots s JOIN root_locators l ON l.root_id=s.id WHERE s.active=1 ORDER BY s.id")?.query_map([],|r|Ok(Anchor{root_id:r.get(0)?,path:r.get(1)?,identity:r.get(2)?,bookmark:r.get(3)?,valid:r.get(4)?,generation:r.get(5)?,permission,revision}))?.collect::<rusqlite::Result<_>>()?)
    }

    // 启动只核对已存在锚点，不调用Foundation、不枚举目录、不为legacy追认身份。
    pub(crate) fn verify_root_locations(&self) -> Result<()> {
        let _guard = self.gate.write().map_err(|_| changed())?;
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        let mut any = false;
        for anchor in Self::anchors_db(&tx)? {
            let valid = matches_directory(Path::new(&anchor.path), &anchor.identity);
            if valid != anchor.valid {
                tx.execute(
                    "UPDATE root_locators SET valid=?1 WHERE root_id=?2",
                    params![valid, anchor.root_id],
                )?;
                tx.execute("UPDATE scan_roots SET generation=generation+1,scan_state=CASE WHEN paused=1 THEN 'paused' ELSE 'partial_failure' END WHERE id=?",[&anchor.root_id])?;
                any = true;
            }
        }
        if any {
            bump(&tx, true)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn root_current(db: &Connection, root_id: &str) -> Result<bool> {
        let row: Option<(String, Option<String>, Option<bool>)> = db.query_row("SELECT s.path,l.identity,l.valid FROM scan_roots s LEFT JOIN root_locators l ON l.root_id=s.id WHERE s.id=? AND s.active=1",[root_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        Ok(row.is_some_and(|(path, identity, valid)| match identity {
            None => true,
            Some(identity) => valid == Some(true) && matches_directory(Path::new(&path), &identity),
        }))
    }

    pub(crate) fn allowed_by_root(db: &Connection, root: &Root, path: &Path) -> Result<bool> {
        Ok(root.active
            && path.starts_with(&root.path)
            && !root
                .exclusions
                .iter()
                .any(|ex| path.starts_with(Path::new(&root.path).join(ex)))
            && Self::root_current(db, &root.id)?)
    }

    pub(crate) fn refresh_roots(&self) -> Result<()> {
        let result = self.refresh_root_locations();
        if let Err(error) = &result {
            if !is_root_error(error) {
                return Err(error.clone());
            }
        }
        let mut locator_error = result.err();
        for root in self
            .roots()?
            .into_iter()
            .filter(|r| r.active && r.paused && r.location_status == "available")
        {
            if let Err(error) = self.refresh_paused_root(&root.id) {
                if !is_root_error(&error) {
                    return Err(error);
                }
                locator_error.get_or_insert(error);
            }
        }
        locator_error.map_or(Ok(()), Err)
    }

    pub fn refresh_root(&self, root_id: &str) -> Result<()> {
        let root = self
            .roots()?
            .into_iter()
            .find(|r| r.id == root_id && r.active)
            .ok_or_else(|| {
                AppError::new(
                    "not_authorized",
                    "目录授权已撤销",
                    "Folder authorization has been revoked.",
                )
            })?;
        if root.paused {
            self.verify_root_locations()?;
            self.refresh_paused_root(root_id)
        } else {
            self.scan_root(root_id)
        }
    }

    fn refresh_paused_root(&self, root_id: &str) -> Result<()> {
        let mut after = String::new();
        let mut locator_error = None;
        loop {
            let ids = {
                let _guard = self.gate.read().map_err(|_| changed())?;
                let db = self.connection()?;
                if !Self::roots_db(&db)?
                    .iter()
                    .any(|r| r.id == root_id && r.active && r.paused)
                {
                    break;
                }
                if !Self::root_current(&db, root_id)? {
                    return Err(unavailable());
                }
                // 暂停只按稳定UUID分页核对已有映射，绝不枚举磁盘上的新文件。
                let ids = db.prepare("SELECT resource_id FROM file_roots WHERE root_id=?1 AND resource_id>?2 ORDER BY resource_id LIMIT 100")?.query_map(params![root_id,after],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
                ids
            };
            if ids.is_empty() {
                break;
            }
            for rid in ids {
                after = rid.clone();
                if let Err(error) = self.recover_root_file_current(&rid, Some(root_id)) {
                    if !is_root_error(&error) {
                        return Err(error);
                    }
                    locator_error.get_or_insert(error);
                }
            }
        }
        locator_error.map_or(Ok(()), Err)
    }

    pub fn refresh_root_locations(&self) -> Result<()> {
        self.verify_root_locations()?;
        let ids = {
            let _guard = self.gate.read().map_err(|_| changed())?;
            Self::anchors_db(&self.connection()?)?
                .into_iter()
                .filter(|a| !a.valid)
                .map(|a| a.root_id)
                .collect::<Vec<_>>()
        };
        let mut errors = Vec::new();
        for id in ids {
            let anchor = {
                let _guard = self.gate.read().map_err(|_| changed())?;
                let mut db = self.connection()?;
                let tx = db.transaction()?;
                Self::anchors_db(&tx)?
                    .into_iter()
                    .find(|a| a.root_id == id && !a.valid)
            };
            let Some(anchor) = anchor else {
                continue;
            };
            let candidate = self.locate_root(&anchor);
            let _guard = self.gate.write().map_err(|_| changed())?;
            let mut db = self.connection()?;
            let tx = db.transaction()?;
            if !anchor.current(&tx)? {
                continue;
            }
            match candidate {
                Ok((path, bookmark)) => {
                    if !matches_directory(&path, &anchor.identity) {
                        errors.push((anchor, unavailable()));
                        continue;
                    }
                    let occupied: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM scan_roots WHERE path=?1 AND id<>?2)",
                        params![path.to_string_lossy(), id],
                        |r| r.get(0),
                    )?;
                    if occupied {
                        errors.push((anchor, unavailable()));
                        continue;
                    }
                    tx.execute(
                        "INSERT OR IGNORE INTO root_aliases VALUES(?1,?2)",
                        params![id, anchor.path],
                    )?;
                    tx.execute("UPDATE scan_roots SET path=?1,generation=generation+1,scan_state=CASE WHEN paused=1 THEN 'paused' ELSE 'idle' END WHERE id=?2",params![path.to_string_lossy(),id])?;
                    tx.execute(
                        "UPDATE root_locators SET bookmark=?1,valid=1 WHERE root_id=?2",
                        params![bookmark, id],
                    )?;
                    bump(&tx, true)?;
                    tx.commit()?;
                }
                Err(e) if is_root_error(&e) => {
                    errors.push((anchor, e));
                }
                Err(e) => return Err(e),
            }
        }
        // 无关根可在本轮继续恢复；最终只报告仍属同一失效授权快照的错误。
        let _guard = self.gate.read().map_err(|_| changed())?;
        let db = self.connection()?;
        for (anchor, error) in errors {
            let pending:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM scan_roots s JOIN root_locators l ON l.root_id=s.id WHERE s.id=?1 AND s.path=?2 AND s.active=1 AND s.generation=?3 AND l.identity=?4 AND l.bookmark IS ?5 AND l.valid=0)",params![anchor.root_id,anchor.path,anchor.generation,anchor.identity,anchor.bookmark],|r|r.get(0))?;
            if pending && !matches_directory(Path::new(&anchor.path), &anchor.identity) {
                return Err(error);
            }
        }
        Ok(())
    }

    fn locate_root(&self, anchor: &Anchor) -> Result<(PathBuf, Option<Vec<u8>>)> {
        #[cfg(target_os = "macos")]
        {
            let bookmark = anchor.bookmark.as_ref().ok_or_else(unavailable)?;
            // 原生解析在gate和写事务之外；路径仅为候选，仍核对目录身份。
            let (candidate, stale) = crate::macos_bookmark::resolve(bookmark)?;
            let candidate = candidate.canonicalize().map_err(|_| unavailable())?;
            if !matches_directory(&candidate, &anchor.identity) {
                return Err(unavailable());
            }
            let bookmark = if stale {
                crate::macos_bookmark::create(&candidate)?
            } else {
                bookmark.clone()
            };
            Ok((candidate, Some(bookmark)))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = anchor;
            Err(unavailable())
        }
    }

    pub(crate) fn recover_root_file(&self, rid: &str) -> Result<bool> {
        {
            let _guard = self.gate.read().map_err(|_| changed())?;
            let mapped: bool = self.connection()?.query_row("SELECT EXISTS(SELECT 1 FROM file_roots f JOIN scan_roots s ON s.id=f.root_id JOIN root_locators l ON l.root_id=s.id WHERE f.resource_id=? AND s.active=1)",[rid],|r|r.get(0))?;
            if !mapped {
                return Ok(false);
            }
        }
        let result = self.refresh_root_locations();
        if let Err(error) = &result {
            if !is_root_error(error) {
                return Err(error.clone());
            }
        }
        // 无关根失败不阻止这个引用按有效根恢复。
        if self.recover_root_file_current(rid, None)? {
            return Ok(true);
        }
        {
            let _guard = self.gate.read().map_err(|_| changed())?;
            if Self::resolve_resource_db(&self.connection()?, rid).is_ok() {
                return Ok(true);
            }
        }
        result.map(|_| false)
    }

    fn recover_root_file_current(&self, rid: &str, only_root: Option<&str>) -> Result<bool> {
        let candidates = {
            let _guard = self.gate.read().map_err(|_| changed())?;
            let mut connection = self.connection()?;
            let db = connection.transaction()?;
            let resource: Option<(String, String)> = db
                .query_row(
                    "SELECT identity,location FROM resources WHERE id=? AND kind='file'",
                    [rid],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((identity, location)) = resource else {
                return Ok(false);
            };
            let anchors = Self::anchors_db(&db)?;
            let mut candidates = Vec::new();
            for anchor in anchors
                .into_iter()
                .filter(|a| a.valid && only_root.is_none_or(|id| id == a.root_id))
            {
                let mapped: bool = db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM file_roots WHERE root_id=?1 AND resource_id=?2)",
                    params![anchor.root_id, rid],
                    |r| r.get(0),
                )?;
                if !mapped {
                    continue;
                }
                let mut prefixes = db.prepare("SELECT path FROM root_aliases WHERE root_id=? ORDER BY length(path) DESC,path")?.query_map([&anchor.root_id],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
                prefixes.insert(0, anchor.path.clone());
                for prefix in prefixes {
                    if let Ok(relative) = Path::new(&location).strip_prefix(&prefix) {
                        let candidate = Path::new(&anchor.path).join(relative);
                        // 不对已在当前路径且身份正常的条目反复写入。
                        if candidate == Path::new(&location)
                            && candidate
                                .metadata()
                                .ok()
                                .and_then(|m| platform::identity(&candidate, &m).ok())
                                .as_deref()
                                == Some(&identity)
                        {
                            continue;
                        }
                        candidates.push(RootRecovery {
                            anchor: anchor.clone(),
                            resource_id: rid.into(),
                            identity: identity.clone(),
                            location: location.clone(),
                            candidate,
                        });
                    }
                }
            }
            candidates
        };
        for mut candidate in candidates {
            let Ok(path) = candidate.candidate.canonicalize() else {
                continue;
            };
            let Ok(meta) = path.metadata() else {
                continue;
            };
            if !meta.is_file() || platform::identity(&path, &meta)?.as_str() != candidate.identity {
                continue;
            }
            candidate.candidate = path.clone();
            match self.import_one_checked(
                &path,
                None,
                false,
                Some(&candidate.identity),
                Some(&candidate),
            ) {
                Ok(_) => return Ok(true),
                Err(error)
                    if matches!(
                        error.code.as_str(),
                        "permission_changed" | "not_authorized" | "identity_unconfirmed"
                    ) =>
                {
                    continue
                }
                Err(error) => return Err(error),
            }
        }
        Ok(false)
    }
}

#[cfg(all(test, target_os = "macos"))]
mod guard_tests {
    use super::*;
    use rusqlite::types::Value;
    use std::fs;

    fn change(store: &Store, mutation: Mutation) {
        store
            .mutate(MutationRequest {
                operation_id: crate::store::id(),
                mutation,
            })
            .unwrap();
    }
    fn snapshot(db: &Connection) -> Vec<Vec<Value>> {
        let mut rows = Vec::new();
        for table in [
            "resources",
            "file_roots",
            "file_grants",
            "file_locators",
            "root_locators",
            "root_aliases",
            "scan_roots",
            "activities",
            "settings",
            "search_documents",
        ] {
            let mut query = db
                .prepare(&format!("SELECT * FROM {table} ORDER BY 1,2"))
                .unwrap();
            let columns = query.column_count();
            rows.extend(
                query
                    .query_map([], |row| {
                        (0..columns)
                            .map(|i| row.get(i))
                            .collect::<rusqlite::Result<Vec<Value>>>()
                    })
                    .unwrap()
                    .map(|r| r.unwrap()),
            );
        }
        rows
    }
    #[test]
    fn root_file_recovery_survives_unrelated_metadata_commit() {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let temp = tempfile::Builder::new()
            .prefix("root-recovery-metadata-")
            .tempdir_in(approved)
            .unwrap();
        let home = temp.path().canonicalize().unwrap();
        let original = home.join("documents");
        fs::create_dir(&original).unwrap();
        fs::write(original.join("one.txt"), b"synthetic original").unwrap();
        let store = Store::open(home.join("app/catalog.sqlite3")).unwrap();
        let root_id = store.grant_root(&original).unwrap();
        store.scan_all().unwrap();
        let (rid, identity, location): (String, String, String) = store
            .connection()
            .unwrap()
            .query_row("SELECT id,identity,location FROM resources", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        let moved = home.join("moved");
        fs::rename(original, &moved).unwrap();
        store.refresh_root_locations().unwrap();
        let recovery = RootRecovery {
            anchor: Store::anchors_db(&store.connection().unwrap())
                .unwrap()
                .into_iter()
                .find(|a| a.root_id == root_id)
                .unwrap(),
            resource_id: rid.clone(),
            identity: identity.clone(),
            location,
            candidate: moved.join("one.txt"),
        };
        change(
            &store,
            Mutation::SaveWeb {
                id: None,
                name: "unrelated synthetic link".into(),
                url: "https://example.invalid/metadata-only".into(),
                workspace_id: None,
            },
        );
        let result = store.import_one_checked(
            &recovery.candidate,
            None,
            false,
            Some(&identity),
            Some(&recovery),
        );
        assert!(result.is_ok(), "an unrelated metadata commit cannot invalidate unchanged file and root authority: {result:?}");
        assert_eq!(
            store.resolve_resource(&rid).unwrap().0,
            recovery.candidate.to_string_lossy()
        );
    }
    #[test]
    fn captured_root_file_guard_cannot_survive_authority_or_resource_changes() {
        for intervention in [
            "revoke", "pause", "exclude", "include", "regrant", "relink", "restore",
        ] {
            let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
            let temp = tempfile::Builder::new()
                .prefix("root-guard-race-")
                .tempdir_in(approved)
                .unwrap();
            let home = temp.path().canonicalize().unwrap();
            let root = home.join("docs");
            fs::create_dir_all(root.join("nested")).unwrap();
            fs::write(root.join("nested/one.txt"), b"original").unwrap();
            let store = Store::open(home.join("app/catalog.sqlite3")).unwrap();
            let root_id = store.grant_root(&root).unwrap();
            store.scan_all().unwrap();
            let (rid, identity, location): (String, String, String) = store
                .connection()
                .unwrap()
                .query_row("SELECT id,identity,location FROM resources", [], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .unwrap();
            // include需要真正改变generation；与候选文件无关的排除也应作废旧快照。
            if intervention == "include" {
                change(
                    &store,
                    Mutation::Exclude {
                        root_id: root_id.clone(),
                        relative_path: "other".into(),
                    },
                );
            }
            let moved = home.join("moved");
            fs::rename(&root, &moved).unwrap();
            store.refresh_root_locations().unwrap();
            let candidate = moved.join("nested/one.txt");
            let anchor = Store::anchors_db(&store.connection().unwrap())
                .unwrap()
                .pop()
                .unwrap();
            let recovery = RootRecovery {
                anchor,
                resource_id: rid.clone(),
                identity: identity.clone(),
                location,
                candidate: candidate.clone(),
            };
            match intervention {
                "revoke" => change(&store, Mutation::RevokeRoot { id: root_id }),
                "pause" => change(
                    &store,
                    Mutation::SetPaused {
                        id: root_id,
                        value: true,
                    },
                ),
                "exclude" => change(
                    &store,
                    Mutation::Exclude {
                        root_id,
                        relative_path: "nested".into(),
                    },
                ),
                "include" => change(
                    &store,
                    Mutation::Include {
                        root_id,
                        relative_path: "other".into(),
                    },
                ),
                "regrant" => {
                    store.grant_root(&moved).unwrap();
                }
                "relink" => {
                    let replacement = moved.join("replacement.txt");
                    fs::write(&replacement, b"new object").unwrap();
                    store.relink(&rid, &replacement).unwrap();
                }
                "restore" => {
                    let backup = home.join("snapshot.sqlite3");
                    store.backup_to(&backup).unwrap();
                    store.restore_from(&backup).unwrap();
                }
                _ => unreachable!(),
            }
            let before = snapshot(&store.connection().unwrap());
            let error = store
                .import_one_checked(&candidate, None, false, Some(&identity), Some(&recovery))
                .unwrap_err();
            assert_eq!(error.code, "permission_changed", "{intervention}");
            assert_eq!(
                snapshot(&store.connection().unwrap()),
                before,
                "{intervention}"
            );
        }
    }

    #[test]
    fn root_guard_rejects_a_replacement_file_even_without_expected_identity_argument() {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let temp = tempfile::Builder::new()
            .prefix("root-guard-replace-")
            .tempdir_in(approved)
            .unwrap();
        let home = temp.path().canonicalize().unwrap();
        let root = home.join("docs");
        fs::create_dir(&root).unwrap();
        let original = root.join("one.txt");
        fs::write(&original, b"original").unwrap();
        let store = Store::open(home.join("app/catalog.sqlite3")).unwrap();
        store.grant_root(&root).unwrap();
        store.scan_all().unwrap();
        let (rid, identity, location): (String, String, String) = store
            .connection()
            .unwrap()
            .query_row("SELECT id,identity,location FROM resources", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        let anchor = Store::anchors_db(&store.connection().unwrap())
            .unwrap()
            .pop()
            .unwrap();
        let replacement = root.join("copy.txt");
        fs::copy(&original, &replacement).unwrap();
        let recovery = RootRecovery {
            anchor,
            resource_id: rid,
            identity,
            location,
            candidate: replacement.clone(),
        };
        let before = snapshot(&store.connection().unwrap());
        assert_eq!(
            store
                .import_one_checked(&replacement, None, false, None, Some(&recovery))
                .unwrap_err()
                .code,
            "identity_unconfirmed"
        );
        assert_eq!(snapshot(&store.connection().unwrap()), before);
    }
}
