//! Shareable-link visibility (Sprint 11, phase C1 / US-11.15 —
//! stretch).
//!
//! `ShareVisibility` maps 1:1 with the `pages.share_visibility`
//! column added in migration 0007. The helpers here are pure
//! functions so routes and use-cases stay slim.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShareVisibility {
    Private,
    Workspace,
    Public,
}

impl ShareVisibility {
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "workspace" => ShareVisibility::Workspace,
            "public" => ShareVisibility::Public,
            _ => ShareVisibility::Private,
        }
    }

    pub fn as_db_str(self) -> &'static str {
        match self {
            ShareVisibility::Private => "private",
            ShareVisibility::Workspace => "workspace",
            ShareVisibility::Public => "public",
        }
    }

    /// Can an anonymous request see this page?
    pub fn allows_anonymous(self) -> bool {
        matches!(self, ShareVisibility::Public)
    }

    /// Can a signed-in member of the page's workspace see it?
    pub fn allows_workspace_members(self) -> bool {
        !matches!(self, ShareVisibility::Private)
    }
}

/// Strip the Referer header down to just its host so we can aggregate
/// without storing full URLs. Returns `None` for obviously invalid
/// input so the route handler can pass the value through to the DB
/// as NULL.
pub fn normalize_referrer(raw: Option<&str>) -> Option<String> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    let without_scheme = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"))
        .unwrap_or(raw);
    let host = without_scheme.split('/').next()?.to_ascii_lowercase();
    if host.is_empty() || host.contains(' ') {
        return None;
    }
    Some(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_db_round_trip() {
        for v in [
            ShareVisibility::Private,
            ShareVisibility::Workspace,
            ShareVisibility::Public,
        ] {
            assert_eq!(ShareVisibility::from_db_str(v.as_db_str()), v);
        }
    }

    #[test]
    fn from_db_str_defaults_to_private() {
        assert_eq!(
            ShareVisibility::from_db_str("???"),
            ShareVisibility::Private
        );
    }

    #[test]
    fn allows_anonymous_only_on_public() {
        assert!(ShareVisibility::Public.allows_anonymous());
        assert!(!ShareVisibility::Workspace.allows_anonymous());
        assert!(!ShareVisibility::Private.allows_anonymous());
    }

    #[test]
    fn allows_workspace_members_for_workspace_and_public() {
        assert!(ShareVisibility::Workspace.allows_workspace_members());
        assert!(ShareVisibility::Public.allows_workspace_members());
        assert!(!ShareVisibility::Private.allows_workspace_members());
    }

    #[test]
    fn normalize_referrer_strips_scheme_and_path() {
        assert_eq!(
            normalize_referrer(Some("https://Example.com/docs/foo?bar=1")),
            Some("example.com".to_string())
        );
        assert_eq!(
            normalize_referrer(Some("http://example.com")),
            Some("example.com".to_string())
        );
    }

    #[test]
    fn normalize_referrer_returns_none_for_empty_or_garbage() {
        assert!(normalize_referrer(None).is_none());
        assert!(normalize_referrer(Some("")).is_none());
        assert!(normalize_referrer(Some(" has spaces ")).is_none());
    }
}
