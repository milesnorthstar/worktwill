use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum UiLanguage {
    Zh,
    En,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum SuggestedLocation {
    Desktop,
    Downloads,
    Documents,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    Unconfigured,
    Idle,
    UpToDate,
    Checking,
    Available,
    Downloading,
    Ready,
    Installing,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateStatus {
    pub revision: u32,
    pub phase: UpdatePhase,
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub downloaded: f64,
    pub total: Option<f64>,
    pub automatic_download: bool,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateResume {
    pub view: String,
    pub workspace_id: Option<String>,
    pub language: UiLanguage,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub message_en: String,
}
pub type Result<T> = std::result::Result<T, AppError>;
impl AppError {
    pub fn new(code: &str, message: &str, message_en: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            message_en: message_en.into(),
        }
    }
}
impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        use rusqlite::ErrorCode;
        let code = match error.sqlite_error_code() {
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => "database_busy",
            Some(ErrorCode::DiskFull) => "storage_full",
            _ => "database_error",
        };
        Self::new(
            code,
            "本地数据操作失败，请重试或检查备份",
            "The local data operation failed. Try again or check your backup.",
        )
    }
}
impl From<std::io::Error> for AppError {
    fn from(_: std::io::Error) -> Self {
        Self::new(
            "resource_unavailable",
            "位置不可访问，请检查权限或磁盘连接",
            "This location is unavailable. Check access permissions or the disk connection.",
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct WebMetadata {
    pub title: Option<String>,
    pub final_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Root {
    pub location_status: String,
    pub id: String,
    pub path: String,
    pub active: bool,
    pub paused: bool,
    pub scan_state: String,
    pub scanned: u32,
    pub failures: u32,
    pub exclusions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Resource {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub location: String,
    pub status: String,
    pub modified_at: f64,
    pub size: f64,
    pub favorite: bool,
    pub workspace_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub opened_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub match_field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Activity {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub resource_id: Option<String>,
    pub at: f64,
    pub source: String,
    pub file_modified_at: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Overview {
    pub session: String,
    pub permission_version: u32,
    pub revision: u32,
    pub sqlite_version: String,
    pub roots: Vec<Root>,
    pub file_grants: Vec<Resource>,
    pub workspaces: Vec<Workspace>,
    pub activities: Vec<Activity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DocumentTrackingStatus {
    pub supported: bool,
    pub roots: Vec<DocumentTrackingRoot>,
    pub files: Vec<DocumentTrackingFile>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DocumentTrackingRoot {
    pub root_id: String,
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DocumentTrackingFile {
    pub resource_id: String,
    pub name: String,
    pub established: bool,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ResourceQuery {
    pub workspace_id: Option<String>,
    pub kind: String,
    pub mode: String,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ResourcePage {
    pub session: String,
    pub permission_version: u32,
    pub revision: u32,
    pub items: Vec<Resource>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SearchRequest {
    pub request_id: u32,
    pub query: String,
    pub workspace_id: Option<String>,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SearchResponse {
    pub request_id: u32,
    pub page: ResourcePage,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next_cursor: Option<SearchCursor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct SearchCriteria {
    pub query: String,
    pub workspace_id: Option<String>,
    pub kind: String,
    pub sort: String,
    pub modified_from: Option<f64>,
    pub modified_to: Option<f64>,
    pub path_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SearchNamePosition {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SearchRankPosition {
    pub rank: u8,
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SearchModifiedPosition {
    pub modified_at: Option<f64>,
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SearchCursor {
    pub version: u32,
    pub session: String,
    pub permission_version: u32,
    pub revision: u32,
    pub next_offset: u32,
    pub criteria: SearchCriteria,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name_position: Option<SearchNamePosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rank_position: Option<SearchRankPosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub modified_position: Option<SearchModifiedPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SearchPageRequest {
    pub request: SearchRequest,
    pub offset: u32,
    pub sort: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cursor: Option<SearchCursor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub modified_from: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub modified_to: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub path_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Mutation {
    CreateWorkspace {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        resource_ids: Option<Vec<String>>,
    },
    ReorderWorkspaces {
        ids: Vec<String>,
    },
    RenameWorkspace {
        id: String,
        name: String,
    },
    DeleteWorkspace {
        id: String,
    },
    Link {
        workspace_id: String,
        resource_ids: Vec<String>,
    },
    Unlink {
        workspace_id: String,
        resource_ids: Vec<String>,
    },
    Transfer {
        source: String,
        target: String,
        resource_ids: Vec<String>,
    },
    Favorite {
        id: String,
        value: bool,
    },
    SaveWeb {
        id: Option<String>,
        name: String,
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace_id: Option<String>,
    },
    DeleteWeb {
        id: String,
    },
    RevokeFile {
        id: String,
    },
    RevokeRoot {
        id: String,
    },
    SetPaused {
        id: String,
        value: bool,
    },
    Exclude {
        root_id: String,
        relative_path: String,
    },
    Include {
        root_id: String,
        relative_path: String,
    },
    RebuildSearch,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MutationRequest {
    pub operation_id: String,
    pub mutation: Mutation,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MutationResponse {
    pub id: Option<String>,
    pub changed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ScratchNote {
    pub id: String,
    pub text: String,
    pub pinned: bool,
    pub version: u32,
    pub updated_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct NotesState {
    pub scope: String,
    pub items: Vec<ScratchNote>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct NoteWrite {
    pub scope: String,
    pub id: String,
    pub text: String,
    pub pinned: bool,
    pub expected_version: u32,
    pub deleted: bool,
}
