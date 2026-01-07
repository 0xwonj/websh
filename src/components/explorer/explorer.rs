//! Main explorer component.
//!
//! The file explorer view with header, file list, and preview panel/sheet.
//!
//! ## Layout
//!
//! - **Desktop (> 768px)**: Dual panel layout with file list on left, preview on right
//! - **Mobile (< 768px)**: Single column with bottom sheet for preview

#![allow(dead_code)]

use leptos::prelude::*;
use leptos_icons::Icon;

use super::{BottomSheet, FileList, PreviewPanel};
use crate::app::AppContext;
use crate::components::icons as ic;
use crate::models::{ExplorerViewType, SheetState, VirtualPath};

stylance::import_crate_style!(css, "src/components/explorer/explorer.module.css");

/// File explorer view component.
///
/// Displays:
/// - Header with back/forward/home buttons and current path
/// - Action buttons: search, view toggle, new, more menu
/// - Dual panel: file list (left) + preview panel (right) on desktop
/// - Bottom sheet for preview on mobile
#[component]
pub fn Explorer() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    // Dropdown menu states
    let (new_menu_open, set_new_menu_open) = signal(false);
    let (more_menu_open, set_more_menu_open) = signal(false);

    // Navigation handlers
    let on_back = move |_: leptos::ev::MouseEvent| {
        ctx.go_back();
    };

    let on_forward = move |_: leptos::ev::MouseEvent| {
        ctx.go_forward();
    };

    let on_home = move |_: leptos::ev::MouseEvent| {
        ctx.navigate_to(VirtualPath::home());
    };

    // Action handlers (placeholder - log only for now)
    let on_search = move |_: leptos::ev::MouseEvent| {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Search clicked".into());
    };

    let on_view_toggle = move |_: leptos::ev::MouseEvent| {
        ctx.explorer.view_type.update(|vt| {
            *vt = match *vt {
                ExplorerViewType::List => ExplorerViewType::Grid,
                ExplorerViewType::Grid => ExplorerViewType::List,
            };
        });
    };

    let on_new_file = move |_: leptos::ev::MouseEvent| {
        set_new_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"New file clicked".into());
    };

    let on_new_folder = move |_: leptos::ev::MouseEvent| {
        set_new_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"New folder clicked".into());
    };

    let on_switch_terminal = move |_: leptos::ev::MouseEvent| {
        set_more_menu_open.set(false);
        ctx.toggle_view_mode();
    };

    // Derived signals for button states
    let can_go_back = Signal::derive(move || ctx.can_go_back());
    let can_go_forward = Signal::derive(move || ctx.can_go_forward());
    let is_home = Signal::derive(move || ctx.current_path.get() == VirtualPath::home());
    let has_selection = Signal::derive(move || ctx.explorer.selected_file.get().is_some());
    let view_type = Signal::derive(move || ctx.explorer.view_type.get());

    view! {
        <div class=css::explorer>
            // Header with navigation
            <header class=css::header>
                // Navigation buttons (segmented control: back/forward/home)
                <div class=css::navButtons>
                    <button
                        class=move || if can_go_back.get() { css::navButton.to_string() } else { format!("{} {}", css::navButton, css::navButtonDisabled) }
                        on:click=on_back
                        disabled=move || !can_go_back.get()
                        title="Go back"
                    >
                        <Icon icon=ic::CHEVRON_LEFT />
                    </button>
                    <button
                        class=move || if can_go_forward.get() { css::navButton.to_string() } else { format!("{} {}", css::navButton, css::navButtonDisabled) }
                        on:click=on_forward
                        disabled=move || !can_go_forward.get()
                        title="Go forward"
                    >
                        <Icon icon=ic::CHEVRON_RIGHT />
                    </button>
                    <button
                        class=move || {
                            let base = format!("{} {}", css::navButton, css::navButtonHome);
                            if is_home.get() { format!("{} {}", base, css::navButtonDisabled) } else { base }
                        }
                        on:click=on_home
                        disabled=move || is_home.get()
                        title="Go home"
                    >
                        <Icon icon=ic::HOME />
                    </button>
                </div>

                // Breadcrumb path
                <nav class=css::breadcrumb>
                    {move || {
                        let path = ctx.current_path.get();
                        let display = path.display();
                        let segments: Vec<&str> = display.split('/').filter(|s| !s.is_empty()).collect();

                        // Build path for each segment
                        segments.iter().enumerate().map(|(idx, segment)| {
                            let is_last = idx == segments.len() - 1;
                            let is_home = *segment == "~";

                            // Build target path for navigation
                            let target_path = if is_home {
                                VirtualPath::home()
                            } else if segments[0] == "~" {
                                // Home-relative path: use resolve() to properly expand ~
                                let relative_path = segments[1..=idx].join("/");
                                VirtualPath::home().resolve(&relative_path)
                            } else {
                                // Absolute path
                                VirtualPath::new(format!("/{}", segments[0..=idx].join("/")))
                            };

                            let icon = if is_home { ic::HOME } else { ic::FOLDER };
                            let segment_str = segment.to_string();

                            let segment_class = if is_last {
                                format!("{} {}", css::breadcrumbSegment, css::breadcrumbSegmentCurrent)
                            } else {
                                css::breadcrumbSegment.to_string()
                            };

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
                        }).collect_view()
                    }}
                </nav>

                // Action buttons (right side)
                <div class=css::actionButtons>
                    // Search button (desktop only)
                    <button
                        class=format!("{} {}", css::actionButton, css::desktopOnly)
                        on:click=on_search
                        title="Search"
                    >
                        <Icon icon=ic::SEARCH />
                    </button>

                    // View toggle (desktop only)
                    <button
                        class=format!("{} {}", css::actionButton, css::desktopOnly)
                        on:click=on_view_toggle
                        title="Toggle view"
                    >
                        {move || if matches!(view_type.get(), ExplorerViewType::List) {
                            view! { <Icon icon=ic::LIST /> }.into_any()
                        } else {
                            view! { <Icon icon=ic::GRID /> }.into_any()
                        }}
                    </button>

                    // New button with dropdown
                    <div class=css::dropdownWrapper>
                        <button
                            class=css::actionButton
                            on:click=move |_| set_new_menu_open.update(|v| *v = !*v)
                            title="New file or folder"
                        >
                            <Icon icon=ic::PLUS />
                        </button>
                        <Show when=move || new_menu_open.get()>
                            <div class=css::dropdownMenu>
                                <button class=css::dropdownItem on:click=on_new_file>
                                    <span class=css::dropdownIcon><Icon icon=ic::FILE /></span>
                                    "New File"
                                </button>
                                <button class=css::dropdownItem on:click=on_new_folder>
                                    <span class=css::dropdownIcon><Icon icon=ic::FOLDER /></span>
                                    "New Folder"
                                </button>
                            </div>
                        </Show>
                    </div>

                    // More menu
                    <div class=css::dropdownWrapper>
                        <button
                            class=css::actionButton
                            on:click=move |_| set_more_menu_open.update(|v| *v = !*v)
                            title="More options"
                        >
                            <Icon icon=ic::MORE />
                        </button>
                        <Show when=move || more_menu_open.get()>
                            <div class=css::dropdownMenu>
                                // Mobile-only items
                                <button class=format!("{} {}", css::dropdownItem, css::mobileOnly) on:click=on_search>
                                    <span class=css::dropdownIcon><Icon icon=ic::SEARCH /></span>
                                    "Search"
                                </button>
                                <button class=format!("{} {}", css::dropdownItem, css::mobileOnly) on:click=move |_| {
                                    set_more_menu_open.set(false);
                                    on_view_toggle(leptos::ev::MouseEvent::new("click").unwrap());
                                }>
                                    <span class=css::dropdownIcon>
                                        {move || if matches!(view_type.get(), ExplorerViewType::List) {
                                            view! { <Icon icon=ic::GRID /> }.into_any()
                                        } else {
                                            view! { <Icon icon=ic::LIST /> }.into_any()
                                        }}
                                    </span>
                                    {move || if matches!(view_type.get(), ExplorerViewType::List) { "Grid View" } else { "List View" }}
                                </button>
                                <button class=format!("{} {}", css::dropdownItem, css::mobileOnly) on:click=on_home>
                                    <span class=css::dropdownIcon><Icon icon=ic::HOME /></span>
                                    "Go Home"
                                </button>
                                <div class=format!("{} {}", css::dropdownDivider, css::mobileOnly)></div>
                                // Always visible items
                                <button class=css::dropdownItem on:click=on_switch_terminal>
                                    <span class=css::dropdownIcon><Icon icon=ic::TERMINAL /></span>
                                    "Switch to Terminal"
                                </button>
                            </div>
                        </Show>
                    </div>
                </div>
            </header>

            // Body: dual panel layout
            <div class=css::body>
                // Left panel: file list (shrinks to 50% when preview is shown)
                <div class=move || {
                    if has_selection.get() {
                        format!("{} {}", css::fileListPane, css::fileListPaneWithPreview)
                    } else {
                        css::fileListPane.to_string()
                    }
                }>
                    <FileList />
                </div>

                // Right panel: preview (desktop only, hidden via CSS on mobile)
                <Show when=move || has_selection.get()>
                    <PreviewPanel />
                </Show>
            </div>

            // Bottom sheet for file preview (mobile only, hidden via CSS on desktop)
            <Show when=move || !matches!(ctx.explorer.sheet_state.get(), SheetState::Closed)>
                <BottomSheet />
            </Show>
        </div>
    }
}
