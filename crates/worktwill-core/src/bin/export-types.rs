use std::path::PathBuf;
use ts_rs::TS;
use worktwill_core::{model::*, timeline::*};

fn main() {
    let declarations = [
        UiLanguage::decl(),
        SuggestedLocation::decl(),
        AppError::decl(),
        WebMetadata::decl(),
        TimelineCriteria::decl(),
        TimelinePosition::decl(),
        TimelineCursor::decl(),
        TimelineRequest::decl(),
        TimelineEntry::decl(),
        CalendarPoint::decl(),
        TimelinePage::decl(),
        worktwill_core::ImportItem::decl(),
        worktwill_core::ImportResult::decl(),
        Root::decl(),
        Workspace::decl(),
        Resource::decl(),
        Activity::decl(),
        Overview::decl(),
        ResourceQuery::decl(),
        ResourcePage::decl(),
        SearchRequest::decl(),
        SearchCriteria::decl(),
        SearchNamePosition::decl(),
        SearchRankPosition::decl(),
        SearchModifiedPosition::decl(),
        SearchCursor::decl(),
        SearchPageRequest::decl(),
        SearchResponse::decl(),
        Mutation::decl(),
        MutationRequest::decl(),
        MutationResponse::decl(),
    ];
    let text = format!(
        "// Generated from worktwill-core Rust DTOs. Run pnpm types.\n{}\n",
        declarations
            .iter()
            .map(|d| format!("export {d}"))
            .collect::<Vec<_>>()
            .join("\n\n")
    );
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../desktop/src/contracts.ts");
    if std::env::args().any(|a| a == "--check") {
        assert_eq!(
            // Git 在 Windows 可能检出 CRLF；仅统一换行，仍逐字检查 DTO。
            std::fs::read_to_string(path)
                .expect("Run pnpm types first")
                .replace("\r\n", "\n"),
            text,
            "Generated contract drift"
        );
    } else {
        std::fs::write(path, text).expect("Write generated contracts");
    }
}
