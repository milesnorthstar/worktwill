// 打开记录只属于当时的物理对象；质量豁免不能继承重新指向前的历史。

fn artifact(name: &str) -> String {
    // 只识别已确认的系统元数据与 Office 锁文件，不概括隐藏文件或媒体。
    format!("(lower({name}) IN ('.ds_store','thumbs.db','desktop.ini') OR (substr({name},1,2)='~$' AND (lower({name}) LIKE '%.docx' OR lower({name}) LIKE '%.xlsx' OR lower({name}) LIKE '%.pptx' OR lower({name}) LIKE '%.doc' OR lower({name}) LIKE '%.xls' OR lower({name}) LIKE '%.ppt')))" )
}
fn artifact_path(path: &str) -> String {
    // 只识别明确的工具链产物段，不泛化为隐藏目录、target或媒体文件过滤。
    let normalized = format!("replace({path},char(92),'/')");
    let segments = [
        "/.git/",
        "/node_modules/",
        "/.cache/",
        "/.next/",
        "/.next-core-local/",
        "/target/debug/",
        "/target/release/",
    ];
    format!(
        "({})",
        segments
            .iter()
            .map(|segment| format!("instr('/'||{normalized},'{segment}')>0"))
            .collect::<Vec<_>>()
            .join(" OR ")
    )
}
pub(crate) fn explicit() -> String {
    let recent_opened = crate::activity_origin::recent_opened();
    format!("(r.favorite=1 OR EXISTS(SELECT 1 FROM workspace_resources wr WHERE wr.resource_id=r.id) OR EXISTS(SELECT 1 FROM activities a WHERE {recent_opened}))")
}
pub(crate) fn recommended() -> String {
    format!(
        "(r.kind<>'file' OR NOT ({} OR {}) OR {})",
        artifact("r.name"),
        artifact_path("r.location"),
        explicit()
    )
}
pub(crate) fn activity_quality() -> String {
    // 历史路径只取当时scope；当前路径仅能用于同一观察身份，不能按重指向后的UUID猜历史。
    let observed_path=format!("(EXISTS(SELECT 1 FROM activity_root_scopes qs WHERE qs.activity_id=a.id AND {}) OR (EXISTS(SELECT 1 FROM activity_origins qo WHERE qo.activity_id=a.id AND qo.identity=r.identity) AND {}))",artifact_path("qs.relative_path"),artifact_path("r.location"));
    format!("(a.kind NOT IN ('file_modified','file_discovered') OR NOT ({} OR {}) OR ({} AND EXISTS(SELECT 1 FROM activity_origins qo WHERE qo.activity_id=a.id AND qo.identity=r.identity)))",artifact("CASE WHEN a.name<>'' THEN a.name ELSE r.name END"),observed_path,explicit())
}
