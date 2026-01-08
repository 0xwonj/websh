//! Preview panel component for desktop file preview.
//!
//! Displays file content preview in a side panel (Midnight Commander style).
//! - Markdown: Rendered HTML with full content
//! - Text files: Raw text with scrolling
//! - Images: Thumbnail
//! - Encrypted: Lock icon with decrypt prompt
//!
//! On mobile, BottomSheet is used instead.

use leptos::prelude::*;
use leptos_icons::Icon;

use crate::app::AppContext;
use crate::components::icons as ic;
use crate::components::terminal::RouteContext;
use crate::models::{AppRoute, FileType, FsEntry};
use crate::utils::{fetch_content, markdown_to_html};

stylance::import_crate_style!(css, "src/components/explorer/preview.module.css");

/// Fetched content for preview.
#[derive(Clone)]
enum PreviewContent {
    /// Rendered HTML from markdown
    Html(String),
    /// Raw text content
    Text(String),
}

/// Desktop preview panel component.
///
/// Shows file preview on the right side of the file list.
/// Hidden on mobile via CSS (BottomSheet is used instead).
#[component]
pub fn PreviewPanel() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    let selected_file = ctx.explorer.selected_file;

    // Extract filename from path
    let filename = Signal::derive(move || {
        selected_file
            .get()
            .and_then(|path| path.rsplit('/').next().map(String::from))
            .unwrap_or_default()
    });

    // Check if file is encrypted
    let is_encrypted = Signal::derive(move || {
        selected_file
            .get()
            .map(|path| {
                ctx.fs.with(|fs| {
                    fs.get_entry(&path)
                        .map(|entry| entry.is_encrypted())
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    });

    // Get content path for fetching
    let content_path = Signal::derive(move || {
        selected_file
            .get()
            .and_then(|path| ctx.fs.with(|fs| fs.get_file_content_path(&path)))
    });

    // Detect file type
    let file_type = Signal::derive(move || {
        content_path
            .get()
            .map(|p| FileType::from_path(&p))
            .unwrap_or(FileType::Unknown)
    });

    // Get file metadata
    let file_meta = Signal::derive(move || {
        selected_file.get().and_then(|path| {
            ctx.fs.with(|fs| {
                fs.get_entry(&path).and_then(|entry| match entry {
                    FsEntry::File {
                        meta, description, ..
                    } => Some((description.clone(), meta.size, meta.modified)),
                    _ => None,
                })
            })
        })
    });

    // Fetch content for preview
    let preview_content = LocalResource::new(move || {
        let path = content_path.get();
        let ftype = file_type.get();
        let encrypted = is_encrypted.get();
        // Get base URL from current route's mount
        let route = route_ctx.0.get();
        let base_url = route.mount().map(|m| m.base_url()).unwrap_or_else(|| {
            crate::config::configured_mounts()
                .into_iter()
                .next()
                .map(|m| m.base_url())
                .unwrap_or_default()
        });
        async move {
            if encrypted {
                return None;
            }
            let path = path?;
            let url = format!("{}/{}", base_url, path);
            match ftype {
                FileType::Markdown => {
                    let content = fetch_content(&url).await.ok()?;
                    let html = markdown_to_html(&content);
                    Some(PreviewContent::Html(html))
                }
                FileType::Unknown => {
                    let content = fetch_content(&url).await.ok()?;
                    Some(PreviewContent::Text(content))
                }
                _ => None,
            }
        }
    });

    let close_preview = move |_: leptos::ev::MouseEvent| {
        ctx.explorer.clear_selection();
    };

    // Build image URL for thumbnails
    let image_url = Signal::derive(move || {
        let route = route_ctx.0.get();
        let base_url = route.mount().map(|m| m.base_url()).unwrap_or_else(|| {
            crate::config::configured_mounts()
                .into_iter()
                .next()
                .map(|m| m.base_url())
                .unwrap_or_default()
        });
        content_path.get().map(|p| format!("{}/{}", base_url, p))
    });

    view! {
        <aside class=css::panel>
            // Header
            <header class=css::header>
                <span class=css::filename>{move || filename.get()}</span>
                <div class=css::actions>
                    <Show when=move || is_encrypted.get()>
                        <button class=css::decryptButton title="Decrypt file">
                            "Decrypt"
                        </button>
                    </Show>
                    <button class=css::closeButton on:click=close_preview title="Close preview">
                        <Icon icon=ic::CLOSE />
                    </button>
                </div>
            </header>

            // Content area (text selectable)
            <div class=css::content>
                {move || {
                    let encrypted = is_encrypted.get();
                    let ftype = file_type.get();

                    if encrypted {
                        // Encrypted file
                        view! {
                            <div class=css::encrypted>
                                <span class=css::lockIcon><Icon icon=ic::LOCK /></span>
                                <p class=css::encryptedText>"This file is encrypted"</p>
                                <p class=css::hint>"Connect wallet to decrypt"</p>
                            </div>
                        }.into_any()
                    } else if ftype == FileType::Image {
                        // Image thumbnail
                        view! {
                            <div class=css::imagePreview>
                                {move || image_url.get().map(|url| view! {
                                    <img
                                        src=url
                                        alt=filename.get()
                                        class=css::thumbnail
                                    />
                                })}
                                {move || file_meta.get().map(|(desc, size, _)| view! {
                                    <p class=css::imageDesc>{desc}</p>
                                    <p class=css::imageSize>
                                        {size.map(format_size).unwrap_or_else(|| "-".to_string())}
                                    </p>
                                })}
                            </div>
                        }.into_any()
                    } else {
                        // Text/Markdown preview
                        view! {
                            <div class=css::textPreview>
                                <Suspense fallback=move || view! {
                                    <div class=css::loading>"Loading..."</div>
                                }>
                                    {move || {
                                        preview_content.get().map(|content| {
                                            match content {
                                                Some(PreviewContent::Html(html)) => view! {
                                                    <div class=css::markdown inner_html=html />
                                                }.into_any(),
                                                Some(PreviewContent::Text(text)) => view! {
                                                    <pre class=css::previewText>{text}</pre>
                                                }.into_any(),
                                                None => view! {
                                                    <div class=css::noPreview>
                                                        {move || file_meta.get().map(|(desc, _, _)| view! {
                                                            <p class=css::description>{desc}</p>
                                                        })}
                                                        <p class=css::hint>"Preview not available"</p>
                                                    </div>
                                                }.into_any(),
                                            }
                                        })
                                    }}
                                </Suspense>
                            </div>
                        }.into_any()
                    }
                }}
            </div>

            // Bottom action bar (only for non-encrypted files)
            <OpenButton
                selected_file=selected_file
                route_ctx=route_ctx
                is_encrypted=is_encrypted
            />
        </aside>
    }
}

/// Open in reader button component.
#[component]
fn OpenButton(
    selected_file: RwSignal<Option<String>>,
    route_ctx: RouteContext,
    is_encrypted: Signal<bool>,
) -> impl IntoView {
    view! {
        <Show when=move || !is_encrypted.get()>
            <button
                class=css::openBar
                on:click=move |_| {
                    if let Some(path) = selected_file.get() {
                        // Get mount from current route
                        let route = route_ctx.0.get();
                        let mount = route.mount().cloned().unwrap_or_else(|| {
                            crate::config::configured_mounts()
                                .into_iter()
                                .next()
                                .unwrap()
                        });
                        // selected_file already contains the full relative path
                        let read_route = AppRoute::Read {
                            mount,
                            path,
                        };
                        read_route.push();
                    }
                }
            >
                "Open in reader"
            </button>
        </Show>
    }
}

/// Format file size in human-readable format.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;

    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
