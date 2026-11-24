# WorkTwill architecture

This document describes technical ownership, data flow, and source entry points. Expected product behavior is owned by the [product contract](docs/PRODUCT.md); user operations by the [user guide](docs/USER_GUIDE.md); versioned delivery by the [release workflow](docs/RELEASING.md). It is not a record of completed platform acceptance. Agent task routing is in [AGENTS.md](AGENTS.md).

## Runtime and ownership

The desktop runtime is Tauri 2, React, TypeScript, Rust, and SQLite. The website is a separate static application.

| Layer | Source | Responsibility |
| --- | --- | --- |
| Desktop UI | `desktop/src/App.tsx`, `workbench/`, `timeline/` | Navigation, inputs, selection, drafts, request lifecycle, and rendering |
| Frontend bridge | `desktop/src/api.ts` | Typed Tauri command calls and search request sessions |
| Native shell | `src-tauri/src/main.rs` and sibling modules | Native pickers, drop tokens, watchers, scan coordination, default-app opening, web metadata, and updates |
| Domain and storage | `crates/worktwill-core/src/` | Authorization, resource UUIDs, relationships, indexing, search, activity, notes, identity, and recovery |
| Cross-platform DTOs | `crates/worktwill-core/src/model.rs` | Rust definitions exported to `desktop/src/contracts.ts` using `pnpm types` |
| Website | `website/` | Bilingual static pages, downloads, and update manifests |

The native `Runtime` holds a shared `Store` and coordination state. These are existing ownership boundaries, not a requirement to introduce a service, hook, crate, or process for each feature. Root-level `app.js`, `styles.css`, `index.html`, and `prototype-b/` are retained prototypes and are not the desktop runtime. `spikes/` contains independent experiments.

## Data ownership

- A resource has a stable application UUID. Paths, URLs, physical file identity, access credentials, and document continuity evidence have distinct roles. Relationships use UUIDs.
- Use existing backend transactions for relationship changes, compound operations, and operation replay.
- Rust owns authorization and persisted business data. React owns interaction state and confirmed snapshots received from Rust. Do not create a second business database in localStorage.
- Indexes and search projections are derived data. Rebuilding them must preserve workspaces, references, favorites, saved links, notes, and authorization rules.

Business semantics for workspaces, navigation, search scope, original-file protection, and user consent live in the [product contract](docs/PRODUCT.md). The sections below describe how technical changes must preserve them.

## Authorization, identity, and file continuity

Relevant modules: `catalog.rs`, `ingest.rs`, `file_locator.rs`, `root_locator.rs`, `platform.rs`, `relink_review.rs`, and `open_target.rs` in the core; `picked_files.rs` and `drop_import.rs` in the native shell.

Directory grants and individual file grants form the effective access scope. Pausing scanning keeps authorization; an unavailable location is not a revoked grant. Revocation hides inaccessible file details across reads, search, history, and opening while preserving applicable references. Overlapping grants remain independently evaluated.

Native pickers and native drop tokens supply candidates. A frontend path is not proof of identity or permission. Import, scanning, opening, and relinking reuse backend validation and resource normalization. Batch receipts must preserve actual per-item outcomes; a frontend summary does not change a backend transaction's failure semantics.

A matching path, name, size, or modification time does not establish identity continuity. Copies and Save As normally produce independent resources. If identity is unknown or conflicting, retain relationships and request explicit relinking rather than silently binding a replacement.

On macOS, `macos_bookmark.rs` and the locator modules use private Bookmark data together with physical identity and authorization checks. A resolved Bookmark path alone does not establish continuity; do not assume Document ID availability or general safe-save support.

On Windows, `windows_snapshot.rs` and `document_continuity.rs` separate physical identity from document anchors. Only after explicit consent for an eligible internal NTFS directory may the application create an Object ID on an important file (favorited or linked to a workspace). This consent is distinct from read authorization. Do not mark every scanned file, write sidecars or ADS, remove existing markers on revocation, or silently merge conflicting resource UUIDs. A continuity commit must keep current binding, permissions, history, projection, and versions consistent.

Opening prepares a checked backend target, invokes the operating system, and revalidates before recording the open. OS acceptance and successful activity recording are distinct outcomes. Do not hold database locks across native calls or promise that path-based OS opening removes all filesystem races.

## Storage, versions, and recovery

`store.rs` owns SQLite access, write/restore coordination, session identity, revisions, and scan epochs. Connections enable foreign keys, WAL, and `synchronous=FULL`. `schema.sql` initializes the base schema; `migrations.rs` is the authority for the current version and upgrade path. Do not duplicate a schema version in the UI or treat the base SQL file as the fully migrated database.

Writes use existing transactions and replay mechanisms. Search and activity reads use consistent read snapshots. Authorization and query revisions invalidate stale responses and cursors; restore coordination additionally prevents old work from modifying restored state.

`notes.rs` owns versioned notes and tombstones, separate from resource revisions. `HomeWidgets.tsx` keeps editing drafts and serializes saves per note. Only an acknowledged draft may appear saved. Restore scopes and conflicts must reject outdated writes rather than overwrite new data.

