//! Utility modules. Cross-platform leaf utilities live in
//! [`websh_core::utils`] and are re-exported here for the legacy crate's
//! transitional shim.

pub use websh_core::content_routes;
pub use websh_core::utils::{
    RingBuffer, UrlValidation, asset, current_timestamp, data_url_for_bytes, dom, format,
    media_type_for_path, object_url_for_bytes, ring_buffer, sysinfo, time, url,
    validate_redirect_url,
};

pub mod breakpoints;
mod fetch;
pub mod markdown;
pub mod theme;
#[cfg(target_arch = "wasm32")]
pub mod wasm_cleanup;

pub use fetch::{RaceResult, fetch_content, fetch_json, race_with_timeout};
pub use markdown::{
    HeadingEntry, RenderedMarkdown, hydrate_math, render_inline_markdown, render_markdown,
    rendered_from_html, sanitize_html,
};
