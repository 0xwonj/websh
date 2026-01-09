//! Mobile bottom sheet component for file preview.
//!
//! Displays file/directory content preview in a draggable bottom sheet.
//! Hidden on desktop via CSS (PreviewPanel is used instead).

use leptos::prelude::*;
use leptos_icons::Icon;

use super::{PreviewBody, PreviewData, PreviewStyles};
use crate::components::icons as ic;
use crate::components::terminal::RouteContext;
use crate::models::AppRoute;

stylance::import_crate_style!(css, "src/components/explorer/sheet.module.css");
stylance::import_crate_style!(md_css, "src/components/explorer/markdown.module.css");

/// CSS styles for the mobile bottom sheet.
fn sheet_styles() -> PreviewStyles {
    PreviewStyles {
        dir_preview: css::dirPreview,
        dir_icon: css::dirIcon,
        dir_title: css::dirTitle,
        dir_description: css::dirDescription,
        dir_info: css::dirInfo,
        dir_details: css::dirDetails,
        dir_tags: css::dirTags,
        dir_tag: css::dirTag,
        dir_thumbnail: None, // Sheet doesn't show directory thumbnail
        encrypted: css::encryptedInfo,
        lock_icon: css::lockIcon,
        encrypted_text: "", // Uses p tag styling in encryptedInfo
        image_preview: css::imagePreview,
        thumbnail: css::thumbnail,
        image_desc: css::imageDesc,
        image_size: None, // Sheet doesn't show size
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

/// Mobile bottom sheet component.
#[component]
pub fn BottomSheet(data: PreviewData) -> impl IntoView {
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    let (is_dragging, set_is_dragging) = signal(false);
    let (drag_start_y, set_drag_start_y) = signal(0.0_f64);
    let (drag_offset, set_drag_offset) = signal(0.0_f64);

    // Reset drag state when selection changes
    Effect::new(move |_| {
        data.selection.get();
        set_drag_offset.set(0.0);
    });

    // Navigate to item (file -> reader, dir -> browse)
    let navigate_to_item = move || {
        let Some(selection) = data.selection.get_untracked() else {
            return;
        };

        let route = route_ctx.0.get_untracked();
        let mount = route
            .mount()
            .cloned()
            .unwrap_or_else(crate::config::default_mount);

        if selection.is_dir {
            // Navigate into directory
            route
                .join(selection.path.rsplit('/').next().unwrap_or_default())
                .push();
        } else {
            // Open file in reader
            AppRoute::Read {
                mount,
                path: selection.path,
            }
            .push();
        }
        data.close();
    };

    // Drag start handler (shared logic for touch and mouse)
    let start_drag = move |y: f64| {
        set_is_dragging.set(true);
        set_drag_start_y.set(y);
    };

    // Drag move handler (shared logic)
    let move_drag = move |y: f64| {
        if !is_dragging.get_untracked() {
            return;
        }
        let delta = y - drag_start_y.get_untracked();
        set_drag_offset.set(delta);
    };

    // Drag end handler (shared logic)
    let end_drag = move || {
        if !is_dragging.get_untracked() {
            return;
        }
        set_is_dragging.set(false);

        let offset = drag_offset.get_untracked();

        // Thresholds
        const CLOSE_THRESHOLD: f64 = 50.0; // Drag down to close
        const OPEN_THRESHOLD: f64 = -80.0; // Drag up to open in reader

        if offset > CLOSE_THRESHOLD {
            // Drag down -> close sheet
            data.close();
        } else if offset < OPEN_THRESHOLD {
            // Drag up -> navigate to item (reader or directory)
            navigate_to_item();
        }

        set_drag_offset.set(0.0);
    };

    // Touch event handlers
    let on_touch_start = move |event: leptos::ev::TouchEvent| {
        if let Some(touch) = event.touches().get(0) {
            start_drag(touch.client_y() as f64);
        }
    };

    let on_touch_move = move |event: leptos::ev::TouchEvent| {
        if let Some(touch) = event.touches().get(0) {
            move_drag(touch.client_y() as f64);
        }
    };

    let on_touch_end = move |_: leptos::ev::TouchEvent| {
        end_drag();
    };

    // Mouse event handlers (for desktop/emulation testing)
    let on_mouse_down = move |event: leptos::ev::MouseEvent| {
        event.prevent_default();
        start_drag(event.client_y() as f64);
    };

    let on_mouse_move = move |event: leptos::ev::MouseEvent| {
        move_drag(event.client_y() as f64);
    };

    let on_mouse_up = move |_: leptos::ev::MouseEvent| {
        end_drag();
    };

    let on_mouse_leave = move |_: leptos::ev::MouseEvent| {
        end_drag();
    };

    // Compute inline transform style during drag
    let drag_style = move || {
        if is_dragging.get() {
            let offset = drag_offset.get();
            // Allow dragging in both directions with limits
            let clamped = offset.clamp(-150.0, 200.0);
            format!("transform: translateY({}px);", clamped)
        } else {
            String::new()
        }
    };

    // Compute class including dragging state
    let sheet_class = move || {
        let base = format!("{} {}", css::sheet, css::sheetPreview);
        if is_dragging.get() {
            format!("{} {}", base, css::sheetDragging)
        } else {
            base
        }
    };

    view! {
        <div
            class=sheet_class
            style=drag_style
            role="dialog"
            aria-label="File preview"
        >
            // Drag handle
            <div
                class=css::handle
                on:touchstart=on_touch_start
                on:touchmove=on_touch_move
                on:touchend=on_touch_end
                on:mousedown=on_mouse_down
                on:mousemove=on_mouse_move
                on:mouseup=on_mouse_up
                on:mouseleave=on_mouse_leave
            >
                <div class=css::handleBar></div>
            </div>

            <SheetHeader
                item_name=data.item_name
                is_encrypted=data.is_encrypted
                on_close=move |_| data.close()
            />

            <div class=css::sheetContent>
                <PreviewBody
                    data=data
                    styles=sheet_styles()
                    dir_hint="Double-tap to open"
                />
            </div>
        </div>
    }
}

/// Sheet header with filename and actions.
#[component]
fn SheetHeader(
    item_name: Signal<String>,
    is_encrypted: Signal<bool>,
    on_close: impl Fn(leptos::ev::MouseEvent) + 'static,
) -> impl IntoView {
    view! {
        <div class=css::sheetHeader>
            <span class=css::filename>{move || item_name.get()}</span>
            <div class=css::sheetActions>
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
                    title="Close"
                    aria-label="Close preview"
                >
                    <Icon icon=ic::CLOSE />
                </button>
            </div>
        </div>
    }
}
