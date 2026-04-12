//! Shared utilities for the API server.

/// Generate a URL-safe slug from a human-readable name.
///
/// Lowercases the input, replaces non-alphanumeric characters with
/// hyphens, collapses consecutive hyphens, and trims leading/trailing
/// hyphens.
pub fn slugify(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse consecutive hyphens and trim.
    let mut result = String::with_capacity(slug.len());
    let mut prev_hyphen = true; // start true to trim leading hyphens
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }

    // Trim trailing hyphen.
    if result.ends_with('-') {
        result.pop();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_slugify() {
        assert_eq!(slugify("Getting Started"), "getting-started");
    }

    #[test]
    fn special_characters() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
    }

    #[test]
    fn consecutive_spaces() {
        assert_eq!(slugify("foo   bar"), "foo-bar");
    }

    #[test]
    fn leading_trailing_special() {
        assert_eq!(slugify("--hello--"), "hello");
    }

    #[test]
    fn unicode_characters() {
        assert_eq!(slugify("Configuração Básica"), "configuração-básica");
    }
}
