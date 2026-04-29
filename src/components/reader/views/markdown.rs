//! Markdown view (rendered) and edit (textarea).
//!
//! The viewer renders Comrak-output sanitized HTML through `MarkdownView`
//! and pairs it with a paper-style outline sidebar (h2 / h3 only). The
//! sidebar floats to the left of the body via negative margin so the body
//! itself stays centered at the page's max-width; the sidebar collapses
//! on narrow viewports.

use leptos::ev;
use leptos::prelude::*;

use crate::components::markdown::MarkdownView;
use crate::components::reader::css;
use crate::utils::{HeadingEntry, RenderedMarkdown};

#[component]
pub fn MarkdownReaderView(rendered: Signal<RenderedMarkdown>) -> impl IntoView {
    let outline = Signal::derive(move || rendered.get().outline);

    view! {
        <div class=css::mdvPaper>
            <TocSide entries=outline />
            <MarkdownView rendered=rendered class=css::mdBody />
        </div>
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

#[component]
fn TocSide(entries: Signal<Vec<HeadingEntry>>) -> impl IntoView {
    view! {
        <Show when=move || !entries.get().is_empty()>
            <aside class=css::tocSide aria-label="Table of contents">
                <div class=css::tocSideLab>"contents"</div>
                {move || {
                    entries.get().into_iter().map(|entry| {
                        let entry_class = if entry.level == 3 {
                            format!("{} {}", css::tocSideEntry, css::tocSideEntryNested)
                        } else {
                            css::tocSideEntry.to_string()
                        };
                        // The href is kept as a same-page anchor so the link
                        // stays meaningful (right-click → copy, hover preview),
                        // but the click handler intercepts navigation: this
                        // app is hash-routed (`#/path/to/page`), and letting
                        // the browser replace the fragment with `#section-id`
                        // would clobber the route and 404 the page.
                        let href = format!("#{}", entry.id);
                        let id = entry.id.clone();
                        view! {
                            <a
                                class=entry_class
                                href=href
                                on:click=move |ev: ev::MouseEvent| {
                                    ev.prevent_default();
                                    scroll_to_anchor(&id);
                                }
                            >
                                {entry.text}
                            </a>
                        }
                    }).collect_view()
                }}
            </aside>
        </Show>
    }
}

#[cfg(target_arch = "wasm32")]
fn scroll_to_anchor(id: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(element) = document.get_element_by_id(id) else {
        return;
    };
    // `align_to_top = true` mirrors the default browser behaviour for
    // `<a href="#anchor">`: place the heading at the top of the viewport.
    element.scroll_into_view_with_bool(true);
}

#[cfg(not(target_arch = "wasm32"))]
fn scroll_to_anchor(_id: &str) {}
