use crate::model::{
    AppError, Result, SearchCriteria, SearchModifiedPosition, SearchNamePosition,
    SearchPageRequest, SearchRankPosition,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{BTreeSet, HashSet};
use unicode_normalization::UnicodeNormalization;

pub fn normalize(input: &str) -> String {
    input.nfc().flat_map(char::to_lowercase).collect()
}

fn token(input: &[char]) -> String {
    let mut result = format!("g{}", input.len());
    for character in input {
        result.push_str(&format!("x{:x}", *character as u32));
    }
    result
}

pub fn short_tokens(name: &str, location: &str) -> String {
    let mut tokens = BTreeSet::new();
    for text in [name, location] {
        let chars: Vec<_> = text.chars().collect();
        for width in [1, 2] {
            for gram in chars.windows(width) {
                tokens.insert(token(gram));
            }
        }
    }
    tokens.into_iter().collect::<Vec<_>>().join(" ")
}

pub fn match_query(query: &str) -> (&'static str, String) {
    let chars: Vec<_> = query.chars().collect();
    if chars.len() < 3 {
        ("search_short", token(&chars))
    } else {
        ("search_tri", format!("\"{}\"", query.replace('"', "\"\"")))
    }
}

pub fn project(
    db: &Connection,
    id: &str,
    kind: &str,
    name: &str,
    location: &str,
    modified_at: Option<f64>,
) -> Result<()> {
    let name = normalize(name);
    let location = normalize(location);
    // 只改mtime不重写FTS；NULL工作区与时间为0的资源必须区分。
    let unchanged = db.query_row("SELECT kind=?2 AND name=?3 AND location=?4 AND EXISTS(SELECT 1 FROM search_tri WHERE rowid=d.rowid) AND EXISTS(SELECT 1 FROM search_short WHERE rowid=d.rowid), modified_at IS ?5 FROM search_documents d WHERE entity_id=?1",params![id,kind,name,location,modified_at],|row|Ok((row.get::<_,bool>(0)?,row.get::<_,bool>(1)?))).optional()?;
    if let Some((true, same_time)) = unchanged {
        if !same_time {
            db.execute(
                "UPDATE search_documents SET modified_at=?1 WHERE entity_id=?2",
                params![modified_at, id],
            )?;
        }
        return Ok(());
    }
    db.execute("INSERT INTO search_documents(entity_id,kind,name,location,modified_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(entity_id) DO UPDATE SET kind=excluded.kind,name=excluded.name,location=excluded.location,modified_at=excluded.modified_at", params![id,kind,name,location,modified_at])?;
    let rowid: i64 = db.query_row(
        "SELECT rowid FROM search_documents WHERE entity_id=?",
        [id],
        |r| r.get(0),
    )?;
    db.execute("DELETE FROM search_tri WHERE rowid=?", [rowid])?;
    db.execute("DELETE FROM search_short WHERE rowid=?", [rowid])?;
    db.execute(
        "INSERT INTO search_tri(rowid,text) VALUES(?1,?2)",
        params![rowid, format!("{name}\n{location}")],
    )?;
    db.execute(
        "INSERT INTO search_short(rowid,tokens) VALUES(?1,?2)",
        params![rowid, short_tokens(&name, &location)],
    )?;
    Ok(())
}

pub fn remove(db: &Connection, id: &str) -> Result<()> {
    db.execute("DELETE FROM search_tri WHERE rowid IN (SELECT rowid FROM search_documents WHERE entity_id=?)", [id])?;
    db.execute("DELETE FROM search_short WHERE rowid IN (SELECT rowid FROM search_documents WHERE entity_id=?)", [id])?;
    db.execute("DELETE FROM search_documents WHERE entity_id=?", [id])?;
    Ok(())
}

pub fn rebuild(db: &Connection) -> Result<()> {
    db.execute_batch(
        "DELETE FROM search_tri; DELETE FROM search_short; DELETE FROM search_documents;",
    )?;
    let records: Vec<(String,String,String,String,Option<f64>)> = db.prepare("SELECT id,kind,name,location,modified_at FROM resources UNION ALL SELECT id,'workspace',name,'',NULL FROM workspaces")?.query_map([], |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)))?.collect::<rusqlite::Result<_>>()?;
    for (id, kind, name, location, modified_at) in records {
        project(db, &id, &kind, &name, &location, modified_at)?;
    }
    Ok(())
}

