//! Minimal edit modal: textarea + Save/Cancel.
//!
//! This component is a pure UI surface. The app layer owns visibility,
//! filesystem reads, and save/cancel effects through props.

use leptos::{ev, prelude::*};
use wasm_bindgen::JsCast;

use websh_core::domain::VirtualPath;

stylance::import_crate_style!(css, "src/shared/components/editor/modal.module.css");

#[component]
pub fn EditModal(
    path: VirtualPath,
    content: RwSignal<String>,
    on_save: Callback<String>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    let modal_ref = NodeRef::<leptos::html::Div>::new();
    let textarea_ref = NodeRef::<leptos::html::Textarea>::new();
    let restore_focus = active_element();

    let on_save = move |_| {
        on_save.run(content.get_untracked());
    };

    let on_cancel = move |_| {
        on_cancel.run(());
    };

    Effect::new(move |_| {
        if let Some(textarea) = textarea_ref.get() {
            let _ = textarea.focus();
        }
    });

    on_cleanup(move || {
        if let Some(element) = restore_focus.as_ref() {
            focus_element(element);
        }
    });

    view! {
        <div class=css::backdrop on:click=on_cancel>
            <div
                node_ref=modal_ref
                class=css::modal
                role="dialog"
                aria-modal="true"
                aria-labelledby="edit-modal-title"
                on:keydown=move |ev| trap_modal_tab(modal_ref, ev)
                on:click=|ev: leptos::ev::MouseEvent| ev.stop_propagation()
            >
                <header class=css::header>
                    <span id="edit-modal-title" class=css::path>
                        {path.as_str().to_string()}
                    </span>
                </header>
                <textarea
                    node_ref=textarea_ref
                    class=css::textarea
                    aria-label="File contents"
                    prop:value=move || content.get()
                    on:input=move |ev| content.set(event_target_value(&ev))
                />
                <footer class=css::footer>
                    <button class=css::cancel on:click=on_cancel>"Cancel"</button>
                    <button class=css::save on:click=on_save>"Save"</button>
                </footer>
            </div>
        </div>
    }
}

fn active_element() -> Option<web_sys::Element> {
    web_sys::window()?.document()?.active_element()
}

fn focus_element(element: &web_sys::Element) {
    if let Some(html_element) = element.dyn_ref::<web_sys::HtmlElement>() {
        let _ = html_element.focus();
    }
}

fn trap_modal_tab(modal_ref: NodeRef<leptos::html::Div>, ev: ev::KeyboardEvent) {
    if ev.key() != "Tab" {
        return;
    }

    let Some(modal) = modal_ref.get_untracked() else {
        return;
    };
    let modal = modal.unchecked_into::<web_sys::Element>();
    let focusable = focusable_descendants(&modal);
    if focusable.is_empty() {
        ev.prevent_default();
        focus_element(&modal);
        return;
    }

    let active = active_element();
    let active_index = active.as_ref().and_then(|active| {
        focusable
            .iter()
            .position(|element| element.is_same_node(Some(active.unchecked_ref())))
    });

    let next = if ev.shift_key() {
        match active_index {
            Some(0) | None => focusable.last(),
            _ => return,
        }
    } else {
        match active_index {
            Some(index) if index + 1 == focusable.len() => focusable.first(),
            None => focusable.first(),
            _ => return,
        }
    };

    if let Some(element) = next {
        ev.prevent_default();
        focus_element(element);
    }
}

fn focusable_descendants(root: &web_sys::Element) -> Vec<web_sys::Element> {
    const FOCUSABLE_SELECTOR: &str = "button:not([disabled]), textarea, input, select, a[href], [tabindex]:not([tabindex=\"-1\"])";

    let Ok(nodes) = root.query_selector_all(FOCUSABLE_SELECTOR) else {
        return Vec::new();
    };

    let mut focusable = Vec::new();
    for index in 0..nodes.length() {
        let Some(node) = nodes.item(index) else {
            continue;
        };
        let Ok(element) = node.dyn_into::<web_sys::Element>() else {
            continue;
        };
        if element
            .dyn_ref::<web_sys::HtmlElement>()
            .is_some_and(|html| html.tab_index() >= 0)
        {
            focusable.push(element);
        }
    }
    focusable
}
