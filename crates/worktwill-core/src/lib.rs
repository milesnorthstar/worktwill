mod activity_origin;
#[cfg(test)]
mod activity_origin_tests;
mod cancellation;
mod catalog;
mod file_locator;
#[cfg(all(test, target_os = "macos"))]
mod file_locator_tests;
mod ingest;
mod known_refresh;
#[cfg(target_os = "macos")]
mod macos_bookmark;
mod migrations;
pub mod model;
mod open_target;
mod platform;
mod relink_review;
mod root_locator;
#[cfg(all(test, target_os = "macos"))]
mod root_locator_tests;
mod search;
mod store;
pub use cancellation::SearchCancellation;
pub use ingest::{capture_import_selections, ImportItem, ImportResult, ImportSelection};
pub use open_target::ResolvedOpen;
pub use relink_review::{RelinkImpact, RelinkReview, RelinkWorkspace};
pub use store::Store;

#[cfg(test)]
mod acceptance_pagination;
#[cfg(all(test, target_os = "macos"))]
mod acceptance_root_locator;
#[cfg(test)]
mod acceptance_scan;
#[cfg(test)]
mod acceptance_search;
#[cfg(test)]
mod acceptance_search_mixed;
#[cfg(test)]
mod search_modified_tests;
#[cfg(test)]
mod search_page_tests;
#[cfg(test)]
mod snapshot_read_tests;

pub mod timeline;

#[cfg(test)]
mod resource_relation_tests;
