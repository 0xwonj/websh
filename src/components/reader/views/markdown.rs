//! Markdown view (rendered) and edit (textarea).

use leptos::prelude::*;

use crate::components::markdown::MarkdownView;
use crate::components::reader::css;
use crate::utils::RenderedMarkdown;

#[component]
pub fn MarkdownReaderView(rendered: Signal<RenderedMarkdown>) -> impl IntoView {
    view! {
        <MarkdownView rendered=rendered class=css::mdBody />
    }
}

#[component]
pub fn MarkdownEditorView(
    draft_body: RwSignal<String>,
    on_input_dirty: Callback<()>,
) -> impl IntoView {
    view! {
        <textarea
            class=css::editorTextarea
            prop:value=move || draft_body.get()
            on:input=move |ev| {
                draft_body.set(event_target_value(&ev));
                on_input_dirty.run(());
            }
        />
    }
}
