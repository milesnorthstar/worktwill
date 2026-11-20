//! 打开请求与活动记账共享同一对象快照；不跨IPC，也不跨系统打开持锁。
use crate::{
    model::*,
    platform,
    store::{bump, event, version},
    Store,
};
use rusqlite::{OptionalExtension, TransactionBehavior};
use std::{path::Path, sync::atomic::Ordering};

/// 仅供原生宿主使用；按值消费且不可克隆，不能用前端传入的路径构造。
pub struct ResolvedOpen {
    resource_id: String,
    kind: String,
    location: String,
    identity: Option<String>,
    observed_name: String,
    permission_version: u32,
    restore_epoch: u64,
    session: String,
}

impl ResolvedOpen {
    pub fn location(&self) -> &str {
        &self.location
    }

    pub fn is_web(&self) -> bool {
        self.kind == "web"
    }
}

fn changed() -> AppError {
    AppError::new(
        "open_record_changed",
        "系统已接受打开请求，但资料或授权已变化，本次活动未记录",
        "The system accepted the open request, but the resource or its authorization changed. This activity was not recorded.",
    )
}

fn unrecorded(mut error: AppError) -> AppError {
    if error.code != "open_record_changed" {
        // 存储失败仍保留实际错误码，不能让已成功的系统打开被描述为未接受。
        error.message = format!("系统已接受打开请求，但活动未能记录：{}", error.message);
        error.message_en = format!(
            "The system accepted the open request, but its activity could not be recorded: {}",
            error.message_en
        );
    }
    error
}

impl Store {
    pub fn prepare_open(&self, rid: &str) -> Result<ResolvedOpen> {
        // 原生定位可能提交新位置；最终对象与版本仍须在下面的同一读事务重新取得。
        self.resolve_resource(rid)?;
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut connection = self.connection()?;
        let db = connection.transaction()?;
        let (location, web) = Self::resolve_resource_db(&db, rid)?;
        if !web {
            platform::safe_open(Path::new(&location))?;
        }
        let (kind, observed_name, identity) = db.query_row(
            "SELECT kind,name,identity FROM resources WHERE id=?",
            [rid],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        Ok(ResolvedOpen {
            resource_id: rid.into(),
            kind,
            location,
            identity,
            observed_name,
            permission_version: version(&db, "permission_version")?,
            restore_epoch: self.scan_epoch.load(Ordering::SeqCst),
            session: self.session.clone(),
        })
    }

    /// 仅在系统接受打开请求后调用；消费快照，不把同一UUID的新对象记成已打开。
    pub fn record_open_checked(&self, target: ResolvedOpen) -> Result<()> {
        self.record_open_target(target).map_err(unrecorded)
    }

    fn record_open_target(&self, target: ResolvedOpen) -> Result<()> {
        let _guard = self.gate.write().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        if self.session != target.session
            || self.scan_epoch.load(Ordering::SeqCst) != target.restore_epoch
        {
            return Err(changed());
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if version(&tx, "permission_version")? != target.permission_version {
            return Err(changed());
        }
        let current: Option<(String, Option<String>)> = tx
            .query_row(
                "SELECT kind,identity FROM resources WHERE id=?",
                [&target.resource_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((kind, identity)) = current else {
            return Err(changed());
        };
        if kind != target.kind || identity != target.identity {
            return Err(changed());
        }
        // 当前授权、canonical路径及物理身份都再核对；不以全revision拒绝无关扫描或打开。
        let (location, web) =
            Self::resolve_resource_db(&tx, &target.resource_id).map_err(|error| {
                if matches!(
                    error.code.as_str(),
                    "not_authorized"
                        | "identity_unconfirmed"
                        | "resource_unavailable"
                        | "invalid_input"
                ) {
                    changed()
                } else {
                    error
                }
            })?;
        if location != target.location || web != target.is_web() {
            return Err(changed());
        }
        event(
            &tx,
            "opened",
            &target.observed_name,
            Some(&target.resource_id),
        )?;
        bump(&tx, false)?;
        tx.commit()?;
        Ok(())
    }
}