App-data backups contain metadata and relationships, not source documents, and may contain sensitive paths or URLs. Store uses SQLite's Online Backup API. Restore stages, validates, and migrates a backup, preserves a pre-restore snapshot, revokes restored file access, rebuilds derived search data, and advances versions/epochs. Older programs must reject unsupported schemas. Do not repair a failed migration by deleting the user's database, copying only an active database's main file without WAL, or granting access from a restored path.

Changes to migration, restore, or concurrent writes require relevant failure and old-data regressions. Code support does not establish a complete real-device restore or rollback guarantee.

## Search, activity, and background refresh

`search.rs`, `cancellation.rs`, `timeline.rs`, `activity_origin.rs`, and `known_refresh.rs` own core query and observation behavior. The shell coordinates requests and filesystem notifications through `search_requests.rs`, `known_changes.rs`, and `priority_scans.rs`.

Quick search groups bounded results; the full page uses backend pagination. Search and timeline cursors bind query conditions and session/permission/revision state. A stale cursor requires the existing bounded refresh behavior, not mixing pages or retrying indefinitely. Ordinary resource lists use their existing offset and version checks. Frontend response rejection and actual backend query cancellation must both work.

Filesystem events are observations that trigger checked refresh or reconciliation. Unchanged scans must not invalidate paginated results through needless query revision increments. Authorization, identity, and restore changes still invalidate stale data. Background work must remain bounded and must not expand authorized scope to recover a missing file.

`catalog-changed` carries a `CatalogVersion` read from one database snapshot. The UI immediately invalidates old content on a changed session/permission version or an unknown version; an ordinary revision change reuses the background refresh path used for opens. That path reads overview and the currently loaded list range concurrently, validates all page versions, then replaces confirmed snapshots without unmounting rows or clearing selection. Concurrent snapshot/cursor changes allow one retry and otherwise report an error. Operation completion handles notifications received during that operation once; other bursts are coalesced. Initial loading establishes the authorization snapshot before querying rows. Views without a resource-list consumer do not request unused resource pages.

`ActivityTimeline` receives separate strong-invalidation and ordinary-refresh tokens. Ordinary refresh and manual retry keep the confirmed event cards/calendar and loaded range; changed criteria or strong invalidation hide the previous snapshot immediately. This keeps timeline visibility tied to authorization without using every catalog write as a reset.

Activity distinguishes observation time, file modification time, and in-app opens. It is not a complete editing audit or version history. Historical visibility must preserve observation identity and authorization provenance. Calendar queries use explicit local-day boundaries rather than assuming every day lasts 24 hours.

## Network and distribution

`src-tauri/src/web_metadata.rs` implements optional, bounded public-web title fetching, including URL and network-target validation. Disabled or failed fetching permits manual titles. It does not supply browser cookies, scripts, or offline page archives.

`src-tauri/src/updates.rs` owns update state and download preferences. The UI displays revisioned state and waits for pending note saves before installation. The updater verifies package signatures and announced versions; this is separate from macOS/Windows OS code signing. `website/release.json` is the source for per-platform download records and generated update manifests. See [installation and updates](docs/INSTALL_AND_UPDATES.md) and [website engineering](website/README.md).

## Verification and performance

Run `pnpm check --fixtures <approved-absolute-path>` for Rust, native-shell, DTO, and cross-layer behavior changes. It runs contract drift, frontend build/UI tests, Rust formatting/workspace tests, and strict Clippy. Relevant explicit disk benchmarks are:

```sh
# WORKTWILL_TEST_ROOT must already point to an approved fixture directory.
pnpm bench:search
pnpm bench:scan
pnpm bench:pagination
```

These commands select ignored, larger tests; the ordinary test suite does not establish their results. Keep tests and benchmark code public and reproducible, while raw machine reports and screenshots stay private.

Existing candidate performance targets remain targets, not claims of release support:

| Metric | Candidate budget | Conditions |
| --- | --- | --- |
| Search activation to editable input | P95 ≤ 50 ms | Main window already running |
| Input to visible first results | P95 ≤ 150 ms; P99 ≤ 300 ms | 100,000 mixed resources, warm cache, actual rendering, at least 500 samples |
| SQLite search core | P95 ≤ 60 ms | Same dataset; report SQL and query plans |
| Large results | Bounded quick results and paginated/windowed full results | No full-result transfer to React |

Record platform, build, hardware/storage, dataset, query/filter distribution, cache conditions, sampling, percentiles, correctness, and failures. SQL latency or UI Automation timing alone does not prove rendered end-to-end latency. Changing budgets, permission semantics, or recovery requirements needs an explicit explanation and maintainer decision.

Native drag/drop, default apps, real editor saves, cross-restart identity, installation, and updates need evidence on each platform and exact build. Internal disks with ordinary local folders are the V1 scope; dedicated network, removable, and cloud-sync adaptations are outside it. Preserve unavailable-location and identity safety regardless. Do not replace missing platform evidence with browser mocks, another OS, or successful compilation.
