//! Explorer header component.
//!
//! Contains navigation buttons, current location title, and action buttons.

use leptos::prelude::*;
use leptos_icons::Icon;

use crate::app::AppContext;
use crate::components::icons as ic;
use crate::components::terminal::RouteContext;
use crate::models::{AppRoute, ExplorerViewType};

stylance::import_crate_style!(css, "src/components/explorer/explorer.module.css");

/// Explorer header with navigation and actions.
#[component]
pub fn Header() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    // Dropdown menu states
    let (new_menu_open, set_new_menu_open) = signal(false);
    let (more_menu_open, set_more_menu_open) = signal(false);

    // Derived signals
    let is_root = Signal::derive(move || matches!(route_ctx.0.get(), AppRoute::Root));
    let is_home = Signal::derive(move || route_ctx.0.get() == AppRoute::home());
    let can_forward = Signal::derive(move || ctx.explorer.can_go_forward());
    let view_type = Signal::derive(move || ctx.explorer.view_type.get());

    // Derive current location name for header title
    let current_name = Memo::new(move |_| {
        let route = route_ctx.0.get();
        let display = route.display_path();

        if matches!(route, AppRoute::Root) {
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

        if matches!(route, AppRoute::Root) {
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
                route_ctx=route_ctx
                is_root=is_root
                is_home=is_home
                can_forward=can_forward
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
#[component]
fn NavButtons(
    route_ctx: RouteContext,
    is_root: Signal<bool>,
    is_home: Signal<bool>,
    can_forward: Signal<bool>,
) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    // Back: navigate to parent directory
    let on_back = move |_: leptos::ev::MouseEvent| {
        let route = route_ctx.0.get();
        let parent = route.parent();
        if parent != route {
            ctx.explorer.push_forward(route);
            parent.push();
        }
    };

    // Forward: pop from forward stack
    let on_forward = move |_: leptos::ev::MouseEvent| {
        if let Some(forward_route) = ctx.explorer.pop_forward() {
            forward_route.push();
        }
    };

    // Home
    let on_home = move |_: leptos::ev::MouseEvent| {
        AppRoute::home().push();
    };

    view! {
        <div class=css::navButtons>
            <button
                class=move || nav_button_class(is_root.get())
                on:click=on_back
                disabled=move || is_root.get()
                title="Go to parent directory"
            >
                <Icon icon=ic::CHEVRON_LEFT />
            </button>
            <button
                class=move || nav_button_class(!can_forward.get())
                on:click=on_forward
                disabled=move || !can_forward.get()
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

    let on_search = move |_: leptos::ev::MouseEvent| {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Search clicked".into());
    };

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
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"New file clicked".into());
    };

    let on_new_folder = move |_: leptos::ev::MouseEvent| {
        set_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"New folder clicked".into());
    };

    // Close menu when focus leaves the dropdown wrapper
    let on_focusout = move |event: web_sys::FocusEvent| {
        // Check if the new focus target is outside the dropdown
        // Use a small delay to allow focus to settle on the new target
        let set_menu = set_menu_open;
        if let Some(related) = event.related_target() {
            // If focus is moving to another element, check if it's within the dropdown
            if let Some(current) = event.current_target() {
                use wasm_bindgen::JsCast;
                if let (Some(wrapper), Some(target)) = (
                    current.dyn_ref::<web_sys::Node>(),
                    related.dyn_ref::<web_sys::Node>(),
                )
                    && !wrapper.contains(Some(target))
                {
                    set_menu.set(false);
                }
            }
        } else {
            // Focus moved outside the document (e.g., clicked elsewhere)
            set_menu.set(false);
        }
    };

    view! {
        <div
            class=css::dropdownWrapper
            on:focusout=on_focusout
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
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Search clicked".into());
    };

    let on_view_toggle = move |_: leptos::ev::MouseEvent| {
        set_menu_open.set(false);
        ctx.explorer.toggle_view_type();
    };

    let on_home = move |_: leptos::ev::MouseEvent| {
        set_menu_open.set(false);
        AppRoute::home().push();
    };

    let on_zoom_in = move |_: leptos::ev::MouseEvent| {
        set_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Zoom in clicked".into());
    };

    let on_zoom_out = move |_: leptos::ev::MouseEvent| {
        set_menu_open.set(false);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&"Zoom out clicked".into());
    };

    // Close menu when focus leaves the dropdown wrapper
    let on_focusout = move |event: web_sys::FocusEvent| {
        let set_menu = set_menu_open;
        if let Some(related) = event.related_target() {
            if let Some(current) = event.current_target() {
                use wasm_bindgen::JsCast;
                if let (Some(wrapper), Some(target)) = (
                    current.dyn_ref::<web_sys::Node>(),
                    related.dyn_ref::<web_sys::Node>(),
                )
                    && !wrapper.contains(Some(target))
                {
                    set_menu.set(false);
                }
            }
        } else {
            set_menu.set(false);
        }
    };

    view! {
        <div
            class=css::dropdownWrapper
            on:focusout=on_focusout
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
