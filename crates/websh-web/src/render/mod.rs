//! Pure rendering helpers used by web features.

pub mod markdown;
pub mod theme;

pub use markdown::{
    HeadingEntry, RenderedMarkdown, hydrate_math, render_inline_markdown, render_markdown,
    rendered_from_html, sanitize_html,
};
