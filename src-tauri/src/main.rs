#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod dialog_copy;
mod drop_coordinates;
mod drop_import;
mod known_changes;
mod picked_files;
mod priority_scans;
mod search_requests;
mod web_metadata;

use notify::{RecursiveMode, Watcher};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{DragDropEvent, Emitter, Manager, State, WindowEvent};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use worktwill_core::{model::*, Store};

struct Runtime {
    store: Arc<Store>,
    scanning: AtomicBool,
    dirty: AtomicBool,
    retry_watches: AtomicBool,
    known_changes: Mutex<known_changes::KnownChanges>,
    refreshing_known: AtomicBool,
    priority_scans: Mutex<priority_scans::PriorityScans>,
    pending_drop: Mutex<drop_import::DropInbox>,
    search_requests: Mutex<search_requests::SearchRequests>,
}

fn internal_error() -> AppError {
    AppError::new(
        "internal_error",
        "后台任务未能完成",
        "The background task could not be completed.",
    )
}

fn notify_scan_completion(app: &tauri::AppHandle, store: &Store, before: Result<u32>) {
    match before.and_then(|before| store.query_revision().map(|after| before != after)) {
        Ok(true) => {
            let _ = app.emit("catalog-changed", ());
        }
        Ok(false) => {
            let _ = app.emit("scan-progress", ());
        }
        Err(error) => {
            // 无法确认版本时明确报告并清除旧资料，不把读取失败当成无变化。
            let _ = app.emit("operation-error", error);
            let _ = app.emit("catalog-changed", ());
        }
    }
}

fn kick_scan(app: tauri::AppHandle) {
    let state = app.state::<Arc<Runtime>>().inner().clone();
    state.dirty.store(true, Ordering::SeqCst);
    if state.scanning.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn_blocking(move || {
        // 合并观察洪峰；每次完成后重新检查 dirty，不为每个事件启动并行扫描。
        loop {
            state.dirty.store(false, Ordering::SeqCst);
            let before = state.store.query_revision();
            if let Err(error) = state.store.scan_all_with_root_updates(|roots| {
                for root in roots.iter().filter(|root| {
                    root.active
                        && !root.paused
                        && root.scan_state == "idle"
                        && root.location_status != "unavailable"
                }) {
                    request_root_scan(app.clone(), &root.id);
                }
            }) {
                let _ = app.emit("operation-error", error);
            }
            notify_scan_completion(&app, &state.store, before);
            if !state.dirty.swap(false, Ordering::SeqCst) {
                break;
            }
        }
        state.scanning.store(false, Ordering::SeqCst);
        if state.dirty.load(Ordering::SeqCst) {
            kick_scan(app);
        }
    });
}

fn request_root_scan(app: tauri::AppHandle, root: &str) {
    let state = app.state::<Arc<Runtime>>().inner().clone();
    let admission = match state.priority_scans.lock() {
        Ok(mut queue) => queue.request(root),
        Err(_) => {
            let _ = app.emit("operation-error", internal_error());
            return;
        }
    };
    match admission {
        priority_scans::Admission::Queued => return,
        priority_scans::Admission::Full => {
            kick_scan(app);
            return;
        }
        priority_scans::Admission::Start => {}
    }
    tauri::async_runtime::spawn_blocking(move || loop {
        let root = match state.priority_scans.lock() {
            Ok(mut queue) => queue.next(),
            Err(_) => {
                let _ = app.emit("operation-error", internal_error());
                return;
            }
        };
        let Some(root) = root else {
            return;
        };
        // 至多一个指定目录任务与原全量任务并行；权限、代次、恢复和批次写锁仍归Core校验。
        let before = state.store.query_revision();
        if let Err(error) = state.store.refresh_root(&root) {
            let _ = app.emit("operation-error", error);
        }
        notify_scan_completion(&app, &state.store, before);
    });
}

