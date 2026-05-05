//! HTML view — sanitized HTML inserted via `MarkdownView`.

use leptos::prelude::*;

use crate::features::reader::css;
use crate::render::RenderedMarkdown;
use crate::shared::components::markdown::MarkdownView;

#[component]
pub fn HtmlReaderView(rendered: Signal<RenderedMarkdown>) -> impl IntoView {
    view! {
        <MarkdownView rendered=rendered class=css::htmlBody />
    }
}
