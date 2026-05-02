//! HTML view — sanitized HTML inserted via `MarkdownView`.

use leptos::prelude::*;

use crate::components::markdown::MarkdownView;
use crate::components::reader::css;
use crate::utils::RenderedMarkdown;

#[component]
pub fn HtmlReaderView(rendered: Signal<RenderedMarkdown>) -> impl IntoView {
    view! {
        <MarkdownView rendered=rendered class=css::htmlBody />
    }
}