fn kick_known_refresh(app: tauri::AppHandle) {
    let state = app.state::<Arc<Runtime>>().inner().clone();
    if state.refreshing_known.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn_blocking(move || {
        let result = (|| -> Result<bool> {
            let paths = state
                .known_changes
                .lock()
                .map_err(|_| internal_error())?
                .take();
            let mut changed = false;
            for path in paths {
                match state.store.refresh_observed_file(&path) {
                    Ok(updated) => changed |= updated,
                    Err(error) => {
                        let _ = app.emit("operation-error", error);
                    }
                }
            }
            Ok(changed)
        })();
        match result {
            Ok(true) => {
                let _ = app.emit("catalog-changed", ());
            }
            Ok(false) => {}
            Err(error) => {
                let _ = app.emit("operation-error", error);
            }
        }
        state.refreshing_known.store(false, Ordering::SeqCst);
    });
}

#[tauri::command]
async fn overview(state: State<'_, Arc<Runtime>>) -> Result<Overview> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || store.overview())
        .await
        .map_err(|_| internal_error())?
}
#[tauri::command]
async fn resources(state: State<'_, Arc<Runtime>>, query: ResourceQuery) -> Result<ResourcePage> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || store.resources(query))
        .await
        .map_err(|_| internal_error())?
}
#[tauri::command]
fn begin_search_session(state: State<'_, Arc<Runtime>>) -> Result<String> {
    Ok(state
        .search_requests
        .lock()
        .map_err(|_| internal_error())?
        .begin_session())
}
#[tauri::command]
fn cancel_search(
    state: State<'_, Arc<Runtime>>,
    session: String,
    lane: search_requests::SearchLane,
    request_id: u32,
) -> Result<bool> {
    state
        .search_requests
        .lock()
        .map_err(|_| internal_error())?
        .cancel(&session, lane, request_id)
}
#[tauri::command]
async fn search(
    state: State<'_, Arc<Runtime>>,
    request: SearchRequest,
    session: String,
) -> Result<SearchResponse> {
    // 在排入线程池之前登记，排队期间的取消同样有效。
    let cancellation = state
        .search_requests
        .lock()
        .map_err(|_| internal_error())?
        .begin(
            &session,
            search_requests::SearchLane::Quick,
            request.request_id,
        )?;
    let store = state.store.clone();
    let worker = cancellation.clone();
    let response = tauri::async_runtime::spawn_blocking(move || {
        store.search_with_cancellation(request, &worker)
    })
    .await
    .map_err(|_| internal_error())??;
    cancellation.check()?;
    Ok(response)
}
#[tauri::command]
async fn mutate(
    app: tauri::AppHandle,
    state: State<'_, Arc<Runtime>>,
    request: MutationRequest,
) -> Result<MutationResponse> {
    let store = state.store.clone();
    let rescan = match &request.mutation {
        Mutation::SetPaused { id, value: false } => Some(id.clone()),
        Mutation::Exclude { root_id, .. } | Mutation::Include { root_id, .. } => {
            Some(root_id.clone())
        }
        _ => None,
    };
    let output = tauri::async_runtime::spawn_blocking(move || store.mutate(request))
        .await
        .map_err(|_| internal_error())??;
    let _ = app.emit("catalog-changed", ());
    if let Some(root) = rescan {
        state.retry_watches.store(true, Ordering::SeqCst);
        request_root_scan(app, &root);
    }
    Ok(output)
}
#[tauri::command]
async fn pick_root(
    app: tauri::AppHandle,
    state: State<'_, Arc<Runtime>>,
    language: UiLanguage,
    suggestion: Option<SuggestedLocation>,
) -> Result<Option<String>> {
    let store = state.store.clone();
    let mut dialog = app.dialog().file().set_title(dialog_copy::text(
        language,
        "选择管理位置",
        "Choose a managed location",
    ));
    if let Some(suggestion) = suggestion {
        // 仅设置原生选择器起始位置；用户最终选中的目录才是授权事实。
        let path = match suggestion {
            SuggestedLocation::Desktop => app.path().desktop_dir(),
            SuggestedLocation::Downloads => app.path().download_dir(),
            SuggestedLocation::Documents => app.path().document_dir(),
        }
        .map_err(|_| {
            AppError::new(
                "resource_unavailable",
                "此位置不可访问，请选择其他文件夹",
                "This location is unavailable. Choose another folder.",
            )
        })?;
        dialog = dialog.set_directory(path);
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        let Some(selected) = dialog.blocking_pick_folder() else {
            return Ok(None);
        };
        let path = selected.into_path().map_err(|_| {
            AppError::new("invalid_input", "请选择本地目录", "Select a local folder.")
        })?;
        store.grant_root(&path).map(Some)
    })
    .await
    .map_err(|_| internal_error())??;
    if let Some(root) = &result {
        state.retry_watches.store(true, Ordering::SeqCst);
        let _ = app.emit("catalog-changed", ());
        request_root_scan(app, root);
    }
    Ok(result)
}
#[tauri::command]
async fn rescan(app: tauri::AppHandle) -> Result<()> {
    app.state::<Arc<Runtime>>()
        .retry_watches
        .store(true, Ordering::SeqCst);
    kick_scan(app);
    Ok(())
}

