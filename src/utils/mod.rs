//! Utility modules for web, DOM, and data structure operations.
//!
//! Provides:
//! - [`RingBuffer`] - Fixed-capacity circular buffer with O(1) push
//! - [`fetch_content`], [`fetch_json`] - Network fetching with timeout
//! - [`markdown_to_html`] - Markdown rendering with XSS sanitization
//! - [`validate_redirect_url`] - URL security validation
//! - [`format`] - Size, date, and address formatting

pub mod cache;
pub mod dom;
mod fetch;
pub mod format;
mod markdown;
mod ring_buffer;
pub mod sysinfo;
mod url;

pub use fetch::{RaceResult, fetch_content, fetch_json, fetch_json_cached, race_with_timeout};
pub use markdown::markdown_to_html;
pub use ring_buffer::RingBuffer;
pub use url::{UrlValidation, validate_redirect_url};
