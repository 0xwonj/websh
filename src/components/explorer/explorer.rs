//! Main explorer component.
//!
//! The file explorer view with header, file list, and preview panel/sheet.
//!
//! ## Layout
//!
//! - **Desktop (> 768px)**: Dual panel layout with file list on left, preview on right
//! - **Mobile (< 768px)**: Single column with bottom sheet for preview

use leptos::prelude::*;

use super::pathbar::PathBar;
use super::preview::{BottomSheet, PreviewPanel, use_preview};
use super::{FileList, Header};
use crate::app::AppContext;

stylance::import_crate_style!(css, "src/components/explorer/explorer.module.css");

/// File explorer view component.
#[component]
pub fn Explorer() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    // Call use_preview() once here, pass to both panel and sheet
    let preview_data = use_preview();

    let has_selection = Signal::derive(move || ctx.explorer.selection.get().is_some());

    view! {
        <div class=css::explorer>
            <Header />

            <div class=css::body>
                // Left panel: file list
                <div class=move || {
                    if has_selection.get() {
                        format!("{} {}", css::fileListPane, css::fileListPaneWithPreview)
                    } else {
                        css::fileListPane.to_string()
                    }
                }>
                    <FileList />
                </div>

                // Right panel: preview (desktop only)
                <Show when=move || has_selection.get()>
                    <PreviewPanel data=preview_data />
                </Show>
            </div>

            // Path bar (bottom, macOS Finder style)
            <PathBar />

            // Bottom sheet (mobile only)
            <Show when=move || has_selection.get()>
                <BottomSheet data=preview_data />
            </Show>
        </div>
    }
}
