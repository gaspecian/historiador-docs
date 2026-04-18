//! Email address. Minimal validation — just requires a single `@`
//! separating two non-empty halves. Full RFC 5322 validation is a
//! waste; we trust the downstream IMAP/SMTP step to reject real garbage
//! and let upstream input validators (the `validator` crate) handle
//! user-facing form feedback.

use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Email(String);

impl Email {
    pub fn parse(raw: impl Into<String>) -> Result<Self, DomainError> {
        let s = raw.into();
        let trimmed = s.trim();
        let (local, domain) = trimmed
            .split_once('@')
            .ok_or_else(|| DomainError::Validation("email must contain '@'".into()))?;
        if local.is_empty() || domain.is_empty() {
            return Err(DomainError::Validation("email has empty local or domain part".into()));
        }
        Ok(Self(trimmed.to_ascii_lowercase()))
    }

    pub fn from_trusted(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for Email {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
