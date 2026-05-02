//! Explorer header component.
//!
//! Contains navigation buttons, current location title, and action buttons.

use leptos::prelude::*;
use leptos_icons::Icon;

use crate::app::AppContext;
use crate::components::icons as ic;
use crate::components::terminal::RouteContext;
use websh_core::filesystem::push_request_path;
use websh_core::domain::ExplorerViewType;

stylance::import_crate_style!(css, "src/components/explorer/header.module.css");

/// Explorer header with navigation and actions.
#[component]
pub fn Header() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    // Dropdown menu states
    let (new_menu_open, set_new_menu_open) = signal(false);
    let (more_menu_open, set_more_menu_open) = signal(false);

    // Derived signals
    let is_root = Signal::derive(move || route_ctx.0.get().is_root());
    let is_home = Signal::derive(move || route_ctx.0.get().is_home());
    let view_type = Signal::derive(move || ctx.explorer.view_type.get());

    // Derive current location name for header title
    let current_name = Memo::new(move |_| {
        let route = route_ctx.0.get();
        let display = route.display_path();

        if route.is_root() {
            return "/".to_string();
        }

        // Get the last segment as the current name
        display
            .split('/')
            .rfind(|s| !s.is_empty())
            .unwrap_or("/")
            .to_string()
    });

    // Derive icon for current location
    let current_icon = Memo::new(move |_| {
        let route = route_ctx.0.get();
        let name = current_name.get();

        if route.is_root() {
            ic::SERVER
        } else if name == "~" {
            ic::HOME
        } else if route.is_file() {
            ic::FILE
        } else {
            ic::FOLDER
        }
    });

    view! {
        <header class=css::header>
            <NavButtons
                is_root=is_root
                is_home=is_home
            />

            // Current location title (center)
            <div class=css::title>
                <span class=css::titleIcon>
                    {move || view! { <Icon icon=current_icon.get() /> }}
                </span>
                <span class=css::titleLabel>{move || current_name.get()}</span>
            </div>

            <ActionButtons
                view_type=view_type
                new_menu_open=new_menu_open
                set_new_menu_open=set_new_menu_open
                more_menu_open=more_menu_open
                set_more_menu_open=set_more_menu_open
            />
        </header>
    }
}

/// Navigation buttons (back, forward, home).
///
/// Back/forward delegate to the browser's own history, so the URL hash
/// and the browser's native back/forward buttons stay in sync.
#[component]
fn NavButtons(is_root: Signal<bool>, is_home: Signal<bool>) -> impl IntoView {
    // Back: browser history back (equivalent to user's browser back button)
    let on_back = move |_: leptos::ev::MouseEvent| {
        if let Some(window) = web_sys::window() {
            let _ = window.history().and_then(|h| h.back());
        }
    };

    // Forward: browser history forward
    let on_forward = move |_: leptos::ev::MouseEvent| {
        if let Some(window) = web_sys::window() {
            let _ = window.history().and_then(|h| h.forward());
        }
    };

    // Home
    let on_home = move |_: leptos::ev::MouseEvent| {
        push_request_path("/websh");
    };

    view! {
        <div class=css::navButtons>
            <button
                class=move || nav_button_class(is_root.get())
                on:click=on_back
                disabled=move || is_root.get()
                title="Go back"
            >
                <Icon icon=ic::CHEVRON_LEFT />
            </button>
            <button
                class=css::navButton
                on:click=on_forward
                title="Go forward"
            >
                <Icon icon=ic::CHEVRON_RIGHT />
            </button>
            <button
                class=move || {
                    let base = format!("{} {}", css::navButton, css::navButtonHome);
                    if is_home.get() {
                        format!("{} {}", base, css::navButtonDisabled)
                    } else {
                        base
                    }
                }
                on:click=on_home
                disabled=move || is_home.get()
                title="Go home"
            >
                <Icon icon=ic::HOME />
            </button>
        </div>
    }
}

fn nav_button_class(disabled: bool) -> String {
    if disabled {
        format!("{} {}", css::navButton, css::navButtonDisabled)
    } else {
        css::navButton.to_string()
    }
}

