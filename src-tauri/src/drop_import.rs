//! 原生拖入只保存短时路径凭据；前端不能以任意路径建立文件授权。
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use worktwill_core::{
    model::{AppError, Result},
    ImportResult, ImportSelection,
};

const DROP_TTL: Duration = Duration::from_secs(300);
const MAX_PATHS: usize = 1000;

#[derive(Default)]
pub struct DropInbox {
    pending: Option<PendingDrop>,
    hover_count: usize,
}

struct PendingDrop {
    token: String,
    paths: Vec<ImportSelection>,
    created_at: Instant,
    target: Option<Option<String>>,
    in_flight: bool,
    prepared: bool,
}

impl DropInbox {
    pub fn hover(&mut self, count: usize) {
        self.hover_count = count;
    }
    pub fn hover_count(&self) -> usize {
        self.hover_count
    }
    pub fn leave(&mut self) {
        self.hover_count = 0;
    }

    pub fn reserve(
        &mut self,
        paths: Vec<String>,
        now: Instant,
    ) -> Result<(String, usize, Vec<String>)> {
        self.leave();
        if paths.is_empty() || paths.len() > MAX_PATHS {
            return Err(AppError::new(
                "invalid_input",
                "每次请拖入 1–1000 个文件",
                "Drop between 1 and 1,000 files at a time.",
            ));
        }
        let mut seen = HashSet::new();
        let paths: Vec<_> = paths
            .into_iter()
            .filter(|path| seen.insert(path.clone()))
            .collect();
        let token = uuid::Uuid::new_v4().to_string();
        let count = paths.len();
        // 新的拖入替换旧凭据；旧任务即使已提交，其完成回包也不能覆盖新会话。
        self.pending = Some(PendingDrop {
            token: token.clone(),
            paths: vec![],
            created_at: now,
            target: None,
            in_flight: false,
            prepared: false,
        });
        Ok((token, count, paths))
    }

    pub fn captured(&mut self, token: &str, selections: Vec<ImportSelection>) -> bool {
        let Some(drop) = self
            .pending
            .as_mut()
            .filter(|drop| drop.token == token && !drop.prepared)
        else {
            return false;
        };
        drop.paths = selections;
        drop.prepared = true;
        true
    }

    pub fn begin(
        &mut self,
        token: &str,
        workspace: Option<&str>,
        now: Instant,
    ) -> Result<Vec<ImportSelection>> {
        if self
            .pending
            .as_ref()
            .is_some_and(|drop| now.saturating_duration_since(drop.created_at) >= DROP_TTL)
        {
            self.pending = None;
        }
        let drop = self
            .pending
            .as_mut()
            .filter(|drop| drop.token == token)
            .ok_or_else(|| {
                AppError::new(
                    "expired_drop",
                    "拖入记录已过期，请重新拖入",
                    "The drop has expired. Drop the files again.",
                )
            })?;
        if !drop.prepared {
            return Err(AppError::new(
                "drop_preparing",
                "正在确认拖入文件，请稍候",
                "The dropped files are being checked. Please wait.",
            ));
        }
        if drop.in_flight {
            return Err(AppError::new(
                "drop_in_progress",
                "正在处理这次拖入，请稍候",
                "This drop is being processed. Please wait.",
            ));
        }
        let target = workspace.map(str::to_owned);
        if drop
            .target
            .as_ref()
            .is_some_and(|original| original != &target)
        {
            return Err(AppError::new(
                "drop_target_changed",
                "拖入目标已变化，请重新拖入",
                "The drop destination has changed. Drop the files again.",
            ));
        }
        drop.target = Some(target);
        drop.in_flight = true;
        Ok(drop.paths.clone())
    }

    pub fn finish(&mut self, token: &str, result: &ImportResult) {
        let Some(drop) = self
            .pending
            .as_mut()
            .filter(|drop| drop.token == token && drop.in_flight)
        else {
            return;
        };
        let unapproved: HashSet<_> = result
            .items
            .iter()
            .filter(|item| item.status == "not_authorized")
            .map(|item| item.path.as_str())
            .collect();
        // 只允许尚未授权的原始候选重试，成功和失败项都不能被同一凭据再次消费。
        drop.paths
            .retain(|selection| unapproved.contains(selection.path.as_str()));
        drop.in_flight = false;
        if drop.paths.is_empty() {
            self.pending = None;
        }
    }

    pub fn failed(&mut self, token: &str) {
        if let Some(drop) = self.pending.as_mut().filter(|drop| drop.token == token) {
            drop.in_flight = false;
        }
    }

