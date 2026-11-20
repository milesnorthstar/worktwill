// Generated from worktwill-core Rust DTOs. Run pnpm types.
export type UiLanguage = "zh" | "en";

export type SuggestedLocation = "desktop" | "downloads" | "documents";

export type AppError = { code: string, message: string, message_en: string, };

export type WebMetadata = { title: string | null, final_url: string, };

export type TimelineCriteria = { from: number | null, to: number | null, month_from: number, month_to: number, day_boundaries: Array<number>, filter: string, basis: string, workspace_id: string | null, };

export type TimelinePosition = { at: number, id: string, };

export type TimelineCursor = { version: number, session: string, permission_version: number, revision: number, next_offset: number, criteria: TimelineCriteria, before: TimelinePosition, };

export type TimelineRequest = { from: number | null, to: number | null, month_from: number, month_to: number, day_boundaries: Array<number>, filter: string, basis: string, workspace_id: string | null, offset: number, cursor?: TimelineCursor, };

export type TimelineEntry = { activity: Activity, resource: Resource | null, effective_at: number, };

export type CalendarPoint = { at: number, count: number, };

export type TimelinePage = { session: string, permission_version: number, revision: number, events: Array<TimelineEntry>, calendar: Array<CalendarPoint>, has_more: boolean, next_cursor?: TimelineCursor, };

export type ImportItem = { path: string, status: string, resource_id: string | null, message: string | null, message_en: string | null, };

export type ImportResult = { items: Array<ImportItem>, };

export type Root = { location_status: string, id: string, path: string, active: boolean, paused: boolean, scan_state: string, scanned: number, failures: number, exclusions: Array<string>, };

export type Workspace = { id: string, name: string, count: number, };

export type Resource = { id: string, kind: string, name: string, location: string, status: string, modified_at: number, size: number, favorite: boolean, workspace_ids: Array<string>, };

export type Activity = { id: string, kind: string, name: string, resource_id: string | null, at: number, source: string, file_modified_at: number | null, };

export type Overview = { session: string, permission_version: number, revision: number, sqlite_version: string, roots: Array<Root>, file_grants: Array<Resource>, workspaces: Array<Workspace>, activities: Array<Activity>, };

export type ResourceQuery = { workspace_id: string | null, kind: string, mode: string, offset: number, };

export type ResourcePage = { session: string, permission_version: number, revision: number, items: Array<Resource>, has_more: boolean, };

export type SearchRequest = { request_id: number, query: string, workspace_id: string | null, kind: string, };

export type SearchCriteria = { query: string, workspace_id: string | null, kind: string, sort: string, modified_from: number | null, modified_to: number | null, path_prefix: string | null, };

export type SearchNamePosition = { name: string, id: string, };

export type SearchRankPosition = { rank: number, name: string, id: string, };

export type SearchModifiedPosition = { modified_at: number | null, name: string, id: string, };

export type SearchCursor = { version: number, session: string, permission_version: number, revision: number, next_offset: number, criteria: SearchCriteria, name_position?: SearchNamePosition, rank_position?: SearchRankPosition, modified_position?: SearchModifiedPosition, };

export type SearchPageRequest = { request: SearchRequest, offset: number, sort: string, cursor?: SearchCursor, modified_from?: number, modified_to?: number, path_prefix?: string, };

export type SearchResponse = { request_id: number, page: ResourcePage, next_cursor?: SearchCursor, };

export type Mutation = { "type": "create_workspace", name: string, resource_ids?: Array<string>, } | { "type": "reorder_workspaces", ids: Array<string>, } | { "type": "rename_workspace", id: string, name: string, } | { "type": "delete_workspace", id: string, } | { "type": "link", workspace_id: string, resource_ids: Array<string>, } | { "type": "unlink", workspace_id: string, resource_ids: Array<string>, } | { "type": "transfer", source: string, target: string, resource_ids: Array<string>, } | { "type": "favorite", id: string, value: boolean, } | { "type": "save_web", id: string | null, name: string, url: string, workspace_id?: string | null, } | { "type": "delete_web", id: string, } | { "type": "revoke_file", id: string, } | { "type": "revoke_root", id: string, } | { "type": "set_paused", id: string, value: boolean, } | { "type": "exclude", root_id: string, relative_path: string, } | { "type": "include", root_id: string, relative_path: string, } | { "type": "rebuild_search" };

export type MutationRequest = { operation_id: string, mutation: Mutation, };

export type MutationResponse = { id: string | null, changed: number, };
