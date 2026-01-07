//! Status bar component.
//!
//! Displays session, location, and network information.
//! Provides view toggle button to switch between Terminal and Explorer.

use leptos::prelude::*;
use leptos_icons::Icon;

use crate::app::AppContext;
use crate::components::icons as ic;
use crate::core::wallet;
use crate::models::ViewMode;

stylance::import_crate_style!(css, "src/components/status/status.module.css");

/// Status bar component displaying session, location, and network information.
///
/// ## Responsive behavior
///
/// | Breakpoint | Display |
/// |------------|---------|
/// | Desktop (> 768px) | Full labels: `Session: guest \| Location: ~ \| Network: Mainnet` |
/// | Tablet (480-768px) | Values only: `guest · ~ · Mainnet` |
/// | Mobile (< 480px) | Minimal: `guest · ~` (network hidden) |
#[component]
pub fn Status() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided at root");

    // Derived signals for reactive display
    let display_path = Signal::derive(move || ctx.current_path.with(|p| p.display()));
    let session_name = Signal::derive(move || ctx.wallet.with(|w| w.display_name()));
    let network_name = Signal::derive(move || {
        ctx.wallet.with(|w| {
            w.chain_id()
                .map(|id| wallet::chain_name(id).to_string())
                .unwrap_or_else(|| "—".to_string())
        })
    });

    // View toggle
    let view_mode = ctx.view_mode;
    let toggle_view = move |_: leptos::ev::MouseEvent| {
        ctx.toggle_view_mode();
    };
    let toggle_title = Signal::derive(move || match view_mode.get() {
        ViewMode::Terminal => "Switch to Explorer",
        ViewMode::Explorer => "Switch to Terminal",
    });

    view! {
        <header class=css::bar>
            // Status information section
            <div class=css::section>
                // Session
                <span class=css::label>
                    <span class=css::labelText>"Session:"</span>
                    <span class=css::labelIcon><Icon icon=ic::USER /></span>
                    <span class=css::value>{session_name}</span>
                </span>

                // Location
                <span class=css::labelCyan>
                    <span class=css::labelText>"Location:"</span>
                    <span class=css::labelIcon><Icon icon=ic::LOCATION /></span>
                    <span class=css::value>{display_path}</span>
                </span>

                // Network
                <span class=css::labelPurple>
                    <span class=css::labelText>"Network:"</span>
                    <span class=css::labelIcon><Icon icon=ic::NETWORK /></span>
                    <span class=css::value>{network_name}</span>
                </span>
            </div>

            // View toggle button
            <button
                class=css::toggleButton
                on:click=toggle_view
                title=toggle_title
            >
                {move || match view_mode.get() {
                    ViewMode::Terminal => view! { <Icon icon=ic::EXPLORER /> }.into_any(),
                    ViewMode::Explorer => view! { <Icon icon=ic::TERMINAL /> }.into_any(),
                }}
            </button>
        </header>
    }
}
