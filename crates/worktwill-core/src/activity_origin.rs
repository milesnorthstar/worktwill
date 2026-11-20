//! 历史名称按观察对象的授权读取，不能借重新指向后的新对象权限复活旧元数据。
use crate::{model::*, store::VISIBLE, Store};
use rusqlite::{params, Connection};
use std::path::Path;

pub(crate) fn activity_visible() -> String {
    let separator = std::path::MAIN_SEPARATOR;
    let historical_scope = format!("EXISTS(SELECT 1 FROM activity_root_scopes hs JOIN scan_roots sr ON sr.id=hs.root_id LEFT JOIN root_locators rl ON rl.root_id=sr.id WHERE hs.activity_id=a.id AND hs.retired_location_valid=1 AND sr.active=1 AND (rl.root_id IS NULL OR rl.valid=1) AND ((hs.root_identity IS NULL AND rl.root_id IS NULL) OR hs.root_identity=rl.identity) AND NOT EXISTS(SELECT 1 FROM scan_exclusions ex WHERE ex.root_id=sr.id AND (hs.relative_path=ex.relative_path OR instr(hs.relative_path,ex.relative_path||'{separator}')=1)))");
    // 同identity就是当前对象，只验一次权限；异identity子查询中的r仅代表原观察对象。
    // COALESCE只在原对象记录不存在时回落，0表示已撤权，不能借旧scope复活。
    format!("CASE WHEN a.resource_id IS NULL THEN 1 WHEN r.kind='web' THEN {VISIBLE} WHEN r.kind='file' THEN EXISTS(SELECT 1 FROM activity_origins o WHERE o.activity_id=a.id AND CASE WHEN o.identity IS NULL THEN 0 WHEN o.identity=r.identity THEN {VISIBLE} ELSE COALESCE((SELECT {VISIBLE} FROM resources r WHERE r.kind='file' AND r.identity=o.identity),{historical_scope}) AND (a.name<>'' OR {VISIBLE}) END) ELSE 0 END")
}

pub(crate) fn capture(db: &Connection, aid: &str, rid: &str) -> Result<()> {
    let (kind, identity, location): (String, Option<String>, String) = db.query_row(
        "SELECT kind,identity,location FROM resources WHERE id=?",
        [rid],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    if kind != "file" {
        return Ok(());
    }
    db.execute(
        "INSERT INTO activity_origins VALUES(?1,?2,?3)",
        params![aid, rid, identity],
    )?;
    for root in Store::roots_db(db)? {
        if !Store::allowed_by_root(db, &root, Path::new(&location))? {
            continue;
        }
        let relative = Path::new(&location).strip_prefix(&root.path).map_err(|_| {
            AppError::new(
                "internal_error",
                "历史位置范围无效",
                "The activity location scope is invalid.",
            )
        })?;
        let root_identity: Option<String> = db.query_row(
            "SELECT (SELECT identity FROM root_locators WHERE root_id=?)",
            [&root.id],
            |r| r.get(0),
        )?;
        db.execute(
            "INSERT INTO activity_root_scopes(activity_id,root_id,root_identity,relative_path) VALUES(?1,?2,?3,?4)",
            params![aid, root.id, root_identity, relative.to_string_lossy()],
        )?;
    }
    Ok(())
}

// 只在v6迁移及明确的旧格式测试夹具中使用；NULL observed UUID承认原始来源不可重建。
pub(crate) fn bind_legacy(db: &Connection) -> Result<()> {
    db.execute("INSERT INTO activity_origins SELECT a.id,NULL,r.identity FROM activities a JOIN resources r ON r.id=a.resource_id WHERE r.kind='file' AND NOT EXISTS(SELECT 1 FROM activity_origins o WHERE o.activity_id=a.id)",[])?;
    let separator = std::path::MAIN_SEPARATOR;
    db.execute_batch(&format!("INSERT INTO activity_root_scopes(activity_id,root_id,root_identity,relative_path) SELECT a.activity_id,s.id,l.identity,substr(r.location,length(rtrim(s.path,'{separator}'))+2) FROM activity_origins a JOIN activities e ON e.id=a.activity_id JOIN resources r ON r.id=e.resource_id JOIN file_roots f ON f.resource_id=r.id JOIN scan_roots s ON s.id=f.root_id LEFT JOIN root_locators l ON l.root_id=s.id WHERE a.observed_resource_id IS NULL AND s.active=1 AND (l.root_id IS NULL OR l.valid=1) AND instr(r.location,rtrim(s.path,'{separator}')||'{separator}')=1 AND NOT EXISTS(SELECT 1 FROM activity_root_scopes present WHERE present.activity_id=a.activity_id AND present.root_id=s.id) AND NOT EXISTS(SELECT 1 FROM scan_exclusions ex WHERE ex.root_id=s.id AND (r.location=rtrim(s.path,'{separator}')||'{separator}'||ex.relative_path OR instr(r.location,rtrim(s.path,'{separator}')||'{separator}'||ex.relative_path||'{separator}')=1));"))?;
    Ok(())
}

// 身份退休不应抹掉已确认移出旧目录的事实；观察字段保持原值，仅保存范围资格。
pub(crate) fn retire(db: &Connection, rid: &str) -> Result<()> {
    let separator = std::path::MAIN_SEPARATOR;
    db.execute(&format!("UPDATE activity_root_scopes SET retired_location_valid=CASE WHEN EXISTS(SELECT 1 FROM scan_roots s JOIN resources current ON current.id=?1 WHERE s.id=activity_root_scopes.root_id AND instr(current.location,rtrim(s.path,'{separator}')||'{separator}')=1 AND NOT EXISTS(SELECT 1 FROM scan_exclusions ex WHERE ex.root_id=s.id AND (current.location=rtrim(s.path,'{separator}')||'{separator}'||ex.relative_path OR instr(current.location,rtrim(s.path,'{separator}')||'{separator}'||ex.relative_path||'{separator}')=1))) THEN 1 ELSE 0 END WHERE activity_id IN(SELECT o.activity_id FROM activity_origins o JOIN resources current ON current.id=?1 WHERE o.identity=current.identity)"), [rid])?;
    Ok(())
}
