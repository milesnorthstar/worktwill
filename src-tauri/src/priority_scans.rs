//! 用户指定目录的有界队列；工作标记和出入队同锁更新，避免退出时遗失新请求。
use std::collections::VecDeque;

const CAPACITY: usize = 16;

#[derive(Debug, PartialEq, Eq)]
pub enum Admission {
    Start,
    Queued,
    Full,
}

#[derive(Default)]
pub struct PriorityScans {
    roots: VecDeque<String>,
    working: bool,
}

impl PriorityScans {
    pub fn request(&mut self, root: &str) -> Admission {
        if self.roots.iter().any(|queued| queued == root) {
            return Admission::Queued;
        }
        if self.roots.len() == CAPACITY {
            return Admission::Full;
        }
        self.roots.push_back(root.into());
        if self.working {
            Admission::Queued
        } else {
            self.working = true;
            Admission::Start
        }
    }

    pub fn next(&mut self) -> Option<String> {
        let root = self.roots.pop_front();
        if root.is_none() {
            self.working = false;
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_deduplicates_bounds_pending_roots_and_keeps_fifo() {
        let mut queue = PriorityScans::default();
        assert_eq!(queue.request("0"), Admission::Start);
        for index in 1..CAPACITY {
            assert_eq!(queue.request(&index.to_string()), Admission::Queued);
        }
        assert_eq!(queue.request("0"), Admission::Queued);
        assert_eq!(queue.request("overflow"), Admission::Full);
        for index in 0..CAPACITY {
            assert_eq!(queue.next(), Some(index.to_string()));
        }
        assert_eq!(queue.next(), None);
    }

    #[test]
    fn request_during_execution_uses_same_worker_and_after_exit_starts_a_new_worker() {
        let mut queue = PriorityScans::default();
        assert_eq!(queue.request("a"), Admission::Start);
        assert_eq!(queue.next().as_deref(), Some("a"));
        assert_eq!(queue.request("b"), Admission::Queued);
        assert_eq!(queue.next().as_deref(), Some("b"));
        assert_eq!(queue.next(), None);
        assert_eq!(queue.request("c"), Admission::Start);
    }
}
