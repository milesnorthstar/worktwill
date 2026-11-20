use serde::Deserialize;
use worktwill_core::{
    model::{AppError, Result},
    SearchCancellation,
};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchLane {
    Quick,
    Page,
}
#[derive(Default)]
struct Lane {
    latest: Option<(u32, SearchCancellation)>,
}
impl Lane {
    fn begin(&mut self, request_id: u32) -> Result<SearchCancellation> {
        if self
            .latest
            .as_ref()
            .is_some_and(|(id, _)| request_id <= *id)
        {
            return Err(cancelled());
        }
        self.reset();
        let token = SearchCancellation::default();
        self.latest = Some((request_id, token.clone()));
        Ok(token)
    }
    fn cancel(&mut self, request_id: u32) -> bool {
        if let Some((id, token)) = &self.latest {
            if request_id < *id {
                return false;
            }
            if request_id == *id {
                token.cancel();
                return true;
            }
        }
        self.reset();
        let token = SearchCancellation::default();
        token.cancel();
        // 取消可能先于搜索命令到达；保留最高 ID，迟到命令不能重新运行。
        self.latest = Some((request_id, token));
        true
    }
    fn reset(&mut self) {
        if let Some((_, token)) = self.latest.take() {
            token.cancel();
        }
    }
}
fn cancelled() -> AppError {
    AppError::new("search_cancelled", "搜索已取消", "Search cancelled.")
}

#[derive(Default)]
pub struct SearchRequests {
    session: Option<String>,
    quick: Lane,
    page: Lane,
}
impl SearchRequests {
    pub fn begin_session(&mut self) -> String {
        self.quick.reset();
        self.page.reset();
        let session = uuid::Uuid::new_v4().to_string();
        self.session = Some(session.clone());
        session
    }
    fn lane(&mut self, session: &str, lane: SearchLane) -> Result<&mut Lane> {
        if self.session.as_deref() != Some(session) {
            return Err(cancelled());
        }
        Ok(match lane {
            SearchLane::Quick => &mut self.quick,
            SearchLane::Page => &mut self.page,
        })
    }
    pub fn begin(
        &mut self,
        session: &str,
        lane: SearchLane,
        request_id: u32,
    ) -> Result<SearchCancellation> {
        self.lane(session, lane)?.begin(request_id)
    }
    pub fn cancel(&mut self, session: &str, lane: SearchLane, request_id: u32) -> Result<bool> {
        Ok(self.lane(session, lane)?.cancel(request_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn newer_requests_cancel_only_their_lane_and_late_older_requests_cannot_take_over() {
        let mut requests = SearchRequests::default();
        let session = requests.begin_session();
        let old = requests.begin(&session, SearchLane::Quick, 1).unwrap();
        let page = requests.begin(&session, SearchLane::Page, 1).unwrap();
        let new = requests.begin(&session, SearchLane::Quick, 3).unwrap();
        assert!(old.is_cancelled());
        assert!(!page.is_cancelled());
        assert!(requests.begin(&session, SearchLane::Quick, 2).is_err());
        assert!(requests.begin(&session, SearchLane::Quick, 3).is_err());
        assert!(!requests.cancel(&session, SearchLane::Quick, 1).unwrap());
        assert!(!new.is_cancelled());
        assert!(requests.cancel(&session, SearchLane::Quick, 3).unwrap());
        assert!(new.is_cancelled());
        assert!(!page.is_cancelled());
    }
    #[test]
    fn cancellation_before_spawn_or_even_before_command_arrival_cannot_be_lost() {
        let mut requests = SearchRequests::default();
        let session = requests.begin_session();
        let before_spawn = requests.begin(&session, SearchLane::Quick, 1).unwrap();
        requests.cancel(&session, SearchLane::Quick, 1).unwrap();
        assert_eq!(before_spawn.check().unwrap_err().code, "search_cancelled");
        let superseded = requests.begin(&session, SearchLane::Quick, 2).unwrap();
        requests.cancel(&session, SearchLane::Quick, 5).unwrap();
        assert!(superseded.is_cancelled());
        assert!(requests.begin(&session, SearchLane::Quick, 4).is_err());
        assert!(requests.begin(&session, SearchLane::Quick, 5).is_err());
        let current = requests.begin(&session, SearchLane::Quick, 6).unwrap();
        assert!(!requests.cancel(&session, SearchLane::Quick, 5).unwrap());
        assert!(!current.is_cancelled());
    }
    #[test]
    fn reloading_session_cancels_both_lanes_resets_ids_and_rejects_old_session() {
        let mut requests = SearchRequests::default();
        assert!(requests.begin("unstarted", SearchLane::Quick, 1).is_err());
        let old_session = requests.begin_session();
        let old_quick = requests
            .begin(&old_session, SearchLane::Quick, 100)
            .unwrap();
        let old_page = requests.begin(&old_session, SearchLane::Page, 200).unwrap();
        let new_session = requests.begin_session();
        assert_ne!(old_session, new_session);
        assert!(old_quick.is_cancelled());
        assert!(old_page.is_cancelled());
        let new = requests.begin(&new_session, SearchLane::Quick, 1).unwrap();
        assert!(requests
            .begin(&old_session, SearchLane::Quick, 999)
            .is_err());
        assert!(requests
            .cancel(&old_session, SearchLane::Quick, 999)
            .is_err());
        assert!(!new.is_cancelled());
    }
}
