//! Markdown preview component for rendered content display.

use leptos::prelude::*;

stylance::import_crate_style!(css, "src/components/reader/preview.module.css");

/// Markdown preview component that renders HTML content.
#[component]
pub fn Preview(
    /// HTML content to render
    html: Memo<String>,
) -> impl IntoView {
    view! {
        <div class=css::preview>
            <article class=css::article inner_html=html />
        </div>
    }
}

/// Raw text preview for non-markdown files.
#[component]
pub fn RawPreview(
    /// Raw text content
    content: Signal<String>,
) -> impl IntoView {
    view! {
        <div class=css::preview>
            <pre class=css::rawText>{content}</pre>
        </div>
    }
}
