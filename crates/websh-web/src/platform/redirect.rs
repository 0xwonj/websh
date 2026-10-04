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

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn accepts_allowed_hosts_and_subdomains() {
        for url in [
            "https://github.com/user/repo",
            "http://twitter.com/user",
            "https://www.github.com/user",
            "https://api.github.com:443/repos",
        ] {
            assert!(
                matches!(validate_redirect_url(url), UrlValidation::Valid(_)),
                "{url}"
            );
        }
    }

    #[wasm_bindgen_test]
    fn rejects_invalid_or_untrusted_destinations() {
        assert!(matches!(
            validate_redirect_url(""),
            UrlValidation::Invalid(UrlValidationError::Empty)
        ));
        for url in ["ftp://example.com", "javascript:alert(1)"] {
            assert!(
                matches!(
                    validate_redirect_url(url),
                    UrlValidation::Invalid(UrlValidationError::InvalidProtocol)
                ),
                "{url}"
            );
        }
        assert!(matches!(
            validate_redirect_url("https://"),
            UrlValidation::Invalid(_)
        ));
        assert!(matches!(
            validate_redirect_url("https://evil.com/phishing"),
            UrlValidation::Invalid(UrlValidationError::DomainNotAllowed(_))
        ));
    }
}
