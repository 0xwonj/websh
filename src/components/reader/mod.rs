#![allow(dead_code)]

use icondata::Icon as IconData;
use leptos::{ev, prelude::*};
use leptos_icons::Icon;
use wasm_bindgen_futures::spawn_local;

use crate::app::AppContext;
use crate::components::icons as ic;
use crate::config::{CONTENT_BASE_URL, HOME_DIR};
use crate::models::{FileType, VirtualPath};
use crate::utils::{UrlValidation, fetch_content, markdown_to_html, validate_redirect_url};

stylance::import_crate_style!(css, "src/components/reader/reader.module.css");

/// Get file icon based on file type
fn get_file_icon(file_type: &FileType) -> IconData {
    match file_type {
        FileType::Markdown => ic::FILE_TEXT,
        FileType::Pdf => ic::FILE_PDF,
        FileType::Image => ic::FILE_IMAGE,
        FileType::Link => ic::FILE_LINK,
        FileType::Unknown => ic::FILE,
    }
}

#[component]
pub fn Reader(
    #[prop(into)] content_path: String,
    #[prop(into)] virtual_path: String,
    on_close: Callback<()>,
) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    let file_type = FileType::from_path(&content_path);
    let content_url = format!("{}/{}", CONTENT_BASE_URL, content_path);
    let file_icon = get_file_icon(&file_type);

    // Parse virtual path into breadcrumb segments (same logic as Explorer)
    // Convert home directory to ~ for display
    let display_path = if virtual_path == HOME_DIR {
        "~".to_string()
    } else if let Some(rest) = virtual_path.strip_prefix(&format!("{}/", HOME_DIR)) {
        format!("~/{}", rest)
    } else {
        virtual_path.clone()
    };

    let breadcrumb_segments: Vec<&str> =
        display_path.split('/').filter(|s| !s.is_empty()).collect();

    // For PDF, use Mozilla's PDF.js viewer
    let pdf_viewer_url = if file_type == FileType::Pdf {
        let encoded = js_sys::encode_uri_component(&content_url);
        format!(
            "https://mozilla.github.io/pdf.js/web/viewer.html?file={}",
            encoded
        )
    } else {
        String::new()
    };

    let (content, set_content) = signal(String::new());
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal::<Option<String>>(None);

    // Load content (only for types that need fetching)
    {
        let content_path = content_path.clone();
        let file_type = file_type.clone();
        spawn_local(async move {
            match file_type {
                FileType::Markdown => match fetch_content(&content_path).await {
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
                FileType::Link => {
                    match fetch_content(&content_path).await {
                        Ok(url) => {
                            let url = url.trim();
                            // Validate URL before redirect for security
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
                    }
                }
                // PDF, Image, Unknown don't need async loading
                _ => {
                    set_loading.set(false);
                }
            }
        });
    }

    // Handle keyboard events for closing
    let handle_keydown = move |ev: ev::KeyboardEvent| match ev.key().as_str() {
        "q" | "Escape" => {
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
    let filename = breadcrumb_segments
        .last()
        .map(|s| s.to_string())
        .unwrap_or_default();

    // For header actions
    let header_link_url = content_url.clone();

    // More menu state
    let (more_menu_open, set_more_menu_open) = signal(false);

    // Placeholder handlers for menu items (UI only)
    let on_edit = move |_: leptos::ev::MouseEvent| {
        set_more_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Edit clicked".into());
    };

    let on_font_increase = move |_: leptos::ev::MouseEvent| {
        set_more_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Font increase clicked".into());
    };

    let on_font_decrease = move |_: leptos::ev::MouseEvent| {
        set_more_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Font decrease clicked".into());
    };

    let on_share = move |_: leptos::ev::MouseEvent| {
        set_more_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Share clicked".into());
    };

    let on_download = move |_: leptos::ev::MouseEvent| {
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
                // Back button (left) - same style as Explorer nav buttons
                <div class=css::navButtons>
                    <button
                        class=css::navButton
                        on:click=move |_| on_close.run(())
                        title="Back (Esc)"
                    >
                        <Icon icon=ic::CHEVRON_LEFT />
                    </button>
                </div>

                // Breadcrumb path (center) - same as Explorer
                <nav class=css::breadcrumb>
                    {breadcrumb_segments.iter().enumerate().map(|(idx, segment)| {
                        let is_last = idx == breadcrumb_segments.len() - 1;
                        let is_home = *segment == "~";

                        // Build target path for navigation (same logic as Explorer)
                        let target_path = if is_home {
                            VirtualPath::home()
                        } else if breadcrumb_segments[0] == "~" {
                            // Home-relative path: use resolve() to properly expand ~
                            let relative_path = breadcrumb_segments[1..=idx].join("/");
                            VirtualPath::home().resolve(&relative_path)
                        } else {
                            // Absolute path
                            VirtualPath::new(format!("/{}", breadcrumb_segments[0..=idx].join("/")))
                        };

                        // Use file icon only for last segment, folder for directories
                        let icon = if is_home {
                            ic::HOME
                        } else if is_last {
                            file_icon
                        } else {
                            ic::FOLDER
                        };

                        let segment_class = if is_last {
                            format!("{} {}", css::breadcrumbSegment, css::breadcrumbSegmentCurrent)
                        } else {
                            css::breadcrumbSegment.to_string()
                        };

                        let segment_str = segment.to_string();

                        view! {
                            <>
                                {if idx > 0 {
                                    Some(view! { <span class=css::breadcrumbSeparator><Icon icon=ic::CHEVRON_RIGHT /></span> })
                                } else {
                                    None
                                }}
                                <button
                                    class=segment_class
                                    on:click=move |_| {
                                        if !is_last {
                                            // Close reader and navigate to directory
                                            on_close.run(());
                                            ctx.navigate_to(target_path.clone());
                                        }
                                    }
                                    disabled=is_last
                                >
                                    <span class=css::breadcrumbIcon><Icon icon=icon /></span>
                                    {segment_str}
                                </button>
                            </>
                        }
                    }).collect_view()}
                </nav>

                // Action buttons (right)
                <div class=css::headerActions>
                    // Open in new tab
                    <a
                        href=header_link_url.clone()
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
                                    href=header_link_url.clone()
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
                            match file_type.clone() {
                                FileType::Markdown => {
                                    view! {
                                        <div class=css::markdown inner_html=content />
                                    }.into_any()
                                }
                                FileType::Pdf => {
                                    view! {
                                        <iframe
                                            src=pdf_viewer_url.clone()
                                            class=css::pdfViewer
                                            title="PDF Viewer"
                                        />
                                    }.into_any()
                                }
                                FileType::Image => {
                                    view! {
                                        <div class=css::imageContainer>
                                            <img src=content_url.clone() alt=filename.clone() class=css::image />
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
                                        <div class=css::error>
                                            <p>"Unsupported file type"</p>
                                        </div>
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
