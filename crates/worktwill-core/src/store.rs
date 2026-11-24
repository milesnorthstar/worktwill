use crate::{model::*, platform, search};
use rusqlite::{
    backup::{Backup, StepResult},
    params, Connection, OptionalExtension, TransactionBehavior,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, RwLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

pub struct Store {
    pub(crate) path: PathBuf,
    pub(crate) gate: RwLock<()>,
    pub(crate) catalog_write: Mutex<()>,
    pub(crate) scan_epoch: AtomicU64,
    pub(crate) calendar_cache: Mutex<crate::timeline::CalendarCache>,
    pub session: String,
}

pub(crate) fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}
pub(crate) fn id() -> String {
    Uuid::new_v4().to_string()
}

// 调用者固定源读快照，More才代表有限库的正常分页；复制遇锁立即交还调用者，不能让busy handler持有Store写锁等待。
fn copy_snapshot(source: &Connection, target: &mut Connection) -> Result<()> {
    target.busy_timeout(Duration::ZERO)?;
    let backup = Backup::new(source, target)?;
    loop {
        match backup.step(100)? {
            StepResult::Done => return Ok(()),
            StepResult::More => std::thread::sleep(Duration::from_millis(5)),
            StepResult::Busy | StepResult::Locked => {
                return Err(AppError::new(
                    "database_busy",
                    "本地数据正忙，请稍后重试",
                    "Local data is busy. Please try again.",
                ))
            }
            _ => {
                return Err(AppError::new(
                    "database_error",
                    "数据库复制状态无效",
                    "The database copy returned an invalid state.",
                ))
            }
        }
    }
}
pub(crate) fn bump(db: &Connection, permission: bool) -> Result<()> {
    db.execute("UPDATE settings SET value=value+1 WHERE key='revision' OR (?1 AND key='permission_version')", [permission])?;
    Ok(())
}
pub(crate) fn version(db: &Connection, key: &str) -> Result<u32> {
    Ok(
        db.query_row("SELECT value FROM settings WHERE key=?", [key], |r| {
            r.get(0)
        })?,
    )
}
pub(crate) fn event(
    db: &Connection,
    kind: &str,
    name: &str,
    resource: Option<&str>,
) -> Result<String> {
    // 保存观察当时的名称；重新指向只更新当前引用，不能重写过去的操作描述。
    let observed_name = if name.is_empty() {
        resource
            .map(|rid| {
                db.query_row("SELECT name FROM resources WHERE id=?", [rid], |row| {
                    row.get::<_, String>(0)
                })
            })
            .transpose()?
            .unwrap_or_default()
    } else {
        name.into()
    };
    let aid = id();
    db.execute(
        "INSERT INTO activities(id,kind,name,resource_id,at) VALUES(?1,?2,?3,?4,?5)",
        params![aid, kind, observed_name, resource, now()],
    )?;
    if let Some(rid) = resource {
        crate::activity_origin::capture(db, &aid, rid)?;
    }
    Ok(aid)
}
fn label(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 200 {
        return Err(AppError::new(
            "invalid_input",
            "名称须为 1–200 个字符",
            "Use a name between 1 and 200 characters.",
        ));
    }
    Ok(value.into())
}

// 该条件引用 r；旧目录归属不能授权已移到其他目录的资源，工作区仅保留匿名占位。
#[cfg(not(windows))]
pub(crate) const VISIBLE: &str = "(r.kind='web' OR EXISTS(SELECT 1 FROM file_grants fg WHERE fg.resource_id=r.id AND fg.active=1) OR EXISTS(
 SELECT 1 FROM file_roots fr JOIN scan_roots sr ON sr.id=fr.root_id
 WHERE fr.resource_id=r.id AND sr.active=1
 AND NOT EXISTS(SELECT 1 FROM root_locators rl WHERE rl.root_id=sr.id AND rl.valid=0)
 AND instr(r.location,rtrim(sr.path,'/')||'/')=1 AND NOT EXISTS(
  SELECT 1 FROM scan_exclusions ex WHERE ex.root_id=sr.id AND
  (r.location=rtrim(sr.path,'/')||'/'||ex.relative_path OR instr(r.location,rtrim(sr.path,'/')||'/'||ex.relative_path||'/')=1)
 )))";