#[tauri::command]
async fn open_resource(
    app: tauri::AppHandle,
    state: State<'_, Arc<Runtime>>,
    id: String,
) -> Result<()> {
    let store = state.store.clone();
    let target = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            let target = store.prepare_open(&id)?;
            if target.is_web() {
                app.opener().open_url(target.location(), None::<&str>)
            } else {
                app.opener().open_path(target.location(), None::<&str>)
            }
            .map_err(|_| {
                AppError::new(
                    "resource_unavailable",
                    "系统未接受打开请求",
                    "The system did not accept the open request.",
                )
            })?;
            // 系统调用前后的观察绑定同一对象，不能给期间重新指向的新文件记账。
            store.record_open_checked(target)?;
            Ok(())
        })();
        // 定位可能已提交，即使随后系统打开失败也要让界面读取新位置。
        let _ = app.emit("catalog-changed", ());
        result
    })
    .await
    .map_err(|_| internal_error())?;
    target
}
#[tauri::command]
async fn reveal_resource(
    app: tauri::AppHandle,
    state: State<'_, Arc<Runtime>>,
    id: String,
) -> Result<()> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            let (location, web) = store.resolve_resource(&id)?;
            if web {
                return Err(AppError::new(
                    "invalid_input",
                    "网页没有本地原始位置",
                    "A web link has no local original location.",
                ));
            }
            app.opener().reveal_item_in_dir(location).map_err(|_| {
                AppError::new(
                    "resource_unavailable",
                    "无法显示原始位置",
                    "The original location could not be shown.",
                )
            })
        })();
        let _ = app.emit("catalog-changed", ());
        result
    })
    .await
    .map_err(|_| internal_error())?
}
#[tauri::command]
async fn pick_relink(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<Runtime>>,
    id: String,
    language: UiLanguage,
) -> Result<bool> {
    let store = state.store.clone();
    let dialog = app.dialog().file().set_title(dialog_copy::text(
        language,
        "选择原件",
        "Choose the original file",
    ));
    let merge_copy = dialog_copy::confirmation(dialog_copy::Confirmation::Merge, language);
    let copy = dialog_copy::confirmation(dialog_copy::Confirmation::Relink, language);
    let dialog_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let review = store.review_relink(&id)?;
        if !dialog_app
            .dialog()
            .message(dialog_copy::relink_body(&review, language))
            .parent(&window)
            .title(copy.title)
            .buttons(copy.buttons())
            .blocking_show()
        {
            return Ok::<bool, AppError>(false);
        }
        let Some(selected) = dialog.blocking_pick_file() else {
            return Ok::<bool, AppError>(false);
        };
        let path = selected.into_path().map_err(|_| {
            AppError::new(
                "invalid_input",
                "请选择本地原件",
                "Select a local original file.",
            )
        })?;
        // 一次选择固定同一物理对象，合并确认后也不能重新按路径授予替换对象。
        let selection =
            worktwill_core::capture_import_selections(&[path.to_string_lossy().into_owned()])
                .into_iter()
                .next()
                .ok_or_else(internal_error)?;
        let review = store.review_relink_selection(&review, &selection)?;
        let merge = review.target.is_some();
        if merge
            && !dialog_app
                .dialog()
                .message(dialog_copy::merge_body(&review, language))
                .parent(&window)
                .title(merge_copy.title)
                .buttons(merge_copy.buttons())
                .blocking_show()
        {
            return Ok(false);
        }
        // 确认后再核对具体受影响关系；变化不能借用旧确认静默并入。
        store.relink_reviewed(&review, merge)?;
        Ok(true)
    })
    .await
    .map_err(|_| internal_error())??;
    if result {
        let _ = app.emit("catalog-changed", ());
    }
    Ok(result)
}
#[tauri::command]
async fn backup(
    app: tauri::AppHandle,
    state: State<'_, Arc<Runtime>>,
    language: UiLanguage,
) -> Result<bool> {
    let store = state.store.clone();
    let dialog = app
        .dialog()
        .file()
        .set_title(dialog_copy::text(
            language,
            "保存本地备份",
            "Save local backup",
        ))
        .set_file_name("worktwill-backup.sqlite3");
    tauri::async_runtime::spawn_blocking(move || {
        let Some(selected) = dialog.blocking_save_file() else {
            return Ok::<bool, AppError>(false);
        };
        let path = selected.into_path().map_err(|_| {
            AppError::new(
                "invalid_input",
                "请选择本地备份位置",
                "Select a local backup destination.",
            )
        })?;
        store.backup_to(&path)?;
        Ok(true)
    })
    .await
    .map_err(|_| internal_error())?
}
#[tauri::command]
async fn restore(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<Runtime>>,
    language: UiLanguage,
) -> Result<bool> {
    let store = state.store.clone();
    let dialog = app.dialog().file().set_title(dialog_copy::text(
        language,
        "选择本地备份",
        "Choose a local backup",
    ));
    let copy = dialog_copy::confirmation(dialog_copy::Confirmation::Restore, language);
    // 确认归属调用窗口；macOS使用窗口sheet，避免无父窗口的系统提醒游离。
    let message = app
        .dialog()
        .message(copy.body)
        .parent(&window)
        .title(copy.title)
        .buttons(copy.buttons());
    let result = tauri::async_runtime::spawn_blocking(move || {
        if !message.blocking_show() {
            return Ok::<bool, AppError>(false);
        }
        let Some(selected) = dialog.blocking_pick_file() else {
            return Ok::<bool, AppError>(false);
        };
        let path = selected.into_path().map_err(|_| {
            AppError::new(
                "invalid_input",
                "请选择本地备份文件",
                "Select a local backup file.",
            )
        })?;
        store.restore_from(&path)?;
        Ok(true)
    })
    .await
    .map_err(|_| internal_error())??;
    if result {
        let _ = app.emit("catalog-changed", ());
    }
    Ok(result)
}