// UTF-8 二进制排序下的前缀上界；跳过非法代理区间，不使用 LIKE 通配符解释用户输入。
fn prefix_end(input: &str) -> Option<String> {
    let mut chars: Vec<_> = input.chars().collect();
    while let Some(last) = chars.pop() {
        let next = last as u32 + 1;
        if let Some(next) = char::from_u32(if next == 0xd800 { 0xe000 } else { next }) {
            chars.push(next);
            return Some(chars.into_iter().collect());
        }
    }
    None
}

pub fn select_ids(
    db: &Connection,
    query: &str,
    request: &crate::model::SearchRequest,
) -> Result<Vec<(String, String)>> {
    let global = select_ranked_ids(db, query, request)?;
    if request.kind != "all" || global.len() <= 30 {
        return Ok(global);
    }
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    // 混合搜索为每类保留至多十席；稀少类别的空位仍交给全局相关性结果。
    for kind in ["workspace", "file", "web"] {
        let available = global.iter().filter(|(_, k)| k == kind).count();
        let candidates = if available >= 10 {
            global
                .iter()
                .filter(|(_, k)| k == kind)
                .take(10)
                .cloned()
                .collect()
        } else {
            let mut typed = request.clone();
            typed.kind = kind.into();
            select_ranked_ids(db, query, &typed)?
                .into_iter()
                .take(10)
                .collect::<Vec<_>>()
        };
        for item in candidates {
            if seen.insert(item.0.clone()) {
                selected.push(item);
            }
        }
    }
    for item in &global {
        if selected.len() == 30 {
            break;
        }
        if seen.insert(item.0.clone()) {
            selected.push(item.clone());
        }
    }
    // 预留席位只影响入选集合，各组内部继续遵守精确/前缀/名称/路径的既有排序。
    let mut ranked = Vec::with_capacity(selected.len());
    for item in selected {
        let name: String = db.query_row(
            "SELECT name FROM search_documents WHERE entity_id=?",
            [&item.0],
            |row| row.get(0),
        )?;
        let rank = if name == query {
            0
        } else if name.starts_with(query) {
            1
        } else if name.contains(query) {
            2
        } else {
            3
        };
        ranked.push(((rank, name, item.0.clone()), item));
    }
    ranked.sort_by(|a, b| a.0.cmp(&b.0));
    let mut result: Vec<_> = ranked.into_iter().map(|(_, item)| item).collect();
    // 第三十一条只供 has_more 判定，不作为已预留集合的重复资料返回。
    if let Some(extra) = global.into_iter().find(|item| !seen.contains(&item.0)) {
        result.push(extra);
    }
    Ok(result)
}

fn candidate_count(db: &Connection, table: &str, term: &str) -> Result<u32> {
    Ok(db.query_row(
        &format!(
            "SELECT count(*) FROM (SELECT rowid FROM {table} WHERE {table} MATCH ?1 LIMIT 2049)"
        ),
        [term],
        |row| row.get(0),
    )?)
}

fn ordered_name_index(kind: &str) -> &'static str {
    if kind == "all" {
        "search_documents_name"
    } else {
        "search_documents_kind_name"
    }
}

fn rank_buckets(query: &str) -> (Option<String>, [String; 4]) {
    let upper = prefix_end(query);
    let prefix = if upper.is_some() {
        "d.name>=?4 AND d.name<?5"
    } else {
        "d.name>=?4"
    };
    (
        upper,
        [
            "d.name=?4".to_owned(),
            format!("({prefix}) AND d.name<>?4"),
            format!("NOT ({prefix}) AND instr(d.name,?4)>0"),
            "instr(d.name,?4)=0 AND instr(d.location,?4)>0".to_owned(),
        ],
    )
}

