//! Utility modules for web, DOM, and data structure operations.
//!
//! Provides:
//! - [`RingBuffer`] - Fixed-capacity circular buffer with O(1) push
//! - [`fetch_content`], [`fetch_json`] - Network fetching with timeout
//! - [`render_markdown`], [`sanitize_html`] - HTML rendering with XSS sanitization
//! - [`validate_redirect_url`] - URL security validation
//! - [`format`] - Size, date, and address formatting

mod asset;
pub mod content_routes;
pub mod dom;
mod fetch;
pub mod format;
mod markdown;
mod ring_buffer;
pub mod sysinfo;
pub mod theme;
mod time;
mod url;

pub use asset::{data_url_for_bytes, media_type_for_path, object_url_for_bytes};
pub use fetch::{RaceResult, fetch_content, fetch_json, race_with_timeout};
pub use markdown::{
    HeadingEntry, RenderedMarkdown, hydrate_math, render_inline_markdown, render_markdown,
    rendered_from_html, sanitize_html,
};
pub use ring_buffer::RingBuffer;
pub use time::current_timestamp;
pub use url::{UrlValidation, validate_redirect_url};
