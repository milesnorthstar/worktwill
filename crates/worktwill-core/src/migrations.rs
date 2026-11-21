use crate::model::{AppError, Result};
use rusqlite::{Connection, TransactionBehavior};

pub(crate) const VERSION: u32 = 9;
pub(crate) fn migrate(db: &mut Connection) -> Result<()> {
    let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version > VERSION {
        return Err(AppError::new(
            "unsupported_data_version",
            "数据由较新版本创建，请使用匹配的程序",
            "This data was created by a newer version. Use a matching app version.",
        ));
    }
    if version == 0 {
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(include_str!("schema.sql"))?;
        tx.commit()?;
    }
    let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 1 {
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch("ALTER TABLE activities ADD COLUMN source TEXT NOT NULL DEFAULT 'app';
            ALTER TABLE activities ADD COLUMN file_modified_at REAL;
            CREATE INDEX activities_time ON activities(at DESC,id);
            CREATE TABLE file_grants(resource_id TEXT PRIMARY KEY REFERENCES resources(id),active INTEGER NOT NULL DEFAULT 1);
            PRAGMA user_version=2;")?;
        tx.commit()?;
    }
    let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 2 {
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch("ALTER TABLE workspaces ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0; PRAGMA user_version=3;")?;
        tx.commit()?;
    }
    let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 3 {
        // 时间是资源事实的派生值；旧库只回填一次，ALTER与版本推进必须一起回滚。
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch("ALTER TABLE search_documents ADD COLUMN modified_at REAL;
            UPDATE search_documents SET modified_at=(SELECT r.modified_at FROM resources r WHERE r.id=search_documents.entity_id) WHERE kind<>'workspace';
            CREATE INDEX search_documents_modified ON search_documents(modified_at DESC,name,entity_id);
            CREATE INDEX search_documents_kind_modified ON search_documents(kind,modified_at DESC,name,entity_id);
            PRAGMA user_version=4;")?;
        tx.commit()?;
    }
    let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 4 {
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch("CREATE TABLE file_locators(resource_id TEXT PRIMARY KEY REFERENCES resources(id) ON DELETE CASCADE, identity TEXT NOT NULL, bookmark BLOB NOT NULL); PRAGMA user_version=5;")?;
        tx.commit()?;
    }
    let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 5 {
        // 旧位置没有历史物理身份；只建空表，不将当前路径追认为原授权目录。
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch("CREATE TABLE root_locators(root_id TEXT PRIMARY KEY REFERENCES scan_roots(id) ON DELETE CASCADE,identity TEXT NOT NULL,bookmark BLOB,valid INTEGER NOT NULL CHECK(valid IN(0,1)));
            CREATE INDEX root_locators_identity ON root_locators(identity);
            CREATE TABLE root_aliases(root_id TEXT NOT NULL REFERENCES scan_roots(id) ON DELETE CASCADE,path TEXT NOT NULL,PRIMARY KEY(root_id,path));
            PRAGMA user_version=6;")?;
        tx.commit()?;
    }
    let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 6 {
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch("CREATE TABLE activity_origins(activity_id TEXT PRIMARY KEY REFERENCES activities(id) ON DELETE CASCADE, observed_resource_id TEXT, identity TEXT);
            CREATE TABLE activity_root_scopes(activity_id TEXT NOT NULL REFERENCES activity_origins(activity_id) ON DELETE CASCADE,root_id TEXT NOT NULL REFERENCES scan_roots(id) ON DELETE CASCADE,root_identity TEXT,relative_path TEXT NOT NULL,retired_location_valid INTEGER NOT NULL DEFAULT 1 CHECK(retired_location_valid IN(0,1)),PRIMARY KEY(activity_id,root_id));
            CREATE TABLE resource_relinks(event_id TEXT PRIMARY KEY REFERENCES activities(id) ON DELETE CASCADE,source_resource_id TEXT NOT NULL,merged_resource_id TEXT,previous_identity TEXT,selected_identity TEXT NOT NULL);")?;
        // 旧事件没有原始UUID证据；只绑定迁移时已有的授权事实，不冒充重建历史来源。
        crate::activity_origin::bind_legacy(&tx)?;
        tx.execute_batch("PRAGMA user_version=7;")?;
        tx.commit()?;
    }
    let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 7 {
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // 已发布v7有四列与五列两种范围表；只迁移这项已知演进，不补造其他缺失结构。
        tx.prepare("SELECT activity_id,root_id,root_identity,relative_path FROM activity_root_scopes LIMIT 0")?;
        let has_retired_flag: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('activity_root_scopes') WHERE name='retired_location_valid')",
            [],
            |row| row.get(0),
        )?;
        if !has_retired_flag {
            // 旧行缺少退休时的位置证据，不能追认为有效；新观察仍使用默认1，由retire冻结实际资格。
            tx.execute_batch("ALTER TABLE activity_root_scopes ADD COLUMN retired_location_valid INTEGER NOT NULL DEFAULT 1 CHECK(retired_location_valid IN(0,1));
                UPDATE activity_root_scopes SET retired_location_valid=0;")?;
        }
        tx.execute_batch("PRAGMA user_version=8;")?;
        tx.commit()?;
    }
    let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 8 {
        // 正式库此前没有便签；保留墓碑阻止删除后的迟到保存重新创建。
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch("CREATE TABLE scratch_notes(id TEXT PRIMARY KEY,text TEXT NOT NULL,pinned INTEGER NOT NULL CHECK(pinned IN(0,1)),version INTEGER NOT NULL,updated_at REAL NOT NULL,deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN(0,1))); PRAGMA user_version=9;")?;
        tx.commit()?;
    }
    db.execute("CREATE INDEX IF NOT EXISTS activities_resource_kind_time ON activities(resource_id,kind,at DESC,id DESC)",[])?;
    db.execute(
        "CREATE INDEX IF NOT EXISTS search_documents_name ON search_documents(name,entity_id)",
        [],
    )?;
    db.execute(
        "CREATE INDEX IF NOT EXISTS search_documents_kind_name ON search_documents(kind,name,entity_id)",
        [],
    )?;
    db.execute(
        "CREATE INDEX IF NOT EXISTS activities_file_time ON activities(CASE WHEN kind='file_modified' AND file_modified_at>0 THEN file_modified_at ELSE at END DESC,id DESC)",
        [],
    )?;
    db.execute(
        "CREATE INDEX IF NOT EXISTS resources_location ON resources(location)",
        [],
    )?;
    db.execute(
        "CREATE INDEX IF NOT EXISTS workspace_resource_memberships ON workspace_resources(resource_id,workspace_id)",
        [],
    )?;
    db.execute(
        "CREATE INDEX IF NOT EXISTS activities_observed_time ON activities(at DESC,id DESC)",
        [],
    )?;
    Ok(())
}
