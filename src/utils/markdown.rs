//! Markdown rendering utilities.
//!
//! Provides safe markdown-to-HTML conversion with XSS protection.

use std::collections::HashMap;

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
    markdown_to_html_with_images(markdown, &HashMap::new())
}

/// Convert markdown content to sanitized HTML with image URL mapping.
///
/// The `image_urls` map allows replacing image src paths with alternative URLs.
/// This is used to display pending/draft images from memory (data URLs) before
/// they are committed to the remote storage.
///
/// # Arguments
/// * `markdown` - The markdown source text
/// * `image_urls` - Map of original paths to replacement URLs (e.g., data URLs)
pub fn markdown_to_html_with_images(markdown: &str, image_urls: &HashMap<String, String>) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(markdown, options);

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    // Replace image URLs if mappings provided
    let html_output = if !image_urls.is_empty() {
        replace_image_urls(&html_output, image_urls)
    } else {
        html_output
    };

    // Sanitize HTML to prevent XSS attacks
    // Configure ammonia to allow data URLs for images
    let mut builder = ammonia::Builder::default();
    builder.url_schemes(std::collections::HashSet::from([
        "http", "https", "mailto", "data",
    ]));
    builder.clean(&html_output).to_string()
}

/// Replace image src URLs in HTML based on the provided mapping.
fn replace_image_urls(html: &str, image_urls: &HashMap<String, String>) -> String {
    let mut result = html.to_string();

    for (original_path, replacement_url) in image_urls {
        // Handle both with and without leading slash
        let patterns = [
            format!(r#"src="{}""#, original_path),
            format!(r#"src='{}'"#, original_path),
        ];

        for pattern in &patterns {
            let replacement = format!(r#"src="{}""#, replacement_url);
            result = result.replace(pattern, &replacement);
        }

        // Also handle URL-encoded paths
        let encoded = urlencoding::encode(original_path);
        let encoded_patterns = [
            format!(r#"src="{}""#, encoded),
            format!(r#"src='{}'"#, encoded),
        ];
        for pattern in &encoded_patterns {
            let replacement = format!(r#"src="{}""#, replacement_url);
            result = result.replace(pattern, &replacement);
        }
    }

    result
}
