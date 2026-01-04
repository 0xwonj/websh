//! Utility modules for web, DOM, and data structure operations.
//!
//! Provides:
//! - [`RingBuffer`] - Fixed-capacity circular buffer with O(1) push
//! - [`fetch_content`], [`fetch_json`] - Network fetching with timeout
//! - [`markdown_to_html`] - Markdown rendering with XSS sanitization
//! - [`validate_redirect_url`] - URL security validation

pub mod cache;
pub mod dom;
mod ring_buffer;
pub mod sysinfo;
mod url;
mod web;

pub use ring_buffer::RingBuffer;
pub use url::{validate_redirect_url, UrlValidation};
pub use web::{fetch_content, fetch_json, fetch_json_cached, markdown_to_html, race_with_timeout, RaceResult};
