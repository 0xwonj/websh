//! Main shell component.
//!
//! The root UI component that manages view switching between Terminal and Explorer,
//! content overlays (Reader), and global UI effects (CRT overlay, scanlines).

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use super::boot;
use super::terminal::Terminal;
use crate::app::AppContext;
use crate::components::explorer::Explorer;
use crate::components::reader::Reader;
use crate::components::status::Status;
use crate::core::wallet;
use crate::models::{ContentOverlay, OutputLine, Route, ScreenMode, ViewMode, WalletState};

stylance::import_crate_style!(css, "src/components/terminal/shell.module.css");

// ============================================================================
// Effect Setup Functions
// ============================================================================

/// Set up the initial boot sequence.
fn setup_boot_effect(ctx: AppContext) {
    Effect::new(move || {
        let initial_route = Route::current();
        boot::run(ctx, initial_route);
    });
}

/// Sync URL when screen mode changes.
fn setup_url_sync_effect(screen_mode: RwSignal<ScreenMode>) {
    Effect::new(move || {
        let mode = screen_mode.get();
        let route = match &mode {
            ScreenMode::Booting | ScreenMode::Terminal => Route::Home,
            ScreenMode::Reader { content_path, .. } => Route::Read {
                path: content_path.clone(),
            },
        };
        route.push();
    });
}

/// Handle browser back/forward navigation (popstate).
///
/// # Note on Memory Management
/// The closure is intentionally leaked using `forget()` since Shell is the
/// root component that lives for the entire application lifetime.
fn setup_popstate_handler(ctx: AppContext) {
    let closure = Closure::<dyn Fn()>::new(move || match Route::current() {
        Route::Home => {
            ctx.terminal.screen_mode.set(ScreenMode::Terminal);
        }
        Route::Read { path } => {
            // For URL-based reader, we don't have the full virtual path
            // Use content path as virtual path (will show relative path in breadcrumb)
            ctx.terminal.screen_mode.set(ScreenMode::Reader {
                content_path: path.clone(),
                virtual_path: path,
            });
        }
    });

    if let Some(window) = web_sys::window() {
        let _ =
            window.add_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref());
    }

    closure.forget();
}

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

/// Root shell component managing views and overlays.
///
/// Handles:
/// - View switching between Terminal and Explorer (via Status bar)
/// - Content overlays (Reader)
/// - Global UI effects (CRT overlay, scanlines)
/// - Wallet event listeners
/// - URL routing sync
#[component]
pub fn Shell() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided at root");

    let output_ref = NodeRef::<leptos::html::Div>::new();

    // Set up all effects
    setup_boot_effect(ctx);
    setup_url_sync_effect(ctx.terminal.screen_mode);
    setup_popstate_handler(ctx);
    setup_autoscroll_effect(ctx.terminal.history, output_ref);
    setup_wallet_events(ctx);

    let on_reader_close = Callback::new(move |_: ()| {
        ctx.terminal.screen_mode.set(ScreenMode::Terminal);
    });

    view! {
        <div class=css::screen>
            <div class=css::crtOverlay></div>
            <div class=css::scanline></div>

            <Status />

            <div class=css::main>
                // Main view (Terminal or Explorer based on view_mode)
                {move || {
                    let screen_mode = ctx.terminal.screen_mode.get();
                    let view_mode = ctx.view_mode.get();

                    match screen_mode {
                        ScreenMode::Booting | ScreenMode::Terminal => {
                            match view_mode {
                                ViewMode::Terminal => {
                                    view! { <Terminal output_ref=output_ref /> }.into_any()
                                }
                                ViewMode::Explorer => {
                                    view! { <Explorer /> }.into_any()
                                }
                            }
                        }
                        ScreenMode::Reader { content_path, virtual_path } => {
                            view! {
                                <Reader
                                    content_path=content_path
                                    virtual_path=virtual_path
                                    on_close=on_reader_close
                                />
                            }.into_any()
                        }
                    }
                }}

                // Content overlay (Reader from Explorer)
                {move || {
                    match ctx.content_overlay.get() {
                        ContentOverlay::None => ().into_any(),
                        ContentOverlay::Reader { content_path, virtual_path } => {
                            let on_close = Callback::new(move |_: ()| {
                                ctx.close_overlay();
                            });
                            view! {
                                <div class=css::overlay>
                                    <Reader
                                        content_path=content_path
                                        virtual_path=virtual_path
                                        on_close=on_close
                                    />
                                </div>
                            }.into_any()
                        }
                    }
                }}
            </div>
        </div>
    }
}
