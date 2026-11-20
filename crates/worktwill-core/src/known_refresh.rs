//! 文件通知在当前授权内刷新文件；新路径沿用目录扫描的自动收录规则。
use crate::{file_locator::RecoveryGuard, model::*, store::version, Store};
use rusqlite::{params, Connection, OptionalExtension};
use std::{path::Path, sync::atomic::Ordering};

struct FileRefresh<'a> {
    store: &'a Store,
    epoch: u64,
    permission: u32,
    id: Option<String>,
    location: String,
    identity: String,
}

fn refresh_allowed(store: &Store, db: &Connection, id: Option<&str>, path: &Path) -> Result<bool> {
    let granted: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM file_grants WHERE resource_id=? AND active=1)",
        [id],
        |r| r.get(0),
    )?;
    if granted {
        return Ok(true);
    }
    let app_data = id
        .is_none()
        .then(|| store.data_directory().canonicalize())
        .transpose()?;
    for root in Store::roots_db(db)? {
        if !root.paused
            && Store::allowed_by_root(db, &root, path)?
            && (id.is_some()
                || crate::catalog::automatic_path_allowed(
                    path,
                    Path::new(&root.path),
                    app_data.as_deref(),
                    false,
                ))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

impl RecoveryGuard for FileRefresh<'_> {
    fn identity(&self) -> &str {
        &self.identity
    }
    fn validate(&self, db: &Connection) -> Result<()> {
        let same: bool = if let Some(id) = &self.id {
            db.query_row(
                "SELECT EXISTS(SELECT 1 FROM resources WHERE id=?1 AND kind='file' AND location=?2 AND identity=?3)",
                params![id, self.location, self.identity], |r| r.get(0),
            )?
        } else {
            !db.query_row(
                "SELECT EXISTS(SELECT 1 FROM resources WHERE kind='file' AND location=?)",
                [&self.location],
                |r| r.get::<_, bool>(0),
            )?
        };
        if self.epoch != self.store.scan_epoch.load(Ordering::SeqCst)
            || self.permission != version(db, "permission_version")?
            || !same
            || !refresh_allowed(
                self.store,
                db,
                self.id.as_deref(),
                Path::new(&self.location),
            )?
        {
            return Err(AppError::new(
                "permission_changed",
                "文件或授权已变化，等待重新核对",
                "The file or its authorization has changed. Waiting for a recheck.",
            ));
        }
        Ok(())
    }
}

impl Store {
    pub fn refresh_known_file(&self, path: &Path) -> Result<bool> {
        self.refresh_notified_file(path, false)
    }

    pub fn refresh_observed_file(&self, path: &Path) -> Result<bool> {
        self.refresh_notified_file(path, true)
    }

