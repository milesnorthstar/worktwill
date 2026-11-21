use crate::{
    model::*,
    platform, search,
    store::{bump, id, now},
    Store,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use std::{
    fs::Metadata,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::UNIX_EPOCH,
};
use walkdir::{DirEntry, WalkDir};

#[cfg(test)]
#[path = "catalog_root_tests.rs"]
mod root_tests;

struct Discovered {
    path: PathBuf,
    name: String,
    identity: String,
    size: f64,
    modified: f64,
}
#[derive(Clone, Copy)]
struct ScanGeneration {
    sequence: i64,
    restore_epoch: u64,
}
pub(crate) fn mtime(meta: &Metadata) -> f64 {
    meta.modified()
        .ok()
        .and_then(|v| v.duration_since(UNIX_EPOCH).ok())
        .map(|v| v.as_millis() as f64)
        .unwrap_or(0.)
}

// 自动扫描和通知收录共用默认跳过规则；显式选择文件的授权规则保持独立。
pub(crate) fn automatic_path_allowed(
    path: &Path,
    root: &Path,
    app_data: Option<&Path>,
    directory: bool,
) -> bool {
    if path == root {
        return true;
    }
    if app_data.is_some_and(|data| path.starts_with(data)) {
        return false;
    }
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let mut parts = relative.components().peekable();
    while let Some(part) = parts.next() {
        if (directory || parts.peek().is_some())
            && [".git", "node_modules", ".cache"]
                .iter()
                .any(|name| part.as_os_str() == *name)
        {
            return false;
        }
    }
    true
}

impl Store {
    pub fn scan_all(&self) -> Result<()> {
        self.scan_all_with_root_updates(|_| {})
    }

    pub fn scan_all_with_root_updates(&self, roots_ready: impl FnOnce(&[Root])) -> Result<()> {
        let root_result = self.refresh_roots();
        if let Err(error) = &root_result {
            if !crate::root_locator::is_root_error(error) {
                return Err(error.clone());
            }
        }
        // 单项原生定位失败仍明确返回，但先完成无关目录的扫描；数据库/锁错误立即停止。
        let file_result = self.refresh_file_grants();
        if let Err(error) = &file_result {
            if !crate::root_locator::is_root_error(error) && error.code != "continuity_conflict" {
                return Err(error.clone());
            }
        }
        let roots = self.roots()?;
        // 位置恢复已提交且没有持锁；原生层可先调度idle目录，避免已有引用等待其他大目录。
        roots_ready(&roots);
        let mut root_error = root_result.err();
        for root in roots
            .into_iter()
            .filter(|r| r.active && !r.paused && r.location_status != "unavailable")
        {
            if let Err(error) = self.scan_root(&root.id) {
                if crate::root_locator::is_root_error(&error) {
                    root_error.get_or_insert(error);
                } else {
                    return Err(error);
                }
            }
        }
        root_error.map_or(file_result, Err)
    }

    pub fn scan_root(&self, root_id: &str) -> Result<()> {
        self.verify_root_locations()?;
        let (root, generation) = {
            let _guard = self.gate.write().map_err(|_| {
                AppError::new(
                    "internal_error",
                    "状态锁不可用",
                    "App state is unavailable.",
                )
            })?;
            let db = self.connection()?;
            let root = Self::roots_db(&db)?
                .into_iter()
                .find(|r| r.id == root_id && r.active && !r.paused)
                .ok_or_else(|| {
                    AppError::new(
                        "not_authorized",
                        "扫描已暂停或授权已撤销",
                        "Scanning is paused or authorization has been revoked.",
                    )
                })?;
            if !Self::root_current(&db, root_id)? {
                return Err(crate::root_locator::unavailable());
            }
            db.execute("UPDATE scan_roots SET generation=generation+1,scan_state='running',scanned=0,failures=0 WHERE id=?",[root_id])?;
            let generation = db.query_row(
                "SELECT generation FROM scan_roots WHERE id=?",
                [root_id],
                |r| r.get::<_, i64>(0),
            )?;
            (
                root,
                ScanGeneration {
                    sequence: generation,
                    restore_epoch: self.scan_epoch.load(Ordering::SeqCst),
                },
            )
        };
        let root_path = Path::new(&root.path);
        let mut batch = Vec::new();
        let mut scanned = 0_u32;
        let mut failures = 0_u32;
        let app_data = self.path.parent().and_then(|p| p.canonicalize().ok());
        let allowed_entry = |entry: &DirEntry| -> bool {
            let path = entry.path();
            if entry.depth() == 0 {
                return true;
            }
            if entry.file_type().is_symlink() {
                return false;
            }
            if !automatic_path_allowed(
                path,
                root_path,
                app_data.as_deref(),
                entry.file_type().is_dir(),
            ) {
                return false;
            }
            !root
                .exclusions
                .iter()
                .any(|relative| path.starts_with(root_path.join(relative)))
        };
        for entry in WalkDir::new(root_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(allowed_entry)
        {
            if scanned.is_multiple_of(100) && !self.scan_valid(root_id, generation)? {
                return self.finish_invalid_scan(root_id, generation);
            }
            match entry {
                Ok(entry) if entry.file_type().is_file() => {
                    let observed = (|| -> Result<Discovered> {
                        let metadata = std::fs::symlink_metadata(entry.path())?;
                        let path = entry.path().canonicalize()?;
                        #[cfg(windows)]
                        self.reconcile_document(&path)?;
                        if !path.starts_with(root_path) || metadata.file_type().is_symlink() {
                            return Err(AppError::new(
                                "not_authorized",
                                "跳过越界文件",
                                "Skipped a file outside the authorized scope.",
                            ));
                        }
                        let name = entry
                            .file_name()
                            .to_str()
                            .ok_or_else(|| {
                                AppError::new(
                                    "invalid_input",
                                    "文件名称编码暂不支持",
                                    "The filename encoding is not supported.",
                                )
                            })?
                            .to_owned();
                        Ok(Discovered {
                            identity: platform::identity(&path, &metadata)?,
                            path,
                            name,
                            size: metadata.len() as f64,
                            modified: mtime(&metadata),
                        })
                    })();
                    match observed {
                        Ok(item) => {
                            batch.push(item);
                            scanned += 1;
                        }
                        Err(_) => failures += 1,
                    }
                }
                Ok(_) => {}
                Err(_) => failures += 1,
            }
            if batch.len() >= 100
                && !self.commit_batch(root_id, generation, &mut batch, scanned, &mut failures)?
            {
                return self.finish_invalid_scan(root_id, generation);
            }
        }
        if !self.commit_batch(root_id, generation, &mut batch, scanned, &mut failures)? {
            return self.finish_invalid_scan(root_id, generation);
        }
        self.verify_root_locations()?;
        let _guard = self.gate.write().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        if !valid(self, &tx, root_id, generation)? {
            drop(tx);
            drop(db);
            drop(_guard);
            self.finish_invalid_scan(root_id, generation)?;
            return Ok(());
        }
        // 枚举完整才核对未发现项；离线、取消、权限拒绝不会把未读到当删除。
        let mut changed = false;
        if failures == 0 {
            // 当前批次已见另一身份占用同一路径时保留身份待确认；独立授权的状态由file grant复验维护。
            let separator = std::path::MAIN_SEPARATOR;
            let sql = format!("UPDATE resources SET status='missing' WHERE kind='file' AND status<>'missing' AND id IN(SELECT resource_id FROM file_roots WHERE root_id=?1 AND generation<>?2) AND NOT EXISTS(SELECT 1 FROM resources replacement JOIN file_roots seen ON seen.resource_id=replacement.id WHERE replacement.kind='file' AND replacement.id<>resources.id AND replacement.location=resources.location AND replacement.status='available' AND seen.root_id=?1 AND seen.generation=?2) AND NOT EXISTS(SELECT 1 FROM file_grants g WHERE g.resource_id=resources.id AND g.active=1) AND NOT EXISTS(SELECT 1 FROM file_roots f JOIN scan_roots s ON s.id=f.root_id WHERE f.resource_id=resources.id AND s.active=1 AND f.generation=s.generation AND s.id<>?1 AND NOT EXISTS(SELECT 1 FROM root_locators l WHERE l.root_id=s.id AND l.valid=0) AND instr(resources.location,rtrim(s.path,'{separator}')||'{separator}')=1 AND NOT EXISTS(SELECT 1 FROM scan_exclusions ex WHERE ex.root_id=s.id AND (resources.location=rtrim(s.path,'{separator}')||'{separator}'||ex.relative_path OR instr(resources.location,rtrim(s.path,'{separator}')||'{separator}'||ex.relative_path||'{separator}')=1)))");
            changed = tx.execute(&sql, params![root_id, generation.sequence])? > 0;
        }
        tx.execute(
            "UPDATE scan_roots SET scan_state=?1,scanned=?2,failures=?3 WHERE id=?4",
            params![
                if failures == 0 {
                    "completed"
                } else {
                    "partial_failure"
                },
                scanned,
                failures,
                root_id
            ],
        )?;
        if changed {
            bump(&tx, false)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn finish_invalid_scan(&self, root: &str, generation: ScanGeneration) -> Result<()> {
        self.verify_root_locations()?;
        let _guard = self.gate.write().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        if self.scan_epoch.load(Ordering::SeqCst) != generation.restore_epoch {
            return Ok(());
        }
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        let changed = tx.execute("UPDATE scan_roots SET scan_state='partial_failure' WHERE id=?1 AND generation=?2 AND scan_state='running'",params![root,generation.sequence])?;
        if changed > 0 {
            bump(&tx, false)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn scan_valid(&self, root: &str, generation: ScanGeneration) -> Result<bool> {
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        valid(self, &self.connection()?, root, generation)
    }

    fn commit_batch(
        &self,
        root: &str,
        generation: ScanGeneration,
        batch: &mut Vec<Discovered>,
        scanned: u32,
        failures: &mut u32,
    ) -> Result<bool> {
        // 批次先排队再取得恢复屏障；WAL 读事务可在扫描写入时继续读取。
        let _writer = self.catalog_write.lock().map_err(|_| {
            AppError::new(
                "internal_error",
                "扫描写入锁不可用",
                "The scan writer is unavailable.",
            )
        })?;
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut db = self.connection()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !valid(self, &tx, root, generation)? {
            return Ok(false);
        }
        let root_path = PathBuf::from(tx.query_row(
            "SELECT path FROM scan_roots WHERE id=?",
            [root],
            |r| r.get::<_, String>(0),
        )?);
        let exclusions = tx
            .prepare("SELECT relative_path FROM scan_exclusions WHERE root_id=?")?
            .query_map([root], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut changed = false;
        for mut item in batch.drain(..) {
            let observed_path = item.path.clone();
            // 提交前重读当前事实；旧批次不得回写旧 mtime/size，父目录换成链接时也不沿旧路径提交。
            let current = std::fs::symlink_metadata(&item.path)
                .ok()
                .filter(|m| m.is_file())
                .filter(|m| {
                    platform::identity(&item.path, m).ok().as_deref()
                        == Some(item.identity.as_str())
                })
                .filter(|_| item.path.canonicalize().ok().as_ref() == Some(&item.path));
            let Some(current) = current else {
                *failures += 1;
                continue;
            };
            item.modified = mtime(&current);
            item.size = current.len() as f64;
            let existing = tx
                .query_row(
                    "SELECT id FROM resources WHERE identity=?",
                    [&item.identity],
                    |r| r.get::<_, String>(0),
                )
                .optional()?;
            let previous: Option<(f64, f64, String, String)> = if let Some(rid) = &existing {
                Some(tx.query_row(
                    "SELECT modified_at,size,name,location FROM resources WHERE id=?",
                    [rid],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )?)
            } else {
                None
            };
            // 同根内的硬链接不抢占仍有效的原路径；旧路径失效或被排除时仍采用本次事实。
            if let Some((_, _, name, location)) = &previous {
                let retained = Path::new(location);
                if retained != item.path
                    && retained.starts_with(&root_path)
                    && automatic_path_allowed(retained, &root_path, self.path.parent(), false)
                    && !exclusions
                        .iter()
                        .any(|relative| retained.starts_with(root_path.join(relative)))
                    && retained.canonicalize().ok().as_deref() == Some(retained)
                    && std::fs::symlink_metadata(retained)
                        .ok()
                        .is_some_and(|meta| {
                            meta.is_file()
                                && platform::identity(retained, &meta).ok().as_deref()
                                    == Some(item.identity.as_str())
                        })
                {
                    item.path = retained.to_path_buf();
                    item.name = name.clone();
                }
            }
            let path = item.path.to_string_lossy();
            let rid = existing.unwrap_or_else(id);
            changed |= tx.execute("UPDATE resources SET status='identity_unconfirmed' WHERE kind='file' AND location IN (?1,?3) AND id<>?2 AND status<>'identity_unconfirmed'",params![path,rid,observed_path.to_string_lossy()])? > 0;
            changed |= tx.execute("INSERT INTO resources(id,kind,name,location,identity,modified_at,size,status,created_at) VALUES(?1,'file',?2,?3,?4,?5,?6,'available',?7) ON CONFLICT(id) DO UPDATE SET name=excluded.name,location=excluded.location,modified_at=excluded.modified_at,size=excluded.size,status='available' WHERE resources.name<>excluded.name OR resources.location<>excluded.location OR resources.modified_at<>excluded.modified_at OR resources.size<>excluded.size OR resources.status<>'available'",params![rid,item.name,path,item.identity,item.modified,item.size,now()])? > 0;
            changed |= tx.execute(
                "INSERT OR IGNORE INTO file_roots VALUES(?1,?2,?3)",
                params![root, rid, generation.sequence],
            )? > 0;
            tx.execute("UPDATE file_roots SET generation=?3 WHERE root_id=?1 AND resource_id=?2 AND generation<>?3",params![root,rid,generation.sequence])?;
            let projection_changes = tx.total_changes();
            search::project(&tx, &rid, "file", &item.name, &path, Some(item.modified))?;
            changed |= tx.total_changes() != projection_changes;
            // 首次发现使用观察时间，不把旧 mtime 伪造成应用过去的编辑记录；重复/重叠扫描不重复记账。
            let kind = match previous {
                None => Some("file_discovered"),
                Some((modified, size, _, _)) if modified != item.modified || size != item.size => {
                    Some("file_modified")
                }
                _ => None,
            };
            if let Some(kind) = kind {
                let aid = id();
                tx.execute("INSERT INTO activities(id,kind,name,resource_id,at,source,file_modified_at) VALUES(?1,?2,?6,?3,?4,'filesystem',?5)",params![aid,kind,rid,now(),item.modified,item.name])?;
                crate::activity_origin::capture(&tx, &aid, &rid)?;
                changed = true;
            }
        }
        tx.execute(
            "UPDATE scan_roots SET scanned=?1,failures=?2 WHERE id=?3",
            params![scanned, *failures, root],
        )?;
        // 扫描代次、进度和空批次不改变查询事实，不能持续作废用户的后续页。
        if changed {
            bump(&tx, false)?;
        }
        tx.commit()?;
        Ok(true)
    }
}
fn valid(
    store: &Store,
    db: &rusqlite::Connection,
    root: &str,
    generation: ScanGeneration,
) -> Result<bool> {
    if store.scan_epoch.load(Ordering::SeqCst) != generation.restore_epoch {
        return Ok(false);
    }
    let current: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM scan_roots WHERE id=?1 AND active=1 AND paused=0 AND generation=?2)",params![root,generation.sequence],|r|r.get(0))?;
    Ok(current && Store::root_current(db, root)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scan_generation(store: &Store, sequence: i64) -> ScanGeneration {
        ScanGeneration {
            sequence,
            restore_epoch: store.scan_epoch.load(Ordering::SeqCst),
        }
    }

    #[test]
    fn catalog_batch_can_commit_while_a_reader_holds_the_restore_barrier() {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("scan-reader-")
            .tempdir_in(approved)
            .unwrap();
        let docs = directory.path().join("documents");
        fs::create_dir(&docs).unwrap();
        let path = docs.join("合同.txt");
        fs::write(&path, b"synthetic fixture").unwrap();
        let store =
            std::sync::Arc::new(Store::open(directory.path().join("app/catalog.sqlite3")).unwrap());
        let root = store.grant_root(&docs).unwrap();
        let meta = fs::symlink_metadata(&path).unwrap();
        let item = Discovered {
            identity: platform::identity(&path, &meta).unwrap(),
            path,
            name: "合同.txt".into(),
            size: meta.len() as f64,
            modified: mtime(&meta),
        };
        let generation = scan_generation(&store, 0);
        let reader = store.gate.read().unwrap();
        let writer = store.clone();
        let (send, receive) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            send.send(writer.commit_batch(&root, generation, &mut vec![item], 1, &mut 0))
                .unwrap()
        });
        let result = receive.recv_timeout(std::time::Duration::from_secs(2));
        drop(reader);
        thread.join().unwrap();
        assert!(result
            .expect("catalog writer waited for a read lease")
            .unwrap());
    }

    #[test]
    fn restore_and_reauthorization_cannot_revive_an_old_batch_with_the_same_generation() {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("scan-restore-aba-")
            .tempdir_in(approved)
            .unwrap();
        let docs = directory.path().join("documents");
        fs::create_dir(&docs).unwrap();
        let store = Store::open(directory.path().join("app/catalog.sqlite3")).unwrap();
        let root = store.grant_root(&docs).unwrap();
        store
            .connection()
            .unwrap()
            .execute("UPDATE scan_roots SET generation=8 WHERE id=?", [&root])
            .unwrap();
        let backup = directory.path().join("snapshot.sqlite3");
        store.backup_to(&backup).unwrap();
        store
            .connection()
            .unwrap()
            .execute(
                "UPDATE scan_roots SET generation=10,scan_state='running' WHERE id=?",
                [&root],
            )
            .unwrap();
        let path = docs.join("旧枚举.txt");
        fs::write(&path, b"synthetic fixture").unwrap();
        let meta = fs::symlink_metadata(&path).unwrap();
        let item = Discovered {
            identity: platform::identity(&path, &meta).unwrap(),
            path,
            name: "旧枚举.txt".into(),
            size: meta.len() as f64,
            modified: mtime(&meta),
        };
        let generation = scan_generation(&store, 10);
        store.restore_from(&backup).unwrap();
        assert_eq!(store.grant_root(&docs).unwrap(), root);
        assert!(!store
            .commit_batch(&root, generation, &mut vec![item], 1, &mut 0)
            .unwrap());
        assert!(store
            .resources(ResourceQuery {
                workspace_id: None,
                kind: "all".into(),
                mode: "modified".into(),
                offset: 0
            })
            .unwrap()
            .items
            .is_empty());
    }

    #[test]
    fn older_overlapping_batch_cannot_restore_stale_metadata_or_invent_an_extra_modification() {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("scan-late-batch-")
            .tempdir_in(approved)
            .unwrap();
        let docs = directory.path().join("documents");
        let child = docs.join("nested");
        fs::create_dir_all(&child).unwrap();
        let path = child.join("合同.txt");
        fs::write(&path, b"old fixture").unwrap();
        let store = Store::open(directory.path().join("app/catalog.sqlite3")).unwrap();
        let parent = store.grant_root(&docs).unwrap();
        store.scan_root(&parent).unwrap();
        let generation = store
            .connection()
            .unwrap()
            .query_row(
                "SELECT generation FROM scan_roots WHERE id=?",
                [&parent],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        let meta = fs::symlink_metadata(&path).unwrap();
        let old = Discovered {
            path: path.clone(),
            name: "合同.txt".into(),
            identity: platform::identity(&path, &meta).unwrap(),
            size: meta.len() as f64,
            modified: mtime(&meta),
        };
        fs::write(&path, b"new synthetic fixture with a different size").unwrap();
        let nested = store.grant_root(&child).unwrap();
        store.scan_root(&nested).unwrap();
        let rid = store
            .connection()
            .unwrap()
            .query_row("SELECT id FROM resources", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap();
        let latest = Store::resource_db(&store.connection().unwrap(), &rid, false).unwrap();
        assert!(store
            .commit_batch(
                &parent,
                scan_generation(&store, generation),
                &mut vec![old],
                1,
                &mut 0
            )
            .unwrap());
        let current = Store::resource_db(&store.connection().unwrap(), &rid, false).unwrap();
        assert_eq!(current.size, latest.size);
        assert_eq!(current.modified_at, latest.modified_at);
        let changes = store
            .connection()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM activities WHERE kind='file_modified'",
                [],
                |row| row.get::<_, u32>(0),
            )
            .unwrap();
        assert_eq!(changes, 1);
    }

    #[test]
    fn vanished_candidate_counts_as_failure_without_publishing_resource_or_projection() {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("scan-vanished-")
            .tempdir_in(approved)
            .unwrap();
        let docs = directory.path().join("documents");
        fs::create_dir(&docs).unwrap();
        let path = docs.join("合同.txt");
        fs::write(&path, b"synthetic fixture").unwrap();
        let store = Store::open(directory.path().join("app/catalog.sqlite3")).unwrap();
        let root = store.grant_root(&docs).unwrap();
        let meta = fs::symlink_metadata(&path).unwrap();
        let item = Discovered {
            path: path.clone(),
            name: "合同.txt".into(),
            identity: platform::identity(&path, &meta).unwrap(),
            size: meta.len() as f64,
            modified: mtime(&meta),
        };
        fs::remove_file(&path).unwrap();
        let mut failures = 0;
        assert!(store
            .commit_batch(
                &root,
                scan_generation(&store, 0),
                &mut vec![item],
                1,
                &mut failures
            )
            .unwrap());
        assert_eq!(failures, 1);
        assert_eq!(store.roots().unwrap()[0].failures, 1);
        for table in ["resources", "search_documents", "activities"] {
            assert_eq!(
                store
                    .connection()
                    .unwrap()
                    .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                        .get::<_, u32>(0))
                    .unwrap(),
                0
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn parent_replaced_with_symlink_cannot_publish_candidate_outside_original_scope() {
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("scan-parent-link-")
            .tempdir_in(approved)
            .unwrap();
        let docs = directory.path().join("documents");
        let nested = docs.join("nested");
        fs::create_dir_all(&nested).unwrap();
        let path = nested.join("合同.txt");
        fs::write(&path, b"synthetic fixture").unwrap();
        let store = Store::open(directory.path().join("app/catalog.sqlite3")).unwrap();
        let root = store.grant_root(&docs).unwrap();
        let meta = fs::symlink_metadata(&path).unwrap();
        let item = Discovered {
            path: path.clone(),
            name: "合同.txt".into(),
            identity: platform::identity(&path, &meta).unwrap(),
            size: meta.len() as f64,
            modified: mtime(&meta),
        };
        let outside = directory.path().join("outside");
        fs::rename(&nested, &outside).unwrap();
        std::os::unix::fs::symlink(&outside, &nested).unwrap();
        let mut failures = 0;
        assert!(store
            .commit_batch(
                &root,
                scan_generation(&store, 0),
                &mut vec![item],
                1,
                &mut failures
            )
            .unwrap());
        assert_eq!(failures, 1);
        assert_eq!(
            store
                .connection()
                .unwrap()
                .query_row("SELECT count(*) FROM resources", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            fs::read(outside.join("合同.txt")).unwrap(),
            b"synthetic fixture"
        );
    }
}
