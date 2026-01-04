use leptos::{ev, prelude::*};
use wasm_bindgen_futures::spawn_local;

use crate::config::CONTENT_BASE_URL;
use crate::models::FileType;
use crate::utils::{fetch_content, markdown_to_html, validate_redirect_url, UrlValidation};

stylance::import_crate_style!(css, "src/components/reader/reader.module.css");

#[component]
pub fn Reader(
    #[prop(into)] content_path: String,
    #[prop(into)] title: String,
    on_close: Callback<()>,
) -> impl IntoView {
    let file_type = FileType::from_path(&content_path);
    let content_url = format!("{}/{}", CONTENT_BASE_URL, content_path);

    // For PDF, use Google Docs Viewer to render inline
    let pdf_viewer_url = if file_type == FileType::Pdf {
        let encoded = js_sys::encode_uri_component(&content_url);
        format!("https://docs.google.com/viewer?url={}&embedded=true", encoded)
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
                FileType::Markdown => {
                    match fetch_content(&content_path).await {
                        Ok(md) => {
                            let html = markdown_to_html(&md);
                            set_content.set(html);
                            set_loading.set(false);
                        }
                        Err(e) => {
                            set_error.set(Some(e.to_string()));
                            set_loading.set(false);
                        }
                    }
                }
                FileType::Link => {
                    match fetch_content(&content_path).await {
                        Ok(url) => {
                            let url = url.trim();
                            // Validate URL before redirect for security
                            match validate_redirect_url(url) {
                                UrlValidation::Valid(safe_url) => {
                                    if let Some(window) = web_sys::window()
                                        && window.location().set_href(&safe_url).is_err() {
                                            set_error.set(Some("Failed to redirect".to_string()));
                                            set_loading.set(false);
                                        }
                                }
                                UrlValidation::Invalid(err) => {
                                    set_error.set(Some(format!(
                                        "Redirect blocked: {}",
                                        err
                                    )));
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
    let handle_keydown = move |ev: ev::KeyboardEvent| {
        match ev.key().as_str() {
            "q" | "Escape" => {
                on_close.run(());
            }
            _ => {}
        }
    };

    // Focus the container on mount for keyboard events
    let container_ref = NodeRef::<leptos::html::Div>::new();
    Effect::new(move || {
        if let Some(el) = container_ref.get() {
            let _ = el.focus();
        }
    });

    let title_for_header = title.clone();
    let title_for_img = title;

    // For header "open in new tab" link (PDF only)
    let is_pdf = file_type == FileType::Pdf;
    let header_link_url = content_url.clone();

    view! {
        <div
            node_ref=container_ref
            tabindex="-1"
            class=format!("{} scrollbar-thin", css::reader)
            on:keydown=handle_keydown
        >
            // Header
            <div class=css::header>
                <div class=css::titleSection>
                    <span class=css::titleLabel>"Reading:"</span>
                    <span class=css::title>{title_for_header}</span>
                </div>
                <div class=css::headerActions>
                    {is_pdf.then(|| view! {
                        <a
                            href=header_link_url.clone()
                            target="_blank"
                            rel="noopener noreferrer"
                            class=css::openLink
                        >
                            "Open in new tab"
                        </a>
                        <span class=css::separator>"|"</span>
                    })}
                    <span class=css::hint>
                        "Press "<span class=css::hintKey>"q"</span>" or "<span class=css::hintKey>"Esc"</span>" to close"
                    </span>
                </div>
            </div>

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
                                            <img src=content_url.clone() alt=title_for_img.clone() class=css::image />
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