fn start_watcher(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let pending = Arc::new(AtomicBool::new(false));
        let watched = Arc::new(Mutex::new(HashSet::<PathBuf>::new()));
        let events = pending.clone();
        let data_directory = app
            .state::<Arc<Runtime>>()
            .store
            .data_directory()
            .to_path_buf();
        let event_app = app.clone();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            match event {
                Ok(event) => {
                    // 自己的数据库、WAL 与恢复备份不触发目录扫描，避免授权上级目录时自激循环。
                    if !matches!(event.kind, notify::EventKind::Access(_))
                        && (event.paths.is_empty()
                            || event.paths.iter().any(|p| !p.starts_with(&data_directory)))
                    {
                        events.store(true, Ordering::SeqCst);
                        let state = event_app.state::<Arc<Runtime>>();
                        match state.known_changes.lock() {
                            Ok(mut changes) => changes.extend(
                                event
                                    .paths
                                    .into_iter()
                                    .filter(|p| !p.starts_with(&data_directory)),
                            ),
                            Err(_) => {
                                let _ = event_app.emit("operation-error", internal_error());
                            }
                        };
                    }
                }
                Err(_) => {
                    events.store(true, Ordering::SeqCst);
                    let _ = event_app.emit(
                        "operation-error",
                        AppError::new(
                            "watcher_rescan_required",
                            "目录通知不完整，正在重新核对",
                            "Folder notifications are incomplete. Rechecking the files.",
                        ),
                    );
                }
            }
        });
        let mut watcher = match watcher {
            Ok(w) => w,
            Err(_) => {
                let _ = app.emit(
                    "operation-error",
                    AppError::new(
                        "watcher_unavailable",
                        "目录监听不可用，请使用重新扫描",
                        "Folder monitoring is unavailable. Use Rescan.",
                    ),
                );
                return;
            }
        };
        let mut failed = HashSet::<PathBuf>::new();
        let mut location_retry = Duration::from_secs(30);
        let mut location_check = Instant::now() + location_retry;
        let mut location_error = None;
        loop {
            std::thread::sleep(Duration::from_secs(2));
            kick_known_refresh(app.clone());
            let state = app.state::<Arc<Runtime>>();
            let explicit_retry = state.retry_watches.swap(false, Ordering::SeqCst);
            // 空监听集和暂停根也能恢复；失败按30→60→…→300秒退避，同一错误只提示一次。
            if explicit_retry || Instant::now() >= location_check {
                let before = state.store.roots();
                let result = state.store.refresh_root_locations();
                let after = state.store.roots();
                if let (Ok(before), Ok(after)) = (before, after) {
                    let changed = after.iter().any(|root| {
                        before
                            .iter()
                            .find(|old| old.id == root.id)
                            .is_none_or(|old| {
                                old.path != root.path || old.location_status != root.location_status
                            })
                    });
                    if changed {
                        failed.clear();
                        let _ = app.emit("catalog-changed", ());
                        for root in after.iter().filter(|root| {
                            root.active
                                && root.location_status == "available"
                                && before
                                    .iter()
                                    .find(|old| old.id == root.id)
                                    .is_some_and(|old| {
                                        old.path != root.path
                                            || old.location_status != root.location_status
                                    })
                        }) {
                            request_root_scan(app.clone(), &root.id);
                        }
                    }
                }
                match result {
                    Ok(()) => {
                        location_retry = Duration::from_secs(30);
                        location_error = None;
                    }
                    Err(error) => {
                        if location_error.as_ref() != Some(&error.code) {
                            let _ = app.emit("operation-error", error.clone());
                        }
                        location_error = Some(error.code);
                        location_retry = (location_retry * 2).min(Duration::from_secs(300));
                    }
                }
                location_check = Instant::now() + location_retry;
            }
            let roots = match state.store.roots() {
                Ok(r) => r,
                Err(_) => continue,
            };
            let mut desired: HashSet<_> = roots
                .into_iter()
                .filter(|r| r.active && !r.paused && r.location_status != "unavailable")
                .map(|r| PathBuf::from(r.path))
                .collect();
            if let Ok(files) = state.store.file_grants() {
                desired.extend(files.into_iter().map(|r| PathBuf::from(r.location)));
            }
            // 失败位置只提示一次；用户重新扫描或重新启用位置后才重试，避免后台无限报错。
            if explicit_retry {
                failed.clear();
            }
            failed.retain(|path| desired.contains(path));
            let Ok(mut current) = watched.lock() else {
                break;
            };
            for removed in current.difference(&desired).cloned().collect::<Vec<_>>() {
                let _ = watcher.unwatch(&removed);
                current.remove(&removed);
            }
            for added in desired.difference(&current).cloned().collect::<Vec<_>>() {
                if failed.contains(&added) {
                    continue;
                }
                if watcher.watch(&added, RecursiveMode::Recursive).is_ok() {
                    current.insert(added);
                } else {
                    failed.insert(added);
                    let _ = app.emit(
                        "operation-error",
                        AppError::new(
                            "watcher_unavailable",
                            "一个管理位置监听失败，请检查磁盘并重新扫描",
                            "A managed location could not be monitored. Check the disk and rescan.",
                        ),
                    );
                }
            }
            drop(current);
            if pending.swap(false, Ordering::SeqCst) {
                kick_scan(app.clone());
            }
        }
    });
}

