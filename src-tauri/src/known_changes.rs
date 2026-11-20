//! 有界合并通知路径；溢出仍由已有的全量扫描兜住，不为每条通知启动任务。
use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
};

const CAPACITY: usize = 256;

#[derive(Default)]
pub struct KnownChanges {
    paths: VecDeque<PathBuf>,
    unique: HashSet<PathBuf>,
}

impl KnownChanges {
    pub fn extend(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        for path in paths {
            if self.unique.contains(&path) {
                continue;
            }
            if self.paths.len() == CAPACITY {
                if let Some(oldest) = self.paths.pop_front() {
                    self.unique.remove(&oldest);
                }
            }
            self.unique.insert(path.clone());
            self.paths.push_back(path);
        }
    }

    pub fn take(&mut self) -> Vec<PathBuf> {
        self.unique.clear();
        self.paths.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_flood_is_bounded_and_paths_are_deduplicated() {
        let mut changes = KnownChanges::default();
        changes.extend((0..1000).map(|n| PathBuf::from(n.to_string())));
        changes.extend([PathBuf::from("0")]);
        assert_eq!(changes.take().len(), CAPACITY);
        assert!(changes.take().is_empty());
        changes.extend([PathBuf::from("a"), PathBuf::from("a")]);
        assert_eq!(changes.take(), vec![PathBuf::from("a")]);
    }
}
