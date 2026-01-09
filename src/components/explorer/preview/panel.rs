//! Desktop preview panel component.
//!
//! Displays file/directory content preview in a side panel (Finder style).
//! Hidden on mobile via CSS (BottomSheet is used instead).

use leptos::prelude::*;
use leptos_icons::Icon;

use super::{OpenButton, PreviewBody, PreviewData, PreviewStyles};
use crate::components::icons as ic;

stylance::import_crate_style!(css, "src/components/explorer/preview.module.css");
stylance::import_crate_style!(md_css, "src/components/explorer/markdown.module.css");

/// CSS styles for the desktop preview panel.
fn panel_styles() -> PreviewStyles {
    PreviewStyles {
        dir_preview: css::dirPreview,
        dir_icon: css::dirIcon,
        dir_title: css::dirTitle,
        dir_description: css::dirDescription,
        dir_info: css::dirInfo,
        dir_details: css::dirDetails,
        dir_tags: css::dirTags,
        dir_tag: css::dirTag,
        dir_thumbnail: Some(css::dirThumbnail),
        encrypted: css::encrypted,
        lock_icon: css::lockIcon,
        encrypted_text: css::encryptedText,
        image_preview: css::imagePreview,
        thumbnail: css::thumbnail,
        image_desc: css::imageDesc,
        image_size: Some(css::imageSize),
        text_preview: css::textPreview,
        preview_text: css::previewText,
        loading: css::loading,
        no_preview: css::noPreview,
        error: css::error,
        description: css::description,
        markdown: md_css::markdown,
        hint: css::hint,
    }
}

/// Desktop preview panel component.
#[component]
pub fn PreviewPanel(data: PreviewData) -> impl IntoView {
    view! {
        <aside class=css::panel role="complementary" aria-label="File preview">
            <PreviewHeader
                item_name=data.item_name
                is_encrypted=data.is_encrypted
                on_close=move |_| data.close()
            />

            <div class=css::content>
                <PreviewBody
                    data=data
                    styles=panel_styles()
                    dir_hint="Double-click to open"
                />
            </div>

            <OpenButton
                selection=data.selection
                is_encrypted=data.is_encrypted
                class=css::openBar
                label="Open in reader"
            />
        </aside>
    }
}

/// Preview header with filename and actions.
#[component]
fn PreviewHeader(
    item_name: Signal<String>,
    is_encrypted: Signal<bool>,
    on_close: impl Fn(leptos::ev::MouseEvent) + 'static,
) -> impl IntoView {
    view! {
        <header class=css::header>
            <span class=css::filename>{move || item_name.get()}</span>
            <div class=css::actions>
                <Show when=move || is_encrypted.get()>
                    <button
                        class=css::decryptButton
                        title="Decrypt file"
                        aria-label="Decrypt this encrypted file"
                    >
                        "Decrypt"
                    </button>
                </Show>
                <button
                    class=css::closeButton
                    on:click=on_close
                    title="Close preview"
                    aria-label="Close preview panel"
                >
                    <Icon icon=ic::CLOSE />
                </button>
            </div>
        </header>
    }
}