#[tauri::command]
async fn search_page(
    state: State<'_, Arc<Runtime>>,
    request: SearchPageRequest,
    session: String,
) -> Result<SearchResponse> {
    let cancellation = state
        .search_requests
        .lock()
        .map_err(|_| internal_error())?
        .begin(
            &session,
            search_requests::SearchLane::Page,
            request.request.request_id,
        )?;
    let store = state.store.clone();
    let worker = cancellation.clone();
    let response = tauri::async_runtime::spawn_blocking(move || {
        store.search_page_with_cancellation(request, &worker)
    })
    .await
    .map_err(|_| internal_error())??;
    cancellation.check()?;
    Ok(response)
}
#[tauri::command]
async fn timeline(
    state: State<'_, Arc<Runtime>>,
    request: worktwill_core::timeline::TimelineRequest,
) -> Result<worktwill_core::timeline::TimelinePage> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || store.timeline(request))
        .await
        .map_err(|_| internal_error())?
}
#[tauri::command]
async fn fetch_web_metadata(url: String) -> Result<WebMetadata> {
    web_metadata::fetch_metadata(url).await
}

#[tauri::command]
async fn import_drop(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<Runtime>>,
    token: String,
    workspace_id: Option<String>,
    authorize: bool,
    language: UiLanguage,
) -> Result<worktwill_core::ImportResult> {
    let paths = state
        .pending_drop
        .lock()
        .map_err(|_| internal_error())?
        .begin(&token, workspace_id.as_deref(), Instant::now())?;
    let store = state.store.clone();
    let dialog = app.dialog().clone();
    let copy = dialog_copy::confirmation(dialog_copy::Confirmation::GrantFiles, language);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let approved = authorize
            && dialog
                .message(copy.body)
                .parent(&window)
                .title(copy.title)
                .buttons(copy.buttons())
                .blocking_show();
        store.import_files_checked(&paths, workspace_id.as_deref(), approved)
    })
    .await
    .map_err(|_| internal_error())
    .and_then(|result| result);
    {
        let mut pending = state.pending_drop.lock().map_err(|_| internal_error())?;
        match &result {
            Ok(output) => pending.finish(&token, output),
            Err(_) => pending.failed(&token),
        }
    }
    let result = result?;
    let _ = app.emit("catalog-changed", ());
    Ok(result)
}
#[tauri::command]
async fn cancel_drop(state: State<'_, Arc<Runtime>>, token: String) -> Result<bool> {
    Ok(state
        .pending_drop
        .lock()
        .map_err(|_| internal_error())?
        .cancel(&token))
}

