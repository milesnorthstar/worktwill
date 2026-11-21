use crate::model::{AppError, Result};
use rusqlite::Connection;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// 每个搜索请求独立持有；取消后不可复用，不能影响其他 SQLite 连接。
#[derive(Clone, Default)]
pub struct SearchCancellation {
    cancelled: Arc<AtomicBool>,
    progress_ticks: Arc<std::sync::atomic::AtomicUsize>,
    interrupted: Arc<std::sync::atomic::AtomicUsize>,
}
impl SearchCancellation {
    /// 仅包含 VM 计数，不包含查询或资料信息；用于核验真实执行中的取消。
    pub fn execution_counts(&self) -> (usize, usize) {
        (
            self.progress_ticks.load(Ordering::SeqCst),
            self.interrupted.load(Ordering::SeqCst),
        )
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(AppError::new(
                "search_cancelled",
                "搜索已取消",
                "Search cancelled.",
            ))
        } else {
            Ok(())
        }
    }
    pub(crate) fn with_connection<T>(
        &self,
        db: &Connection,
        query: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        self.check()?;
        let cancellation = self.clone();
        // SQLite 在执行中主动停止 VM；不是仅丢弃已算完的旧响应。
        db.progress_handler(
            1000,
            Some(move || {
                cancellation.progress_ticks.fetch_add(1, Ordering::SeqCst);
                let cancelled = cancellation.is_cancelled();
                if cancelled {
                    cancellation.interrupted.fetch_add(1, Ordering::SeqCst);
                }
                cancelled
            }),
        );
        let _handler = ProgressGuard(db);
        let result = query();
        self.check()?;
        result
    }
}
struct ProgressGuard<'a>(&'a Connection);
impl Drop for ProgressGuard<'_> {
    fn drop(&mut self) {
        self.0.progress_handler(0, None::<fn() -> bool>);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        thread,
        time::{Duration, Instant},
    };

    #[test]
    fn cancellation_interrupts_an_already_executing_vm_and_leaves_connections_usable() {
        let db = Connection::open_in_memory().unwrap();
        let sibling = Connection::open_in_memory().unwrap();
        let cancellation = SearchCancellation::default();
        let worker = cancellation.clone();
        let cancel_thread = thread::spawn(move || {
            let start = Instant::now();
            while worker.progress_ticks.load(Ordering::SeqCst) == 0 {
                assert!(
                    start.elapsed() < Duration::from_secs(5),
                    "query never executed"
                );
                thread::yield_now();
            }
            worker.cancel();
        });
        let start = Instant::now();
        let result: Result<i64> = cancellation.with_connection(&db, || {
            let error = db.query_row("WITH RECURSIVE work(n) AS (VALUES(0) UNION ALL SELECT n+1 FROM work WHERE n<100000000) SELECT sum(n) FROM work", [], |r| r.get::<_, i64>(0)).unwrap_err();
            assert_eq!(error.sqlite_error_code(), Some(rusqlite::ErrorCode::OperationInterrupted));
            Err(error.into())
        });
        cancel_thread.join().unwrap();
        assert_eq!(result.unwrap_err().code, "search_cancelled");
        assert!(start.elapsed() < Duration::from_secs(5));
        assert_eq!(
            db.query_row("SELECT 42", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            42
        );
        assert_eq!(
            sibling
                .query_row("SELECT 43", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            43
        );
        assert!(SearchCancellation::default()
            .with_connection(&db, || Ok(44))
            .is_ok());
    }

    #[test]
    fn public_store_quick_and_page_queries_really_interrupt_their_private_vm() {
        use crate::{
            model::{SearchPageRequest, SearchRequest},
            Store,
        };
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("search-running-cancel-")
            .tempdir_in(approved)
            .unwrap();
        let path = directory.path().join("app/catalog.sqlite3");
        let store = Store::open(&path).unwrap();
        let db = Connection::open(&path).unwrap();
        // 为真实 Store 查询设置确定的慢 SQL；不依赖机器速度或候选算法的实现细节。
        db.execute_batch("ALTER TABLE settings RENAME TO settings_saved; CREATE VIEW settings AS SELECT key,value+(WITH RECURSIVE work(n) AS (VALUES(0) UNION ALL SELECT n+1 FROM work WHERE n<100000000) SELECT sum(n)*0 FROM work) AS value FROM settings_saved;").unwrap();
        for paged in [false, true] {
            let cancellation = SearchCancellation::default();
            let worker = cancellation.clone();
            let cancel_thread = thread::spawn(move || {
                let start = Instant::now();
                while worker.progress_ticks.load(Ordering::SeqCst) == 0 {
                    assert!(
                        start.elapsed() < Duration::from_secs(5),
                        "Store query never executed"
                    );
                    thread::yield_now();
                }
                worker.cancel();
            });
            let request = SearchRequest {
                request_id: 1,
                query: if paged {
                    "missing".into()
                } else {
                    String::new()
                },
                workspace_id: None,
                kind: "all".into(),
            };
            let result = if paged {
                store.search_page_with_cancellation(
                    SearchPageRequest {
                        request,
                        offset: 0,
                        sort: "relevance".into(),
                        cursor: None,
                        modified_from: None,
                        modified_to: None,
                        path_prefix: None,
                    },
                    &cancellation,
                )
            } else {
                store.search_with_cancellation(request, &cancellation)
            };
            cancel_thread.join().unwrap();
            assert_eq!(result.unwrap_err().code, "search_cancelled");
            assert!(
                cancellation.interrupted.load(Ordering::SeqCst) > 0,
                "Store query completed without a VM interrupt"
            );
        }
        db.execute_batch("DROP VIEW settings; ALTER TABLE settings_saved RENAME TO settings;")
            .unwrap();
        assert!(store
            .search(SearchRequest {
                request_id: 2,
                query: "missing".into(),
                workspace_id: None,
                kind: "all".into()
            })
            .unwrap()
            .page
            .items
            .is_empty());
    }

    #[test]
    fn cancelling_before_binding_never_starts_the_query() {
        let db = Connection::open_in_memory().unwrap();
        let cancellation = SearchCancellation::default();
        cancellation.cancel();
        let result: Result<()> =
            cancellation.with_connection(&db, || panic!("cancelled query ran"));
        assert_eq!(result.unwrap_err().code, "search_cancelled");
    }

    #[test]
    fn public_queries_keep_items_and_versions_in_one_snapshot_during_external_commit() {
        use crate::{model::*, Store};
        let approved = std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed fixture root");
        let directory = tempfile::Builder::new()
            .prefix("search-snapshot-")
            .tempdir_in(approved)
            .unwrap();
        let path = directory.path().join("app/catalog.sqlite3");
        let store = Store::open(&path).unwrap();
        store
            .mutate(MutationRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                mutation: Mutation::SaveWeb {
                    id: None,
                    name: "needle".into(),
                    url: "https://example.com".into(),
                    workspace_id: None,
                },
            })
            .unwrap();
        let db = Connection::open(&path).unwrap();
        // 在正式读取版本的 SQL 中制造时间窗口，由另一连接真实提交，不替换查询结果。
        db.execute_batch("ALTER TABLE settings RENAME TO settings_saved; CREATE VIEW settings AS SELECT key,value+(WITH RECURSIVE work(n) AS (VALUES(0) UNION ALL SELECT n+1 FROM work WHERE n<300000) SELECT sum(n)*0 FROM work) AS value FROM settings_saved;").unwrap();
        for paged in [false, true] {
            db.execute_batch(
                "UPDATE resources SET favorite=0; UPDATE settings_saved SET value=10;",
            )
            .unwrap();
            let cancellation = SearchCancellation::default();
            let worker = cancellation.clone();
            let path = path.clone();
            let writer = thread::spawn(move || {
                let start = Instant::now();
                while worker.progress_ticks.load(Ordering::SeqCst) == 0 {
                    assert!(
                        start.elapsed() < Duration::from_secs(5),
                        "query never executed"
                    );
                    thread::yield_now();
                }
                let db = Connection::open(path).unwrap();
                db.execute_batch("BEGIN IMMEDIATE; UPDATE resources SET favorite=1; UPDATE settings_saved SET value=11; COMMIT;").unwrap();
            });
            let request = SearchRequest {
                request_id: 1,
                query: "needle".into(),
                workspace_id: None,
                kind: "all".into(),
            };
            let response = if paged {
                store.search_page_with_cancellation(
                    SearchPageRequest {
                        request,
                        offset: 0,
                        sort: "name".into(),
                        cursor: None,
                        modified_from: None,
                        modified_to: None,
                        path_prefix: None,
                    },
                    &cancellation,
                )
            } else {
                store.search_with_cancellation(request, &cancellation)
            }
            .unwrap();
            writer.join().unwrap();
            assert_eq!(response.page.revision, 10);
            assert_eq!(response.page.permission_version, 10);
            assert_eq!(response.page.items.len(), 1);
            assert!(!response.page.items[0].favorite);
            assert_eq!(
                db.query_row(
                    "SELECT value FROM settings_saved WHERE key='revision'",
                    [],
                    |row| row.get::<_, u32>(0)
                )
                .unwrap(),
                11
            );
        }
    }
}