#[cfg(windows)]
pub(crate) const VISIBLE: &str = r"(r.kind='web' OR EXISTS(SELECT 1 FROM file_grants fg WHERE fg.resource_id=r.id AND fg.active=1) OR EXISTS(
 SELECT 1 FROM file_roots fr JOIN scan_roots sr ON sr.id=fr.root_id
 WHERE fr.resource_id=r.id AND sr.active=1
 AND NOT EXISTS(SELECT 1 FROM root_locators rl WHERE rl.root_id=sr.id AND rl.valid=0)
 AND instr(r.location,rtrim(sr.path,'\')||'\')=1 AND NOT EXISTS(
  SELECT 1 FROM scan_exclusions ex WHERE ex.root_id=sr.id AND
  (r.location=rtrim(sr.path,'\')||'\'||ex.relative_path OR instr(r.location,rtrim(sr.path,'\')||'\'||ex.relative_path||'\')=1)
 )))";

impl Store {
    pub fn data_directory(&self) -> &Path {
        self.path.parent().expect("database path has parent")
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = Self {
            path,
            gate: RwLock::new(()),
            catalog_write: Mutex::new(()),
            scan_epoch: AtomicU64::new(0),
            calendar_cache: Mutex::new(crate::timeline::CalendarCache::default()),
            session: id(),
        };
        let mut db = store.connection()?;
        let previous: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if previous > 0 && previous < crate::migrations::VERSION {
            // 升级前用在线快照保留旧程序可用的数据组合，不复制活动 WAL 主文件。
            let snapshot_path = store.data_directory().join(format!(
                "before-schema-{previous}-to-{}-{}.sqlite3",
                crate::migrations::VERSION,
                id()
            ));
            let snapshot = db.unchecked_transaction()?;
            version(&snapshot, "revision")?;
            let mut target = Connection::open(snapshot_path)?;
            copy_snapshot(&snapshot, &mut target)?;
        }
        crate::migrations::migrate(&mut db)?;
        db.execute(
            "UPDATE scan_roots SET scan_state='partial_failure' WHERE scan_state='running'",
            [],
        )?;
        store.verify_root_locations()?;
        Ok(store)
    }

    pub(crate) fn connection(&self) -> Result<Connection> {
        let db = Connection::open(&self.path)?;
        db.busy_timeout(Duration::from_millis(1500))?;
        db.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )?;
        #[cfg(test)]
        crate::snapshot_read_tests::configure(&db);
        Ok(db)
    }

    pub(crate) fn roots_db(db: &Connection) -> Result<Vec<Root>> {
        let mut roots: Vec<Root> = db.prepare("SELECT s.id,s.path,s.active,s.paused,s.scan_state,s.scanned,s.failures,CASE WHEN l.root_id IS NULL THEN 'legacy' WHEN l.valid=1 THEN 'available' ELSE 'unavailable' END FROM scan_roots s LEFT JOIN root_locators l ON l.root_id=s.id ORDER BY s.path")?.query_map([],|r|Ok(Root{id:r.get(0)?,path:r.get(1)?,active:r.get(2)?,paused:r.get(3)?,scan_state:r.get(4)?,scanned:r.get(5)?,failures:r.get(6)?,location_status:r.get(7)?,exclusions:vec![]}))?.collect::<rusqlite::Result<_>>()?;
        for root in &mut roots {
            root.exclusions=db.prepare("SELECT relative_path FROM scan_exclusions WHERE root_id=? ORDER BY relative_path")?.query_map([&root.id],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
        }
        Ok(roots)
    }

    pub fn roots(&self) -> Result<Vec<Root>> {
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        Self::roots_db(&self.connection()?)
    }

    pub fn query_revision(&self) -> Result<u32> {
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        version(&self.connection()?, "revision")
    }

    pub fn catalog_version(&self) -> Result<CatalogVersion> {
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut connection = self.connection()?;
        let db = connection.transaction()?;
        // 通知只读取同一快照的版本，不为判断刷新范围重复组装整个概览。
        Ok(CatalogVersion {
            session: self.session.clone(),
            permission_version: version(&db, "permission_version")?,
            revision: version(&db, "revision")?,
        })
    }

    pub fn overview(&self) -> Result<Overview> {
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        // 扫描可并发提交；概览的各部分与版本必须属于同一读取快照。
        let mut connection = self.connection()?;
        let db = connection.transaction()?;
        let roots = Self::roots_db(&db)?;
        let workspaces=db.prepare("SELECT w.id,w.name,count(wr.resource_id) FROM workspaces w LEFT JOIN workspace_resources wr ON wr.workspace_id=w.id GROUP BY w.id ORDER BY w.sort_order,w.created_at DESC,w.id")?.query_map([],|r|Ok(Workspace{id:r.get(0)?,name:r.get(1)?,count:r.get(2)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let activity_visible = format!(
            "({}) AND ({})",
            crate::activity_origin::activity_visible(),
            crate::quality::activity_quality()
        );
        let sql=format!("SELECT a.id,a.kind,CASE WHEN a.name<>'' THEN a.name WHEN a.resource_id IS NOT NULL THEN r.name ELSE a.name END,a.resource_id,a.at,a.source,a.file_modified_at FROM activities a LEFT JOIN resources r ON r.id=a.resource_id WHERE {activity_visible} ORDER BY a.at DESC,a.id LIMIT 100");
        let activities = db
            .prepare(&sql)?
            .query_map([], |r| {
                Ok(Activity {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    name: r.get(2)?,
                    resource_id: r.get(3)?,
                    at: r.get(4)?,
                    source: r.get(5)?,
                    file_modified_at: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Overview {
            session: self.session.clone(),
            permission_version: version(&db, "permission_version")?,
            revision: version(&db, "revision")?,
            sqlite_version: rusqlite::version().into(),
            roots,
            file_grants: Self::file_grants_db(&db)?,
            workspaces,
            activities,
        })
    }

    pub(crate) fn resource_db(db: &Connection, rid: &str, placeholder: bool) -> Result<Resource> {
        let sql=format!("SELECT r.id,r.kind,r.name,r.location,r.status,r.modified_at,r.size,r.favorite,{VISIBLE} FROM resources r WHERE r.id=?");
        let (mut item, visible) = db.query_row(&sql, [rid], |row| {
            Ok((
                Resource {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    name: row.get(2)?,
                    location: row.get(3)?,
                    status: row.get(4)?,
                    modified_at: row.get(5)?,
                    size: row.get(6)?,
                    favorite: row.get(7)?,
                    workspace_ids: vec![],
                    opened_at: None,
                    match_field: None,
                },
                row.get::<_, bool>(8)?,
            ))
        })?;
        item.opened_at = db.query_row(
            &format!(
                "SELECT MAX(a.at) FROM activities a JOIN resources r ON r.id=?1 WHERE {}",
                crate::activity_origin::recent_opened()
            ),
            [rid],
            |r| r.get(0),
        )?;
        item.workspace_ids = db
            .prepare("SELECT workspace_id FROM workspace_resources WHERE resource_id=?")?
            .query_map([rid], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        if !visible {
            if !placeholder {
                return Err(AppError::new(
                    "not_authorized",
                    "请重新授权管理位置",
                    "Reauthorize the managed location.",
                ));
            }
            item.name = "未授权的文件".into();
            item.location = String::new();
            item.status = "permission_denied".into();
            item.modified_at = 0.;
            item.size = 0.;
            item.favorite = false;
            item.opened_at = None;
        }
        Ok(item)
    }

    pub fn resources(&self, query: ResourceQuery) -> Result<ResourcePage> {
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut connection = self.connection()?;
        let db = connection.transaction()?;
        let in_workspace = query.workspace_id.is_some();
        let filter = if in_workspace { "1" } else { VISIBLE };
        // 文件打开还须属于当前物理对象；网页没有文件origin，保留其记录语义。筛选和排序共用条件。
        let recent_opened = crate::activity_origin::recent_opened();
        let recommended = crate::quality::recommended();
        let sql=format!("SELECT r.id FROM resources r WHERE {filter} AND (?3<>'modified' OR ?1 IS NOT NULL OR {recommended}) AND (?1 IS NULL OR EXISTS(SELECT 1 FROM workspace_resources wr WHERE wr.workspace_id=?1 AND wr.resource_id=r.id)) AND (?2='all' OR r.kind=?2) AND (?3<>'favorites' OR r.favorite=1) AND (?3<>'recent' OR EXISTS(SELECT 1 FROM activities a WHERE {recent_opened})) ORDER BY CASE WHEN ?3='recent' THEN (SELECT MAX(a.at) FROM activities a WHERE {recent_opened}) ELSE r.modified_at END DESC,r.id LIMIT 101 OFFSET ?4");
        let ids = db
            .prepare(&sql)?
            .query_map(
                params![query.workspace_id, query.kind, query.mode, query.offset],
                |r| r.get::<_, String>(0),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let has_more = ids.len() > 100;
        let items = ids
            .iter()
            .take(100)
            .map(|rid| Self::resource_db(&db, rid, in_workspace))
            .collect::<Result<_>>()?;
        Ok(ResourcePage {
            session: self.session.clone(),
            permission_version: version(&db, "permission_version")?,
            revision: version(&db, "revision")?,
            items,
            has_more,
        })
    }

    pub fn search(&self, request: SearchRequest) -> Result<SearchResponse> {
        self.search_with_cancellation(request, &crate::SearchCancellation::default())
    }

    pub fn search_with_cancellation(
        &self,
        request: SearchRequest,
        cancellation: &crate::SearchCancellation,
    ) -> Result<SearchResponse> {
        cancellation.check()?;
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        cancellation.check()?;
        let mut connection = self.connection()?;
        let db = connection.transaction()?;
        cancellation.with_connection(&db, || {
            let q = search::normalize(request.query.trim());
            search::validate_query_text(&q)?;
            if q.chars().count() > 256 {
                return Err(AppError::new(
                    "invalid_input",
                    "搜索内容过长",
                    "The search text is too long.",
                ));
            }
            let mut items = vec![];
            let mut has_more = false;
            if !q.is_empty() {
                let ids = search::select_ids(&db, &q, &request)?;
                has_more = ids.len() > 30;
                for (rid, kind) in ids.into_iter().take(30) {
                    cancellation.check()?;
                    if kind == "workspace" {
                        let name =
                            db.query_row("SELECT name FROM workspaces WHERE id=?", [&rid], |r| {
                                r.get(0)
                            })?;
                        items.push(Resource {
                            id: rid,
                            kind,
                            name,
                            location: String::new(),
                            status: "available".into(),
                            modified_at: 0.,
                            size: 0.,
                            favorite: false,
                            workspace_ids: vec![],
                            opened_at: None,
                            match_field: None,
                        });
                    } else {
                        items.push(Self::resource_db(&db, &rid, false)?);
                    }
                }
            }
            for item in &mut items {
                item.match_field = search::matched_field(item, &q);
            }
            Ok(SearchResponse {
                request_id: request.request_id,
                next_cursor: None,
                page: ResourcePage {
                    session: self.session.clone(),
                    permission_version: version(&db, "permission_version")?,
                    revision: version(&db, "revision")?,
                    items,
                    has_more,
                },
            })
        })
    }

    pub fn search_page(&self, request: SearchPageRequest) -> Result<SearchResponse> {
        self.search_page_with_cancellation(request, &crate::SearchCancellation::default())
    }

    pub fn search_page_with_cancellation(
        &self,
        request: SearchPageRequest,
        cancellation: &crate::SearchCancellation,
    ) -> Result<SearchResponse> {
        cancellation.check()?;
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        cancellation.check()?;
        let mut connection = self.connection()?;
        let db = connection.transaction()?;
        cancellation.with_connection(&db, || {
            let criteria = search::page_criteria(&request)?;
            let permission_version = version(&db, "permission_version")?;
            let revision = version(&db, "revision")?;
            // 只有同一查询和数据版本的后续页可继续；请求序号不属于查询条件。
            let valid_cursor = match (&request.cursor, request.offset) {
                (None, 0) => true,
                (Some(cursor), offset) if offset > 0 => {
                    cursor.version == 4
                        && cursor.session == self.session
                        && cursor.permission_version == permission_version
                        && cursor.revision == revision
                        && cursor.next_offset == offset
                        && offset.is_multiple_of(100)
                        && cursor.criteria == criteria
                        && match (&cursor.name_position,&cursor.rank_position,&cursor.modified_position,criteria.sort.as_str()) {
                            (Some(position),None,None,"name") => !position.id.is_empty() && position.name.len()<=32768,
                            (None,Some(position),None,"relevance") => position.rank<=3 && !position.id.is_empty() && position.name.len()<=32768,
                            (None,None,Some(position),"modified") => !position.id.is_empty() && position.name.len()<=32768 && position.modified_at.is_none_or(|time|time.is_finite() && time.abs()<=8_640_000_000_000_000.),
                            _=>false,
                        }
                }
                _ => false,
            };
            if !valid_cursor {
                return Err(AppError::new(
                    "stale_cursor",
                    "搜索结果或条件已变化，请从第一页重新加载",
                    "The search results or criteria have changed. Reload from the first page.",
                ));
            }
            // 名称续页按上一页末项定位；offset仅保留协议计数，不再重复扫描前页。
            let ids = search::select_page_ids(
                &db,
                &criteria,
                request.offset,
                request
                    .cursor
                    .as_ref()
                    .and_then(|cursor| cursor.name_position.as_ref()),
                request.cursor.as_ref().and_then(|cursor|cursor.rank_position.as_ref()),
                request.cursor.as_ref().and_then(|cursor|cursor.modified_position.as_ref()),
            )?;
            let more = ids.len() > 100;
            let mut items = Vec::new();
            for (rid, kind) in ids.into_iter().take(100) {
                cancellation.check()?;
                if kind == "workspace" {
                    let name =
                        db.query_row("SELECT name FROM workspaces WHERE id=?", [&rid], |r| {
                            r.get(0)
                        })?;
                    items.push(Resource {
                        id: rid,
                        kind,
                        name,
                        location: String::new(),
                        status: "available".into(),
                        modified_at: 0.,
                        size: 0.,
                        favorite: false,
                        workspace_ids: vec![],
                    opened_at: None,
                    match_field: None,
                    });
                } else {
                    items.push(Self::resource_db(&db, &rid, false)?);
                }
            }
            for item in &mut items { item.match_field=search::matched_field(item,&criteria.query); }
            let (name_position,rank_position)=if more && criteria.sort!="modified" {
                let id=&items.last().expect("a full page has a last item").id;
                let (name,rank):(String,u8)=db.query_row("SELECT name,CASE WHEN name=?2 THEN 0 WHEN substr(name,1,length(?2))=?2 THEN 1 WHEN instr(name,?2)>0 THEN 2 ELSE 3 END FROM search_documents WHERE entity_id=?1",params![id,criteria.query],|row|Ok((row.get(0)?,row.get(1)?)))?;
                if criteria.sort=="name" { (Some(SearchNamePosition{name,id:id.clone()}),None) }
                else { (None,Some(SearchRankPosition{rank,name,id:id.clone()})) }
            } else { (None,None) };
            let modified_position = if more && criteria.sort=="modified" {
                let id=&items.last().expect("a full page has a last item").id;
                let (name,modified_at)=db.query_row("SELECT name,modified_at FROM search_documents WHERE entity_id=?",[id],|row|Ok((row.get(0)?,row.get(1)?)))?;
                Some(SearchModifiedPosition{name,modified_at,id:id.clone()})
            } else { None };
            Ok(SearchResponse {
                request_id: request.request.request_id,
                next_cursor: if more {
                    Some(SearchCursor {
                        version: 4,
                        session: self.session.clone(),
                        permission_version,
                        revision,
                        next_offset: request.offset.checked_add(100).ok_or_else(|| {
                            AppError::new(
                                "invalid_input",
                                "搜索分页范围过大",
                                "The requested search page is too far ahead.",
                            )
                        })?,
                        criteria,
                        name_position,
                        rank_position,
                        modified_position,
                    })
                } else {
                    None
                },
                page: ResourcePage {
                    session: self.session.clone(),
                    permission_version,
                    revision,
                    items,
                    has_more: more,
                },
            })
        })
    }

    pub(crate) fn visible(db: &Connection, rid: &str) -> Result<()> {
        let sql = format!("SELECT EXISTS(SELECT 1 FROM resources r WHERE r.id=? AND {VISIBLE})");
        if !db.query_row(&sql, [rid], |r| r.get::<_, bool>(0))? {
            return Err(AppError::new(
                "not_authorized",
                "资源不存在或访问授权已失效",
                "The resource no longer exists or its access authorization is no longer valid.",
            ));
        }
        Ok(())
    }
    fn workspace_exists(db: &Connection, wid: &str) -> Result<()> {
        if !db.query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?)",
            [wid],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(AppError::new(
                "conflict",
                "工作区已不存在",
                "The workspace no longer exists.",
            ));
        }
        Ok(())
    }

    fn link_resources(db: &Connection, workspace_id: &str, resource_ids: &[String]) -> Result<u32> {
        if resource_ids.len() > 1000 {
            return Err(AppError::new(
                "invalid_input",
                "每次最多处理 1000 项",
                "Process up to 1,000 items at a time.",
            ));
        }
        let mut total = 0;
        for rid in resource_ids {
            Self::visible(db, rid)?;
            let changed = db.execute(
                "INSERT OR IGNORE INTO workspace_resources VALUES(?1,?2,?3)",
                params![workspace_id, rid, now()],
            )?;
            if changed > 0 {
                event(db, "linked", "", Some(rid))?;
            }
            total += changed as u32;
        }
        Ok(total)
    }

    pub fn mutate(&self, request: MutationRequest) -> Result<MutationResponse> {
        if request.operation_id.len() > 100 || Uuid::parse_str(&request.operation_id).is_err() {
            return Err(AppError::new(
                "invalid_input",
                "操作标识无效",
                "The operation ID is invalid.",
            ));
        }
        let payload = serde_json::to_string(&request.mutation)
            .map_err(|_| AppError::new("invalid_input", "操作无效", "The operation is invalid."))?;
        let _guard = self.gate.write().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut db = self.connection()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some((old, result)) = tx
            .query_row(
                "SELECT payload,result FROM operations WHERE id=?",
                [&request.operation_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?
        {
            if old != payload {
                return Err(AppError::new(
                    "conflict",
                    "操作标识已用于其他请求",
                    "This operation ID has already been used for another request.",
                ));
            }
            return serde_json::from_str(&result).map_err(|_| {
                AppError::new(
                    "database_error",
                    "操作回执损坏",
                    "The saved operation result is damaged.",
                )
            });
        }
        let mut output = MutationResponse {
            id: None,
            changed: 0,
        };
        let mut permission = false;
        let linking = matches!(&request.mutation, Mutation::Link { .. });
        match request.mutation {
            Mutation::CreateWorkspace { name, resource_ids } => {
                let name = label(&name)?;
                let wid = id();
                tx.execute(
                    "INSERT INTO workspaces(id,name,created_at) VALUES(?1,?2,?3)",
                    params![wid, name, now()],
                )?;
                search::project(&tx, &wid, "workspace", &name, "", None)?;
                event(&tx, "workspace_created", &name, None)?;
                // 创建与加入共用同一事务；任一资料失效时不留下空工作区或部分关联。
                output.changed =
                    1 + Self::link_resources(&tx, &wid, &resource_ids.unwrap_or_default())?;
                output.id = Some(wid);
            }
            Mutation::ReorderWorkspaces { ids } => {
                let existing = tx
                    .prepare("SELECT id FROM workspaces")?
                    .query_map([], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<std::collections::HashSet<_>>>()?;
                let requested: std::collections::HashSet<_> = ids.iter().cloned().collect();
                if existing != requested || requested.len() != ids.len() {
                    return Err(AppError::new(
                        "conflict",
                        "工作区列表已变化，请刷新后重试",
                        "The workspace list has changed. Refresh and try again.",
                    ));
                }
                for (index, id) in ids.iter().enumerate() {
                    tx.execute(
                        "UPDATE workspaces SET sort_order=?1 WHERE id=?2",
                        params![index, id],
                    )?;
                }
                output.changed = ids.len() as u32;
            }
            Mutation::RenameWorkspace { id: wid, name } => {
                Self::workspace_exists(&tx, &wid)?;
                let name = label(&name)?;
                tx.execute(
                    "UPDATE workspaces SET name=?1 WHERE id=?2",
                    params![name, wid],
                )?;
                search::project(&tx, &wid, "workspace", &name, "", None)?;
                event(&tx, "workspace_renamed", &name, None)?;
                output.changed = 1;
            }
            Mutation::DeleteWorkspace { id: wid } => {
                Self::workspace_exists(&tx, &wid)?;
                let name: String =
                    tx.query_row("SELECT name FROM workspaces WHERE id=?", [&wid], |r| {
                        r.get(0)
                    })?;
                tx.execute("DELETE FROM workspaces WHERE id=?", [&wid])?;
                search::remove(&tx, &wid)?;
                event(&tx, "workspace_deleted", &name, None)?;
                output.changed = 1;
            }
            Mutation::Link {
                workspace_id,
                resource_ids,
            }
            | Mutation::Unlink {
                workspace_id,
                resource_ids,
            } => {
                Self::workspace_exists(&tx, &workspace_id)?;
                if linking {
                    output.changed = Self::link_resources(&tx, &workspace_id, &resource_ids)?;
                } else {
                    if resource_ids.len() > 1000 {
                        return Err(AppError::new(
                            "invalid_input",
                            "每次最多处理 1000 项",
                            "Process up to 1,000 items at a time.",
                        ));
                    }
                    for rid in resource_ids {
                        let changed = tx.execute("DELETE FROM workspace_resources WHERE workspace_id=?1 AND resource_id=?2",params![workspace_id,rid])?;
                        output.changed += changed as u32;
                        if changed > 0 {
                            event(&tx, "unlinked", "", Some(&rid))?;
                        }
                    }
                }
            }
            Mutation::Transfer {
                source,
                target,
                resource_ids,
            } => {
                Self::workspace_exists(&tx, &source)?;
                Self::workspace_exists(&tx, &target)?;
                if resource_ids.len() > 1000 {
                    return Err(AppError::new(
                        "invalid_input",
                        "每次最多处理 1000 项",
                        "Process up to 1,000 items at a time.",
                    ));
                }
                if source != target {
                    for rid in resource_ids {
                        Self::visible(&tx, &rid)?;
                        if !tx.query_row("SELECT EXISTS(SELECT 1 FROM workspace_resources WHERE workspace_id=?1 AND resource_id=?2)",params![source,rid],|r|r.get::<_,bool>(0))? {return Err(AppError::new("conflict","来源引用已变更，请刷新","The source reference has changed. Refresh and try again."));}
                        tx.execute(
                            "INSERT OR IGNORE INTO workspace_resources VALUES(?1,?2,?3)",
                            params![target, rid, now()],
                        )?;
                        tx.execute("DELETE FROM workspace_resources WHERE workspace_id=?1 AND resource_id=?2",params![source,rid])?;
                        event(&tx, "transferred", "", Some(&rid))?;
                        output.changed += 1;
                    }
                }
            }
            Mutation::Favorite { id: rid, value } => {
                Self::visible(&tx, &rid)?;
                output.changed = tx.execute(
                    "UPDATE resources SET favorite=?1 WHERE id=?2",
                    params![value, rid],
                )? as u32;
            }
            Mutation::SaveWeb {
                id: rid,
                name,
                url,
                workspace_id,
            } => {
                if let Some(wid) = &workspace_id {
                    Self::workspace_exists(&tx, wid)?;
                }
                let name = label(&name)?;
                let parsed = url::Url::parse(url.trim()).map_err(|_| {
                    AppError::new(
                        "invalid_input",
                        "请输入有效的 http/https 网址",
                        "Enter a valid HTTP/HTTPS URL.",
                    )
                })?;
                if !["http", "https"].contains(&parsed.scheme())
                    || parsed.host_str().is_none()
                    || !parsed.username().is_empty()
                    || parsed.password().is_some()
                    || url.len() > 8192
                {
                    return Err(AppError::new(
                        "invalid_input",
                        "仅支持不含登录凭据的 http/https 网址",
                        "Only HTTP/HTTPS URLs without login credentials are supported.",
                    ));
                }
                let rid = if let Some(rid) = rid {
                    let kind: Option<String> = tx
                        .query_row("SELECT kind FROM resources WHERE id=?", [&rid], |r| {
                            r.get(0)
                        })
                        .optional()?;
                    if kind.as_deref() != Some("web") {
                        return Err(AppError::new(
                            "conflict",
                            "网页已不存在",
                            "The saved link no longer exists.",
                        ));
                    }
                    rid
                } else {
                    id()
                };
                let saved_at = now();
                tx.execute("INSERT INTO resources(id,kind,name,location,created_at,modified_at) VALUES(?1,'web',?2,?3,?4,?4) ON CONFLICT(id) DO UPDATE SET name=excluded.name,location=excluded.location,modified_at=excluded.modified_at",params![rid,name,parsed.as_str(),saved_at])?;
                search::project(&tx, &rid, "web", &name, parsed.as_str(), Some(saved_at))?;
                event(&tx, "web_saved", "", Some(&rid))?;
                // 工作区内添加网页与建立引用在同一事务中完成，失败不留下孤立网页。
                if let Some(wid) = workspace_id {
                    let changed = tx.execute(
                        "INSERT OR IGNORE INTO workspace_resources VALUES(?1,?2,?3)",
                        params![wid, rid, now()],
                    )?;
                    if changed > 0 {
                        event(&tx, "linked", "", Some(&rid))?;
                    }
                }
                output.id = Some(rid);
                output.changed = 1;
            }
            Mutation::DeleteWeb { id: rid } => {
                let kind: Option<String> = tx
                    .query_row("SELECT kind FROM resources WHERE id=?", [&rid], |r| {
                        r.get(0)
                    })
                    .optional()?;
                if kind.as_deref() != Some("web") {
                    return Err(AppError::new(
                        "conflict",
                        "网页已不存在",
                        "The saved link no longer exists.",
                    ));
                }
                // 网页删除仅移除应用记录；先解除引用，避免留下不可解析的活动外键。
                tx.execute(
                    "DELETE FROM workspace_resources WHERE resource_id=?",
                    [&rid],
                )?;
                tx.execute("DELETE FROM activities WHERE resource_id=?", [&rid])?;
                search::remove(&tx, &rid)?;
                output.changed = tx.execute("DELETE FROM resources WHERE id=?", [&rid])? as u32;
            }
            Mutation::RevokeFile { id: rid } => {
                output.changed = tx.execute(
                    "UPDATE file_grants SET active=0 WHERE resource_id=? AND active=1",
                    [rid],
                )? as u32;
                permission = output.changed > 0;
            }
            Mutation::RevokeRoot { id: root } => {
                output.changed=tx.execute("UPDATE scan_roots SET active=0,generation=generation+1,scan_state='cancelled' WHERE id=? AND active=1",[root])? as u32;
                permission = output.changed > 0;
            }
            Mutation::SetPaused { id: root, value } => {
                output.changed=tx.execute("UPDATE scan_roots SET paused=?1,generation=generation+1,scan_state=CASE WHEN ?1 THEN 'paused' ELSE 'idle' END WHERE id=?2 AND active=1",params![value,root])? as u32;
            }
            Mutation::Exclude {
                root_id,
                relative_path,
            } => {
                let path = Path::new(relative_path.trim());
                if path.as_os_str().is_empty()
                    || path
                        .components()
                        .any(|c| !matches!(c, std::path::Component::Normal(_)))
                {
                    return Err(AppError::new(
                        "invalid_input",
                        "请输入管理位置内的相对子目录",
                        "Enter a relative subfolder within the managed location.",
                    ));
                }
                tx.execute(
                    "INSERT OR IGNORE INTO scan_exclusions VALUES(?1,?2)",
                    params![root_id, path.to_string_lossy()],
                )?;
                tx.execute(
                    "UPDATE scan_roots SET generation=generation+1,scan_state='idle' WHERE id=?",
                    [root_id],
                )?;
                permission = true;
                output.changed = 1;
            }
            Mutation::Include {
                root_id,
                relative_path,
            } => {
                output.changed = tx.execute(
                    "DELETE FROM scan_exclusions WHERE root_id=?1 AND relative_path=?2",
                    params![root_id, relative_path],
                )? as u32;
                if output.changed > 0 {
                    tx.execute("UPDATE scan_roots SET generation=generation+1,scan_state='idle' WHERE id=?", [&root_id])?;
                    permission = true;
                }
            }
            Mutation::RebuildSearch => {
                search::rebuild(&tx)?;
                output.changed = 1;
            }
        }
        bump(&tx, permission)?;
        tx.execute(
            "INSERT INTO operations VALUES(?1,?2,?3)",
            params![
                request.operation_id,
                payload,
                serde_json::to_string(&output).map_err(|_| AppError::new(
                    "internal_error",
                    "回执序列化失败",
                    "The operation result could not be saved."
                ))?
            ],
        )?;
        tx.commit()?;
        Ok(output)
    }

    pub fn grant_root(&self, path: &Path) -> Result<String> {
        let path = path.canonicalize()?;
        let metadata = path.metadata()?;
        if !metadata.is_dir() {
            return Err(AppError::new(
                "invalid_input",
                "请选择目录",
                "Select a folder.",
            ));
        }
        let identity = platform::identity(&path, &metadata)?;
        let path_text = path.to_str().ok_or_else(|| {
            AppError::new(
                "invalid_input",
                "目录名称编码暂不支持",
                "The folder name encoding is not supported.",
            )
        })?;
        // 显式选择的原生定位捕获不得持有Store gate或SQLite写锁。
        #[cfg(target_os = "macos")]
        let bookmark = Some(crate::macos_bookmark::create(&path)?);
        #[cfg(not(target_os = "macos"))]
        let bookmark: Option<Vec<u8>> = None;
        let _guard = self.gate.write().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        if !path.is_dir()
            || platform::identity(&path, &path.metadata()?)? != identity
            || path.canonicalize()? != path
        {
            return Err(AppError::new(
                "identity_unconfirmed",
                "目录已变化，请重新选择",
                "The folder has changed. Select it again.",
            ));
        }
        let by_identity: Option<(String,String)> = tx.query_row("SELECT s.id,s.path FROM scan_roots s JOIN root_locators l ON l.root_id=s.id WHERE l.identity=?",[&identity],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let by_path: Option<String> = tx
            .query_row("SELECT id FROM scan_roots WHERE path=?", [path_text], |r| {
                r.get(0)
            })
            .optional()?;
        if by_identity
            .as_ref()
            .is_some_and(|(id, _)| by_path.as_ref().is_some_and(|other| other != id))
        {
            return Err(AppError::new(
                "conflict",
                "此路径已属于另一管理位置，请先核对授权",
                "This path belongs to another managed location. Review its authorization first.",
            ));
        }
        let rid = by_identity
            .as_ref()
            .map(|(id, _)| id.clone())
            .or(by_path)
            .unwrap_or_else(id);
        let previous_identity: Option<String> = tx
            .query_row(
                "SELECT identity FROM root_locators WHERE root_id=?",
                [&rid],
                |r| r.get(0),
            )
            .optional()?;
        if previous_identity.as_deref() != Some(&identity) {
            tx.execute("DELETE FROM root_aliases WHERE root_id=?", [&rid])?;
        } else if let Some((_, old)) = &by_identity {
            if old != path_text {
                tx.execute(
                    "INSERT OR IGNORE INTO root_aliases VALUES(?1,?2)",
                    params![rid, old],
                )?;
            }
        }
        tx.execute("INSERT INTO scan_roots(id,path,active) VALUES(?1,?2,1) ON CONFLICT(id) DO UPDATE SET path=excluded.path,active=1,paused=0,generation=generation+1,scan_state='idle'",params![rid,path_text])?;
        tx.execute("INSERT INTO root_locators VALUES(?1,?2,?3,1) ON CONFLICT(root_id) DO UPDATE SET identity=excluded.identity,bookmark=excluded.bookmark,valid=1",params![rid,identity,bookmark])?;
        bump(&tx, true)?;
        tx.commit()?;
        Ok(rid)
    }

    pub(crate) fn path_allowed(db: &Connection, path: &Path) -> Result<bool> {
        for root in Self::roots_db(db)? {
            if Self::allowed_by_root(db, &root, path)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn resolve_resource(&self, rid: &str) -> Result<(String, bool)> {
        let result = self.resolve_resource_current(rid);
        #[cfg(windows)]
        if result.as_ref().err().is_some_and(|e| {
            matches!(
                e.code.as_str(),
                "resource_unavailable" | "identity_unconfirmed"
            )
        }) {
            let location: Option<String> = {
                let db = self.connection()?;
                db.query_row(
                    "SELECT location FROM resources WHERE id=? AND kind='file'",
                    [rid],
                    |r| r.get(0),
                )
                .optional()?
            };
            if let Some(location) = location {
                if Path::new(&location).exists() && self.reconcile_document(Path::new(&location))? {
                    return self.resolve_resource_current(rid);
                }
            }
        }
        if result.as_ref().err().is_some_and(|e| {
            matches!(
                e.code.as_str(),
                "resource_unavailable" | "identity_unconfirmed" | "not_authorized"
            )
        }) {
            #[cfg(target_os = "macos")]
            let mut locator_error = None;
            #[cfg(not(target_os = "macos"))]
            let locator_error: Option<AppError> = None;
            #[cfg(target_os = "macos")]
            match self.recover_single_file(rid) {
                Ok(true) => return self.resolve_resource_current(rid),
                Ok(false) => {}
                Err(error)
                    if crate::file_locator::is_locator_error(&error)
                        || error.code == "identity_unconfirmed" =>
                {
                    locator_error = Some(error);
                }
                Err(error) => return Err(error),
            }
            if self.recover_root_file(rid)? {
                return self.resolve_resource_current(rid);
            }
            if let Some(error) = locator_error {
                return Err(error);
            }
        }
        result
    }

    fn resolve_resource_current(&self, rid: &str) -> Result<(String, bool)> {
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let db = self.connection()?;
        Self::resolve_resource_db(&db, rid)
    }

    pub(crate) fn resolve_resource_db(db: &Connection, rid: &str) -> Result<(String, bool)> {
        Self::visible(db, rid)?;
        let (kind, location, identity): (String, String, Option<String>) = db.query_row(
            "SELECT kind,location,identity FROM resources WHERE id=?",
            [rid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        if kind == "web" {
            let parsed = url::Url::parse(&location)
                .map_err(|_| AppError::new("invalid_input", "网址无效", "The URL is invalid."))?;
            if !["http", "https"].contains(&parsed.scheme()) {
                return Err(AppError::new(
                    "invalid_input",
                    "网址协议无效",
                    "The URL scheme is invalid.",
                ));
            }
            return Ok((location, true));
        }
        let path = Path::new(&location).canonicalize()?;
        if !Self::path_allowed(db, &path)?
            && !db.query_row(
                "SELECT EXISTS(SELECT 1 FROM file_grants WHERE resource_id=? AND active=1)",
                [rid],
                |r| r.get::<_, bool>(0),
            )?
        {
            return Err(AppError::new(
                "not_authorized",
                "原件已移出授权范围",
                "The original has moved outside the authorized scope.",
            ));
        }
        if identity.as_deref() != Some(platform::identity(&path, &path.metadata()?)?.as_str()) {
            return Err(AppError::new(
                "identity_unconfirmed",
                "原件身份已变化，请重新指向确认",
                "The original file identity has changed. Relink it to confirm.",
            ));
        }
        Ok((path.to_string_lossy().into(), false))
    }
    pub fn check_open_type(&self, location: &str) -> Result<()> {
        platform::safe_open(Path::new(location))
    }
    pub fn record_open(&self, rid: &str) -> Result<()> {
        let _guard = self.gate.write().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        Self::visible(&tx, rid)?;
        event(&tx, "opened", "", Some(rid))?;
        bump(&tx, false)?;
        tx.commit()?;
        Ok(())
    }

    pub fn relink(&self, rid: &str, path: &Path) -> Result<()> {
        self.relink_confirmed(rid, path, false)
    }

    pub fn relink_confirmed(&self, rid: &str, path: &Path, merge: bool) -> Result<()> {
        self.relink_impl(rid, path, merge, false, None, None)
    }

    /// 仅供原生文件选择器调用；显式选择只授予该对象，不扩大到父目录。
    pub fn relink_selected(
        &self,
        rid: &str,
        selection: &crate::ImportSelection,
        merge: bool,
    ) -> Result<()> {
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
        self.relink_impl(
            rid,
            Path::new(&selection.path),
            merge,
            true,
            Some(identity),
            None,
        )
    }

    pub(crate) fn relink_impl(
        &self,
        rid: &str,
        path: &Path,
        merge: bool,
        grant_selected: bool,
        expected_identity: Option<&str>,
        review: Option<&crate::RelinkReview>,
    ) -> Result<()> {
        let path = path.canonicalize()?;
        let meta = path.metadata()?;
        if !meta.is_file() {
            return Err(AppError::new(
                "invalid_input",
                "请选择原文件",
                "Select the original file.",
            ));
        }
        let identity = platform::identity(&path, &meta)?;
        if expected_identity.is_some_and(|expected| expected != identity) {
            return Err(AppError::new(
                "identity_unconfirmed",
                "所选文件已被替换，请重新选择",
                "The selected file has been replaced. Select it again.",
            ));
        }
        let locator = self.capture_import_locator(&path, &identity, true, None)?;
        let _guard = self.gate.write().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        if let Some(review) = review {
            review.validate(&tx)?;
        }
        let file_granted: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM file_grants WHERE resource_id=? AND active=1)",
            [rid],
            |r| r.get(0),
        )?;
        let old_identity: Option<String> = tx
            .query_row(
                "SELECT identity FROM resources WHERE id=? AND kind='file'",
                [rid],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::new("conflict", "资源已不存在", "The resource no longer exists.")
            })?;
        let same_identity = old_identity.as_deref() == Some(identity.as_str());
        let scoped = Self::path_allowed(&tx, &path)?;
        let other: Option<String> = tx
            .query_row(
                "SELECT id FROM resources WHERE identity=?1 AND id<>?2",
                params![identity, rid],
                |r| r.get(0),
            )
            .optional()?;
        let other_granted = if let Some(other) = &other {
            tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM file_grants WHERE resource_id=? AND active=1)",
                [other],
                |r| r.get::<_, bool>(0),
            )?
        } else {
            false
        };
        if !(scoped || (file_granted && same_identity) || other_granted || grant_selected) {
            return Err(AppError::new(
                "not_authorized",
                "请重新选择该文件以授予单文件访问权限",
                "Select this file again to authorize access to that individual file.",
            ));
        }
        if other.is_some() && !merge {
            return Err(AppError::new("merge_confirmation_required","该文件已在资料库中，需确认合并它的工作区关联与收藏","This file already has a library record. Confirm merging its workspace references and favorites."));
        }
        // 原 UUID 的授权绑定旧对象；换对象不能因复用 UUID 自动继承授权。
        if !same_identity && file_granted {
            tx.execute("UPDATE file_grants SET active=0 WHERE resource_id=?", [rid])?;
        }
        let permission_changed = (!same_identity && file_granted)
            || other_granted
            || (grant_selected && !scoped && !file_granted);
        if !same_identity {
            crate::activity_origin::retire(&tx, rid)?;
        }
        let merged_origin = other.clone();
        if let Some(other) = other {
            // 保留用户正在修复的 UUID，合并已发现对象的引用，所有关系在同一事务更新。
            tx.execute("INSERT OR IGNORE INTO workspace_resources SELECT workspace_id,?1,added_at FROM workspace_resources WHERE resource_id=?2",params![rid,other])?;
            tx.execute(
                "DELETE FROM workspace_resources WHERE resource_id=?",
                [&other],
            )?;
            tx.execute("UPDATE resources SET favorite=MAX(favorite,(SELECT favorite FROM resources WHERE id=?2)) WHERE id=?1",params![rid,other])?;
            tx.execute(
                "UPDATE activities SET resource_id=?1 WHERE resource_id=?2",
                params![rid, other],
            )?;
            tx.execute("INSERT INTO file_grants SELECT ?1,active FROM file_grants WHERE resource_id=?2 ON CONFLICT(resource_id) DO UPDATE SET active=MAX(file_grants.active,excluded.active)",params![rid,other])?;
            tx.execute("DELETE FROM file_grants WHERE resource_id=?", [&other])?;
            tx.execute("DELETE FROM file_roots WHERE resource_id=?", [&other])?;
            search::remove(&tx, &other)?;
            tx.execute("DELETE FROM resources WHERE id=?", [&other])?;
        }
        let name = path.file_name().and_then(|v| v.to_str()).ok_or_else(|| {
            AppError::new(
                "invalid_input",
                "文件名称编码暂不支持",
                "The filename encoding is not supported.",
            )
        })?;
        // 与导入一样，在写入前重新读取对象身份；失败会回滚前面的关系合并。
        let current_meta = path.metadata()?;
        if platform::identity(&path, &current_meta)? != identity {
            return Err(AppError::new(
                "identity_unconfirmed",
                "文件已变化，请重新选择",
                "The file has changed. Select it again.",
            ));
        }
        if grant_selected && !scoped {
            tx.execute("INSERT INTO file_grants VALUES(?1,1) ON CONFLICT(resource_id) DO UPDATE SET active=1", [rid])?;
        }
        let count=tx.execute("UPDATE resources SET identity=?1,name=?2,location=?3,status='available',size=?4,modified_at=?5 WHERE id=?6 AND kind='file'",params![identity,name,path.to_string_lossy(),meta.len() as f64,crate::catalog::mtime(&meta),rid])?;
        if count != 1 {
            return Err(AppError::new(
                "conflict",
                "资源已不存在",
                "The resource no longer exists.",
            ));
        }
        tx.execute("DELETE FROM file_roots WHERE resource_id=?", [rid])?;
        for root in Self::roots_db(&tx)? {
            if !Self::allowed_by_root(&tx, &root, &path)? {
                continue;
            }
            tx.execute(
                "INSERT INTO file_roots VALUES(?1,?2,0)",
                params![root.id, rid],
            )?;
        }
        search::project(
            &tx,
            rid,
            "file",
            name,
            &path.to_string_lossy(),
            Some(crate::catalog::mtime(&meta)),
        )?;
        tx.execute(
            "DELETE FROM file_locators WHERE resource_id=?1 AND identity<>?2",
            params![rid, identity],
        )?;
        if let Some(bookmark) = locator {
            tx.execute("INSERT INTO file_locators VALUES(?1,?2,?3) ON CONFLICT(resource_id) DO UPDATE SET identity=excluded.identity,bookmark=excluded.bookmark",params![rid,identity,bookmark])?;
        }
        let aid = event(&tx, "relinked", "", Some(rid))?;
        tx.execute(
            "INSERT INTO resource_relinks VALUES(?1,?2,?3,?4,?5)",
            params![aid, rid, merged_origin, old_identity, identity],
        )?;
        bump(&tx, permission_changed)?;
        tx.commit()?;
        Ok(())
    }

    pub fn backup_to(&self, path: &Path) -> Result<()> {
        if path.exists() {
            return Err(AppError::new(
                "conflict",
                "备份目标已存在，请选择新文件名",
                "The backup destination already exists. Choose a new filename.",
            ));
        }
        let _guard = self.gate.write().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let source = self.connection()?;
        let snapshot = source.unchecked_transaction()?;
        version(&snapshot, "revision")?;
        let mut target = Connection::open(path)?;
        copy_snapshot(&snapshot, &mut target)?;
        Ok(())
    }

    pub fn restore_from(&self, path: &Path) -> Result<()> {
        let _guard = self.gate.write().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let mut staging = Connection::open_in_memory()?;
        {
            let incoming =
                Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            incoming.busy_timeout(Duration::from_millis(1500))?;
            let snapshot = incoming.unchecked_transaction()?;
            let version: u32 = snapshot.pragma_query_value(None, "user_version", |r| r.get(0))?;
            if version == 0 || version > crate::migrations::VERSION {
                return Err(AppError::new(
                    "unsupported_data_version",
                    "备份版本不受支持",
                    "This backup version is not supported.",
                ));
            }
            let integrity: String =
                snapshot.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
            let fk: Option<String> = snapshot
                .query_row("PRAGMA foreign_key_check", [], |r| r.get(0))
                .optional()?;
            if integrity != "ok" || fk.is_some() {
                return Err(AppError::new(
                    "database_error",
                    "备份完整性校验失败",
                    "The backup failed its integrity check.",
                ));
            }
            copy_snapshot(&snapshot, &mut staging)?;
        }
        // 先在内存校验业务结构并撤销备份授权；源事务在写回前释放，避免自读锁阻塞恢复。
        crate::migrations::migrate(&mut staging)?;
        for table in [
            "resources",
            "workspaces",
            "workspace_resources",
            "operations",
            "settings",
            "scan_roots",
        ] {
            staging.prepare(&format!("SELECT * FROM {table} LIMIT 0"))?;
        }
        staging.prepare(
            "SELECT id,text,pinned,version,updated_at,deleted FROM scratch_notes LIMIT 0",
        )?;
        staging.prepare("SELECT resource_id,identity,bookmark FROM file_locators LIMIT 0")?;
        staging.prepare("SELECT root_id,identity,bookmark,valid FROM root_locators LIMIT 0")?;
        staging.prepare("SELECT root_id,path FROM root_aliases LIMIT 0")?;
        staging.prepare(
            "SELECT activity_id,observed_resource_id,identity FROM activity_origins LIMIT 0",
        )?;
        staging.prepare("SELECT activity_id,root_id,root_identity,relative_path,retired_location_valid FROM activity_root_scopes LIMIT 0")?;
        staging.prepare("SELECT event_id,source_resource_id,merged_resource_id,previous_identity,selected_identity FROM resource_relinks LIMIT 0")?;
        let missing_origin:bool=staging.query_row("SELECT EXISTS(SELECT 1 FROM activities a JOIN resources r ON r.id=a.resource_id WHERE r.kind='file' AND NOT EXISTS(SELECT 1 FROM activity_origins o WHERE o.activity_id=a.id))",[],|r|r.get(0))?;
        if missing_origin {
            return Err(AppError::new(
                "database_error",
                "备份缺少必要的历史授权记录",
                "The backup is missing required activity authorization records.",
            ));
        }
        staging.execute("UPDATE root_locators SET valid=0", [])?;
        staging.execute(
            "UPDATE scan_roots SET active=0,paused=0,generation=generation+1,scan_state='idle'",
            [],
        )?;
        staging.execute("UPDATE file_grants SET active=0", [])?;
        // 恢复不复活历史追踪同意；锚点与绑定历史保留，重新启用仍须当前授权。
        staging.execute("UPDATE document_tracking_policies SET enabled=0", [])?;
        staging.execute("DELETE FROM operations", [])?;
        let current = self.connection()?;
        let current_snapshot = current.unchecked_transaction()?;
        let current_permission = version_value(&current_snapshot, "permission_version")?;
        let current_revision = version_value(&current_snapshot, "revision")?;
        staging.execute(
            "UPDATE settings SET value=?1 WHERE key='permission_version'",
            [current_permission + 1],
        )?;
        staging.execute(
            "UPDATE settings SET value=?1 WHERE key='revision'",
            [current_revision + 1],
        )?;
        search::rebuild(&staging)?;
        let safety = self
            .path
            .with_file_name(format!("pre-restore-{}.sqlite3", id()));
        let mut safety_database = Connection::open(safety)?;
        copy_snapshot(&current_snapshot, &mut safety_database)?;
        drop(safety_database);
        drop(current_snapshot);
        drop(current);
        let mut target = self.connection()?;
        copy_snapshot(&staging, &mut target)?;
        // 备份里的代次可能回退；进程内 epoch 永久作废恢复前仍在枚举的扫描。
        self.scan_epoch.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
fn version_value(db: &Connection, key: &str) -> Result<u32> {
    version(db, key)
}

#[cfg(test)]
#[path = "restore_backup_tests.rs"]
mod restore_backup_tests;
