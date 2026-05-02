//! Shared sanitized Markdown insertion and math hydration components.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::utils::{RenderedMarkdown, hydrate_math};

#[component]
pub fn MarkdownView(rendered: Signal<RenderedMarkdown>, class: &'static str) -> impl IntoView {
    let node_ref = NodeRef::<leptos::html::Div>::new();

    Effect::new(move |_| {
        if let Some(node) = node_ref.get() {
            mount_rendered_html(&node, rendered.get());
        }
    });

    view! {
        <div node_ref=node_ref class=class></div>
    }
}

#[component]
pub fn InlineMarkdownView(rendered: Signal<RenderedMarkdown>) -> impl IntoView {
    let node_ref = NodeRef::<leptos::html::Span>::new();

    Effect::new(move |_| {
        if let Some(node) = node_ref.get() {
            mount_rendered_html(&node, rendered.get());
        }
    });

    view! {
        <span node_ref=node_ref></span>
    }
}

fn mount_rendered_html(node: &web_sys::HtmlElement, rendered: RenderedMarkdown) {
    node.set_inner_html(&rendered.html);
    if rendered.has_math {
        let element: web_sys::Element = node.clone().unchecked_into();
        hydrate_math(&element);
    }
}
