//! Lifecycle state of a page / page_version.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageStatus {
    Draft,
    Published,
}
