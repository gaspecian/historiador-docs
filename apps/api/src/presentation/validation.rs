//! Custom validators reused across multiple HTTP DTOs.

use validator::ValidationError;

/// Validate that a string is a usable LLM base URL per spec §4:
/// parses, http/https scheme only, no userinfo (credentials embedded
/// in the URL).
///
/// Trailing-slash normalization happens at the application layer
/// (see `UpdateLlmConfigUseCase::execute` and
/// `InitializeInstallationUseCase::execute`).
pub fn validate_llm_base_url(value: &str) -> Result<(), ValidationError> {
    let url = url::Url::parse(value).map_err(|_| ValidationError::new("invalid_url"))?;

    match url.scheme() {
        "http" | "https" => {}
        _ => return Err(ValidationError::new("scheme_must_be_http_or_https")),
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(ValidationError::new("userinfo_not_allowed"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_https() {
        assert!(validate_llm_base_url("https://api.example/v1").is_ok());
    }

    #[test]
    fn accepts_http() {
        assert!(validate_llm_base_url("http://localhost:8000").is_ok());
    }

    #[test]
    fn rejects_unparseable() {
        assert!(validate_llm_base_url("not a url").is_err());
    }

    #[test]
    fn rejects_ftp() {
        assert!(validate_llm_base_url("ftp://example.com").is_err());
    }

    #[test]
    fn rejects_userinfo() {
        assert!(validate_llm_base_url("https://user:pass@example.com").is_err());
        assert!(validate_llm_base_url("https://user@example.com").is_err());
    }

    #[test]
    fn accepts_with_trailing_slash() {
        assert!(validate_llm_base_url("https://api.example/v1/").is_ok());
    }
}
