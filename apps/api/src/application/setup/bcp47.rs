//! BCP 47 tag validation for the small subset v1 accepts.

use regex::Regex;

use crate::domain::error::{ApplicationError, DomainError};

pub fn validate_pair(
    languages: &[String],
    primary: &str,
) -> Result<(), ApplicationError> {
    let re = Regex::new(r"^[a-z]{2,3}(-[A-Z]{2})?$").unwrap();
    for tag in languages {
        if !re.is_match(tag) {
            return Err(DomainError::Validation(format!(
                "invalid BCP 47 language tag: {tag}"
            ))
            .into());
        }
    }
    if !re.is_match(primary) {
        return Err(DomainError::Validation(format!(
            "invalid BCP 47 primary_language: {primary}"
        ))
        .into());
    }
    if !languages.iter().any(|l| l == primary) {
        return Err(DomainError::Validation(
            "primary_language must be one of languages".into(),
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_common_tags() {
        assert!(validate_pair(&["pt-BR".into(), "en-US".into()], "pt-BR").is_ok());
        assert!(validate_pair(&["en".into()], "en").is_ok());
    }

    #[test]
    fn rejects_bad_shapes() {
        assert!(validate_pair(&["EN".into()], "EN").is_err());
        assert!(validate_pair(&["pt_BR".into()], "pt_BR").is_err());
    }

    #[test]
    fn primary_must_be_in_languages() {
        assert!(validate_pair(&["pt-BR".into()], "en-US").is_err());
    }
}
