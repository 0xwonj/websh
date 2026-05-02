//! Main shell component.
//!
//! Container component that manages view switching between Terminal and Explorer.
//! Receives the current route from the parent Router component and passes it
//! to child components via context.

use leptos::prelude::*;

use super::terminal::Terminal;
use crate::app::AppContext;
use crate::components::chrome::SiteChrome;
use crate::components::explorer::Explorer;
use websh_core::filesystem::{
    RenderIntent, RouteFrame, RouteSurface, request_path_for_canonical_path, route_cwd,
};
use websh_core::runtime::wallet;
use websh_core::domain::{OutputLine, ViewMode, WalletState};

stylance::import_crate_style!(css, "src/components/terminal/shell.module.css");

/// Context for accessing the current route from any component.
///
/// This allows child components (Terminal and Explorer) to access the current
/// route without prop drilling.
#[derive(Clone, Copy)]
pub struct RouteContext(pub Memo<RouteFrame>);

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
            let _ = crate::components::wallet::disconnect(&ctx_for_accounts);
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

/// Shell component managing Terminal/Explorer views.
///
/// This is a container component that:
/// - Receives the current route from the Router
/// - Provides route context to child components
/// - Manages view switching between Terminal and Explorer (via ViewMode)
/// - Handles boot sequence initialization
/// - Provides terminal surface effects
/// - Sets up wallet event listeners
///
/// # Props
/// - `route`: The current route frame (derived from URL + engine resolution)
#[component]
pub fn Shell(route: Memo<RouteFrame>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided at root");

    // Provide route context for child components
    provide_context(RouteContext(route));

    Effect::new(move |_| {
        let frame = route.get();
        ctx.cwd.set(route_cwd(&frame));
        match frame.surface() {
            RouteSurface::Shell => {
                let canonical =
                    request_path_for_canonical_path(&route_cwd(&frame), RouteSurface::Shell);
                if frame.request.url_path != canonical {
                    websh_core::filesystem::RouteRequest::new(canonical).replace();
                }
                ctx.view_mode.set(ViewMode::Terminal);
            }
            RouteSurface::Explorer => ctx.view_mode.set(ViewMode::Explorer),
            RouteSurface::Content => {
                if matches!(frame.intent, RenderIntent::DirectoryListing { .. }) {
                    ctx.view_mode.set(ViewMode::Explorer);
                }
            }
        }
    });

    let output_ref = NodeRef::<leptos::html::Div>::new();

    setup_autoscroll_effect(ctx.terminal.history, output_ref);
    setup_wallet_events(ctx);

    view! {
        <div class=css::screen>
            <SiteChrome route=route />

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
