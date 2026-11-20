//! 原生重新指向流程的影响快照；不跨IPC传递，不构成文件授权。
use crate::{model::*, store::version, ImportSelection, Store};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelinkWorkspace {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelinkImpact {
    pub resource_id: String,
    pub name: String,
    pub authorized: bool,
    pub favorite: bool,
    pub workspaces: Vec<RelinkWorkspace>,
    identity: Option<String>,
    location: String,
    cached_name: String,
    latest_relink_event: Option<String>,
}

#[derive(Clone)]
pub struct RelinkReview {
    pub source: RelinkImpact,
    pub target: Option<RelinkImpact>,
    permission_version: u32,
    candidate: Option<(PathBuf, String)>,
}

fn changed() -> AppError {
    AppError::new(
        "permission_changed",
        "资料或受影响的工作区已变化，请重新确认",
        "The file or affected workspaces have changed. Review the change again.",
    )
}

fn impact_db(db: &Connection, rid: &str) -> Result<RelinkImpact> {
    let row: Option<(String, String, Option<String>, bool)> = db
        .query_row(
            "SELECT name,location,identity,favorite FROM resources WHERE id=? AND kind='file'",
            [rid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((cached_name, location, identity, favorite)) = row else {
        return Err(AppError::new(
            "conflict",
            "资源已不存在",
            "The resource no longer exists.",
        ));
    };
    let visible = Store::resource_db(db, rid, true)?;
    let workspaces=db.prepare("SELECT w.id,w.name FROM workspaces w JOIN workspace_resources wr ON wr.workspace_id=w.id WHERE wr.resource_id=? ORDER BY w.id")?.query_map([rid],|r|Ok(RelinkWorkspace{id:r.get(0)?,name:r.get(1)?}))?.collect::<rusqlite::Result<_>>()?;
    Ok(RelinkImpact {
        resource_id: rid.into(),
        name: visible.name,
        authorized: visible.status != "permission_denied",
        favorite,
        workspaces,
        identity,
        location,
        cached_name,
        latest_relink_event: db.query_row("SELECT id FROM activities WHERE resource_id=? AND kind='relinked' ORDER BY at DESC,id DESC LIMIT 1",[rid],|r|r.get(0)).optional()?,
    })
}

fn target_db(db: &Connection, rid: &str, identity: &str) -> Result<Option<RelinkImpact>> {
    let id: Option<String> = db
        .query_row(
            "SELECT id FROM resources WHERE identity=?1 AND id<>?2",
            params![identity, rid],
            |r| r.get(0),
        )
        .optional()?;
    id.map(|id| impact_db(db, &id)).transpose()
}

impl RelinkReview {
    pub fn selected_path(&self) -> Option<&Path> {
        self.candidate.as_ref().map(|(path, _)| path.as_path())
    }

    pub(crate) fn validate(&self, db: &Connection) -> Result<()> {
        // 只比较实际影响及授权版本；无关扫描/打开不使确认失效，引用增减或改名则必须重看。
        if version(db, "permission_version")? != self.permission_version
            || impact_db(db, &self.source.resource_id).map_err(|error| {
                if error.code == "conflict" {
                    changed()
                } else {
                    error
                }
            })? != self.source
        {
            return Err(changed());
        }
        if let Some((_, identity)) = &self.candidate {
            if target_db(db, &self.source.resource_id, identity)? != self.target {
                return Err(changed());
            }
        }
        Ok(())
    }
}

impl Store {
    pub fn review_relink(&self, rid: &str) -> Result<RelinkReview> {
        let _guard = self.gate.read().map_err(|_| changed())?;
        let mut connection = self.connection()?;
        let db = connection.transaction()?;
        Ok(RelinkReview {
            source: impact_db(&db, rid)?,
            target: None,
            permission_version: version(&db, "permission_version")?,
            candidate: None,
        })
    }

    pub fn review_relink_selection(
        &self,
        review: &RelinkReview,
        selection: &ImportSelection,
    ) -> Result<RelinkReview> {
        if let Some(error) = &selection.capture_error {
            return Err(error.clone());
        }
        let identity = selection.identity.as_deref().ok_or_else(|| {
            AppError::new(
                "identity_unconfirmed",
                "无法确认所选文件身份，请重新选择",
                "The selected file identity could not be confirmed. Select it again.",
            )
        })?;
        let path = Path::new(&selection.path).canonicalize()?;
        let meta = path.metadata()?;
        if !meta.is_file() || crate::platform::identity(&path, &meta)? != identity {
            return Err(AppError::new(
                "identity_unconfirmed",
                "所选文件已被替换，请重新选择",
                "The selected file has been replaced. Select it again.",
            ));
        }
        let _guard = self.gate.read().map_err(|_| changed())?;
        let mut connection = self.connection()?;
        let db = connection.transaction()?;
        review.validate(&db)?;
        Ok(RelinkReview {
            source: review.source.clone(),
            target: target_db(&db, &review.source.resource_id, identity)?,
            permission_version: review.permission_version,
            candidate: Some((path, identity.into())),
        })
    }

    pub fn relink_reviewed(&self, review: &RelinkReview, merge: bool) -> Result<()> {
        let (path, identity) = review.candidate.as_ref().ok_or_else(|| {
            AppError::new(
                "invalid_input",
                "请明确选择原文件",
                "Select the original file.",
            )
        })?;
        self.relink_impl(
            &review.source.resource_id,
            path,
            merge,
            true,
            Some(identity),
            Some(review),
        )
    }
}
