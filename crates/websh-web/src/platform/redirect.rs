//! Redirect validation owned by the browser reader.

use thiserror::Error;

/// Allowed domains for external link redirects.
const ALLOWED_REDIRECT_DOMAINS: &[&str] = &[
    "github.com",
    "twitter.com",
    "x.com",
    "linkedin.com",
    "etherscan.io",
    "arbiscan.io",
    "optimistic.etherscan.io",
    "basescan.org",
    "polygonscan.com",
    "medium.com",
    "mirror.xyz",
    "notion.so",
    "docs.google.com",
    "drive.google.com",
    "youtube.com",
    "youtu.be",
];

#[derive(Debug, Clone, PartialEq)]
pub enum UrlValidation {
    Valid(String),
    Invalid(UrlValidationError),
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum UrlValidationError {
    #[error("URL is empty")]
    Empty,
    #[error("URL must start with http:// or https://")]
    InvalidProtocol,
    #[error("URL has no host")]
    NoHost,
    #[error("Domain '{0}' is not allowed")]
    DomainNotAllowed(String),
}

pub fn validate_redirect_url(url: &str) -> UrlValidation {
    let url = url.trim();

    if url.is_empty() {
        return UrlValidation::Invalid(UrlValidationError::Empty);
    }

    let url_lower = url.to_lowercase();
    if !url_lower.starts_with("http://") && !url_lower.starts_with("https://") {
        return UrlValidation::Invalid(UrlValidationError::InvalidProtocol);
    }

    let Some(host) = extract_host(url) else {
        return UrlValidation::Invalid(UrlValidationError::NoHost);
    };

    if !is_domain_allowed(&host) {
        return UrlValidation::Invalid(UrlValidationError::DomainNotAllowed(host));
    }

    UrlValidation::Valid(url.to_string())
}

fn extract_host(url: &str) -> Option<String> {
    let without_protocol = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("HTTPS://"))
        .or_else(|| url.strip_prefix("HTTP://"))?;

    let host_part = without_protocol.split('/').next()?;
    let host = host_part.split(':').next()?;
    let host = host.strip_prefix("www.").unwrap_or(host);

    if host.is_empty() {
        return None;
    }

    Some(host.to_lowercase())
}

fn is_domain_allowed(host: &str) -> bool {
    let host_lower = host.to_lowercase();

    ALLOWED_REDIRECT_DOMAINS
        .iter()
        .any(|allowed| host_lower == *allowed || host_lower.ends_with(&format!(".{allowed}")))
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn allows_exact_allowed_domains() {
        assert!(matches!(
            validate_redirect_url("https://github.com/user/repo"),
            UrlValidation::Valid(_)
        ));
        assert!(matches!(
            validate_redirect_url("http://twitter.com/user"),
            UrlValidation::Valid(_)
        ));
    }

    #[wasm_bindgen_test]
    fn allows_www_and_subdomains() {
        assert!(matches!(
            validate_redirect_url("https://www.github.com/user"),
            UrlValidation::Valid(_)
        ));
        assert!(matches!(
            validate_redirect_url("https://api.github.com/repos"),
            UrlValidation::Valid(_)
        ));
    }

    #[wasm_bindgen_test]
    fn rejects_invalid_protocols() {
        assert!(matches!(
            validate_redirect_url("ftp://example.com"),
            UrlValidation::Invalid(UrlValidationError::InvalidProtocol)
        ));
        assert!(matches!(
            validate_redirect_url("javascript:alert(1)"),
            UrlValidation::Invalid(UrlValidationError::InvalidProtocol)
        ));
    }

    #[wasm_bindgen_test]
    fn rejects_empty_urls() {
        assert!(matches!(
            validate_redirect_url(""),
            UrlValidation::Invalid(UrlValidationError::Empty)
        ));
    }

    #[wasm_bindgen_test]
    fn rejects_blocked_domains() {
        assert!(matches!(
            validate_redirect_url("https://evil.com/phishing"),
            UrlValidation::Invalid(UrlValidationError::DomainNotAllowed(_))
        ));
    }

    #[wasm_bindgen_test]
    fn extracts_hosts_for_validation() {
        assert_eq!(
            extract_host("https://github.com/user"),
            Some("github.com".to_string())
        );
        assert_eq!(
            extract_host("https://www.github.com/user"),
            Some("github.com".to_string())
        );
        assert_eq!(
            extract_host("https://api.github.com:443/repos"),
            Some("api.github.com".to_string())
        );
        assert_eq!(extract_host("https://"), None);
    }
}
