//! Web-side utility modules. Cross-platform leaves are re-exported from
//! [`websh_core::utils`]; web-only leaves (markdown, breakpoints, theme,
//! wasm_cleanup) are declared as `pub mod` here.

pub use websh_core::content_routes;
pub use websh_core::utils::{
    RaceResult, RingBuffer, UrlValidation, asset, current_timestamp, data_url_for_bytes, dom,
    fetch_content, fetch_json, format, media_type_for_path, object_url_for_bytes,
    race_with_timeout, ring_buffer, sysinfo, time, url, validate_redirect_url,
};

pub mod breakpoints;
pub mod markdown;
pub mod theme;
#[cfg(target_arch = "wasm32")]
pub mod wasm_cleanup;

pub use markdown::{
    HeadingEntry, RenderedMarkdown, hydrate_math, render_inline_markdown, render_markdown,
    rendered_from_html, sanitize_html,
};
