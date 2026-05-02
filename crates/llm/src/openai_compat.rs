//! `Config` impl that supports custom base URLs and optional auth
//! for OpenAI-compatible HTTP endpoints (vLLM, LiteLLM, OpenRouter,
//! Azure-via-proxy, LM Studio, etc.).

use async_openai::config::{Config, OPENAI_API_BASE};
use reqwest::header::{HeaderMap, AUTHORIZATION};
use secrecy::{ExposeSecret, SecretString};

#[derive(Clone, Debug)]
pub struct OpenAiCompatConfig {
    api_base: String,
    api_key: SecretString,
    auth: bool,
}

impl OpenAiCompatConfig {
    /// `base_url = None` → use the canonical `https://api.openai.com/v1`.
    /// `api_key = None` → omit the `Authorization` header entirely
    /// (for unauthenticated self-hosted servers).
    pub fn new(base_url: Option<&str>, api_key: Option<&str>) -> Self {
        let api_base = base_url
            .map(|u| u.trim_end_matches('/').to_string())
            .unwrap_or_else(|| OPENAI_API_BASE.to_string());
        let (key_str, auth) = match api_key {
            Some(k) if !k.is_empty() => (k.to_string(), true),
            _ => (String::new(), false),
        };
        Self {
            api_base,
            api_key: SecretString::from(key_str),
            auth,
        }
    }
}

impl Config for OpenAiCompatConfig {
    fn headers(&self) -> HeaderMap {
        let mut h = HeaderMap::new();
        if self.auth {
            let v = format!("Bearer {}", self.api_key.expose_secret());
            h.insert(AUTHORIZATION, v.parse().expect("valid header value"));
        }
        h
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.api_base, path)
    }

    fn query(&self) -> Vec<(&str, &str)> {
        Vec::new()
    }

    fn api_base(&self) -> &str {
        &self.api_base
    }

    fn api_key(&self) -> &SecretString {
        &self.api_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::AUTHORIZATION;

    #[test]
    fn defaults_to_openai_when_base_url_none() {
        let cfg = OpenAiCompatConfig::new(None, Some("sk-test"));
        assert_eq!(cfg.api_base(), "https://api.openai.com/v1");
        assert_eq!(
            cfg.headers().get(AUTHORIZATION).unwrap(),
            "Bearer sk-test"
        );
    }

    #[test]
    fn uses_custom_base_url_with_bearer() {
        let cfg = OpenAiCompatConfig::new(Some("https://litellm.example/v1"), Some("k"));
        assert_eq!(cfg.api_base(), "https://litellm.example/v1");
        assert_eq!(cfg.headers().get(AUTHORIZATION).unwrap(), "Bearer k");
    }

    #[test]
    fn strips_trailing_slash() {
        let cfg = OpenAiCompatConfig::new(Some("https://litellm.example/v1/"), Some("k"));
        assert_eq!(cfg.api_base(), "https://litellm.example/v1");
    }

    #[test]
    fn omits_authorization_when_no_key() {
        let cfg = OpenAiCompatConfig::new(Some("http://localhost:8000/v1"), None);
        assert!(cfg.headers().get(AUTHORIZATION).is_none());
    }

    #[test]
    fn omits_authorization_when_empty_key() {
        let cfg = OpenAiCompatConfig::new(Some("http://localhost:8000/v1"), Some(""));
        assert!(cfg.headers().get(AUTHORIZATION).is_none());
    }
}
