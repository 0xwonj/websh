//! Markdown rendering utilities.
//!
//! Provides safe markdown-to-HTML conversion with XSS protection.

use pulldown_cmark::{Options, Parser, html};

/// Convert markdown content to sanitized HTML.
///
/// Supports extended markdown syntax including:
/// - Strikethrough (`~~text~~`)
/// - Tables
/// - Footnotes
///
/// The output is sanitized using `ammonia` to prevent XSS attacks
/// by removing potentially dangerous HTML elements and attributes.
pub fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(markdown, options);

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    // Sanitize HTML to prevent XSS attacks
    ammonia::clean(&html_output)
}
