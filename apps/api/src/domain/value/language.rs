//! Language tag (BCP 47). Per ADR-005 every content-bearing entity
//! carries one. The domain layer does not validate the full BCP 47
//! grammar — it only rejects empty strings. Workspace configuration
//! constrains what tags are accepted at the boundary.

use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Language(String);

impl Language {
    pub fn parse(raw: impl Into<String>) -> Result<Self, DomainError> {
        let s = raw.into();
        if s.trim().is_empty() {
            return Err(DomainError::Validation("language tag is empty".into()));
        }
        Ok(Self(s))
    }

    /// Construct without validation. Use only in adapters that read
    /// already-validated values out of persistence.
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

impl AsRef<str> for Language {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
