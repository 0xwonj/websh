//! Bottom sheet component for file preview (mobile).
//!
//! Displays file content preview in a draggable bottom sheet.
//! - Markdown: Rendered HTML with full content
//! - Text files: Raw text with scrolling
//! - Images: Thumbnail
//! - Encrypted: Lock icon with decrypt prompt

#![allow(dead_code)]

use leptos::prelude::*;
use leptos_icons::Icon;

use crate::app::AppContext;
use crate::components::icons as ic;
use crate::components::terminal::RouteContext;
use crate::models::{AppRoute, FileType, FsEntry, SheetState};
use crate::utils::{fetch_content, markdown_to_html};

stylance::import_crate_style!(css, "src/components/explorer/sheet.module.css");

/// Fetched content for preview.
#[derive(Clone)]
enum PreviewContent {
    /// Rendered HTML from markdown
    Html(String),
    /// Raw text content
    Text(String),
}

#[component]
pub fn BottomSheet() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    let sheet_state = ctx.explorer.sheet_state;
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

    let close_sheet = move |_: leptos::ev::MouseEvent| {
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

    let sheet_class = Signal::derive(move || match sheet_state.get() {
        SheetState::Closed => css::sheetClosed.to_string(),
        SheetState::Preview => css::sheetPreview.to_string(),
        SheetState::Expanded => css::sheetExpanded.to_string(),
    });

    view! {
        <div class=move || format!("{} {}", css::sheet, sheet_class.get())>
            // Drag handle
            <div class=css::handle>
                <div class=css::handleBar></div>
            </div>

            // Header
            <div class=css::sheetHeader>
                <span class=css::filename>{move || filename.get()}</span>
                <div class=css::sheetActions>
                    <OpenFileButton
                        selected_file=selected_file
                        route_ctx=route_ctx
                        is_encrypted=is_encrypted
                    />
                    <Show when=move || is_encrypted.get()>
                        <button class=css::decryptButton title="Decrypt file">
                            "Decrypt"
                        </button>
                    </Show>
                    <button class=css::closeButton on:click=close_sheet title="Close">
                        <Icon icon=ic::CLOSE />
                    </button>
                </div>
            </div>

            // Content
            <div class=css::sheetContent>
                {move || {
                    let encrypted = is_encrypted.get();
                    let ftype = file_type.get();

                    if encrypted {
                        view! {
                            <div class=css::encryptedInfo>
                                <span class=css::lockIcon><Icon icon=ic::LOCK /></span>
                                <p>"This file is encrypted"</p>
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
                                {move || file_meta.get().map(|(desc, _, _)| view! {
                                    <p class=css::imageDesc>{desc}</p>
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
        </div>
    }
}

/// Open file button component.
#[component]
fn OpenFileButton(
    selected_file: RwSignal<Option<String>>,
    route_ctx: RouteContext,
    is_encrypted: Signal<bool>,
) -> impl IntoView {
    view! {
        <Show when=move || !is_encrypted.get()>
            <button
                class=css::actionButton
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
                title="Open file"
            >
                "Open"
            </button>
        </Show>
    }
}