const RANK_SQL: &str = "CASE WHEN d.name=?4 THEN 0 WHEN substr(d.name,1,length(?4))=?4 THEN 1 WHEN instr(d.name,?4)>0 THEN 2 ELSE 3 END";

fn select_ranked_ids(
    db: &Connection,
    query: &str,
    request: &crate::model::SearchRequest,
) -> Result<Vec<(String, String)>> {
    let (table, term) = match_query(query);
    // 只估计前 2049 条候选。高命中词改用按名称有序的投影，避免排序/授权数十万条记录。
    let count = candidate_count(db, table, &term)?;
    let visible = crate::store::VISIBLE;
    let kind_filter = if request.kind == "all" {
        "?2='all'"
    } else {
        "d.kind=?2"
    };
    let scope = format!("({kind_filter}) AND (d.kind='workspace' OR {visible}) AND (?3 IS NULL OR (d.kind='workspace' AND d.entity_id=?3) OR EXISTS(SELECT 1 FROM workspace_resources wr WHERE wr.workspace_id=?3 AND wr.resource_id=d.entity_id))");
    if count <= 2048 {
        let sql = format!("SELECT d.entity_id,d.kind FROM search_documents d JOIN {table} f ON f.rowid=d.rowid LEFT JOIN resources r ON r.id=d.entity_id WHERE {table} MATCH ?1 AND {scope} AND (instr(d.name,?4)>0 OR instr(d.location,?4)>0) ORDER BY CASE WHEN d.name=?4 THEN 0 WHEN substr(d.name,1,length(?4))=?4 THEN 1 WHEN instr(d.name,?4)>0 THEN 2 ELSE 3 END,d.name,d.entity_id LIMIT 31");
        return Ok(db
            .prepare(&sql)?
            .query_map(
                params![term, request.kind, request.workspace_id, query],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?
            .collect::<rusqlite::Result<_>>()?);
    }
    let (upper, buckets) = rank_buckets(query);
    let mut result = Vec::new();
    let index = ordered_name_index(&request.kind);
    for bucket in buckets {
        // 每个相关性档次内依次流式取可见项；授权和工作区条件始终在 LIMIT 之前。
        let sql = format!("SELECT d.entity_id,d.kind FROM search_documents d INDEXED BY {index} LEFT JOIN resources r ON r.id=d.entity_id WHERE {scope} AND {bucket} ORDER BY d.name,d.entity_id LIMIT ?6");
        let mut stmt = db.prepare(&sql)?;
        let ids = stmt
            .query_map(
                params![
                    rusqlite::types::Null,
                    request.kind,
                    request.workspace_id,
                    query,
                    upper,
                    31 - result.len()
                ],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        result.extend(ids);
        if result.len() == 31 {
            break;
        }
    }
    Ok(result)
}

pub(crate) fn validate_query_text(query: &str) -> Result<()> {
    // SQLite TEXT的length/substr遇NUL截断，不能让分桶前缀与签发rank解释成两个查询。
    if query.contains('\0') {
        return Err(AppError::new(
            "invalid_input",
            "搜索内容不能包含空字符",
            "Search text cannot contain null characters.",
        ));
    }
    Ok(())
}

pub fn page_criteria(request: &SearchPageRequest) -> Result<SearchCriteria> {
    let query = normalize(request.request.query.trim());
    validate_query_text(&query)?;
    if query.is_empty() || query.chars().count() > 256 {
        return Err(AppError::new(
            "invalid_input",
            "请输入 1–256 字搜索内容",
            "Enter between 1 and 256 characters to search.",
        ));
    }
    if !["all", "file", "web", "workspace"].contains(&request.request.kind.as_str()) {
        return Err(AppError::new(
            "invalid_input",
            "资料类型无效",
            "The resource type is invalid.",
        ));
    }
    if !["relevance", "name", "modified"].contains(&request.sort.as_str()) {
        return Err(AppError::new(
            "invalid_input",
            "排序条件无效",
            "The sort order is invalid.",
        ));
    }
    let valid_time = |time: f64| time.is_finite() && time.abs() <= 8_640_000_000_000_000.;
    if request.modified_from.is_some_and(|time| !valid_time(time))
        || request.modified_to.is_some_and(|time| !valid_time(time))
        || matches!((request.modified_from, request.modified_to), (Some(from), Some(to)) if from >= to)
    {
        return Err(AppError::new(
            "invalid_input",
            "请选择有效的修改时间范围",
            "Select a valid modification date range.",
        ));
    }
    let prefix = request.path_prefix.as_deref().map(normalize);
    if prefix
        .as_ref()
        .is_some_and(|value| value.len() > 8192 || value.contains('\0'))
    {
        return Err(AppError::new(
            "invalid_input",
            "路径筛选内容无效",
            "The path filter is invalid.",
        ));
    }
    let prefix = prefix.filter(|value| !value.is_empty());
    Ok(SearchCriteria {
        query,
        workspace_id: request.request.workspace_id.clone(),
        kind: request.request.kind.clone(),
        sort: request.sort.clone(),
        modified_from: request.modified_from,
        modified_to: request.modified_to,
        path_prefix: prefix,
    })
}

fn page_sql(
    db: &Connection,
    criteria: &SearchCriteria,
    seek: bool,
    modified: Option<&SearchModifiedPosition>,
) -> Result<(String, String)> {
    let (table, term) = match_query(&criteria.query);
    let visible = crate::store::VISIBLE;
    let order = match criteria.sort.as_str() {
        "name" => "d.name,d.entity_id",
        "modified" => "d.modified_at DESC,d.name,d.entity_id",
        "relevance" => "CASE WHEN d.name=?4 THEN 0 WHEN substr(d.name,1,length(?4))=?4 THEN 1 WHEN instr(d.name,?4)>0 THEN 2 ELSE 3 END,d.name,d.entity_id",
        _ => return Err(AppError::new("invalid_input", "排序条件无效", "The sort order is invalid.")),
    };
    let kind = if criteria.kind == "all" {
        "?2='all'"
    } else {
        "d.kind=?2"
    };
    // 高命中名称排序复用现有有序投影，在权限/范围筛选后提前取满一页；低命中仍走FTS。
    let ordered = criteria.sort == "name" && candidate_count(db, table, &term)? > 2048;
    let source = if ordered {
        let index = if criteria.kind == "all" {
            "search_documents_name"
        } else {
            "search_documents_kind_name"
        };
        format!("search_documents d INDEXED BY {index}")
    } else {
        format!("search_documents d JOIN {table} f ON f.rowid=d.rowid")
    };
    let candidate = if ordered {
        "1".to_owned()
    } else {
        format!("{table} MATCH ?1")
    };
    let position_filter = if let Some(modified) = modified {
        if modified.modified_at.is_some() {
            "(d.modified_at<?13 OR (d.modified_at=?13 AND (d.name,d.entity_id)>(?10,?11)) OR d.modified_at IS NULL) AND ?12 IS NULL".into()
        } else {
            "d.modified_at IS NULL AND (d.name,d.entity_id)>(?10,?11) AND ?12 IS NULL AND ?13 IS NULL".into()
        }
    } else if seek && criteria.sort == "relevance" {
        format!("({RANK_SQL}>?12 OR ({RANK_SQL}=?12 AND (d.name,d.entity_id)>(?10,?11))) AND ?13 IS NULL")
    } else if seek {
        "(d.name,d.entity_id)>(?10,?11) AND ?12 IS NULL AND ?13 IS NULL".into()
    } else {
        "?10 IS NULL AND ?11 IS NULL AND ?12 IS NULL AND ?13 IS NULL".into()
    };
    // 日期与字面前缀、授权和范围都在LIMIT/OFFSET前，不先取一页再在内存丢弃。
    let sql = format!("SELECT d.entity_id,d.kind FROM {source} LEFT JOIN resources r ON r.id=d.entity_id WHERE {candidate} AND ({kind}) AND (d.kind='workspace' OR {visible}) AND (?3 IS NULL OR (d.kind='workspace' AND d.entity_id=?3) OR EXISTS(SELECT 1 FROM workspace_resources wr WHERE wr.workspace_id=?3 AND wr.resource_id=d.entity_id)) AND (instr(d.name,?4)>0 OR instr(d.location,?4)>0) AND (?7 IS NULL OR r.modified_at>=?7) AND (?8 IS NULL OR r.modified_at<?8) AND (?9 IS NULL OR (d.kind<>'workspace' AND substr(d.location,1,length(?9))=?9)) AND ({position_filter}) ORDER BY {order} LIMIT 101 OFFSET ?6");
    Ok((sql, term))
}

fn uses_ranked_page(db: &Connection, criteria: &SearchCriteria) -> Result<bool> {
    let (table, term) = match_query(&criteria.query);
    Ok(criteria.sort == "relevance" && candidate_count(db, table, &term)? > 2048)
}

fn ranked_page_sql(criteria: &SearchCriteria, bucket: &str, seek: bool) -> String {
    let index = ordered_name_index(&criteria.kind);
    let visible = crate::store::VISIBLE;
    let kind = if criteria.kind == "all" {
        "?2='all'"
    } else {
        "d.kind=?2"
    };
    let position = if seek {
        "(d.name,d.entity_id)>(?10,?11)"
    } else {
        "?10 IS NULL AND ?11 IS NULL"
    };
    format!("SELECT d.entity_id,d.kind FROM search_documents d INDEXED BY {index} LEFT JOIN resources r ON r.id=d.entity_id WHERE ({kind}) AND (d.kind='workspace' OR {visible}) AND (?3 IS NULL OR (d.kind='workspace' AND d.entity_id=?3) OR EXISTS(SELECT 1 FROM workspace_resources wr WHERE wr.workspace_id=?3 AND wr.resource_id=d.entity_id)) AND ({bucket}) AND (?7 IS NULL OR r.modified_at>=?7) AND (?8 IS NULL OR r.modified_at<?8) AND (?9 IS NULL OR (d.kind<>'workspace' AND substr(d.location,1,length(?9))=?9)) AND ({position}) ORDER BY d.name,d.entity_id LIMIT ?6")
}

fn select_ranked_page(
    db: &Connection,
    criteria: &SearchCriteria,
    position: Option<&SearchRankPosition>,
    mut plans: Option<&mut Vec<String>>,
) -> Result<Vec<(String, String)>> {
    let (upper, buckets) = rank_buckets(&criteria.query);
    let mut result = Vec::new();
    for (rank, bucket) in buckets.iter().enumerate() {
        if position.is_some_and(|position| rank < usize::from(position.rank)) {
            continue;
        }
        let anchor = position.filter(|position| rank == usize::from(position.rank));
        let sql = ranked_page_sql(criteria, bucket, anchor.is_some());
        let values = params![
            rusqlite::types::Null,
            criteria.kind,
            criteria.workspace_id,
            criteria.query,
            upper,
            101 - result.len(),
            criteria.modified_from,
            criteria.modified_to,
            criteria.path_prefix,
            anchor.map(|position| &position.name),
            anchor.map(|position| &position.id)
        ];
        if let Some(plans) = plans.as_deref_mut() {
            let details = db
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?
                .query_map(values, |row| row.get::<_, String>(3))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            plans.extend(
                details
                    .into_iter()
                    .map(|line| format!("rank={rank}: {line}")),
            );
        }
        // 每档完整过滤后取尚缺的101项，游标所在档按名称/ID继续；后档从头读。
        let ids = db
            .prepare(&sql)?
            .query_map(values, |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        result.extend(ids);
        if result.len() == 101 {
            break;
        }
    }
    Ok(result)
}

fn uses_modified_page(db: &Connection, criteria: &SearchCriteria) -> Result<bool> {
    let (table, term) = match_query(&criteria.query);
    Ok(criteria.sort == "modified" && candidate_count(db, table, &term)? > 2048)
}

#[derive(Clone, Copy, Debug)]
enum ModifiedRange {
    All,
    Equal,
    Earlier,
    Null,
}

fn select_modified_page(
    db: &Connection,
    criteria: &SearchCriteria,
    position: Option<&SearchModifiedPosition>,
    mut plans: Option<&mut Vec<String>>,
) -> Result<Vec<(String, String)>> {
    let stages: &[ModifiedRange] = match position {
        None => &[ModifiedRange::All, ModifiedRange::Null],
        Some(position) if position.modified_at.is_some() => &[
            ModifiedRange::Equal,
            ModifiedRange::Earlier,
            ModifiedRange::Null,
        ],
        Some(_) => &[ModifiedRange::Null],
    };
    let index = if criteria.kind == "all" {
        "search_documents_modified"
    } else {
        "search_documents_kind_modified"
    };
    let kind = if criteria.kind == "all" {
        "?2='all'"
    } else {
        "d.kind=?2"
    };
    let mut result = Vec::new();
    for stage in stages {
        let null = matches!(stage, ModifiedRange::Null);
        if null && (criteria.modified_from.is_some() || criteria.modified_to.is_some()) {
            continue;
        }
        let anchor = position.filter(|position| {
            matches!(stage, ModifiedRange::Equal) || (null && position.modified_at.is_none())
        });
        let time = position
            .filter(|_| !null)
            .and_then(|position| position.modified_at)
            .map(|time| {
                if matches!(stage, ModifiedRange::Earlier) {
                    criteria.modified_to.map_or(time, |to| time.min(to))
                } else {
                    time
                }
            });
        let range = match stage {
            ModifiedRange::All => "d.modified_at IS NOT NULL AND ?13 IS NULL",
            ModifiedRange::Equal => "d.modified_at=?13",
            ModifiedRange::Earlier => "d.modified_at<?13",
            ModifiedRange::Null => "d.modified_at IS NULL AND ?13 IS NULL",
        };
        let after = if anchor.is_some() {
            "(d.name,d.entity_id)>(?10,?11)"
        } else {
            "?10 IS NULL AND ?11 IS NULL"
        };
        let mut filters = vec![range, after];
        if !null {
            if criteria.modified_from.is_some() {
                filters.push("d.modified_at>=?7");
            }
            // Earlier只保留更紧的一个上界，避免SQLite从较宽日期边界重扫。
            if criteria.modified_to.is_some() && !matches!(stage, ModifiedRange::Earlier) {
                filters.push("d.modified_at<?8");
            }
        }
        let filters = filters.join(" AND ");
        let visible = crate::store::VISIBLE;
        let sql=format!("SELECT d.entity_id,d.kind FROM search_documents d INDEXED BY {index} LEFT JOIN resources r ON r.id=d.entity_id WHERE ({kind}) AND (d.kind='workspace' OR {visible}) AND (?3 IS NULL OR (d.kind='workspace' AND d.entity_id=?3) OR EXISTS(SELECT 1 FROM workspace_resources wr WHERE wr.workspace_id=?3 AND wr.resource_id=d.entity_id)) AND (instr(d.name,?4)>0 OR instr(d.location,?4)>0) AND (?9 IS NULL OR (d.kind<>'workspace' AND substr(d.location,1,length(?9))=?9)) AND ?12 IS NULL AND ({filters}) ORDER BY d.modified_at DESC,d.name,d.entity_id LIMIT ?6");
        let values = params![
            rusqlite::types::Null,
            criteria.kind,
            criteria.workspace_id,
            criteria.query,
            rusqlite::types::Null,
            101 - result.len(),
            criteria.modified_from,
            criteria.modified_to,
            criteria.path_prefix,
            anchor.map(|p| &p.name),
            anchor.map(|p| &p.id),
            rusqlite::types::Null,
            time
        ];
        if let Some(plans) = plans.as_deref_mut() {
            let details = db
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?
                .query_map(values, |row| row.get::<_, String>(3))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            plans.extend(
                details
                    .into_iter()
                    .map(|line| format!("modified={stage:?}: {line}")),
            );
        }
        // 相等时间、较早时间、NULL尾段分别定位，避免OR把深页退化为索引全扫。
        let ids = db
            .prepare(&sql)?
            .query_map(values, |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        result.extend(ids);
        if result.len() == 101 {
            break;
        }
    }
    Ok(result)
}

pub fn select_page_ids(
    db: &Connection,
    criteria: &SearchCriteria,
    _offset: u32,
    position: Option<&SearchNamePosition>,
    rank_position: Option<&SearchRankPosition>,
    modified_position: Option<&SearchModifiedPosition>,
) -> Result<Vec<(String, String)>> {
    if uses_ranked_page(db, criteria)? {
        return select_ranked_page(db, criteria, rank_position, None);
    }
    if uses_modified_page(db, criteria)? {
        return select_modified_page(db, criteria, modified_position, None);
    }
    let (sql, term) = page_sql(
        db,
        criteria,
        position.is_some() || rank_position.is_some(),
        modified_position,
    )?;
    Ok(db
        .prepare(&sql)?
        .query_map(
            params![
                term,
                criteria.kind,
                criteria.workspace_id,
                criteria.query,
                rusqlite::types::Null,
                0,
                criteria.modified_from,
                criteria.modified_to,
                criteria.path_prefix,
                position
                    .map(|position| &position.name)
                    .or_else(|| rank_position.map(|position| &position.name))
                    .or_else(|| modified_position.map(|position| &position.name)),
                position
                    .map(|position| &position.id)
                    .or_else(|| rank_position.map(|position| &position.id))
                    .or_else(|| modified_position.map(|position| &position.id)),
                rank_position.map(|position| position.rank),
                modified_position.and_then(|position| position.modified_at)
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?
        .collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
pub(crate) fn explain_page(
    db: &Connection,
    criteria: &SearchCriteria,
    offset: u32,
) -> Result<Vec<String>> {
    explain_page_after(db, criteria, offset, None)
}

#[cfg(test)]
pub(crate) fn explain_page_after(
    db: &Connection,
    criteria: &SearchCriteria,
    offset: u32,
    position: Option<&SearchNamePosition>,
) -> Result<Vec<String>> {
    explain_page_positions(db, criteria, offset, position, None, None)
}

#[cfg(test)]
pub(crate) fn explain_page_positions(
    db: &Connection,
    criteria: &SearchCriteria,
    _offset: u32,
    position: Option<&SearchNamePosition>,
    rank_position: Option<&SearchRankPosition>,
    modified_position: Option<&SearchModifiedPosition>,
) -> Result<Vec<String>> {
    if uses_ranked_page(db, criteria)? {
        let mut plans = Vec::new();
        select_ranked_page(db, criteria, rank_position, Some(&mut plans))?;
        return Ok(plans);
    }
    if uses_modified_page(db, criteria)? {
        let mut plans = Vec::new();
        select_modified_page(db, criteria, modified_position, Some(&mut plans))?;
        return Ok(plans);
    }
    let (sql, term) = page_sql(
        db,
        criteria,
        position.is_some() || rank_position.is_some(),
        modified_position,
    )?;
    Ok(db
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?
        .query_map(
            params![
                term,
                criteria.kind,
                criteria.workspace_id,
                criteria.query,
                rusqlite::types::Null,
                0,
                criteria.modified_from,
                criteria.modified_to,
                criteria.path_prefix,
                position
                    .map(|position| &position.name)
                    .or_else(|| rank_position.map(|position| &position.name))
                    .or_else(|| modified_position.map(|position| &position.name)),
                position
                    .map(|position| &position.id)
                    .or_else(|| rank_position.map(|position| &position.id))
                    .or_else(|| modified_position.map(|position| &position.id)),
                rank_position.map(|position| position.rank),
                modified_position.and_then(|position| position.modified_at)
            ],
            |row| row.get(3),
        )?
        .collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod projection_tests {
    use super::*;

    fn matches(db: &Connection, query: &str) -> u32 {
        let (table, term) = match_query(&normalize(query));
        db.query_row(
            &format!("SELECT count(*) FROM {table} WHERE {table} MATCH ?1"),
            [term],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn unchanged_normalized_projection_does_not_write_and_missing_fts_rows_are_repaired() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::migrations::migrate(&mut db).unwrap();
        project(
            &db,
            "item",
            "file",
            "合同 Café_2026.txt",
            "/synthetic/Café/合同.txt",
            Some(1.),
        )
        .unwrap();
        let writes = db.total_changes();
        project(
            &db,
            "item",
            "file",
            "合同 CAFÉ_2026.TXT",
            "/SYNTHETIC/CAFE\u{301}/合同.txt",
            Some(1.),
        )
        .unwrap();
        assert_eq!(db.total_changes(), writes);
        assert_eq!(matches(&db, "合同"), 1);
        assert_eq!(matches(&db, "café_2026"), 1);
        for table in ["search_tri", "search_short"] {
            db.execute(&format!("DELETE FROM {table}"), []).unwrap();
            project(
                &db,
                "item",
                "file",
                "合同 Café_2026.txt",
                "/synthetic/Café/合同.txt",
                Some(1.),
            )
            .unwrap();
            assert_eq!(matches(&db, "合同"), 1);
            assert_eq!(matches(&db, "café_2026"), 1);
        }
    }

    #[test]
    fn renamed_projection_drops_old_terms_and_explicit_rebuild_repairs_existing_bad_content() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::migrations::migrate(&mut db).unwrap();
        db.execute(
            "INSERT INTO workspaces(id,name,created_at) VALUES('item','预算归档',0)",
            [],
        )
        .unwrap();
        project(&db, "item", "workspace", "历史合同", "", None).unwrap();
        assert_eq!(matches(&db, "历史合同"), 1);
        project(&db, "item", "workspace", "预算归档", "", None).unwrap();
        assert_eq!(matches(&db, "历史合同"), 0);
        assert_eq!(matches(&db, "预算"), 1);
        db.execute("UPDATE search_tri SET text='incorrect'", [])
            .unwrap();
        db.execute("UPDATE search_short SET tokens='incorrect'", [])
            .unwrap();
        rebuild(&db).unwrap();
        assert_eq!(matches(&db, "预算"), 1);
        assert_eq!(matches(&db, "预算归档"), 1);
        assert_eq!(matches(&db, "incorrect"), 0);
    }

    #[test]
    fn mtime_only_updates_one_projection_row_without_rewriting_fts_and_keeps_null_distinct() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::migrations::migrate(&mut db).unwrap();
        project(&db, "item", "file", "合同", "/test/合同", Some(0.)).unwrap();
        for time in [Some(-10.), Some(0.), None, Some(0.)] {
            let writes = db.total_changes();
            project(&db, "item", "file", "合同", "/test/合同", time).unwrap();
            assert_eq!(
                db.total_changes() - writes,
                1,
                "only the time projection should be written"
            );
            assert_eq!(
                db.query_row(
                    "SELECT modified_at FROM search_documents WHERE entity_id='item'",
                    [],
                    |r| r.get::<_, Option<f64>>(0)
                )
                .unwrap(),
                time
            );
            assert_eq!(matches(&db, "合同"), 1);
            let writes = db.total_changes();
            project(&db, "item", "file", "合同", "/test/合同", time).unwrap();
            assert_eq!(
                db.total_changes(),
                writes,
                "unchanged projections must remain write free"
            );
        }
        db.execute("DELETE FROM search_short", []).unwrap();
        project(&db, "item", "file", "合同", "/test/合同", Some(2.)).unwrap();
        assert_eq!(matches(&db, "合同"), 1);
        assert_eq!(
            db.query_row("SELECT modified_at FROM search_documents", [], |r| r
                .get::<_, f64>(0))
                .unwrap(),
            2.
        );
    }
}