    pub fn cancel(&mut self, token: &str) -> bool {
        if self
            .pending
            .as_ref()
            .is_some_and(|drop| drop.token == token)
        {
            // 仅丢弃凭据，不撤销或中止已经执行的业务事务。
            self.pending = None;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use worktwill_core::{capture_import_selections, ImportItem};
    fn register(
        inbox: &mut DropInbox,
        paths: Vec<String>,
        now: Instant,
    ) -> Result<(String, usize)> {
        let (token, count, paths) = inbox.reserve(paths, now)?;
        assert!(inbox.captured(&token, capture_import_selections(&paths)));
        Ok((token, count))
    }
    fn result(items: &[(&str, &str)]) -> ImportResult {
        ImportResult {
            items: items
                .iter()
                .map(|(path, status)| ImportItem {
                    path: (*path).into(),
                    status: (*status).into(),
                    resource_id: None,
                    message: None,
                    message_en: None,
                })
                .collect(),
        }
    }

    #[test]
    fn preparation_is_bound_to_latest_token_and_can_be_cancelled() {
        let mut inbox = DropInbox::default();
        let now = Instant::now();
        let (old, _, paths) = inbox.reserve(vec!["old".into()], now).unwrap();
        assert_eq!(
            inbox.begin(&old, None, now).unwrap_err().code,
            "drop_preparing"
        );
        let (new, _, new_paths) = inbox.reserve(vec!["new".into()], now).unwrap();
        assert!(!inbox.captured(&old, capture_import_selections(&paths)));
        assert!(inbox.captured(&new, capture_import_selections(&new_paths)));
        assert!(!inbox.captured(&new, vec![]));
        let (cancelled, _, cancelled_paths) = inbox.reserve(vec!["cancelled".into()], now).unwrap();
        assert!(inbox.cancel(&cancelled));
        assert!(!inbox.captured(&cancelled, capture_import_selections(&cancelled_paths)));
    }

    #[test]
    fn rejects_forged_expired_and_parallel_consumption() {
        let mut inbox = DropInbox::default();
        let now = Instant::now();
        let (token, _) = register(&mut inbox, vec!["a".into()], now).unwrap();
        assert_eq!(
            inbox.begin("forged", None, now).unwrap_err().code,
            "expired_drop"
        );
        assert_eq!(
            inbox
                .begin(&token, None, now)
                .unwrap()
                .into_iter()
                .map(|item| item.path)
                .collect::<Vec<_>>(),
            vec!["a"]
        );
        assert_eq!(
            inbox.begin(&token, None, now).unwrap_err().code,
            "drop_in_progress"
        );
        inbox.failed(&token);
        assert_eq!(
            inbox.begin(&token, None, now + DROP_TTL).unwrap_err().code,
            "expired_drop"
        );
    }

    #[test]
    fn partial_success_retries_only_unapproved_and_binds_original_workspace() {
        let mut inbox = DropInbox::default();
        let now = Instant::now();
        let (token, count) = register(
            &mut inbox,
            vec!["a".into(), "b".into(), "c".into(), "a".into()],
            now,
        )
        .unwrap();
        assert_eq!(count, 3);
        inbox.begin(&token, Some("workspace-a"), now).unwrap();
        inbox.finish(
            &token,
            &result(&[("a", "added"), ("b", "not_authorized"), ("c", "failed")]),
        );
        assert_eq!(
            inbox
                .begin(&token, Some("workspace-b"), now)
                .unwrap_err()
                .code,
            "drop_target_changed"
        );
        assert_eq!(
            inbox
                .begin(&token, Some("workspace-a"), now)
                .unwrap()
                .into_iter()
                .map(|item| item.path)
                .collect::<Vec<_>>(),
            vec!["b"]
        );
        inbox.finish(&token, &result(&[("b", "added")]));
        assert_eq!(
            inbox
                .begin(&token, Some("workspace-a"), now)
                .unwrap_err()
                .code,
            "expired_drop"
        );
    }

    #[test]
    fn cancellation_discards_only_matching_credentials_even_during_work() {
        let mut inbox = DropInbox::default();
        let now = Instant::now();
        let (token, _) = register(&mut inbox, vec!["a".into()], now).unwrap();
        inbox.begin(&token, None, now).unwrap();
        assert!(!inbox.cancel("forged"));
        assert!(inbox.cancel(&token));
        inbox.finish(&token, &result(&[("a", "not_authorized")]));
        assert!(inbox.begin(&token, None, now).is_err());
    }

    #[test]
    fn old_completion_failure_or_cancel_cannot_replace_new_drop() {
        let mut inbox = DropInbox::default();
        let now = Instant::now();
        let (old, _) = register(&mut inbox, vec!["old".into()], now).unwrap();
        inbox.begin(&old, None, now).unwrap();
        let (new, _) = register(&mut inbox, vec!["new".into()], now).unwrap();
        inbox.finish(&old, &result(&[("old", "not_authorized")]));
        inbox.failed(&old);
        assert!(!inbox.cancel(&old));
        assert_eq!(
            inbox
                .begin(&new, None, now)
                .unwrap()
                .into_iter()
                .map(|item| item.path)
                .collect::<Vec<_>>(),
            vec!["new"]
        );
    }

    #[test]
    fn rejects_invalid_batch_and_clears_hover_without_consuming_pending() {
        let mut inbox = DropInbox::default();
        let now = Instant::now();
        inbox.hover(4);
        assert_eq!(inbox.hover_count(), 4);
        inbox.leave();
        assert_eq!(inbox.hover_count(), 0);
        assert!(register(&mut inbox, vec![], now).is_err());
        assert!(register(&mut inbox, vec!["a".into(); 1001], now).is_err());
        let (token, _) = register(&mut inbox, vec!["a".into()], now).unwrap();
        inbox.hover(2);
        inbox.leave();
        assert_eq!(
            inbox
                .begin(&token, None, now)
                .unwrap()
                .into_iter()
                .map(|item| item.path)
                .collect::<Vec<_>>(),
            vec!["a"]
        );
    }
}
