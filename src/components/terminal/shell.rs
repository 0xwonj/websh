//! Main shell component.
//!
//! Container component that manages view switching between Terminal and Explorer.
//! Receives the current route from the parent Router component and passes it
//! to child components via context.

use leptos::prelude::*;

use super::boot;
use super::terminal::Terminal;
use crate::app::AppContext;
use crate::components::explorer::Explorer;
use crate::components::status::Status;
use crate::core::wallet;
use crate::models::{AppRoute, OutputLine, ViewMode, WalletState};

stylance::import_crate_style!(css, "src/components/terminal/shell.module.css");

/// Re-export overlay class for router component.
pub const OVERLAY_CLASS: &str = css::overlay;

// ============================================================================
// Route Context
// ============================================================================

/// Context for accessing the current route from any component.
///
/// This allows child components (Terminal, Explorer, Status) to access
/// the current route without prop drilling.
#[derive(Clone, Copy)]
pub struct RouteContext(pub Memo<AppRoute>);

// ============================================================================
// Effect Setup Functions
// ============================================================================

/// Auto-scroll output to bottom when history changes.
fn setup_autoscroll_effect(
    history: RwSignal<crate::utils::RingBuffer<OutputLine>>,
    output_ref: NodeRef<leptos::html::Div>,
) {
    Effect::new(move || {
        history.track();
        if let Some(el) = output_ref.get() {
            el.set_scroll_top(el.scroll_height());
        }
    });
}

/// Set up wallet event listeners for account and chain changes.
fn setup_wallet_events(ctx: AppContext) {
    let ctx_for_accounts = ctx;
    let _ = wallet::on_accounts_changed(move |account: Option<String>| match account {
        Some(new_addr) => {
            ctx_for_accounts.wallet.update(|w| {
                if let WalletState::Connected { chain_id, .. } = w {
                    *w = WalletState::Connected {
                        address: new_addr,
                        ens_name: None,
                        chain_id: *chain_id,
                    };
                }
            });
        }
        None => {
            wallet::clear_session();
            ctx_for_accounts.wallet.set(WalletState::Disconnected);
        }
    });

    let ctx_for_chain = ctx;
    let _ = wallet::on_chain_changed(move |chain_id_hex: String| {
        let new_chain_id = u64::from_str_radix(chain_id_hex.trim_start_matches("0x"), 16).ok();

        ctx_for_chain.wallet.update(|w| {
            if let WalletState::Connected { chain_id, .. } = w {
                *chain_id = new_chain_id;
            }
        });
    });
}

// ============================================================================
// Shell Component
// ============================================================================

/// Shell component managing Terminal/Explorer views.
///
/// This is a container component that:
/// - Receives the current route from the Router
/// - Provides route context to child components
/// - Manages view switching between Terminal and Explorer (via ViewMode)
/// - Handles boot sequence initialization
/// - Provides global UI effects (CRT overlay, scanlines)
/// - Sets up wallet event listeners
///
/// # Props
/// - `route`: The current application route (derived from URL)
#[component]
pub fn Shell(route: Memo<AppRoute>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided at root");

    // Provide route context for child components
    provide_context(RouteContext(route));

    let output_ref = NodeRef::<leptos::html::Div>::new();

    // Boot sequence runs once
    let boot_started = StoredValue::new(false);
    Effect::new(move || {
        if !boot_started.get_value() {
            boot_started.set_value(true);
            boot::run(ctx);
        }
    });

    setup_autoscroll_effect(ctx.terminal.history, output_ref);
    setup_wallet_events(ctx);

    view! {
        <div class=css::screen>
            <div class=css::crtOverlay></div>
            <div class=css::scanline></div>

            <Status />

            <div class=css::main>
                {move || {
                    match ctx.view_mode.get() {
                        ViewMode::Terminal => {
                            view! { <Terminal output_ref=output_ref /> }.into_any()
                        }
                        ViewMode::Explorer => {
                            view! { <Explorer /> }.into_any()
                        }
                    }
                }}
            </div>
        </div>
    }
}
