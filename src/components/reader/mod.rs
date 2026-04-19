//! Reader component for displaying file content.
//!
//! HackMD-style editor with three view modes: Read, Write, and Split.
//!
//! ## Component Structure
//! - `Reader`: Container component with view mode toggle and header
//! - `Editor`: Markdown editor with line numbers and formatting toolbar
//! - `Preview`: Rendered markdown/content display

#![allow(dead_code)]

mod editor;
mod preview;

use leptos::{ev, prelude::*};
use leptos_icons::Icon;
use wasm_bindgen_futures::spawn_local;

use crate::app::AppContext;
use crate::components::icons as ic;
use crate::components::Breadcrumb;
use crate::core::storage::{ChangeType, save_pending_changes};
use crate::models::{AppRoute, FileMetadata, FileType, ReaderViewMode};
use crate::utils::{
    UrlValidation, fetch_content, markdown_to_html, markdown_to_html_with_images,
    validate_redirect_url,
};

use editor::{Editor, ImageUpload};
use preview::{Preview, RawPreview};

stylance::import_crate_style!(css, "src/components/reader/reader.module.css");

/// Reader component for displaying file content.
#[component]
pub fn Reader(route: Memo<AppRoute>, on_close: Callback<()>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    // View mode state - default to Read for viewing
    let (view_mode, set_view_mode) = signal(ReaderViewMode::Read);

    // Derive content path and filename from route
    let content_path = Memo::new(move |_| route.get().path().to_string());
    let filename = Memo::new(move |_| {
        content_path
            .get()
            .rsplit('/')
            .next()
            .unwrap_or("untitled")
            .to_string()
    });

    // Derive file type from content path
    let file_type = Memo::new(move |_| FileType::from_path(&content_path.get()));

    // Derive content URL from route
    let content_url = Memo::new(move |_| {
        route
            .get()
            .content_url()
            .unwrap_or_else(|| content_path.get())
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

    // Content signals
    let (content, set_content) = signal(String::new());
    let edit_content = RwSignal::new(String::new());
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    // Dirty state - tracks if content has been modified since last save
    let (original_content, set_original_content) = signal(String::new());
    let is_dirty = Memo::new(move |_| edit_content.get() != original_content.get());

    // Close confirmation dialog state
    let (show_close_confirm, set_show_close_confirm) = signal(false);

    // Live preview HTML with pending image resolution
    let preview_html = Memo::new(move |_| {
        if file_type.get() == FileType::Markdown {
            // Get pending binary files as data URLs for preview
            let image_urls = ctx.fs.pending().with(|p| p.get_all_binary_data_urls());
            markdown_to_html_with_images(&edit_content.get(), &image_urls)
        } else {
            edit_content.get()
        }
    });

    // Load content
    Effect::new(move |_| {
        let url = content_url.get();
        let ft = file_type.get();
        let path = content_path.get();

        set_error.set(None);
        set_content.set(String::new());
        edit_content.set(String::new());
        set_original_content.set(String::new());

        // Check if there's pending content for this file
        let pending_content = ctx.fs.pending().with(|p| {
            p.get(&path).and_then(|change| match &change.change_type {
                ChangeType::CreateFile { content, .. } => Some(content.clone()),
                ChangeType::UpdateFile { content, .. } => Some(content.clone()),
                _ => None,
            })
        });

        match ft {
            FileType::Markdown | FileType::Link | FileType::Unknown => {
                set_loading.set(true);

                // If we have pending content, use it instead of fetching
                if let Some(pending) = pending_content {
                    let html = if ft == FileType::Markdown {
                        markdown_to_html(&pending)
                    } else {
                        pending.clone()
                    };
                    edit_content.set(pending.clone());
                    set_original_content.set(pending); // Mark as "saved" since it's from pending
                    set_content.set(html);
                    set_loading.set(false);
                    return;
                }

                spawn_local(async move {
                    match ft {
                        FileType::Markdown => match fetch_content(&url).await {
                            Ok(md) => {
                                let html = markdown_to_html(&md);
                                edit_content.set(md.clone());
                                set_original_content.set(md);
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
                                edit_content.set(text.clone());
                                set_original_content.set(text.clone());
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
            _ => {
                set_loading.set(false);
            }
        }
    });

    // Save function - stores content in pending changes
    let save_content = move || {
        let path = content_path.get();
        let current_content = edit_content.get();

        web_sys::console::log_1(&format!("save_content: path={}", path).into());
        web_sys::console::log_1(&format!("save_content: content length={}", current_content.len()).into());

        // Check if this is a new file or update
        let is_new_file = original_content.get().is_empty();
        web_sys::console::log_1(&format!("save_content: is_new_file={}", is_new_file).into());

        ctx.fs.pending().update(|pending| {
            web_sys::console::log_1(&"save_content: inside pending update".into());
            if is_new_file {
                pending.add(
                    path.clone(),
                    ChangeType::CreateFile {
                        content: current_content.clone(),
                        description: String::new(),
                        meta: FileMetadata::default(),
                    },
                );
            } else {
                pending.add(
                    path.clone(),
                    ChangeType::UpdateFile {
                        content: current_content.clone(),
                        description: None,
                    },
                );
            }
            web_sys::console::log_1(&format!("save_content: pending count={}", pending.len()).into());
        });

        // Save to localStorage
        ctx.fs.pending().with(|p| {
            web_sys::console::log_1(&format!("save_content: saving to localStorage, count={}", p.len()).into());
            match save_pending_changes(p) {
                Ok(_) => web_sys::console::log_1(&"save_content: localStorage save OK".into()),
                Err(e) => web_sys::console::log_1(&format!("save_content: localStorage error: {:?}", e).into()),
            }
        });

        // Update original content to mark as saved
        set_original_content.set(current_content.clone());
        web_sys::console::log_1(&"save_content: done".into());
    };

    // Keyboard handler
    let handle_keydown = move |ev: ev::KeyboardEvent| {
        let key = ev.key();

        // Ctrl+S or Cmd+S to save
        if (ev.ctrl_key() || ev.meta_key()) && key == "s" {
            ev.prevent_default();
            if is_dirty.get() {
                save_content();
            }
            return;
        }

        // Escape to close (with confirmation if dirty)
        if key == "Escape" {
            ev.prevent_default();
            web_sys::console::log_1(&format!("Reader: Escape pressed, is_dirty={}", is_dirty.get()).into());
            if is_dirty.get() {
                web_sys::console::log_1(&"Reader: showing close confirm dialog".into());
                set_show_close_confirm.set(true);
            } else {
                on_close.run(());
            }
        }
    };

    // Focus container on mount
    let container_ref = NodeRef::<leptos::html::Div>::new();
    Effect::new(move || {
        if let Some(el) = container_ref.get() {
            let _ = el.focus();
        }
    });

    // Check if file supports editing
    let supports_editing = Memo::new(move |_| {
        matches!(file_type.get(), FileType::Markdown | FileType::Unknown)
    });

    // Content as signal for RawPreview
    let content_signal = Signal::derive(move || content.get());

    // Image upload callback - stores image in pending changes and returns path
    let on_image_upload = Callback::new(move |upload: ImageUpload| {
        // Generate path in .assets folder
        let path = format!("/.assets/{}", upload.filename);

        // Add to pending changes
        ctx.fs.pending().update(|pending| {
            pending.add(
                path.clone(),
                ChangeType::CreateBinaryFile {
                    content_base64: upload.content_base64,
                    mime_type: upload.mime_type,
                    description: format!("Image: {}", upload.filename),
                    meta: FileMetadata::default(),
                },
            );
        });

        // Save to localStorage
        ctx.fs.pending().with(|p| {
            let _ = save_pending_changes(p);
        });

        path
    });

    // Save callback for Editor
    let on_save = Callback::new(move |()| {
        web_sys::console::log_1(&format!("Reader on_save: is_dirty={}", is_dirty.get()).into());
        if is_dirty.get() {
            web_sys::console::log_1(&"Reader: calling save_content".into());
            save_content();
        }
    });

    view! {
        <div
            node_ref=container_ref
            tabindex="-1"
            class=css::reader
            on:keydown=handle_keydown
        >
            // Header with view toggle (left), breadcrumb (center), actions (right)
            <header class=css::header>
                // Left: Back button + View mode toggle
                <div class=css::headerLeft>
                    <button
                        class=css::headerButton
                        on:click=move |_| {
                            if is_dirty.get() {
                                set_show_close_confirm.set(true);
                            } else {
                                on_close.run(());
                            }
                        }
                        title="Close (Esc)"
                    >
                        <Icon icon=ic::CHEVRON_LEFT />
                    </button>

                    // Segmented view mode toggle (only for editable files)
                    <Show when=move || supports_editing.get()>
                        <div class=css::segmentedToggle>
                            <button
                                class=move || if view_mode.get() == ReaderViewMode::Write {
                                    format!("{} {}", css::segmentButton, css::segmentButtonActive)
                                } else {
                                    css::segmentButton.to_string()
                                }
                                on:click=move |_| set_view_mode.set(ReaderViewMode::Write)
                                title="Write"
                            >
                                <Icon icon=ic::EDIT />
                            </button>
                            <button
                                class=move || if view_mode.get() == ReaderViewMode::Split {
                                    format!("{} {}", css::segmentButton, css::segmentButtonActive)
                                } else {
                                    css::segmentButton.to_string()
                                }
                                on:click=move |_| set_view_mode.set(ReaderViewMode::Split)
                                title="Both"
                            >
                                <Icon icon=ic::COLUMNS />
                            </button>
                            <button
                                class=move || if view_mode.get() == ReaderViewMode::Read {
                                    format!("{} {}", css::segmentButton, css::segmentButtonActive)
                                } else {
                                    css::segmentButton.to_string()
                                }
                                on:click=move |_| set_view_mode.set(ReaderViewMode::Read)
                                title="Read"
                            >
                                <Icon icon=ic::EYE />
                            </button>
                        </div>
                    </Show>
                </div>

                // Center: Breadcrumb (absolute positioned)
                <div class=css::headerCenter>
                    <Breadcrumb />
                </div>

                // Right: Action buttons
                <div class=css::headerRight>
                    <button class=css::headerButton title="Search">
                        <Icon icon=ic::SEARCH />
                    </button>
                    <button class=css::headerButton title="More">
                        <Icon icon=ic::MORE />
                    </button>
                </div>
            </header>

            // Content area
            <Show
                when=move || loading.get()
                fallback=move || {
                    if let Some(err) = error.get() {
                        view! {
                            <div class=css::contentArea>
                                <div class=css::error>
                                    <p class=css::errorTitle>"Error loading content:"</p>
                                    <p>{err}</p>
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        match (view_mode.get(), file_type.get()) {
                            // Split view
                            (ReaderViewMode::Split, FileType::Markdown | FileType::Unknown) => {
                                view! {
                                    <div class=css::splitContainer>
                                        <div class=css::editorPane>
                                            <Editor
                                                content=edit_content
                                                show_toolbar=true
                                                on_image_upload=on_image_upload
                                                on_save=on_save
                                            />
                                        </div>
                                        <div class=css::previewPane>
                                            <Preview html=preview_html />
                                        </div>
                                    </div>
                                }.into_any()
                            }
                            // Write mode only
                            (ReaderViewMode::Write, FileType::Markdown | FileType::Unknown) => {
                                view! {
                                    <div class=css::editorFull>
                                        <Editor
                                            content=edit_content
                                            show_toolbar=true
                                            on_image_upload=on_image_upload
                                            on_save=on_save
                                        />
                                    </div>
                                }.into_any()
                            }
                            // Read mode for markdown
                            (_, FileType::Markdown) => {
                                view! {
                                    <div class=css::contentArea>
                                        <Preview html=preview_html />
                                    </div>
                                }.into_any()
                            }
                            // PDF viewer
                            (_, FileType::Pdf) => {
                                view! {
                                    <div class=css::contentArea>
                                        <iframe
                                            src=pdf_viewer_url.get()
                                            class=css::pdfViewer
                                            title="PDF Viewer"
                                        />
                                    </div>
                                }.into_any()
                            }
                            // Image viewer
                            (_, FileType::Image) => {
                                view! {
                                    <div class=css::contentArea>
                                        <div class=css::imageContainer>
                                            <img src=content_url.get() alt=filename.get() class=css::image />
                                        </div>
                                    </div>
                                }.into_any()
                            }
                            // Link redirect
                            (_, FileType::Link) => {
                                view! {
                                    <div class=css::contentArea>
                                        <div class=css::loading>"Redirecting..."</div>
                                    </div>
                                }.into_any()
                            }
                            // Unknown file type - raw text
                            (_, FileType::Unknown) => {
                                view! {
                                    <div class=css::contentArea>
                                        <RawPreview content=content_signal />
                                    </div>
                                }.into_any()
                            }
                        }
                    }
                }
            >
                <div class=css::contentArea>
                    <div class=css::loading>"Loading..."</div>
                </div>
            </Show>

            // Close confirmation dialog
            <Show when=move || show_close_confirm.get()>
                <div class=css::overlay>
                    <div class=css::dialog>
                        <p class=css::dialogMessage>"Unsaved changes will be lost."</p>
                        <div class=css::dialogButtons>
                            <button
                                class=css::dialogButtonSecondary
                                on:click=move |_| set_show_close_confirm.set(false)
                            >
                                "Cancel"
                            </button>
                            <button
                                class=css::dialogButtonPrimary
                                on:click=move |_| {
                                    save_content();
                                    on_close.run(());
                                }
                            >
                                "Save"
                            </button>
                            <button
                                class=css::dialogButtonDanger
                                on:click=move |_| on_close.run(())
                            >
                                "Discard"
                            </button>
                        </div>
                    </div>
                </div>
            </Show>
        </div>
    }
}
