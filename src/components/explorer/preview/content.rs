//! Shared preview content components.
//!
//! These components are used by both PreviewPanel (desktop) and BottomSheet (mobile).
//! CSS classes are passed as props to allow each container to use its own CSS module.

use leptos::prelude::*;
use leptos_icons::Icon;

use super::{DirMeta, FileMeta, PreviewContent, PreviewData};
use crate::components::icons as ic;
use crate::components::terminal::RouteContext;
use crate::models::{AppRoute, FileType, Selection};
use crate::utils::format::format_size;

/// CSS class names for preview components.
///
/// Each platform (panel/sheet) provides its own CSS module classes.
#[derive(Clone, Copy)]
pub struct PreviewStyles {
    // Directory preview
    pub dir_preview: &'static str,
    pub dir_icon: &'static str,
    pub dir_title: &'static str,
    pub dir_description: &'static str,
    pub dir_info: &'static str,
    pub dir_details: &'static str,
    pub dir_tags: &'static str,
    pub dir_tag: &'static str,
    pub dir_thumbnail: Option<&'static str>,
    // Encrypted preview
    pub encrypted: &'static str,
    pub lock_icon: &'static str,
    pub encrypted_text: &'static str,
    // Image preview
    pub image_preview: &'static str,
    pub thumbnail: &'static str,
    pub image_desc: &'static str,
    pub image_size: Option<&'static str>,
    // Text preview
    pub text_preview: &'static str,
    pub preview_text: &'static str,
    pub loading: &'static str,
    pub no_preview: &'static str,
    pub error: &'static str,
    pub description: &'static str,
    pub markdown: &'static str,
    // Common
    pub hint: &'static str,
}

/// Preview body content (directory, encrypted, image, or text).
#[component]
pub fn PreviewBody(
    data: PreviewData,
    styles: PreviewStyles,
    #[prop(default = "Double-click to open")] dir_hint: &'static str,
) -> impl IntoView {
    view! {
        {move || {
            let is_directory = data.is_dir.get();
            let encrypted = data.is_encrypted.get();
            let ftype = data.file_type.get();

            if is_directory {
                view! {
                    <DirectoryPreview
                        dir_meta=data.dir_meta
                        hint=dir_hint
                        styles=styles
                    />
                }.into_any()
            } else if encrypted {
                view! { <EncryptedPreview styles=styles /> }.into_any()
            } else if ftype == FileType::Image {
                view! {
                    <ImagePreview
                        image_url=data.image_url
                        item_name=data.item_name
                        file_meta=data.file_meta
                        styles=styles
                    />
                }.into_any()
            } else {
                let meta_desc = data.file_meta.get()
                    .map(|(desc, _, _)| desc)
                    .filter(|d| !d.is_empty());
                view! {
                    <TextPreview
                        content=data.content
                        meta_desc=meta_desc
                        styles=styles
                    />
                }.into_any()
            }
        }}
    }
}

/// Directory preview content.
#[component]
fn DirectoryPreview(
    dir_meta: Signal<Option<DirMeta>>,
    hint: &'static str,
    styles: PreviewStyles,
) -> impl IntoView {
    view! {
        <div class=styles.dir_preview>
            {move || dir_meta.get().map(|meta| {
                let has_thumbnail = meta.thumbnail.is_some() && styles.dir_thumbnail.is_some();
                let tags = meta.tags.clone();
                let tags_for_check = tags.clone();

                view! {
                    // Thumbnail (if available)
                    {meta.thumbnail.clone().and_then(|thumb| {
                        styles.dir_thumbnail.map(|class| view! {
                            <img src=thumb alt="" class=class />
                        })
                    })}

                    // Icon (only if no thumbnail)
                    <Show when=move || !has_thumbnail>
                        <span class=styles.dir_icon><Icon icon=ic::FOLDER /></span>
                    </Show>

                    // Title
                    <p class=styles.dir_title>{meta.title.clone()}</p>

                    // Description
                    {meta.description.clone().map(|desc| view! {
                        <p class=styles.dir_description>{desc}</p>
                    })}

                    // Item counts
                    {meta.counts.map(|(files, dirs)| view! {
                        <p class=styles.dir_info>
                            {format!("{} items", files + dirs)}
                        </p>
                        <p class=styles.dir_details>
                            {format!("{} folders, {} files", dirs, files)}
                        </p>
                    })}

                    // Tags
                    <Show when=move || !tags_for_check.is_empty()>
                        <div class=styles.dir_tags>
                            {tags.iter().map(|tag: &String| view! {
                                <span class=styles.dir_tag>{tag.clone()}</span>
                            }).collect_view()}
                        </div>
                    </Show>

                    <p class=styles.hint>{hint}</p>
                }
            })}
        </div>
    }
}

