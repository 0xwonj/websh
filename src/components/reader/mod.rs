//! Reader component for displaying file content.
//!
//! Supports markdown, PDF, images, and link files.
//! Navigation is handled via AppRoute::push().

#![allow(dead_code)]

use leptos::{ev, prelude::*};
use leptos_icons::Icon;
use wasm_bindgen_futures::spawn_local;

use crate::components::Breadcrumb;
use crate::components::icons as ic;
use crate::models::{AppRoute, FileType};
use crate::utils::{UrlValidation, fetch_content, markdown_to_html, validate_redirect_url};

stylance::import_crate_style!(css, "src/components/reader/reader.module.css");

/// Reader component for displaying file content.
///
/// # Props
/// - `route`: The current AppRoute (must be a Read route)
/// - `on_close`: Callback invoked when the reader should be closed
#[component]
pub fn Reader(route: Memo<AppRoute>, on_close: Callback<()>) -> impl IntoView {
    // Derive content path from route
    let content_path = Memo::new(move |_| route.get().path().to_string());

    // Derive display path for breadcrumb
    let display_path = Memo::new(move |_| route.get().display_path());

    // Derive file type from content path
    let file_type = Memo::new(move |_| FileType::from_path(&content_path.get()));

    // Derive content URL from route (uses mount's base_url)
    let content_url = Memo::new(move |_| {
        route
            .get()
            .content_url()
            .unwrap_or_else(|| content_path.get())
    });

    // Parse display path into breadcrumb segments (for filename extraction)
    let breadcrumb_segments = Memo::new(move |_| {
        display_path
            .get()
            .split('/')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect::<Vec<_>>()
    });

    // For PDF, use Mozilla's PDF.js viewer
    let pdf_viewer_url = Memo::new(move |_| {
        if file_type.get() == FileType::Pdf {
            let encoded = js_sys::encode_uri_component(&content_url.get());
            format!(
                "https://mozilla.github.io/pdf.js/web/viewer.html?file={}",
                encoded
            )
        } else {
            String::new()
        }
    });

    let (content, set_content) = signal(String::new());
    let (loading, set_loading) = signal(false); // Start as false, only true for async content
    let (error, set_error) = signal::<Option<String>>(None);

    // Load content when content_path changes
    Effect::new(move |_| {
        let url = content_url.get();
        let ft = file_type.get();

        set_error.set(None);
        set_content.set(String::new());

        // Only set loading for types that need async fetch
        match ft {
            FileType::Markdown | FileType::Link | FileType::Unknown => {
                set_loading.set(true);
                spawn_local(async move {
                    match ft {
                        FileType::Markdown => match fetch_content(&url).await {
                            Ok(md) => {
                                let html = markdown_to_html(&md);
                                set_content.set(html);
                                set_loading.set(false);
                            }
                            Err(e) => {
                                set_error.set(Some(e.to_string()));
                                set_loading.set(false);
                            }
                        },
                        FileType::Link => match fetch_content(&url).await {
                            Ok(url) => {
                                let url = url.trim();
                                match validate_redirect_url(url) {
                                    UrlValidation::Valid(safe_url) => {
                                        if let Some(window) = web_sys::window()
                                            && window.location().set_href(&safe_url).is_err()
                                        {
                                            set_error.set(Some("Failed to redirect".to_string()));
                                            set_loading.set(false);
                                        }
                                    }
                                    UrlValidation::Invalid(err) => {
                                        set_error.set(Some(format!("Redirect blocked: {}", err)));
                                        set_loading.set(false);
                                    }
                                }
                            }
                            Err(e) => {
                                set_error.set(Some(e.to_string()));
                                set_loading.set(false);
                            }
                        },
                        FileType::Unknown => match fetch_content(&url).await {
                            Ok(text) => {
                                set_content.set(text);
                                set_loading.set(false);
                            }
                            Err(e) => {
                                set_error.set(Some(e.to_string()));
                                set_loading.set(false);
                            }
                        },
                        _ => {}
                    }
                });
            }
            // PDF, Image don't need loading - render immediately
            _ => {
                set_loading.set(false);
            }
        }
    });

    // Handle keyboard events for closing
    let handle_keydown = move |ev: ev::KeyboardEvent| match ev.key().as_str() {
        "q" | "Escape" => {
            ev.prevent_default();
            on_close.run(());
        }
        _ => {}
    };

    // Focus the container on mount for keyboard events
    let container_ref = NodeRef::<leptos::html::Div>::new();
    Effect::new(move || {
        if let Some(el) = container_ref.get() {
            let _ = el.focus();
        }
    });

    // Extract filename for image alt text
    let filename = Memo::new(move |_| {
        breadcrumb_segments
            .get()
            .last()
            .cloned()
            .unwrap_or_default()
    });

    // More menu state
    let (more_menu_open, set_more_menu_open) = signal(false);

    // Placeholder handlers for menu items (UI only)
    let on_edit = move |_: ev::MouseEvent| {
        set_more_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Edit clicked".into());
    };

    let on_font_increase = move |_: ev::MouseEvent| {
        set_more_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Font increase clicked".into());
    };

    let on_font_decrease = move |_: ev::MouseEvent| {
        set_more_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Font decrease clicked".into());
    };

    let on_share = move |_: ev::MouseEvent| {
        set_more_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Share clicked".into());
    };

    let on_download = move |_: ev::MouseEvent| {
        set_more_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Download clicked".into());
    };

    view! {
        <div
            node_ref=container_ref
            tabindex="-1"
            class=format!("{} scrollbar-thin", css::reader)
            on:keydown=handle_keydown
        >
            // Header
            <header class=css::header>
                // Back button (left)
                <div class=css::navButtons>
                    <button
                        class=css::navButton
                        on:click=move |_| on_close.run(())
                        title="Back (Esc)"
                    >
                        <Icon icon=ic::CHEVRON_LEFT />
                    </button>
                </div>

                // Breadcrumb path (center)
                <Breadcrumb />

                // Action buttons (right)
                <div class=css::headerActions>
                    // Open in new tab
                    <a
                        href=move || content_url.get()
                        target="_blank"
                        rel="noopener noreferrer"
                        class=format!("{} {}", css::actionButton, css::desktopOnly)
                        title="Open in new tab"
                    >
                        <Icon icon=ic::EXTERNAL_LINK />
                    </a>

                    // More menu
                    <div class=css::dropdownWrapper>
                        <button
                            class=css::actionButton
                            on:click=move |_| set_more_menu_open.update(|v| *v = !*v)
                            title="More"
                        >
                            <Icon icon=ic::MORE />
                        </button>
                        <Show when=move || more_menu_open.get()>
                            <div class=css::dropdownMenu>
                                // Edit
                                <button class=css::dropdownItem on:click=on_edit>
                                    <span class=css::dropdownIcon><Icon icon=ic::EDIT /></span>
                                    "Edit"
                                </button>

                                // Open in new tab (mobile)
                                <a
                                    href=move || content_url.get()
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    class=format!("{} {}", css::dropdownItem, css::mobileOnly)
                                >
                                    <span class=css::dropdownIcon><Icon icon=ic::EXTERNAL_LINK /></span>
                                    "Open in new tab"
                                </a>

                                <div class=css::dropdownDivider />

                                // Font size
                                <button class=css::dropdownItem on:click=on_font_increase>
                                    <span class=css::dropdownIcon><Icon icon=ic::FONT_INCREASE /></span>
                                    "Increase font"
                                </button>
                                <button class=css::dropdownItem on:click=on_font_decrease>
                                    <span class=css::dropdownIcon><Icon icon=ic::FONT_DECREASE /></span>
                                    "Decrease font"
                                </button>

                                <div class=css::dropdownDivider />

                                // Share & Download
                                <button class=css::dropdownItem on:click=on_share>
                                    <span class=css::dropdownIcon><Icon icon=ic::SHARE /></span>
                                    "Share"
                                </button>
                                <button class=css::dropdownItem on:click=on_download>
                                    <span class=css::dropdownIcon><Icon icon=ic::DOWNLOAD /></span>
                                    "Download"
                                </button>
                            </div>
                        </Show>
                    </div>
                </div>
            </header>

            // Content
            <div class=css::content>
                <Show
                    when=move || loading.get()
                    fallback=move || {
                        // Show error if present, otherwise show content
                        if let Some(err) = error.get() {
                            view! {
                                <div class=css::error>
                                    <p class=css::errorTitle>"Error loading content:"</p>
                                    <p>{err}</p>
                                </div>
                            }.into_any()
                        } else {
                            // Render content based on file type
                            match file_type.get() {
                                FileType::Markdown => {
                                    view! {
                                        <div class=css::markdown inner_html=content />
                                    }.into_any()
                                }
                                FileType::Pdf => {
                                    view! {
                                        <iframe
                                            src=pdf_viewer_url.get()
                                            class=css::pdfViewer
                                            title="PDF Viewer"
                                        />
                                    }.into_any()
                                }
                                FileType::Image => {
                                    view! {
                                        <div class=css::imageContainer>
                                            <img src=content_url.get() alt=filename.get() class=css::image />
                                        </div>
                                    }.into_any()
                                }
                                FileType::Link => {
                                    view! {
                                        <div class=css::loading>
                                            <span>"Redirecting..."</span>
                                        </div>
                                    }.into_any()
                                }
                                FileType::Unknown => {
                                    view! {
                                        <div class=css::rawText>{content}</div>
                                    }.into_any()
                                }
                            }
                        }
                    }
                >
                    // Loading state
                    <div class=css::loading>
                        <span>"Loading content..."</span>
                    </div>
                </Show>
            </div>
        </div>
    }
}