/// Build a focusout handler that closes a menu when focus leaves the wrapper.
///
/// Intended for use on the dropdown container's `on:focusout`. If the newly
/// focused element is outside the container's subtree (or no longer inside the
/// document), the menu closes. Used by both `NewMenu` and `MoreMenu` to avoid
/// duplication.
fn close_on_focus_out(set_open: WriteSignal<bool>) -> impl Fn(web_sys::FocusEvent) + 'static {
    move |event: web_sys::FocusEvent| {
        use wasm_bindgen::JsCast;
        if let Some(related) = event.related_target() {
            // Focus moving to another element: only close if it's outside the wrapper.
            if let Some(current) = event.current_target()
                && let (Some(wrapper), Some(target)) = (
                    current.dyn_ref::<web_sys::Node>(),
                    related.dyn_ref::<web_sys::Node>(),
                )
                && !wrapper.contains(Some(target))
            {
                set_open.set(false);
            }
        } else {
            // Focus moved outside the document (e.g., clicked elsewhere).
            set_open.set(false);
        }
    }
}

/// Action buttons (search, view toggle, new, more).
#[component]
fn ActionButtons(
    view_type: Signal<ExplorerViewType>,
    new_menu_open: ReadSignal<bool>,
    set_new_menu_open: WriteSignal<bool>,
    more_menu_open: ReadSignal<bool>,
    set_more_menu_open: WriteSignal<bool>,
) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    let on_search = move |_: leptos::ev::MouseEvent| {};

    let on_view_toggle = move |_: leptos::ev::MouseEvent| {
        ctx.explorer.toggle_view_type();
    };

    view! {
        <div class=css::actionButtons>
            // Search (desktop only)
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

            // New menu
            <NewMenu
                menu_open=new_menu_open
                set_menu_open=set_new_menu_open
            />

            // More menu
            <MoreMenu
                menu_open=more_menu_open
                set_menu_open=set_more_menu_open
                view_type=view_type
            />
        </div>
    }
}

/// New file/folder dropdown menu.
#[component]
fn NewMenu(menu_open: ReadSignal<bool>, set_menu_open: WriteSignal<bool>) -> impl IntoView {
    let on_new_file = move |_: leptos::ev::MouseEvent| {
        set_menu_open.set(false);
    };

    let on_new_folder = move |_: leptos::ev::MouseEvent| {
        set_menu_open.set(false);
    };

    view! {
        <div
            class=css::dropdownWrapper
            on:focusout=close_on_focus_out(set_menu_open)
        >
            <button
                class=css::actionButton
                on:click=move |_| set_menu_open.update(|v| *v = !*v)
                title="New file or folder"
            >
                <Icon icon=ic::PLUS />
            </button>
            <Show when=move || menu_open.get()>
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
    }
}

/// More options dropdown menu.
#[component]
fn MoreMenu(
    menu_open: ReadSignal<bool>,
    set_menu_open: WriteSignal<bool>,
    view_type: Signal<ExplorerViewType>,
) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    let on_search = move |_: leptos::ev::MouseEvent| {
        set_menu_open.set(false);
    };

    let on_view_toggle = move |_: leptos::ev::MouseEvent| {
        set_menu_open.set(false);
        ctx.explorer.toggle_view_type();
    };

    let on_home = move |_: leptos::ev::MouseEvent| {
        set_menu_open.set(false);
        push_request_path("/websh");
    };

    let on_zoom_in = move |_: leptos::ev::MouseEvent| {
        set_menu_open.set(false);
    };

    let on_zoom_out = move |_: leptos::ev::MouseEvent| {
        set_menu_open.set(false);
    };

    view! {
        <div
            class=css::dropdownWrapper
            on:focusout=close_on_focus_out(set_menu_open)
        >
            <button
                class=css::actionButton
                on:click=move |_| set_menu_open.update(|v| *v = !*v)
                title="More options"
            >
                <Icon icon=ic::MORE />
            </button>
            <Show when=move || menu_open.get()>
                <div class=css::dropdownMenu>
                    // Mobile-only items
                    <button class=format!("{} {}", css::dropdownItem, css::mobileOnly) on:click=on_search>
                        <span class=css::dropdownIcon><Icon icon=ic::SEARCH /></span>
                        "Search"
                    </button>
                    <button class=format!("{} {}", css::dropdownItem, css::mobileOnly) on:click=on_view_toggle>
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
                    // Zoom controls
                    <button class=css::dropdownItem on:click=on_zoom_in>
                        <span class=css::dropdownIcon><Icon icon=ic::FONT_INCREASE /></span>
                        "Zoom In"
                    </button>
                    <button class=css::dropdownItem on:click=on_zoom_out>
                        <span class=css::dropdownIcon><Icon icon=ic::FONT_DECREASE /></span>
                        "Zoom Out"
                    </button>
                </div>
            </Show>
        </div>
    }
}
