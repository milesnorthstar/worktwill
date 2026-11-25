# Product contract

WorkTwill is a local personal workspace for file references and saved web links. It helps people find and organize existing material without moving their originals. This is the public behavior contract for maintenance, not an internal roadmap or a claim that every platform scenario has passed acceptance.

## Core behavior

| Area | Contract |
| --- | --- |
| Resources | Stable application UUIDs identify resources; paths and physical file identities are separate attributes. Copies and Save As normally create independent resources. |
| Workspaces | One resource can belong to multiple workspaces. Removing or transferring a reference changes relationships, not original files or unrelated references/favorites. |
| Workspace navigation | Opening a known workspace loads its own resources. Returning to a recently visited workspace can show its first page immediately when the catalog and permissions are current. |
| List actions | Workspace rows offer removal of that reference without prior selection; batch actions remain available. Favorites use the star to unfavorite. Deleting a saved web link remains a separate, confirmed application-wide action. No list action deletes original files. |
| Context menus | File and link rows offer actions for the clicked resource in the workbench, favorites, workspace details, and full search. Sidebar workspaces offer rename and confirmed deletion. Text inputs keep their editing menu. |
| Refresh and selection | Ordinary catalog refresh retains confirmed workspaces, counts, rows, loaded range, selection, and scroll until replacement data is ready. Selection actions appear in the list header while row positions stay steady. Authorization/session invalidation clears old details immediately. Search-based workspace additions retain the query/results and mark existing membership as added. |
| Manual rescan | Starting a rescan leaves unrelated controls and the existing resource list usable. Scan progress and catalog updates come from the desktop service. |
| Authorization | Access requires an explicit directory or individual-file grant. An individual grant does not authorize its parent. Paused scanning, unavailable locations, and revoked access are distinct states. |
| First use | The personal workbench and file search offer a folder picker when there is no file authorization. Picking or cancelling from search keeps the search context. Folder status reflects the catalog and users can open available results as reading progresses. |
| Revocation | Hide inaccessible file details across global queries and history as applicable; retain anonymous workspace references where needed. Do not delete originals or restore access from a remembered path. |
| Search | Search names/locations, saved titles/URLs, and workspace names. Keep quick results bounded and full results paginated; do not index document contents. |
| Date filters | Accept a typed date or a choice from the month calendar. Include both dates of a range and disable days that would reverse the range. |
| Navigation | Start at the personal workbench; provide workspaces, activity, favorites, and global search. There is no independent “all files” navigation page. |
| Motion | Use short transitions for navigation, interactive controls, overlays and feedback. Background refresh keeps existing rows steady. Respect the system’s reduced-motion preference. |
| Activity | Distinguish observations, file modification times, and in-app opens. Do not present them as complete edit history, document versions, or recovered events while the app was closed. |
| Notes | Persist plain-text notes separately from files. “Saved” means the latest draft was acknowledged; errors/conflicts retain an explicit recovery path. Forced exit does not guarantee recovery of unsaved drafts. |
| Opening | Use the system's default app/browser after backend permission and identity checks. OS acceptance and recording an activity are distinct results. |

## File and data safety

Source document contents remain untouched; organization does not move, rename, or delete originals or create sidecar files. The existing Windows exception requires separate consent for an eligible internal NTFS directory: document tracking may create an Object ID on favorited or workspace-linked files. Do not extend it to every scanned file, write ADS, or remove markers when tracking stops or access is revoked.

A matching path, name, size, modification time, or Bookmark result alone does not prove file continuity. When identity is unknown or conflicting, preserve relationships and require explicit confirmation/relinking. Do not silently substitute a same-name replacement or merge UUIDs. Actual editor safe-save behavior remains platform- and evidence-dependent.

Application backups preserve metadata and relationships, not original documents. Restoring requires renewed file-access authorization. Index rebuilding must preserve business data, references, favorites, notes, and authorization semantics. Technical ownership and migration rules are in [architecture](../ARCHITECTURE.md).

## Offline, network, and language

Core file search and organization work offline without an account or AI service. Optional web-title fetching contacts the destination and Cloudflare DNS; disabled or failed fetching still allows manual titles. Saved links are references, not offline archives or browser sessions.

Update-enabled builds check versions over the network. Automatic download can be disabled; installation requires confirmation and must respect pending note saves. Update signatures and OS code signing are separate. See [installation and updates](INSTALL_AND_UPDATES.md).

Maintain English and Simplified Chinese UI text through existing translation patterns. Use supplied product wording without adding explanations or interactions that were not requested.

## Scope and change decisions

V1 focuses on ordinary local folders on internal disks. Dedicated network-drive, removable-drive, and cloud-sync adaptation is outside this scope. Basic unavailable-location and identity safety still applies. macOS and Windows support is established separately for the exact build and relevant editor; compilation or another platform's evidence does not establish compatibility.

Fixes should restore the agreed behavior. Proposed behavior or scope changes need an explicit task/issue decision, then corresponding updates here, in user guidance, architecture if affected, and the [changelog](../CHANGELOG.md). Keep unreleased plans private; publish accurate available features and known limitations. Do not recreate an internal PRD as a contribution prerequisite.
