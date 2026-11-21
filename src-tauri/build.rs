fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "overview",
            "begin_search_session",
            "cancel_search",
            "search_page",
            "timeline",
            "fetch_web_metadata",
            "import_drop",
            "cancel_drop",
            "pick_files",
            "resources",
            "notes",
            "write_note",
            "search",
            "mutate",
            "pick_root",
            "rescan",
            "open_resource",
            "reveal_resource",
            "pick_relink",
            "backup",
            "restore",
        ]),
    ))
    .expect("Tauri build configuration failed");
}
