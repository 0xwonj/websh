//! PDF view — abstract section + iframe wrapper with fullscreen toggle.

use leptos::prelude::*;

use crate::features::reader::css;
use crate::platform::BrowserAssetUrl;
use crate::shared::components::{
    WindowActionLink, WindowFrame, WindowTrafficButton, WindowTrafficTone,
};
use websh_core::domain::PageSize;

#[component]
pub fn PdfReaderView(
    title: Signal<String>,
    url: BrowserAssetUrl,
    size_pretty: Option<String>,
    abstract_text: String,
    page_size: Option<PageSize>,
    page_count: Option<u32>,
) -> impl IntoView {
    let url = StoredValue::new_local(url);
    let asset_url = Signal::derive(move || url.with_value(|url| url.as_str().to_string()));
    let aspect_style = page_size.map(|geom| {
        let padded_height = geom.height + (geom.height / 32);
        format!("aspect-ratio: {} / {padded_height};", geom.width)
    });
    let page_count_label =
        page_count.map(|n| format!("{n} {}", if n == 1 { "page" } else { "pages" }));

    // Fullscreen the outer div, not the iframe — keeps chrome visible.
    let frame_ref = NodeRef::<leptos::html::Div>::new();
    let is_fullscreen = RwSignal::new(false);
    let pdf_collapsed = RwSignal::new(false);
    let pdf_expanded = Signal::derive(move || !pdf_collapsed.get());
    let pdf_collapsed_signal = Signal::derive(move || pdf_collapsed.get());
    let title_label = Signal::derive(move || {
        let mut label = title.get();
        if let Some(page_count_label) = page_count_label.as_ref() {
            label.push_str(" · ");
            label.push_str(page_count_label);
        }
        if let Some(size_pretty) = size_pretty.as_ref() {
            label.push_str(" · ");
            label.push_str(size_pretty);
        }
        label
    });

    // Built-in PDF viewers honor these fragments unevenly, but Chrome and
    // Firefox use them as non-network fit hints.
    let viewer_url = move || {
        let fragment = if is_fullscreen.get() {
            "view=Fit&zoom=page-fit"
        } else {
            "view=FitH&zoom=page-width"
        };
        url.with_value(|url| format!("{}#{fragment}", url.as_str()))
    };

    install_fullscreen_sync(frame_ref, is_fullscreen);

    let close_pdf = Callback::new(move |()| {
        exit_fullscreen_or_collapse(is_fullscreen, pdf_collapsed);
    });
    let minimize_pdf = Callback::new(move |()| {
        exit_fullscreen_or_collapse(is_fullscreen, pdf_collapsed);
    });
    let on_toggle_fullscreen = Callback::new(move |()| {
        if pdf_collapsed.get_untracked() {
            pdf_collapsed.set(false);
        }
        toggle_fullscreen(frame_ref);
    });
    let left_controls = view! {
        <WindowTrafficButton
            tone=WindowTrafficTone::Close
            aria_label="Collapse PDF"
            aria_controls="pdf-document-viewer"
            aria_expanded=pdf_expanded
            on_click=close_pdf
        />
        <WindowTrafficButton
            tone=WindowTrafficTone::Minimize
            aria_label="Minimize PDF"
            aria_controls="pdf-document-viewer"
            aria_expanded=pdf_expanded
            on_click=minimize_pdf
        />
        <WindowTrafficButton
            tone=WindowTrafficTone::Zoom
            aria_label="Toggle PDF fullscreen"
            aria_controls="pdf-document-viewer"
            on_click=on_toggle_fullscreen
        />
    }
    .into_any();
    let right_actions = view! {
        <WindowActionLink href=asset_url aria_label="Download PDF" download=true>"⤓"</WindowActionLink>
        <WindowActionLink href=asset_url aria_label="Open PDF" external=true>"↗"</WindowActionLink>
    }
    .into_any();

    view! {
        {(!abstract_text.is_empty()).then(|| view! {
            <h2 class=css::sectionTitle data-n="">"Abstract"</h2>
            <p class=css::abstractText>{abstract_text}</p>
        })}

        <h2 class=css::sectionTitle data-n="">"Document"</h2>
        <div class=css::pdfFrame>
            <WindowFrame
                title=title_label
                left_controls=left_controls
                right_actions=right_actions
                collapsed=pdf_collapsed_signal
                body_id="pdf-document-viewer"
                node_ref=frame_ref
            >
            <iframe
                src=viewer_url
                class=css::pdfViewer
                title=move || title.get()
                style=aspect_style
                allow="fullscreen"
            />
            </WindowFrame>
        </div>
    }
}

/// Mirror the browser's fullscreen state into `is_fullscreen` so Esc /
/// native exit also flip the label.
#[cfg(target_arch = "wasm32")]
fn install_fullscreen_sync(frame_ref: NodeRef<leptos::html::Div>, is_fullscreen: RwSignal<bool>) {
    use crate::platform::wasm_cleanup::WasmCleanup;
    use leptos::prelude::on_cleanup;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };

    let document_for_handler = document.clone();
    let closure = Closure::wrap(Box::new(move || {
        let active = match (
            document_for_handler.fullscreen_element(),
            frame_ref.get_untracked(),
        ) {
            (Some(fs_el), Some(our)) => {
                let our_node = our.unchecked_into::<web_sys::Node>();
                let fs_node = fs_el.unchecked_ref::<web_sys::Node>();
                our_node.is_same_node(Some(fs_node))
            }
            _ => false,
        };
        is_fullscreen.set(active);
    }) as Box<dyn Fn()>);

    let _ = document
        .add_event_listener_with_callback("fullscreenchange", closure.as_ref().unchecked_ref());

    let cleanup = WasmCleanup(closure);
    on_cleanup(move || {
        let _ =
            document.remove_event_listener_with_callback("fullscreenchange", cleanup.js_function());
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn install_fullscreen_sync(_frame_ref: NodeRef<leptos::html::Div>, _is_fullscreen: RwSignal<bool>) {
}

fn exit_fullscreen_or_collapse(is_fullscreen: RwSignal<bool>, collapsed: RwSignal<bool>) {
    if is_fullscreen.get_untracked() {
        exit_fullscreen();
    } else {
        collapsed.update(|collapsed| *collapsed = !*collapsed);
    }
}

#[cfg(target_arch = "wasm32")]
fn exit_fullscreen() {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if document.fullscreen_element().is_some() {
        document.exit_fullscreen();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn exit_fullscreen() {}

#[cfg(target_arch = "wasm32")]
fn toggle_fullscreen(frame_ref: NodeRef<leptos::html::Div>) {
    use wasm_bindgen::JsCast;

    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if document.fullscreen_element().is_some() {
        document.exit_fullscreen();
    } else if let Some(node) = frame_ref.get_untracked() {
        let element = node.unchecked_into::<web_sys::Element>();
        let _ = element.request_fullscreen();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn toggle_fullscreen(_frame_ref: NodeRef<leptos::html::Div>) {}