    fn refresh_notified_file(&self, path: &Path, discover: bool) -> Result<bool> {
        let refresh = {
            let _guard = self.gate.read().map_err(|_| {
                AppError::new(
                    "internal_error",
                    "状态锁不可用",
                    "App state is unavailable.",
                )
            })?;
            let mut connection = self.connection()?;
            let db = connection.transaction()?;
            let known = db.query_row(
                "SELECT id,identity,modified_at,size FROM resources WHERE kind='file' AND location=? ORDER BY status='available' DESC LIMIT 1",
                [path.to_string_lossy()],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?, r.get::<_, f64>(3)?)),
            ).optional()?;
            if known.is_none() && !discover {
                return Ok(false);
            }
            let (id, identity, previous) = match known {
                Some((id, identity, modified, size)) => {
                    (Some(id), identity, Some((modified, size)))
                }
                None => (None, String::new(), None),
            };
            if !refresh_allowed(self, &db, id.as_deref(), path)? {
                return Ok(false);
            }
            let refresh = FileRefresh {
                store: self,
                epoch: self.scan_epoch.load(Ordering::SeqCst),
                permission: version(&db, "permission_version")?,
                id,
                location: path.to_string_lossy().into_owned(),
                identity,
            };
            (refresh, previous)
        };
        let (mut refresh, previous) = refresh;
        // 删除、重命名和身份替换由完整扫描核对；通知不能让旧引用继承新对象。
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if !metadata.is_file()
            || path.canonicalize()? != path
            || previous.is_some_and(|(modified, size)| {
                crate::catalog::mtime(&metadata) == modified && metadata.len() as f64 == size
            })
        {
            return Ok(false);
        }
        let identity = crate::platform::identity(path, &metadata)?;
        if refresh.id.is_some() && identity != refresh.identity {
            return Ok(false);
        }
        refresh.identity = identity;
        match self.import_one_checked(path, None, false, Some(&refresh.identity), Some(&refresh)) {
            Ok(_) => Ok(true),
            Err(error)
                if matches!(
                    error.code.as_str(),
                    "permission_changed" | "identity_unconfirmed"
                ) =>
            {
                Ok(false)
            }
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    struct Fixture {
        _directory: tempfile::TempDir,
        store: Store,
        root: String,
        path: PathBuf,
        rid: String,
    }

    impl Fixture {
        fn new() -> Self {
            let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
            let directory = tempfile::Builder::new()
                .prefix("known-refresh-")
                .tempdir_in(approved)
                .unwrap();
            let home = directory.path().canonicalize().unwrap();
            let docs = home.join("documents");
            fs::create_dir(&docs).unwrap();
            let path = docs.join("合同.txt");
            fs::write(&path, b"synthetic original").unwrap();
            let store = Store::open(home.join("app/catalog.sqlite3")).unwrap();
            let root = store.grant_root(&docs).unwrap();
            store.scan_all().unwrap();
            let rid = store
                .connection()
                .unwrap()
                .query_row(
                    "SELECT id FROM resources WHERE location=?",
                    [path.to_string_lossy()],
                    |r| r.get(0),
                )
                .unwrap();
            Self {
                _directory: directory,
                store,
                root,
                path,
                rid,
            }
        }

        fn mutate(&self, mutation: Mutation) {
            self.store
                .mutate(MutationRequest {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    mutation,
                })
                .unwrap();
        }

        fn snapshot(&self) -> FileRefresh<'_> {
            let db = self.store.connection().unwrap();
            FileRefresh {
                store: &self.store,
                epoch: self.store.scan_epoch.load(Ordering::SeqCst),
                permission: version(&db, "permission_version").unwrap(),
                id: Some(self.rid.clone()),
                location: self.path.to_string_lossy().into_owned(),
                identity: db
                    .query_row(
                        "SELECT identity FROM resources WHERE id=?",
                        [&self.rid],
                        |r| r.get(0),
                    )
                    .unwrap(),
            }
        }
    }

    #[test]
    fn known_save_refresh_does_not_wait_for_catalog_writer_and_preserves_references() {
        let f = Fixture::new();
        f.mutate(Mutation::Favorite {
            id: f.rid.clone(),
            value: true,
        });
        f.mutate(Mutation::CreateWorkspace {
            name: "合成工作区".into(),
            resource_ids: Some(vec![f.rid.clone()]),
        });
        fs::write(&f.path, b"synthetic changed document with a different size").unwrap();
        let _writer = f.store.catalog_write.lock().unwrap();
        assert!(f.store.refresh_known_file(&f.path).unwrap());
        let db = f.store.connection().unwrap();
        let resource = Store::resource_db(&db, &f.rid, false).unwrap();
        assert!(resource.favorite);
        assert_eq!(resource.workspace_ids.len(), 1);
        assert_eq!(resource.size, fs::metadata(&f.path).unwrap().len() as f64);
        let modified: i64 = db
            .query_row(
                "SELECT count(*) FROM activities WHERE resource_id=? AND kind='file_modified'",
                [&f.rid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(modified, 1);
        let rev = version(&db, "revision").unwrap();
        assert!(!f.store.refresh_known_file(&f.path).unwrap());
        assert_eq!(version(&db, "revision").unwrap(), rev);
        let projection: f64 = db
            .query_row(
                "SELECT modified_at FROM search_documents WHERE entity_id=?",
                [&f.rid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(resource.modified_at, projection);
    }

    #[test]
    fn notification_does_not_discover_new_files_or_inherit_replacement_identity() {
        let f = Fixture::new();
        let new = f.path.with_file_name("新文件.txt");
        fs::write(&new, b"unknown fixture").unwrap();
        assert!(!f.store.refresh_known_file(&new).unwrap());
        fs::rename(&f.path, f.path.with_extension("old")).unwrap();
        fs::write(&f.path, b"replacement fixture").unwrap();
        assert!(!f.store.refresh_known_file(&f.path).unwrap());
        let count: i64 = f
            .store
            .connection()
            .unwrap()
            .query_row("SELECT count(*) FROM resources", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn paused_and_revoked_roots_cannot_refresh_but_independent_grant_can() {
        let f = Fixture::new();
        fs::write(&f.path, b"synthetic changed document").unwrap();
        let guard = f.snapshot();
        f.mutate(Mutation::SetPaused {
            id: f.root.clone(),
            value: true,
        });
        assert!(guard.validate(&f.store.connection().unwrap()).is_err());
        assert!(!f.store.refresh_known_file(&f.path).unwrap());
        f.store
            .import_files(&[f.path.to_string_lossy().into_owned()], None, true)
            .unwrap();
        // 在目录仍授权时，显式选择不额外制造独立授权；撤根后单独授权再刷新。
        f.mutate(Mutation::RevokeRoot { id: f.root.clone() });
        assert!(!f.store.refresh_known_file(&f.path).unwrap());
        f.store
            .import_files(&[f.path.to_string_lossy().into_owned()], None, true)
            .unwrap();
        fs::write(&f.path, b"independently authorized synthetic change").unwrap();
        assert!(f.store.refresh_known_file(&f.path).unwrap());
        f.mutate(Mutation::RevokeFile { id: f.rid.clone() });
        fs::write(&f.path, b"change after all authorization revoked").unwrap();
        assert!(!f.store.refresh_known_file(&f.path).unwrap());
    }

    #[test]
    fn exclusion_blocks_a_pending_refresh_without_creating_a_new_reference() {
        let f = Fixture::new();
        let guard = f.snapshot();
        fs::write(&f.path, b"synthetic excluded change").unwrap();
        f.mutate(Mutation::Exclude {
            root_id: f.root.clone(),
            relative_path: "合同.txt".into(),
        });
        assert!(guard.validate(&f.store.connection().unwrap()).is_err());
        assert!(!f.store.refresh_known_file(&f.path).unwrap());
    }

    #[test]
    fn captured_refresh_rejects_restore_epoch_and_resource_repointing() {
        let f = Fixture::new();
        let guard = f.snapshot();
        f.store.scan_epoch.fetch_add(1, Ordering::SeqCst);
        assert_eq!(
            guard
                .validate(&f.store.connection().unwrap())
                .unwrap_err()
                .code,
            "permission_changed"
        );
        let guard = f.snapshot();
        let db = f.store.connection().unwrap();
        db.execute(
            "UPDATE resources SET location=location||'.changed' WHERE id=?",
            [&f.rid],
        )
        .unwrap();
        assert_eq!(guard.validate(&db).unwrap_err().code, "permission_changed");
    }

    #[test]
    fn new_authorized_file_is_discovered_once_without_inheriting_copy_references() {
        let f = Fixture::new();
        f.mutate(Mutation::Favorite {
            id: f.rid.clone(),
            value: true,
        });
        f.mutate(Mutation::CreateWorkspace {
            name: "原件工作区".into(),
            resource_ids: Some(vec![f.rid.clone()]),
        });
        let copy = f.path.with_file_name("合同副本.txt");
        fs::copy(&f.path, &copy).unwrap();
        assert!(!f.store.refresh_known_file(&copy).unwrap());
        assert!(f.store.refresh_observed_file(&copy).unwrap());
        assert!(!f.store.refresh_observed_file(&copy).unwrap());
        let db = f.store.connection().unwrap();
        let copy_id: String = db
            .query_row(
                "SELECT id FROM resources WHERE location=?",
                [copy.to_string_lossy()],
                |r| r.get(0),
            )
            .unwrap();
        assert_ne!(copy_id, f.rid);
        let copy = Store::resource_db(&db, &copy_id, false).unwrap();
        assert!(!copy.favorite);
        assert!(copy.workspace_ids.is_empty());
        let count: i64 = db
            .query_row(
                "SELECT count(*) FROM activities WHERE resource_id=? AND kind='file_discovered'",
                [&copy_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        let original = Store::resource_db(&db, &f.rid, false).unwrap();
        assert!(original.favorite);
        assert_eq!(original.workspace_ids.len(), 1);
    }

    #[test]
    fn new_file_notifications_respect_default_skips_app_data_and_explicit_root_exclusions() {
        let f = Fixture::new();
        for name in ["node_modules", ".git", ".cache", "排除"] {
            let dir = f.path.parent().unwrap().join(name);
            fs::create_dir(&dir).unwrap();
            let path = dir.join("合同.txt");
            fs::write(&path, b"synthetic excluded notification").unwrap();
            if name == "排除" {
                f.mutate(Mutation::Exclude {
                    root_id: f.root.clone(),
                    relative_path: name.into(),
                });
            }
            assert!(!f.store.refresh_observed_file(&path).unwrap(), "{name}");
        }
        let app_data = f.store.data_directory().join("generated.txt");
        fs::write(&app_data, b"synthetic own data").unwrap();
        f.store
            .grant_root(f.path.parent().unwrap().parent().unwrap())
            .unwrap();
        assert!(!f.store.refresh_observed_file(&app_data).unwrap());
        // 文件名与默认跳过目录同名时仍是普通文件。
        let file = f.path.parent().unwrap().join("ordinary");
        fs::create_dir(&file).unwrap();
        let file = file.join(".git");
        fs::write(&file, b"synthetic ordinary dot-git file").unwrap();
        assert!(f.store.refresh_observed_file(&file).unwrap());
    }

    #[test]
    fn new_file_notification_is_rejected_outside_authorization_or_while_paused() {
        let f = Fixture::new();
        let inside = f.path.with_file_name("新合同.txt");
        let outside = f
            .path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("未授权.txt");
        fs::write(&inside, b"new synthetic file").unwrap();
        fs::write(&outside, b"outside synthetic file").unwrap();
        assert!(!f.store.refresh_observed_file(&outside).unwrap());
        f.mutate(Mutation::SetPaused {
            id: f.root.clone(),
            value: true,
        });
        assert!(!f.store.refresh_observed_file(&inside).unwrap());
        f.mutate(Mutation::SetPaused {
            id: f.root.clone(),
            value: false,
        });
        f.mutate(Mutation::RevokeRoot { id: f.root.clone() });
        assert!(!f.store.refresh_observed_file(&inside).unwrap());
    }

    #[test]
    fn new_file_guard_rechecks_restore_permission_and_competing_discovery() {
        let f = Fixture::new();
        let path = f.path.with_file_name("新合同.txt");
        fs::write(&path, b"new synthetic file").unwrap();
        let mut guard = f.snapshot();
        guard.id = None;
        guard.location = path.to_string_lossy().into_owned();
        guard.identity = crate::platform::identity(&path, &fs::metadata(&path).unwrap()).unwrap();
        assert!(guard.validate(&f.store.connection().unwrap()).is_ok());
        assert!(f.store.refresh_observed_file(&path).unwrap());
        assert!(guard.validate(&f.store.connection().unwrap()).is_err());
        guard.location = f
            .path
            .with_file_name("另一个.txt")
            .to_string_lossy()
            .into_owned();
        f.store.scan_epoch.fetch_add(1, Ordering::SeqCst);
        assert!(guard.validate(&f.store.connection().unwrap()).is_err());
    }

    #[test]
    fn notified_discovery_marks_current_scan_generation_as_observed() {
        let f = Fixture::new();
        let db = f.store.connection().unwrap();
        db.execute(
            "UPDATE scan_roots SET generation=generation+1,scan_state='running' WHERE id=?",
            [&f.root],
        )
        .unwrap();
        let path = f.path.with_file_name("枚举后创建.txt");
        fs::write(&path, b"synthetic late discovery").unwrap();
        assert!(f.store.refresh_observed_file(&path).unwrap());
        let (seen, current): (i64,i64) = db.query_row("SELECT f.generation,s.generation FROM file_roots f JOIN scan_roots s ON s.id=f.root_id JOIN resources r ON r.id=f.resource_id WHERE r.location=?", [path.to_string_lossy()], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(
            seen, current,
            "current verified presence must survive the older enumeration's missing check"
        );
    }
}
