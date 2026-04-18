//! URL slug — lowercase, hyphenated, non-empty.

use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Slug(String);

impl Slug {
    pub fn parse(raw: impl Into<String>) -> Result<Self, DomainError> {
        let s = raw.into();
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(DomainError::Validation("slug is empty".into()));
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(DomainError::Validation(
                "slug must contain only lowercase letters, digits, and hyphens".into(),
            ));
        }
        Ok(Self(trimmed.to_string()))
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

impl AsRef<str> for Slug {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
