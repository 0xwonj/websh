//! PDF view — abstract section + iframe wrapper with fullscreen toggle.

use leptos::prelude::*;

use crate::components::reader::css;
use crate::models::PageSize;

#[component]
pub fn PdfReaderView(
    title: Signal<String>,
    url: String,
    size_pretty: Option<String>,
    abstract_text: String,
    page_size: Option<PageSize>,
    page_count: Option<u32>,
) -> impl IntoView {
    let url_for_open = url.clone();
    let url_for_download = url.clone();
    // Fit-page-width hint for built-in viewers (Chrome/Firefox honor it,
    // Safari ignores). Hash fragment, no network impact.
    let viewer_url = format!("{url}#view=FitH&zoom=page-width");
    let aspect_style =
        page_size.map(|geom| format!("aspect-ratio: {} / {};", geom.width, geom.height));
    let page_count_label =
        page_count.map(|n| format!("{n} {}", if n == 1 { "page" } else { "pages" }));

    // Fullscreen the outer div, not the iframe — keeps chrome visible.
    let frame_ref = NodeRef::<leptos::html::Div>::new();
    let is_fullscreen = RwSignal::new(false);

    install_fullscreen_sync(frame_ref, is_fullscreen);

    let on_toggle_fullscreen = move |_: leptos::ev::MouseEvent| {
        toggle_fullscreen(frame_ref);
    };

    view! {
        {(!abstract_text.is_empty()).then(|| view! {
            <h2 class=css::sectionTitle data-n="">"Abstract"</h2>
            <p class=css::abstractText>{abstract_text}</p>
        })}

        <h2 class=css::sectionTitle data-n="">"Document"</h2>
        <div class=css::pdfFrame node_ref=frame_ref>
            <div class=css::pdfChrome>
                <span class=css::pdfChromeDot></span>
                <span class=css::pdfChromeTitle>
                    {move || title.get()}
                    {page_count_label.map(|label| view! {
                        " · "{label}
                    })}
                    {size_pretty.map(|s| view! {
                        " · "{s}
                    })}
                </span>
                <button
                    type="button"
                    class=css::pdfChromeCtrl
                    on:click=on_toggle_fullscreen
                    aria-label=move || if is_fullscreen.get() {
                        "Exit fullscreen"
                    } else {
                        "Enter fullscreen"
                    }
                >
                    {move || if is_fullscreen.get() { "⛶ exit" } else { "⛶ full" }}
                </button>
                <a class=css::pdfChromeCtrl href=url_for_download download="">"⤓ pdf"</a>
                <a class=css::pdfChromeCtrl href=url_for_open target="_blank" rel="noopener">"↗ open"</a>
            </div>
            <iframe
                src=viewer_url
                class=css::pdfViewer
                title=move || title.get()
                style=aspect_style
                allow="fullscreen"
            />
        </div>
    }
}

/// Mirror the browser's fullscreen state into `is_fullscreen` so Esc /
/// native exit also flip the label.
#[cfg(target_arch = "wasm32")]
fn install_fullscreen_sync(frame_ref: NodeRef<leptos::html::Div>, is_fullscreen: RwSignal<bool>) {
    use crate::utils::wasm_cleanup::WasmCleanup;
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