#[tauri::command]
async fn pick_files(
    app: tauri::AppHandle,
    state: State<'_, Arc<Runtime>>,
    workspace_id: Option<String>,
    language: UiLanguage,
) -> Result<Option<worktwill_core::ImportResult>> {
    let store = state.store.clone();
    let dialog = app.dialog().file().set_title(dialog_copy::text(
        language,
        "选择本地文件",
        "Choose local files",
    ));
    let result = tauri::async_runtime::spawn_blocking(move || {
        let Some(files) = dialog.blocking_pick_files() else {
            return Ok(None);
        };
        let paths = files
            .into_iter()
            .map(|p| {
                p.into_path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .map_err(|_| {
                        AppError::new("invalid_input", "请选择本地文件", "Select a local file.")
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        let selected = picked_files::PickedFiles::capture(paths);
        selected.submit(&store, workspace_id.as_deref()).map(Some)
    })
    .await
    .map_err(|_| internal_error())??;
    if result.is_some() {
        let _ = app.emit("catalog-changed", ());
    }
    Ok(result)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window,event| {
            let WindowEvent::DragDrop(event)=event else {return;};
            let state=window.state::<Arc<Runtime>>();
            let scale=match window.scale_factor(){Ok(scale)=>scale,Err(_)=>{let _=window.emit("operation-error",internal_error());return;}};
            match event {
                DragDropEvent::Enter{paths,position} => {
                    let (x,y)=drop_coordinates::physical_point(position.x,position.y,scale,cfg!(target_os="macos"));
                    if let Ok(mut pending)=state.pending_drop.lock(){pending.hover(paths.len());}
                    let _=window.emit("files-dragging",serde_json::json!({"x":x,"y":y,"count":paths.len()}));
                }
                DragDropEvent::Over{position} => {
                    let (x,y)=drop_coordinates::physical_point(position.x,position.y,scale,cfg!(target_os="macos"));
                    let count=state.pending_drop.lock().map(|pending|pending.hover_count()).unwrap_or(0);
                    let _=window.emit("files-dragging",serde_json::json!({"x":x,"y":y,"count":count}));
                }
                DragDropEvent::Leave => {
                    if let Ok(mut pending)=state.pending_drop.lock(){pending.leave();}
                    let _=window.emit("files-drag-left",());
                }
                DragDropEvent::Drop{paths,position} => {
                    let (x,y)=drop_coordinates::physical_point(position.x,position.y,scale,cfg!(target_os="macos"));
                    let _=window.emit("files-drag-left",());
                    let reserved=state.pending_drop.lock().map_err(|_|internal_error()).and_then(|mut pending|pending.reserve(paths.iter().map(|p|p.to_string_lossy().into_owned()).collect(),Instant::now()));
                    match reserved {
                        Ok((token,count,paths))=>{
                            // 松手时立即固定目标；身份捕获完成前切换工作区不能改变本次拖入归属。
                            let _=window.emit("files-drop-started",serde_json::json!({"token":token,"count":count,"x":x,"y":y}));
                            let app=window.app_handle().clone();
                            let window=window.clone();
                            // 身份快照可能涉及慢盘；在后台捕获，且旧任务不能覆盖后来拖入。
                            tauri::async_runtime::spawn_blocking(move || {
                                let selections=worktwill_core::capture_import_selections(&paths);
                                let runtime=app.state::<Arc<Runtime>>();
                                let captured=runtime.pending_drop.lock().map(|mut pending|pending.captured(&token,selections)).map_err(|_|internal_error());
                                match captured {
                                    Ok(true)=>{let _=window.emit("files-dropped",serde_json::json!({"token":token,"count":count,"x":x,"y":y}));}
                                    Ok(false)=>{}
                                    Err(error)=>{let _=window.emit("operation-error",error);}
                                }
                            });
                        }
                        Err(error)=>{let _=window.emit("operation-error",error);}
                    }
                }
                _=>{}
            }
        })
        .setup(|app| {
            let path = app.path().app_data_dir()?.join("worktwill.sqlite3");
            let store = Store::open(path).map_err(|error| std::io::Error::other(error.message))?;
            app.manage(Arc::new(Runtime {
                store: Arc::new(store),
                scanning: AtomicBool::new(false),
                dirty: AtomicBool::new(false),
                retry_watches: AtomicBool::new(false),
                known_changes: Mutex::new(known_changes::KnownChanges::default()),
                refreshing_known: AtomicBool::new(false),
                priority_scans: Mutex::new(priority_scans::PriorityScans::default()),
                pending_drop: Mutex::new(drop_import::DropInbox::default()),
                search_requests: Mutex::new(search_requests::SearchRequests::default()),
            }));
            start_watcher(app.handle().clone());
            kick_scan(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            begin_search_session,
            cancel_search,
            search_page,
            timeline,
            fetch_web_metadata,
            import_drop,
            cancel_drop,
            pick_files,
            overview,
            resources,
            search,
            mutate,
            pick_root,
            rescan,
            open_resource,
            reveal_resource,
            pick_relink,
            backup,
            restore
        ])
        .run(tauri::generate_context!())
        .expect("WorkTwill startup failed");
}