/// Encrypted file preview.
#[component]
fn EncryptedPreview(styles: PreviewStyles) -> impl IntoView {
    view! {
        <div class=styles.encrypted>
            <span class=styles.lock_icon><Icon icon=ic::LOCK /></span>
            <p class=styles.encrypted_text>"This file is encrypted"</p>
            <p class=styles.hint>"Connect wallet to decrypt"</p>
        </div>
    }
}

/// Image preview with thumbnail.
#[component]
fn ImagePreview(
    image_url: Signal<Option<String>>,
    item_name: Signal<String>,
    file_meta: Signal<Option<FileMeta>>,
    styles: PreviewStyles,
) -> impl IntoView {
    let image_preview_class = styles.image_preview;
    let thumbnail_class = styles.thumbnail;
    let image_desc_class = styles.image_desc;
    let image_size_class = styles.image_size;

    view! {
        <div class=image_preview_class>
            {move || image_url.get().map(|url| {
                view! {
                    <img
                        src=url
                        alt=item_name.get()
                        class=thumbnail_class
                    />
                }
            })}
            {move || file_meta.get().map(|(desc, size, _)| {
                view! {
                    <p class=image_desc_class>{desc}</p>
                    {image_size_class.map(|class| view! {
                        <p class=class>
                            {format_size(size, false)}
                        </p>
                    })}
                }
            })}
        </div>
    }
}

/// Text/Markdown preview.
#[component]
fn TextPreview(
    content: LocalResource<Option<PreviewContent>>,
    meta_desc: Option<String>,
    styles: PreviewStyles,
) -> impl IntoView {
    let text_preview_class = styles.text_preview;
    let loading_class = styles.loading;
    let markdown_class = styles.markdown;
    let preview_text_class = styles.preview_text;
    let no_preview_class = styles.no_preview;
    let error_class = styles.error;
    let description_class = styles.description;
    let hint_class = styles.hint;

    view! {
        <div class=text_preview_class>
            <Suspense fallback=move || view! { <div class=loading_class>"Loading..."</div> }>
                {move || {
                    let desc = meta_desc.clone();
                    content.get().map(move |c| {
                        match c {
                            Some(PreviewContent::Html(html)) => view! {
                                <div class=markdown_class inner_html=html />
                            }.into_any(),
                            Some(PreviewContent::Text(text)) => view! {
                                <pre class=preview_text_class>{text}</pre>
                            }.into_any(),
                            Some(PreviewContent::Error(err)) => view! {
                                <div class=error_class>
                                    <span class=styles.lock_icon><Icon icon=ic::WARNING /></span>
                                    <p class=hint_class>"Failed to load preview"</p>
                                    <p class=description_class>{err}</p>
                                </div>
                            }.into_any(),
                            None => view! {
                                <div class=no_preview_class>
                                    {desc.clone().map(|d| view! {
                                        <p class=description_class>{d}</p>
                                    })}
                                    <p class=hint_class>"Preview not available"</p>
                                </div>
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}

/// Open in reader button.
///
/// Shared by both PreviewPanel and BottomSheet.
#[component]
pub fn OpenButton(
    selection: RwSignal<Option<Selection>>,
    is_encrypted: Signal<bool>,
    class: &'static str,
    #[prop(default = "Open")] label: &'static str,
) -> impl IntoView {
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    let show_button = Signal::derive(move || {
        selection
            .get()
            .map(|s| !s.is_dir && !is_encrypted.get())
            .unwrap_or(false)
    });

    view! {
        <Show when=move || show_button.get()>
            <button
                class=class
                on:click=move |_| {
                    if let Some(s) = selection.get()
                        && !s.is_dir
                    {
                        let route = route_ctx.0.get();
                        let mount = route.mount().cloned()
                            .unwrap_or_else(crate::config::default_mount);
                        AppRoute::Read { mount, path: s.path }.push();
                    }
                }
                title="Open file"
                aria-label="Open file in reader"
            >
                {label}
            </button>
        </Show>
    }
}
