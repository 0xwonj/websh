//! URL validation and security utilities.
//!
//! Provides URL validation with domain whitelisting for safe redirects.

use crate::config::ALLOWED_REDIRECT_DOMAINS;

/// Result of URL validation
#[derive(Debug, Clone, PartialEq)]
pub enum UrlValidation {
    /// URL is valid and safe to redirect
    Valid(String),
    /// URL is invalid or unsafe
    Invalid(UrlValidationError),
}

/// Errors that can occur during URL validation.
#[derive(Debug, Clone, PartialEq)]
pub enum UrlValidationError {
    /// URL is empty
    Empty,
    /// URL doesn't start with http:// or https://
    InvalidProtocol,
    /// URL has no host/domain
    NoHost,
    /// Domain is not in the allowed list
    DomainNotAllowed(String),
}

impl std::fmt::Display for UrlValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "URL is empty"),
            Self::InvalidProtocol => write!(f, "URL must start with http:// or https://"),
            Self::NoHost => write!(f, "URL has no host"),
            Self::DomainNotAllowed(domain) => write!(f, "Domain '{}' is not allowed", domain),
        }
    }
}

/// Validate a URL for safe redirect
///
/// Checks:
/// 1. URL is not empty
/// 2. URL starts with http:// or https://
/// 3. URL has a valid host
/// 4. Host is in the allowed domains list
pub fn validate_redirect_url(url: &str) -> UrlValidation {
    let url = url.trim();

    if url.is_empty() {
        return UrlValidation::Invalid(UrlValidationError::Empty);
    }

    // Check protocol
    let url_lower = url.to_lowercase();
    if !url_lower.starts_with("http://") && !url_lower.starts_with("https://") {
        return UrlValidation::Invalid(UrlValidationError::InvalidProtocol);
    }

    // Extract host from URL
    let Some(host) = extract_host(url) else {
        return UrlValidation::Invalid(UrlValidationError::NoHost);
    };

    // Check if host is in allowed list
    if !is_domain_allowed(&host) {
        return UrlValidation::Invalid(UrlValidationError::DomainNotAllowed(host));
    }

    UrlValidation::Valid(url.to_string())
}

/// Extract host from a URL
fn extract_host(url: &str) -> Option<String> {
    // Remove protocol
    let without_protocol = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("HTTPS://"))
        .or_else(|| url.strip_prefix("HTTP://"))?;

    // Get the host part (before first / or end of string)
    let host_part = without_protocol.split('/').next()?;

    // Remove port if present
    let host = host_part.split(':').next()?;

    // Remove www. prefix for matching
    let host = host.strip_prefix("www.").unwrap_or(host);

    if host.is_empty() {
        return None;
    }

    Some(host.to_lowercase())
}

/// Check if a domain is in the allowed list
fn is_domain_allowed(host: &str) -> bool {
    let host_lower = host.to_lowercase();

    for allowed in ALLOWED_REDIRECT_DOMAINS {
        // Exact match
        if host_lower == *allowed {
            return true;
        }
        // Subdomain match (e.g., "api.github.com" matches "github.com")
        if host_lower.ends_with(&format!(".{}", allowed)) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_urls() {
        assert!(matches!(
            validate_redirect_url("https://github.com/user/repo"),
            UrlValidation::Valid(_)
        ));
        assert!(matches!(
            validate_redirect_url("https://www.github.com/user"),
            UrlValidation::Valid(_)
        ));
        assert!(matches!(
            validate_redirect_url("https://api.github.com/repos"),
            UrlValidation::Valid(_)
        ));
        assert!(matches!(
            validate_redirect_url("http://twitter.com/user"),
            UrlValidation::Valid(_)
        ));
    }

    #[test]
    fn test_invalid_urls() {
        // Empty
        assert!(matches!(
            validate_redirect_url(""),
            UrlValidation::Invalid(UrlValidationError::Empty)
        ));

        // Invalid protocol
        assert!(matches!(
            validate_redirect_url("ftp://example.com"),
            UrlValidation::Invalid(UrlValidationError::InvalidProtocol)
        ));
        assert!(matches!(
            validate_redirect_url("javascript:alert(1)"),
            UrlValidation::Invalid(UrlValidationError::InvalidProtocol)
        ));

        // Domain not allowed
        assert!(matches!(
            validate_redirect_url("https://evil.com/phishing"),
            UrlValidation::Invalid(UrlValidationError::DomainNotAllowed(_))
        ));
    }

    #[test]
    fn test_extract_host() {
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
