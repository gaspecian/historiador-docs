//! Page-related use cases.

pub mod create_page;
pub mod draft_page;
pub mod get_page;
pub mod list_pages;
pub mod page_versions;
pub mod publish_page;
pub mod search_pages;
pub mod update_page;
pub mod version_history;
pub mod view;

pub use create_page::{CreatePageCommand, CreatePageUseCase};
pub use draft_page::DraftPageUseCase;
pub use get_page::GetPageUseCase;
pub use list_pages::ListPagesUseCase;
pub use page_versions::GetPageVersionsUseCase;
pub use publish_page::PublishPageUseCase;
pub use search_pages::SearchPagesUseCase;
pub use update_page::{UpdatePageCommand, UpdatePageUseCase};
pub use version_history::{
    GetVersionHistoryItemUseCase, ListVersionHistoryCommand, ListVersionHistoryUseCase,
    RestoreVersionUseCase, VersionHistoryPage,
};
pub use view::{PageVersionsView, PageView};
