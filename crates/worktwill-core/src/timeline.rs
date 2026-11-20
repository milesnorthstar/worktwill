use crate::{model::*, store::version, Store};
use rusqlite::{params, params_from_iter, types::Value, Connection};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TimelineRequest {
    pub from: Option<f64>,
    pub to: Option<f64>,
    pub month_from: f64,
    pub month_to: f64,
    pub day_boundaries: Vec<f64>,
    pub filter: String,
    pub basis: String,
    pub workspace_id: Option<String>,
    pub offset: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cursor: Option<TimelineCursor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct TimelineCriteria {
    pub from: Option<f64>,
    pub to: Option<f64>,
    pub month_from: f64,
    pub month_to: f64,
    pub day_boundaries: Vec<f64>,
    pub filter: String,
    pub basis: String,
    pub workspace_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TimelinePosition {
    pub at: f64,
    pub id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TimelineCursor {
    pub version: u32,
    pub session: String,
    pub permission_version: u32,
    pub revision: u32,
    pub next_offset: u32,
    pub criteria: TimelineCriteria,
    pub before: TimelinePosition,
}
impl TimelineRequest {
    fn criteria(&self) -> TimelineCriteria {
        TimelineCriteria {
            from: self.from,
            to: self.to,
            month_from: self.month_from,
            month_to: self.month_to,
            day_boundaries: self.day_boundaries.clone(),
            filter: self.filter.clone(),
            basis: self.basis.clone(),
            workspace_id: self.workspace_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TimelineEntry {
    pub activity: Activity,
    pub resource: Option<Resource>,
    pub effective_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CalendarPoint {
    pub at: f64,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TimelinePage {
    pub session: String,
    pub permission_version: u32,
    pub revision: u32,
    pub events: Vec<TimelineEntry>,
    pub calendar: Vec<CalendarPoint>,
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next_cursor: Option<TimelineCursor>,
}

// 每个Store独占缓存，session由所有者隐含；仅存最多4份完整月历计数。
#[derive(Default)]
pub(crate) struct CalendarCache {
    entries: std::collections::VecDeque<(CalendarKey, Vec<CalendarPoint>)>,
    #[cfg(test)]
    hits: usize,
}
#[derive(Clone, PartialEq)]
struct CalendarKey {
    permission_version: u32,
    revision: u32,
    day_boundaries: Vec<f64>,
    filter: String,
    basis: String,
    workspace_id: Option<String>,
}
impl CalendarCache {
    fn get(&mut self, key: &CalendarKey) -> Option<Vec<CalendarPoint>> {
        let index = self.entries.iter().position(|(stored, _)| stored == key)?;
        let entry = self.entries.remove(index)?;
        let points = entry.1.clone();
        self.entries.push_back(entry);
        #[cfg(test)]
        {
            self.hits += 1;
        }
        Some(points)
    }
    fn insert(&mut self, key: CalendarKey, points: Vec<CalendarPoint>) {
        if let Some(index) = self.entries.iter().position(|(stored, _)| stored == &key) {
            self.entries.remove(index);
        }
        self.entries.push_back((key, points));
        while self.entries.len() > 4 {
            self.entries.pop_front();
        }
    }
}

fn issue_cursor(request: &TimelineRequest, page: &TimelinePage) -> Result<Option<TimelineCursor>> {
    if !page.has_more {
        return Ok(None);
    }
    let last = page
        .events
        .last()
        .expect("a full event page has a last item");
    Ok(Some(TimelineCursor {
        version: 1,
        session: page.session.clone(),
        permission_version: page.permission_version,
        revision: page.revision,
        next_offset: request.offset.checked_add(100).ok_or_else(|| {
            AppError::new(
                "invalid_input",
                "时间线分页范围过大",
                "The timeline page is too far ahead.",
            )
        })?,
        criteria: request.criteria(),
        before: TimelinePosition {
            at: last.effective_at,
            id: last.activity.id.clone(),
        },
    }))
}

// 首次发现永远使用发现时间；仅已观察到的修改允许切换到文件 mtime。
fn effective(basis: &str) -> &'static str {
    if basis == "file_modified" {
        "CASE WHEN a.kind='file_modified' AND a.file_modified_at>0 THEN a.file_modified_at ELSE a.at END"
    } else {
        "a.at"
    }
}

fn validate(request: &TimelineRequest) -> Result<()> {
    if !["all", "modified", "discovered", "opened", "system"].contains(&request.filter.as_str())
        || !["observed", "file_modified"].contains(&request.basis.as_str())
    {
        return Err(AppError::new(
            "invalid_input",
            "时间线筛选条件无效",
            "The timeline filter is invalid.",
        ));
    }
    // 拒绝非有限值及 Date 无法显示的时间，避免 NaN 被 SQLite 当成空值而扩大查询。
    let valid_time = |time: f64| time.is_finite() && time.abs() <= 8_640_000_000_000_000.;
    if !valid_time(request.month_from)
        || !valid_time(request.month_to)
        || request.month_from >= request.month_to
        || request.from.is_some_and(|time| !valid_time(time))
        || request.to.is_some_and(|time| !valid_time(time))
        || matches!((request.from, request.to), (Some(from), Some(to)) if from >= to)
    {
        return Err(AppError::new(
            "invalid_input",
            "请选择有效的时间范围",
            "Select a valid date range.",
        ));
    }
    // 由显示端提供本地午夜；最多 31 天，不把夏令时日误当成固定 24 小时。
    if !(2..=32).contains(&request.day_boundaries.len())
        || request.day_boundaries.first() != Some(&request.month_from)
        || request.day_boundaries.last() != Some(&request.month_to)
        || request.month_to - request.month_from > 32. * 86_400_000.
        || request.day_boundaries.iter().any(|time| !valid_time(*time))
        || request
            .day_boundaries
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(AppError::new(
            "invalid_input",
            "日历日期边界无效，请刷新后重试",
            "The calendar date boundaries are invalid. Refresh and try again.",
        ));
    }
    Ok(())
}

fn filter_sql(filter: &str) -> &'static str {
    match filter {
        "modified" => "a.kind='file_modified'",
        "discovered" => "a.kind='file_discovered'",
        "opened" => "a.kind='opened'",
        "system" => "a.kind NOT IN ('file_modified','file_discovered','opened')",
        _ => "a.kind IN ('file_modified','file_discovered','opened')",
    }
}

fn row_activity(row: &rusqlite::Row<'_>) -> rusqlite::Result<Activity> {
    Ok(Activity {
        id: row.get(0)?,
        kind: row.get(1)?,
        name: row.get(2)?,
        resource_id: row.get(3)?,
        at: row.get(4)?,
        source: row.get(5)?,
        file_modified_at: row.get(6)?,
    })
}

fn event_sql(request: &TimelineRequest, where_sql: &str, timestamp: &str) -> String {
    // 已选择的日期直接生成范围条件，避免可空OR阻碍时间索引定位。
    let from = if request.from.is_some() {
        format!("{timestamp}>=?2")
    } else {
        "?2 IS NULL".into()
    };
    let to = if request.cursor.is_some() {
        // 续页索引只使用更窄的游标上界；原日期上界仍逐项校验，不与它竞争索引范围。
        format!("(?3 IS NULL OR {timestamp}<?3)")
    } else if request.to.is_some() {
        format!("{timestamp}<?3")
    } else {
        "?3 IS NULL".into()
    };
    // 冗余标量上界让SQLite表达式索引定位到游标时间；tuple继续负责同时间戳的ID边界。
    let seek = if request.cursor.is_some() {
        format!("{timestamp}<=?5 AND ({timestamp},a.id)<(?5,?6)")
    } else {
        "?5 IS NULL AND ?6 IS NULL".into()
    };
    format!("SELECT a.id,a.kind,CASE WHEN a.name<>'' THEN a.name WHEN a.resource_id IS NOT NULL THEN r.name ELSE a.name END,a.resource_id,a.at,a.source,a.file_modified_at,{timestamp} AS effective_at FROM activities a LEFT JOIN resources r ON r.id=a.resource_id WHERE {where_sql} AND ({from}) AND ({to}) AND ({seek}) ORDER BY effective_at DESC,a.id DESC LIMIT 101")
}

fn read_events(
    db: &Connection,
    request: &TimelineRequest,
    where_sql: &str,
    timestamp: &str,
) -> Result<(Vec<TimelineEntry>, bool)> {
    let sql = event_sql(request, where_sql, timestamp);
    let observed = db
        .prepare(&sql)?
        .query_map(
            params![
                request.workspace_id,
                request.from,
                request.to,
                rusqlite::types::Null,
                request.cursor.as_ref().map(|cursor| cursor.before.at),
                request.cursor.as_ref().map(|cursor| &cursor.before.id)
            ],
            |row| Ok((row_activity(row)?, row.get::<_, f64>(7)?)),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let has_more = observed.len() > 100;
    let events = observed
        .into_iter()
        .take(100)
        .map(|(activity, effective_at)| {
            let resource = activity
                .resource_id
                .as_deref()
                .map(|id| Store::resource_db(db, id, true))
                .transpose()?;
            Ok(TimelineEntry {
                activity,
                resource,
                effective_at,
            })
        })
        .collect::<Result<_>>()?;
    Ok((events, has_more))
}

fn calendar_query(
    request: &TimelineRequest,
    where_sql: &str,
    timestamp: &str,
) -> (String, Vec<Value>) {
    let days = (2..request.day_boundaries.len() + 1)
        .map(|start| format!("(?{start},?{})", start + 1))
        .collect::<Vec<_>>()
        .join(",");
    // 每天一个索引范围计数，避免对每条活动执行最多 31 个 CASE 分支及全月排序。
    let sql = format!("WITH days(day_start,day_end) AS (VALUES {days}) SELECT day_start,(SELECT count(*) FROM activities a LEFT JOIN resources r ON r.id=a.resource_id WHERE {where_sql} AND {timestamp}>=days.day_start AND {timestamp}<days.day_end) FROM days ORDER BY day_start");
    let mut values = vec![request
        .workspace_id
        .clone()
        .map_or(Value::Null, Value::Text)];
    values.extend(request.day_boundaries.iter().copied().map(Value::Real));
    (sql, values)
}

impl Store {
    pub fn timeline(&self, request: TimelineRequest) -> Result<TimelinePage> {
        validate(&request)?;
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        // 先在本次读事务建立版本快照；仅复用同版本、同日历条件的计数。
        let permission_version = version(&tx, "permission_version")?;
        let revision = version(&tx, "revision")?;
        let valid_cursor = match (&request.cursor, request.offset) {
            (None, 0) => true,
            (Some(cursor), offset) if offset > 0 => {
                cursor.version == 1
                    && cursor.session == self.session
                    && cursor.permission_version == permission_version
                    && cursor.revision == revision
                    && cursor.next_offset == offset
                    && offset.is_multiple_of(100)
                    && cursor.criteria == request.criteria()
                    && cursor.before.at.is_finite()
                    && cursor.before.at.abs() <= 8_640_000_000_000_000.
                    && !cursor.before.id.is_empty()
                    && cursor.before.id.len() <= 256
            }
            _ => false,
        };
        if !valid_cursor {
            return Err(AppError::new(
                "stale_cursor",
                "时间线或条件已变化，请从第一页重新加载",
                "The timeline or conditions have changed. Reload from the first page.",
            ));
        }
        let key = CalendarKey {
            permission_version,
            revision,
            day_boundaries: request.day_boundaries.clone(),
            filter: request.filter.clone(),
            basis: request.basis.clone(),
            workspace_id: request.workspace_id.clone(),
        };
        let timestamp = effective(&request.basis);
        // 日历与列表使用同一授权/类型/工作区条件，日期与分页分别应用。
        let activity_visible = crate::activity_origin::activity_visible();
        let where_sql = format!("({}) AND {activity_visible} AND (?1 IS NULL OR EXISTS(SELECT 1 FROM workspace_resources wr WHERE wr.resource_id=a.resource_id AND wr.workspace_id=?1))",filter_sql(&request.filter));
        let (events, has_more) = read_events(&tx, &request, &where_sql, timestamp)?;
        let cached = self
            .calendar_cache
            .lock()
            .map_err(|_| {
                AppError::new(
                    "internal_error",
                    "日历缓存不可用",
                    "The calendar cache is unavailable.",
                )
            })?
            .get(&key);
        let calendar = if let Some(points) = cached {
            points
        } else {
            let (sql, values) = calendar_query(&request, &where_sql, timestamp);
            // 在数据库内归日，完整月响应最多 31 点；仍在同一事务内按当前授权过滤。
            let calendar: Vec<CalendarPoint> = tx
                .prepare(&sql)?
                .query_map(params_from_iter(values), |row| {
                    Ok(CalendarPoint {
                        at: row.get(0)?,
                        count: row.get(1)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .filter(|point| point.count > 0)
                .collect();
            self.calendar_cache
                .lock()
                .map_err(|_| {
                    AppError::new(
                        "internal_error",
                        "日历缓存不可用",
                        "The calendar cache is unavailable.",
                    )
                })?
                .insert(key, calendar.clone());
            calendar
        };
        let mut page = TimelinePage {
            session: self.session.clone(),
            permission_version,
            revision,
            events,
            calendar,
            has_more,
            next_cursor: None,
        };
        page.next_cursor = issue_cursor(&request, &page)?;
        Ok(page)
    }
}

// 分段诊断仅编入显式验收器；SQL与正式入口共用，不进入产品遥测或对外IPC。
#[cfg(test)]
impl Store {
    pub(crate) fn calendar_cache_stats(&self) -> (usize, usize) {
        let cache = self.calendar_cache.lock().unwrap();
        (cache.entries.len(), cache.hits)
    }

    pub(crate) fn profile_timeline(
        &self,
        request: &TimelineRequest,
    ) -> Result<(TimelinePage, serde_json::Value)> {
        use std::time::Instant;
        let total = Instant::now();
        validate(request)?;
        let _guard = self.gate.read().unwrap();
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        let timestamp = effective(&request.basis);
        let activity_visible = crate::activity_origin::activity_visible();
        let where_sql = format!("({}) AND {activity_visible} AND (?1 IS NULL OR EXISTS(SELECT 1 FROM workspace_resources wr WHERE wr.resource_id=a.resource_id AND wr.workspace_id=?1))", filter_sql(&request.filter));
        let event_query = event_sql(request, &where_sql, timestamp);
        let (calendar_query, values) = calendar_query(request, &where_sql, timestamp);
        let started = Instant::now();
        let observed = tx
            .prepare(&event_query)?
            .query_map(
                params![
                    request.workspace_id,
                    request.from,
                    request.to,
                    rusqlite::types::Null,
                    request.cursor.as_ref().map(|cursor| cursor.before.at),
                    request.cursor.as_ref().map(|cursor| &cursor.before.id)
                ],
                |row| Ok((row_activity(row)?, row.get::<_, f64>(7)?)),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let event_ms = started.elapsed().as_secs_f64() * 1000.;
        let has_more = observed.len() > 100;
        let started = Instant::now();
        let events = observed
            .into_iter()
            .take(100)
            .map(|(activity, effective_at)| {
                let resource = activity
                    .resource_id
                    .as_deref()
                    .map(|id| Store::resource_db(&tx, id, true))
                    .transpose()?;
                Ok(TimelineEntry {
                    activity,
                    resource,
                    effective_at,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let resources_ms = started.elapsed().as_secs_f64() * 1000.;
        let started = Instant::now();
        let calendar = tx
            .prepare(&calendar_query)?
            .query_map(params_from_iter(values.iter()), |row| {
                Ok(CalendarPoint {
                    at: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter(|point| point.count > 0)
            .collect();
        let calendar_ms = started.elapsed().as_secs_f64() * 1000.;
        let mut page = TimelinePage {
            session: self.session.clone(),
            permission_version: version(&tx, "permission_version")?,
            revision: version(&tx, "revision")?,
            events,
            calendar,
            has_more,
            next_cursor: None,
        };
        page.next_cursor = issue_cursor(request, &page)?;
        let total_ms = total.elapsed().as_secs_f64() * 1000.;
        // 计划采集放在计时之后，不混入SQL/资源/日历分段样本。
        let event_plan = tx
            .prepare(&format!("EXPLAIN QUERY PLAN {event_query}"))?
            .query_map(
                params![
                    request.workspace_id,
                    request.from,
                    request.to,
                    rusqlite::types::Null,
                    request.cursor.as_ref().map(|cursor| cursor.before.at),
                    request.cursor.as_ref().map(|cursor| &cursor.before.id)
                ],
                |row| row.get::<_, String>(3),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let calendar_plan = tx
            .prepare(&format!("EXPLAIN QUERY PLAN {calendar_query}"))?
            .query_map(params_from_iter(values.iter()), |row| {
                row.get::<_, String>(3)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let relationship_plan = tx.prepare("EXPLAIN QUERY PLAN SELECT workspace_id FROM workspace_resources WHERE resource_id=?")?.query_map([page.events.first().and_then(|event| event.resource.as_ref()).map(|resource| &resource.id)], |row| row.get::<_,String>(3))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok((
            page,
            serde_json::json!({"offset":request.offset,"event_ms":event_ms,"resources_ms":resources_ms,"calendar_ms":calendar_ms,"total_ms_excludes_plan_collection":total_ms,"event_plan":event_plan,"calendar_plan":calendar_plan,"relationship_plan":relationship_plan}),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Fixture {
        _directory: tempfile::TempDir,
        store: Store,
        root: String,
        rid: String,
    }
    impl Fixture {
        fn new() -> Self {
            let approved =
                std::env::var_os("WORKTWILL_TEST_ROOT").expect("confirmed disposable fixture root");
            let directory = tempfile::Builder::new()
                .prefix("timeline-")
                .tempdir_in(approved)
                .unwrap();
            let docs = directory.path().join("documents");
            fs::create_dir(&docs).unwrap();
            fs::write(docs.join("合同.txt"), b"synthetic fixture").unwrap();
            let store = Store::open(directory.path().join("app/data.sqlite3")).unwrap();
            let root = store.grant_root(&docs).unwrap();
            store.scan_all().unwrap();
            let rid = store
                .resources(ResourceQuery {
                    workspace_id: None,
                    kind: "file".into(),
                    mode: "modified".into(),
                    offset: 0,
                })
                .unwrap()
                .items[0]
                .id
                .clone();
            store
                .connection()
                .unwrap()
                .execute("DELETE FROM activities", [])
                .unwrap();
            Self {
                _directory: directory,
                store,
                root,
                rid,
            }
        }
        fn insert(
            &self,
            id: &str,
            kind: &str,
            at: f64,
            mtime: Option<f64>,
            resource: Option<&str>,
        ) {
            let mut db = self.store.connection().unwrap();
            let tx = db.transaction().unwrap();
            tx.execute("INSERT INTO activities(id,kind,name,resource_id,at,source,file_modified_at) VALUES(?1,?2,'fixture',?3,?4,?5,?6)",params![id,kind,resource,at,if kind.starts_with("file_"){"filesystem"}else{"app"},mtime]).unwrap();
            if let Some(rid) = resource {
                crate::activity_origin::capture(&tx, id, rid).unwrap();
            }
            crate::store::bump(&tx, false).unwrap();
            tx.commit().unwrap();
        }
        fn request(&self) -> TimelineRequest {
            TimelineRequest {
                from: None,
                to: None,
                month_from: 0.,
                month_to: 1_000_000.,
                day_boundaries: vec![0., 100., 200., 1_000_000.],
                filter: "all".into(),
                basis: "observed".into(),
                workspace_id: None,
                offset: 0,
                cursor: None,
            }
        }
        fn change(&self, mutation: Mutation) -> MutationResponse {
            self.store
                .mutate(MutationRequest {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    mutation,
                })
                .unwrap()
        }
    }

    #[test]
    fn calendar_covers_whole_range_while_events_paginate_with_stable_ties() {
        let f = Fixture::new();
        for n in 0..105 {
            f.insert(
                &format!("event-{n:03}"),
                "file_modified",
                100.,
                Some(50.),
                Some(&f.rid),
            );
        }
        f.insert("outside", "file_modified", 200., Some(199.), Some(&f.rid));
        f.insert("management", "workspace_created", 100., None, None);
        let mut request = f.request();
        request.from = Some(100.);
        request.to = Some(200.);
        let first = f.store.timeline(request.clone()).unwrap();
        assert_eq!(first.events.len(), 100);
        assert!(first.has_more);
        assert_eq!(first.events[0].activity.id, "event-104");
        assert_eq!(
            first.calendar.iter().map(|point| point.count).sum::<u32>(),
            106
        );
        request.offset = 100;
        request.cursor = first.next_cursor.clone();
        let rest = f.store.timeline(request).unwrap();
        assert_eq!(rest.events.len(), 5);
        assert!(!rest.has_more);
        assert_eq!(rest.events[4].activity.id, "event-000");
        assert_eq!(
            rest.calendar.iter().map(|point| point.count).sum::<u32>(),
            106
        );
    }

    #[test]
    fn file_time_basis_preserves_discovery_time_and_half_open_boundaries() {
        let f = Fixture::new();
        f.insert(
            "discovery",
            "file_discovered",
            200.,
            Some(50.),
            Some(&f.rid),
        );
        f.insert("change", "file_modified", 200., Some(100.), Some(&f.rid));
        f.insert("opened", "opened", 300., None, Some(&f.rid));
        let mut request = f.request();
        request.basis = "file_modified".into();
        request.from = Some(100.);
        request.to = Some(200.);
        let result = f.store.timeline(request.clone()).unwrap();
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].activity.id, "change");
        assert_eq!(result.events[0].effective_at, 100.);
        assert_eq!(result.events[0].activity.at, 200.);
        assert_eq!(result.calendar.len(), 2);
        assert_eq!(result.calendar[0].count, 1);
        assert_eq!(result.calendar[1].count, 2);
        request.filter = "discovered".into();
        request.from = None;
        request.to = None;
        let discovered = f.store.timeline(request).unwrap();
        assert_eq!(discovered.events[0].effective_at, 200.);
    }

    #[test]
    fn revocation_hides_both_event_text_and_calendar_counts() {
        let f = Fixture::new();
        f.insert("change", "file_modified", 100., Some(90.), Some(&f.rid));
        let before = f.store.timeline(f.request()).unwrap();
        assert_eq!(before.events.len(), 1);
        f.change(Mutation::RevokeRoot { id: f.root.clone() });
        let after = f.store.timeline(f.request()).unwrap();
        assert!(after.events.is_empty());
        assert!(after.calendar.is_empty());
        assert!(after.permission_version > before.permission_version);
    }

    #[test]
    fn workspace_scope_and_system_filter_are_explicit_and_share_calendar_filter() {
        let f = Fixture::new();
        let wid = f
            .change(Mutation::CreateWorkspace {
                name: "客户 A".into(),
                resource_ids: None,
            })
            .id
            .unwrap();
        f.change(Mutation::Link {
            workspace_id: wid.clone(),
            resource_ids: vec![f.rid.clone()],
        });
        f.store
            .connection()
            .unwrap()
            .execute("DELETE FROM activities", [])
            .unwrap();
        f.insert("changed", "file_modified", 100., Some(90.), Some(&f.rid));
        f.insert("management", "workspace_created", 101., None, None);
        f.insert("linked", "linked", 102., None, Some(&f.rid));
        let mut request = f.request();
        request.workspace_id = Some(wid.clone());
        let result = f.store.timeline(request.clone()).unwrap();
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.calendar.len(), 1);
        request.filter = "system".into();
        let system = f.store.timeline(request).unwrap();
        assert_eq!(system.events.len(), 1);
        assert_eq!(system.events[0].activity.id, "linked");
        let mut all_system = f.request();
        all_system.filter = "system".into();
        assert_eq!(f.store.timeline(all_system).unwrap().events.len(), 2);
        let mut missing = f.request();
        missing.workspace_id = Some("nonexistent".into());
        let absent = f.store.timeline(missing).unwrap();
        assert!(absent.events.is_empty());
        assert!(absent.calendar.is_empty());
    }

    #[test]
    fn invalid_filters_and_nonfinite_dates_do_not_expand_query() {
        let f = Fixture::new();
        for basis in ["utc", "unknown"] {
            let mut request = f.request();
            request.basis = basis.into();
            assert!(f.store.timeline(request).is_err());
        }
        let mut request = f.request();
        request.from = Some(f64::NAN);
        assert!(f.store.timeline(request).is_err());
        let mut request = f.request();
        request.month_to = request.month_from;
        assert!(f.store.timeline(request).is_err());
        let mut request = f.request();
        request.from = Some(100.);
        request.to = Some(99.);
        assert!(f.store.timeline(request).is_err());
    }

    #[test]
    fn calendar_aggregates_variable_length_days_with_exact_half_open_boundaries() {
        let f = Fixture::new();
        let hour = 3_600_000.;
        // 覆盖 23/25 小时日；后端接受显示时区的边界，不自行做 UTC 截断。
        for hours in [23., 25.] {
            f.store
                .connection()
                .unwrap()
                .execute("DELETE FROM activities", [])
                .unwrap();
            let boundaries = vec![0., 24. * hour, (24. + hours) * hour, (48. + hours) * hour];
            for (id, at) in [
                ("first", 0.),
                ("before", boundaries[1] - 1.),
                ("next", boundaries[1]),
                ("last-second", boundaries[2] - 1.),
                ("third", boundaries[2]),
                ("outside", boundaries[3]),
            ] {
                f.insert(id, "file_modified", at + 1000., Some(at), Some(&f.rid));
            }
            let mut request = f.request();
            request.month_to = boundaries[3];
            request.day_boundaries = boundaries.clone();
            request.basis = "file_modified".into();
            request.from = Some(boundaries[1]);
            request.to = Some(boundaries[2]);
            let result = f.store.timeline(request).unwrap();
            assert_eq!(
                result
                    .events
                    .iter()
                    .map(|entry| entry.activity.id.as_str())
                    .collect::<Vec<_>>(),
                ["last-second", "next"]
            );
            assert_eq!(
                result
                    .calendar
                    .iter()
                    .map(|point| (point.at, point.count))
                    .collect::<Vec<_>>(),
                [(boundaries[0], 2), (boundaries[1], 2), (boundaries[2], 1)]
            );
        }
    }

    #[test]
    fn calendar_rejects_unbounded_or_inconsistent_day_requests() {
        let f = Fixture::new();
        for boundaries in [
            vec![],
            vec![0.],
            vec![0., 1_000_001.],
            vec![1., 1_000_000.],
            vec![0., f64::NAN, 1_000_000.],
            vec![0., 500., 400., 1_000_000.],
            vec![0., 500., 500., 1_000_000.],
            (0..=32).map(|n| f64::from(n) * 1_000_000. / 32.).collect(),
        ] {
            let mut request = f.request();
            request.day_boundaries = boundaries;
            assert!(f.store.timeline(request).is_err());
        }
        let mut request = f.request();
        request.month_to = 33. * 86_400_000.;
        request.day_boundaries = vec![request.month_from, request.month_to];
        assert!(f.store.timeline(request).is_err());
    }

    #[test]
    fn mixed_resources_and_months_keep_calendar_authorization_and_workspace_scope() {
        let f = Fixture::new();
        let mut db = f.store.connection().unwrap();
        let tx = db.transaction().unwrap();
        for (id, kind, location) in [
            ("allowed-web", "web", "https://example.com"),
            ("denied-file", "file", "/outside/synthetic.txt"),
        ] {
            tx.execute("INSERT INTO resources(id,kind,name,location,created_at) VALUES(?1,?2,'fixture',?3,0)",params![id,kind,location]).unwrap();
        }
        tx.execute(
            "INSERT INTO workspaces(id,name,created_at) VALUES('scope','scope',0)",
            [],
        )
        .unwrap();
        for rid in [&f.rid, "denied-file"] {
            tx.execute(
                "INSERT INTO workspace_resources VALUES('scope',?1,0)",
                [rid],
            )
            .unwrap();
        }
        tx.commit().unwrap();
        let mut request = f.request();
        request.month_from = 100.;
        request.month_to = 400.;
        request.day_boundaries = vec![100., 200., 300., 400.];
        request.from = Some(100.);
        request.to = Some(400.);
        f.insert(
            "visible-first",
            "file_discovered",
            100.,
            Some(1.),
            Some(&f.rid),
        );
        f.insert(
            "visible-next",
            "file_modified",
            200.,
            Some(199.),
            Some(&f.rid),
        );
        f.insert("visible-web", "opened", 300., None, Some("allowed-web"));
        for (id, at) in [("hidden-same-day", 100.), ("hidden-other-day", 250.)] {
            f.insert(id, "file_modified", at, Some(at), Some("denied-file"));
        }
        f.insert(
            "previous-month",
            "file_modified",
            99.,
            Some(99.),
            Some(&f.rid),
        );
        f.insert(
            "next-month",
            "file_modified",
            400.,
            Some(400.),
            Some(&f.rid),
        );
        let all = f.store.timeline(request.clone()).unwrap();
        assert_eq!(all.events.len(), 3);
        assert_eq!(
            all.calendar
                .iter()
                .map(|point| (point.at, point.count))
                .collect::<Vec<_>>(),
            [(100., 1), (200., 1), (300., 1)]
        );
        request.workspace_id = Some("scope".into());
        let scoped = f.store.timeline(request.clone()).unwrap();
        assert_eq!(scoped.events.len(), 2);
        assert_eq!(
            scoped
                .calendar
                .iter()
                .map(|point| (point.at, point.count))
                .collect::<Vec<_>>(),
            [(100., 1), (200., 1)]
        );
        request.basis = "file_modified".into();
        let modified = f.store.timeline(request).unwrap();
        assert_eq!(modified.calendar.len(), 1);
        assert_eq!(modified.calendar[0].count, 2);
    }

    #[test]
    #[ignore = "explicit timeline scale benchmark; requires confirmed WORKTWILL_TEST_ROOT"]
    fn disk_timeline_acceptance() {
        let f = Fixture::new();
        let day = 86_400_000.;
        let mut request = f.request();
        request.month_from = day;
        request.month_to = day * 31.;
        request.day_boundaries = (1..=31).map(|n| f64::from(n) * day).collect();
        request.from = Some(day);
        request.to = Some(day * 31.);
        request.basis = "file_modified".into();
        let mut reports = Vec::new();
        for count in [100_000, 500_000] {
            let mut db = f.store.connection().unwrap();
            let tx = db.transaction().unwrap();
            let first = if count == 100_000 { 0 } else { 100_000 };
            {
                let mut insert = tx.prepare("INSERT INTO activities(id,kind,name,resource_id,at,source,file_modified_at) VALUES(?1,'file_modified','fixture',?2,?3,'filesystem',?4)").unwrap();
                for n in first..count {
                    let at = day + (n % 30) as f64 * day + (n / 30) as f64 * 1000.;
                    insert
                        .execute(params![format!("event-{n:06}"), f.rid, at + 1000., at])
                        .unwrap();
                }
            }
            crate::activity_origin::bind_legacy(&tx).unwrap();
            crate::store::bump(&tx, false).unwrap();
            tx.commit().unwrap();
            let mut timings = Vec::new();
            let mut first_uncached_ms = None;
            let mut calendar_points = 0;
            let mut response_bytes = 0;
            for repeat in 0..11 {
                let started = std::time::Instant::now();
                let page = f.store.timeline(request.clone()).unwrap();
                let elapsed = started.elapsed().as_secs_f64() * 1000.;
                assert_eq!(page.events.len(), 100);
                assert!(page.has_more);
                assert_eq!(
                    page.calendar
                        .iter()
                        .map(|point| u64::from(point.count))
                        .sum::<u64>(),
                    count as u64
                );
                calendar_points = page.calendar.len();
                response_bytes = serde_json::to_vec(&page).unwrap().len();
                assert_eq!(calendar_points, 30);
                assert!(
                    response_bytes < 150_000,
                    "bounded calendar response with 100 fixture event cards"
                );
                for (index, point) in page.calendar.iter().enumerate() {
                    assert_eq!(point.at, (index + 1) as f64 * day);
                    assert_eq!(
                        point.count as usize,
                        count / 30 + usize::from(index < count % 30)
                    );
                }
                if repeat > 0 {
                    timings.push(elapsed);
                } else {
                    first_uncached_ms = Some(elapsed);
                }
            }
            timings.sort_by(f64::total_cmp);
            reports.push(serde_json::json!({"events":count,"first_calendar_miss_ms":first_uncached_ms,"warm_samples_use_calendar_cache":true,"samples":timings.len(),"p50_ms":timings[4],"p95_ms":timings[9],"calendar_points":calendar_points,"response_bytes":response_bytes}));
        }
        let report = serde_json::json!({"evidence":"synthetic_events_on_disk_store_timeline_not_scan_or_ui_latency","distribution":"one_authorized_file_repeated_changes_in_one_month_unique_event_timestamps","profile":if cfg!(debug_assertions){"debug"}else{"release"},"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"sqlite_version":rusqlite::version(),"results":reports});
        let output = f._directory.path().parent().unwrap().join(format!(
            "rust-timeline-report-{}.json",
            uuid::Uuid::new_v4()
        ));
        fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        println!("TIMELINE_ACCEPTANCE {report}");
        println!("TIMELINE_REPORT {}", output.display());
    }
    #[test]
    fn selected_dates_use_index_ranges_with_open_and_half_open_boundaries() {
        let f = Fixture::new();
        f.insert("at-start", "file_modified", 100., Some(100.), Some(&f.rid));
        f.insert("at-end", "file_modified", 200., Some(200.), Some(&f.rid));
        for basis in ["observed", "file_modified"] {
            for (from, to, expected) in [
                (Some(100.), Some(200.), vec!["at-start"]),
                (Some(200.), None, vec!["at-end"]),
                (None, Some(200.), vec!["at-start"]),
            ] {
                let mut request = f.request();
                request.basis = basis.into();
                request.from = from;
                request.to = to;
                let timestamp = effective(basis);
                let activity_visible = crate::activity_origin::activity_visible();
                let where_sql=format!("({}) AND {activity_visible} AND (?1 IS NULL OR EXISTS(SELECT 1 FROM workspace_resources wr WHERE wr.resource_id=a.resource_id AND wr.workspace_id=?1))",filter_sql(&request.filter));
                let sql = event_sql(&request, &where_sql, timestamp);
                let plan = f
                    .store
                    .connection()
                    .unwrap()
                    .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                    .unwrap()
                    .query_map(
                        params![
                            request.workspace_id,
                            request.from,
                            request.to,
                            rusqlite::types::Null,
                            request.cursor.as_ref().map(|cursor| cursor.before.at),
                            request.cursor.as_ref().map(|cursor| &cursor.before.id)
                        ],
                        |row| row.get::<_, String>(3),
                    )
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap();
                assert!(
                    plan.iter()
                        .any(|line| line.contains("SEARCH a USING INDEX activities_")
                            && line.contains('>')
                            || line.contains("SEARCH a USING INDEX activities_")
                                && line.contains('<')),
                    "{plan:?}"
                );
                let page = f.store.timeline(request).unwrap();
                assert_eq!(
                    page.events
                        .iter()
                        .map(|event| event.activity.id.as_str())
                        .collect::<Vec<_>>(),
                    expected
                );
            }
        }
    }
    #[test]
    fn calendar_cache_keys_include_all_calendar_conditions_but_not_list_pagination() {
        let f = Fixture::new();
        f.insert("change", "file_modified", 100., Some(50.), Some(&f.rid));
        let request = f.request();
        let original = f.store.timeline(request.clone()).unwrap();
        assert_eq!(f.store.calendar_cache.lock().unwrap().hits, 0);
        let mut subset = request.clone();
        subset.from = Some(100.);
        subset.to = Some(200.);
        subset.from = Some(200.);
        subset.to = Some(300.);
        let page = f.store.timeline(subset).unwrap();
        assert!(page.events.is_empty());
        assert_eq!(
            serde_json::to_value(page.calendar).unwrap(),
            serde_json::to_value(original.calendar).unwrap()
        );
        assert_eq!(f.store.calendar_cache.lock().unwrap().hits, 1);
        let mut variants = Vec::new();
        for filter in ["modified", "opened", "system"] {
            let mut q = request.clone();
            q.filter = filter.into();
            variants.push(q);
        }
        let mut q = request.clone();
        q.basis = "file_modified".into();
        variants.push(q);
        let mut q = request.clone();
        q.workspace_id = Some("unlinked-workspace".into());
        variants.push(q);
        let mut q = request.clone();
        q.day_boundaries = vec![0., 150., 1_000_000.];
        variants.push(q);
        for q in variants {
            f.store.timeline(q).unwrap();
            assert!(f.store.calendar_cache.lock().unwrap().entries.len() <= 4);
        }
        assert_eq!(f.store.calendar_cache.lock().unwrap().hits, 1);
        f.store.timeline(request.clone()).unwrap(); // 原key已被LRU淘汰。
        assert_eq!(f.store.calendar_cache.lock().unwrap().hits, 1);
        f.store.timeline(request.clone()).unwrap();
        assert_eq!(f.store.calendar_cache.lock().unwrap().hits, 2);
        // 状态刷新可不增加计数版本；缓存不能截留资源正文。
        f.store
            .connection()
            .unwrap()
            .execute("UPDATE resources SET status='missing' WHERE id=?", [&f.rid])
            .unwrap();
        let fresh = f.store.timeline(request).unwrap();
        assert_eq!(fresh.events[0].resource.as_ref().unwrap().status, "missing");
        assert_eq!(f.store.calendar_cache.lock().unwrap().hits, 3);
    }

    #[test]
    fn cached_calendar_invalidates_for_transfer_exclusion_activity_writes_and_restore() {
        let f = Fixture::new();
        f.insert("change", "file_modified", 100., Some(90.), Some(&f.rid));
        let a = f
            .change(Mutation::CreateWorkspace {
                name: "A".into(),
                resource_ids: Some(vec![f.rid.clone()]),
            })
            .id
            .unwrap();
        let b = f
            .change(Mutation::CreateWorkspace {
                name: "B".into(),
                resource_ids: None,
            })
            .id
            .unwrap();
        let mut scoped = f.request();
        scoped.workspace_id = Some(a.clone());
        let count = |q: TimelineRequest| {
            f.store
                .timeline(q)
                .unwrap()
                .calendar
                .iter()
                .map(|point| point.count)
                .sum::<u32>()
        };
        assert_eq!(count(scoped.clone()), 1);
        assert_eq!(count(scoped.clone()), 1);
        f.change(Mutation::Transfer {
            source: a,
            target: b.clone(),
            resource_ids: vec![f.rid.clone()],
        });
        assert_eq!(count(scoped.clone()), 0);
        scoped.workspace_id = Some(b);
        assert_eq!(count(scoped.clone()), 1);
        f.change(Mutation::Exclude {
            root_id: f.root.clone(),
            relative_path: "合同.txt".into(),
        });
        assert_eq!(count(scoped.clone()), 0);
        f.change(Mutation::Include {
            root_id: f.root.clone(),
            relative_path: "合同.txt".into(),
        });
        assert_eq!(count(scoped.clone()), 1);
        f.insert(
            "change-again",
            "file_modified",
            150.,
            Some(140.),
            Some(&f.rid),
        );
        assert_eq!(count(scoped.clone()), 2);
        let backup = f._directory.path().join("snapshot.sqlite3");
        f.store.backup_to(&backup).unwrap();
        let before = f.store.timeline(scoped.clone()).unwrap();
        f.store.restore_from(&backup).unwrap();
        let after = f.store.timeline(scoped).unwrap();
        assert!(after.calendar.is_empty());
        assert!(after.events.is_empty());
        assert!(
            after.permission_version > before.permission_version
                && after.revision > before.revision
        );
    }
    #[test]
    fn timeline_cursor_binds_versions_all_conditions_and_rejects_missing_or_invalid_positions() {
        let f = Fixture::new();
        for n in 0..105 {
            f.insert(
                &format!("tie-{n:03}"),
                "file_modified",
                100.,
                Some(90.),
                Some(&f.rid),
            );
        }
        let base = f.request();
        let first = f.store.timeline(base.clone()).unwrap();
        let issued = first.next_cursor.as_ref().unwrap();
        assert_eq!(issued.before.id, first.events[99].activity.id);
        assert_eq!(issued.before.at, first.events[99].effective_at);
        let mut next = base.clone();
        next.offset = issued.next_offset;
        next.cursor = Some(issued.clone());
        let mut invalid = Vec::new();
        for index in 0..16 {
            let mut q = next.clone();
            match index {
                0 => q.cursor = None,
                1 => q.offset = 200,
                2 => q.offset = 0,
                3 => q.filter = "modified".into(),
                4 => q.basis = "file_modified".into(),
                5 => q.workspace_id = Some("other".into()),
                6 => q.from = Some(1.),
                7 => q.to = Some(200.),
                8 => q.day_boundaries[1] = 50.,
                9 => {
                    q.month_to = 2_000_000.;
                    *q.day_boundaries.last_mut().unwrap() = q.month_to;
                }
                10 => q.cursor.as_mut().unwrap().version += 1,
                11 => q.cursor.as_mut().unwrap().session = "other".into(),
                12 => q.cursor.as_mut().unwrap().permission_version += 1,
                13 => q.cursor.as_mut().unwrap().revision += 1,
                14 => q.cursor.as_mut().unwrap().before.at = f64::INFINITY,
                _ => q.cursor.as_mut().unwrap().before.id.clear(),
            }
            invalid.push(q);
        }
        for q in invalid {
            assert_eq!(f.store.timeline(q).unwrap_err().code, "stale_cursor");
        }
        let rest = f.store.timeline(next.clone()).unwrap();
        assert_eq!(rest.events.len(), 5);
        assert!(!rest.has_more && rest.next_cursor.is_none());
        assert_eq!(rest.events[0].activity.id, "tie-004");
        f.change(Mutation::Favorite {
            id: f.rid.clone(),
            value: true,
        });
        assert_eq!(
            f.store.timeline(next.clone()).unwrap_err().code,
            "stale_cursor"
        );
        let fresh = f.store.timeline(base.clone()).unwrap();
        let cursor = fresh.next_cursor.unwrap();
        let mut before_restore = base.clone();
        before_restore.offset = cursor.next_offset;
        before_restore.cursor = Some(cursor);
        let backup = f._directory.path().join("cursor-backup.sqlite3");
        f.store.backup_to(&backup).unwrap();
        f.store.restore_from(&backup).unwrap();
        assert_eq!(
            f.store.timeline(before_restore).unwrap_err().code,
            "stale_cursor"
        );
        assert!(f.store.timeline(base).unwrap().events.is_empty());
        let reopened = Store::open(&f.store.path).unwrap();
        assert_eq!(reopened.timeline(next).unwrap_err().code, "stale_cursor");
    }
    #[test]
    fn both_time_bases_traverse_tied_mixed_event_chains_without_duplicates() {
        let f = Fixture::new();
        let mut raw = Vec::new();
        for n in 0..225 {
            let id = format!("mixed-{n:03}");
            let kind = ["file_modified", "file_discovered", "opened"][n % 3];
            let at = 100. + (n % 3) as f64;
            let mtime = 50. + (n % 5) as f64;
            f.insert(&id, kind, at, Some(mtime), Some(&f.rid));
            raw.push((id, kind, at, mtime));
        }
        for basis in ["observed", "file_modified"] {
            let mut expected = raw
                .iter()
                .map(|(id, kind, at, mtime)| {
                    (
                        if basis == "file_modified" && *kind == "file_modified" {
                            *mtime
                        } else {
                            *at
                        },
                        id.clone(),
                    )
                })
                .collect::<Vec<_>>();
            expected.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
            let mut request = f.request();
            request.basis = basis.into();
            let mut actual = Vec::new();
            loop {
                let page = f.store.timeline(request.clone()).unwrap();
                actual.extend(
                    page.events
                        .iter()
                        .map(|event| (event.effective_at, event.activity.id.clone())),
                );
                let Some(cursor) = page.next_cursor else {
                    assert!(!page.has_more);
                    break;
                };
                request.offset = cursor.next_offset;
                request.cursor = Some(cursor);
                let (_, diagnostic) = f.store.profile_timeline(&request).unwrap();
                let plan = diagnostic["event_plan"].as_array().unwrap();
                assert!(
                    plan.iter().any(|line| line
                        .as_str()
                        .unwrap()
                        .contains("SEARCH a USING INDEX activities_")),
                    "{plan:?}"
                );
                assert!(
                    !plan
                        .iter()
                        .any(|line| line.as_str().unwrap().contains("TEMP B-TREE")),
                    "{plan:?}"
                );
            }
            assert_eq!(actual, expected);
        }
    }
    #[test]
    fn forged_legal_cursor_position_cannot_bypass_authorization_or_date_range() {
        let f = Fixture::new();
        for n in 0..105 {
            f.insert(
                &format!("visible-{n:03}"),
                "file_modified",
                100.,
                Some(90.),
                Some(&f.rid),
            );
        }
        f.insert(
            "outside-selected-day",
            "file_modified",
            300.,
            Some(290.),
            Some(&f.rid),
        );
        f.store.connection().unwrap().execute("INSERT INTO resources(id,kind,name,location,created_at) VALUES('denied','file','不得泄露','/ungranted/secret.txt',0)",[]).unwrap();
        f.insert("z-secret", "file_modified", 100., Some(90.), Some("denied"));
        let mut request = f.request();
        request.from = Some(100.);
        request.to = Some(200.);
        let first = f.store.timeline(request.clone()).unwrap();
        let mut cursor = first.next_cursor.unwrap();
        cursor.before.at = 1_000_000.;
        request.offset = cursor.next_offset;
        request.cursor = Some(cursor);
        let page = f.store.timeline(request).unwrap();
        assert!(page.events.iter().all(|event| event.effective_at >= 100.
            && event.effective_at < 200.
            && event.activity.resource_id.as_deref() == Some(f.rid.as_str())));
        assert_eq!(page.events.len(), 100);
        assert_eq!(
            page.calendar.iter().map(|point| point.count).sum::<u32>(),
            106
        );
    }
}
