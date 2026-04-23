//! Reader component for displaying file content.
//!
//! Supports markdown, PDF, images, and link files.
//! Navigation is handled by the router's canonical request helpers.

#![allow(dead_code)]

use leptos::{ev, prelude::*};
use leptos_icons::Icon;

use crate::app::AppContext;
use crate::components::Breadcrumb;
use crate::components::icons as ic;
use crate::core::engine::{RouteFrame, request_path_for_canonical_path};
use crate::models::FileType;
use crate::utils::{
    UrlValidation, data_url_for_bytes, markdown_to_html, media_type_for_path, sanitize_html,
    validate_redirect_url,
};

stylance::import_crate_style!(css, "src/components/reader/reader.module.css");

/// Async result variants produced by the reader's content fetch.
#[derive(Clone)]
enum ReaderContent {
    /// Markdown rendered to sanitized HTML.
    Html(String),
    /// Unknown type, plain text.
    Text(String),
    /// Binary asset rendered from the engine read surface.
    AssetUrl(String),
    /// Link type: navigation was triggered.
    Redirected,
    /// An error occurred while fetching or processing.
    Error(String),
}

/// Reader component for displaying file content.
///
/// # Props
/// - `route`: The current route frame (must resolve to a reader-capable intent)
/// - `on_close`: Callback invoked when the reader should be closed
#[component]
pub fn Reader(route: Memo<RouteFrame>, on_close: Callback<()>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let canonical_path = Memo::new(move |_| route.get().resolution.node_path.clone());

    // Derive display path for breadcrumb
    let display_path = Memo::new(move |_| route.get().display_path());

    // Derive file type from canonical path
    let file_type = Memo::new(move |_| FileType::from_path(canonical_path.get().as_str()));

    let route_href = Memo::new(move |_| {
        format!(
            "#{}",
            request_path_for_canonical_path(&canonical_path.get())
        )
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

    // Async resource: fetches content for types that need it. Using LocalResource
    // ensures stale futures are dropped when inputs change (fixes race condition
    // where a late-returning fetch could overwrite a newer result).
    let resource = LocalResource::new(move || {
        let canonical = canonical_path.get();
        let ft = file_type.get();
        async move {
            match ft {
                FileType::Html => match ctx.read_text(&canonical).await {
                    Ok(html) => ReaderContent::Html(sanitize_html(&html)),
                    Err(e) => ReaderContent::Error(e.to_string()),
                },
                FileType::Markdown => match ctx.read_text(&canonical).await {
                    Ok(md) => ReaderContent::Html(markdown_to_html(&md)),
                    Err(e) => ReaderContent::Error(e.to_string()),
                },
                FileType::Unknown => match ctx.read_text(&canonical).await {
                    Ok(text) => ReaderContent::Text(text),
                    Err(e) => ReaderContent::Error(e.to_string()),
                },
                FileType::Link => match ctx.read_text(&canonical).await {
                    Ok(target) => {
                        let target = target.trim();
                        match validate_redirect_url(target) {
                            UrlValidation::Valid(safe_url) => {
                                if let Some(window) = web_sys::window()
                                    && window.location().set_href(&safe_url).is_err()
                                {
                                    return ReaderContent::Error("Failed to redirect".to_string());
                                }
                                ReaderContent::Redirected
                            }
                            UrlValidation::Invalid(err) => {
                                ReaderContent::Error(format!("Redirect blocked: {}", err))
                            }
                        }
                    }
                    Err(e) => ReaderContent::Error(e.to_string()),
                },
                FileType::Pdf | FileType::Image => match ctx.read_bytes(&canonical).await {
                    Ok(bytes) => ReaderContent::AssetUrl(data_url_for_bytes(
                        &bytes,
                        media_type_for_path(canonical.as_str()),
                    )),
                    Err(e) => ReaderContent::Error(e.to_string()),
                },
            }
        }
    });

    // Loading: true while the resource has no value yet for the current file.
    let loading = Signal::derive(move || resource.get().is_none());

    // Content text/html for display. Empty string when not applicable.
    let content = Signal::derive(move || match resource.get() {
        Some(ReaderContent::Html(h)) => h,
        Some(ReaderContent::Text(t)) => t,
        _ => String::new(),
    });

    let asset_url = Signal::derive(move || match resource.get() {
        Some(ReaderContent::AssetUrl(url)) => Some(url),
        _ => None,
    });

    // Error message if the fetch failed.
    let error = Signal::derive(move || match resource.get() {
        Some(ReaderContent::Error(e)) => Some(e),
        _ => None,
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
    };

    let on_font_increase = move |_: ev::MouseEvent| {
        set_more_menu_open.set(false);
    };

    let on_font_decrease = move |_: ev::MouseEvent| {
        set_more_menu_open.set(false);
    };

    let on_share = move |_: ev::MouseEvent| {
        set_more_menu_open.set(false);
    };

    let on_download = move |_: ev::MouseEvent| {
        set_more_menu_open.set(false);
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
                        href=move || route_href.get()
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
                                    href=move || route_href.get()
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
                                FileType::Html | FileType::Markdown => {
                                    view! {
                                        <div class=css::markdown inner_html=content />
                                    }.into_any()
                                }
                                FileType::Pdf => {
                                    view! {
                                        <iframe
                                            src=move || asset_url.get().unwrap_or_default()
                                            class=css::pdfViewer
                                            title="PDF Viewer"
                                        />
                                    }.into_any()
                                }
                                FileType::Image => {
                                    view! {
                                        <div class=css::imageContainer>
                                            <img
                                                src=move || asset_url.get().unwrap_or_default()
                                                alt=filename.get()
                                                class=css::image
                                            />
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
