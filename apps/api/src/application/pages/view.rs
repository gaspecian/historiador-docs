//! Composite read model returned by most page use cases: a page plus
//! all of its language versions. Presentation maps this to `PageResponse`.

use crate::domain::entity::{Page, PageVersion};

#[derive(Debug, Clone)]
pub struct PageView {
    pub page: Page,
    pub versions: Vec<PageVersion>,
}

/// Full versions-with-completeness read model.
#[derive(Debug, Clone)]
pub struct PageVersionsView {
    pub page: Page,
    pub workspace_languages: Vec<crate::domain::value::Language>,
    pub primary_language: crate::domain::value::Language,
    pub versions: Vec<PageVersion>,
    pub missing_languages: Vec<crate::domain::value::Language>,
}

impl PageVersionsView {
    pub fn complete(&self) -> bool {
        self.missing_languages.is_empty()
    }
}
