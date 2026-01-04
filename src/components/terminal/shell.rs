//! Main shell component.
//!
//! The primary terminal interface that handles user input,
//! command execution, and screen mode switching.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use super::boot;
use crate::app::AppContext;
use crate::components::reader::Reader;
use crate::components::status::Status;
use crate::components::terminal::{Input, Output};
use crate::core::{autocomplete, execute_pipeline, get_hint, parse_input, wallet};
use crate::models::{OutputLine, Route, ScreenMode, WalletState};

stylance::import_crate_style!(css, "src/components/terminal/shell.module.css");

// ============================================================================
// Wallet Handlers
// ============================================================================

/// Execute wallet login command asynchronously.
///
/// Attempts to connect to the user's wallet (MetaMask) and updates both
/// terminal output and wallet state accordingly. ENS resolution is performed
/// in the background after connection succeeds.
fn handle_login(ctx: AppContext) {
    wasm_bindgen_futures::spawn_local(async move {
        if !wallet::is_available() {
            ctx.terminal.push_output(OutputLine::error(
                "MetaMask not found. Please install MetaMask extension.",
            ));
            return;
        }

        ctx.wallet.set(WalletState::Connecting);
        ctx.terminal
            .push_output(OutputLine::info("Connecting to wallet..."));

        match wallet::connect().await {
            Ok(address) => {
                wallet::save_session();
                let chain_id = wallet::get_chain_id().await;

                // Set connected state without ENS first
                ctx.wallet.set(WalletState::Connected {
                    address: address.clone(),
                    ens_name: None,
                    chain_id,
                });
                ctx.terminal
                    .push_output(OutputLine::success(format!("Connected: {}", address)));

                if let Some(id) = chain_id {
                    ctx.terminal.push_output(OutputLine::info(format!(
                        "Network: {} (chain_id={})",
                        wallet::chain_name(id),
                        id
                    )));
                }

                // Resolve ENS in background
                ctx.terminal
                    .push_output(OutputLine::info("Resolving ENS..."));
                if let Some(ens_name) = wallet::resolve_ens(&address).await {
                    ctx.wallet.set(WalletState::Connected {
                        address: address.clone(),
                        ens_name: Some(ens_name.clone()),
                        chain_id,
                    });
                    ctx.terminal
                        .push_output(OutputLine::success(format!("ENS: {}", ens_name)));
                }
            }
            Err(e) => {
                ctx.wallet.set(WalletState::Disconnected);
                ctx.terminal
                    .push_output(OutputLine::error(format!("Connection failed: {}", e)));
            }
        }
    });
}

/// Execute wallet logout command.
///
/// Disconnects the wallet and clears the stored session.
fn handle_logout(ctx: &AppContext) {
    if ctx.wallet.with(|w| w.is_connected()) {
        wallet::clear_session();
        ctx.wallet.set(WalletState::Disconnected);
        ctx.terminal
            .push_output(OutputLine::success("Disconnected from wallet."));
    } else {
        ctx.terminal
            .push_output(OutputLine::info("No wallet connected."));
    }
}

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
            ScreenMode::Reader { content, .. } => Route::Read {
                path: content.clone(),
            },
        };
        route.push();
    });
}

/// Handle browser back/forward navigation (popstate).
///
/// # Note on Memory Management
/// The closure is intentionally leaked using `forget()` since Shell is the
/// root component that lives for the entire application lifetime. The event
/// listener persists until the page is unloaded, at which point all memory
/// is freed by the browser.
fn setup_popstate_handler(ctx: AppContext) {
    let closure = Closure::<dyn Fn()>::new(move || match Route::current() {
        Route::Home => {
            ctx.terminal.screen_mode.set(ScreenMode::Terminal);
        }
        Route::Read { path } => {
            let title = extract_title_from_path(&path);
            ctx.terminal.screen_mode.set(ScreenMode::Reader {
                content: path,
                title,
            });
        }
    });

    if let Some(window) = web_sys::window() {
        let _ =
            window.add_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref());
    }

    // Intentionally leak the closure - it must live for the app's lifetime
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
///
/// These listeners automatically update the wallet state when:
/// - User switches accounts in MetaMask
/// - User switches networks in MetaMask
/// - User disconnects their wallet
fn setup_wallet_events(ctx: AppContext) {
    // Listen for account changes
    let ctx_for_accounts = ctx;
    let _ = wallet::on_accounts_changed(move |account: Option<String>| {
        match account {
            Some(new_addr) => {
                // Account changed - update state
                ctx_for_accounts.wallet.update(|w| if let WalletState::Connected {
                        chain_id,
                        ..
                    } = w {
                    *w = WalletState::Connected {
                        address: new_addr,
                        ens_name: None, // Clear ENS, will need to re-resolve
                        chain_id: *chain_id,
                    };
                });
            }
            None => {
                // Disconnected
                wallet::clear_session();
                ctx_for_accounts.wallet.set(WalletState::Disconnected);
            }
        }
    });

    // Listen for chain changes
    let ctx_for_chain = ctx;
    let _ = wallet::on_chain_changed(move |chain_id_hex: String| {
        // Parse hex chain ID (e.g., "0x1" -> 1)
        let new_chain_id = u64::from_str_radix(chain_id_hex.trim_start_matches("0x"), 16).ok();

        ctx_for_chain.wallet.update(|w| {
            if let WalletState::Connected { chain_id, .. } = w {
                *chain_id = new_chain_id;
            }
        });
    });
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract title from file path (filename without extension).
fn extract_title_from_path(path: &str) -> String {
    path.rsplit('/')
        .next()
        .and_then(|f| f.rsplit_once('.'))
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Focus the terminal input element.
fn focus_input() {
    if let Some(window) = web_sys::window()
        && let Some(document) = window.document()
        && let Some(input) = document.query_selector("input").ok().flatten()
        && let Ok(element) = input.dyn_into::<web_sys::HtmlElement>()
    {
        let _ = element.focus();
    }
}

// ============================================================================
// Shell Component
// ============================================================================

#[component]
pub fn Shell() -> impl IntoView {
    // Get context provided by App component
    let ctx = use_context::<AppContext>()
        .expect("AppContext must be provided at root");

    let output_ref = NodeRef::<leptos::html::Div>::new();

    // Set up all effects
    setup_boot_effect(ctx);
    setup_url_sync_effect(ctx.terminal.screen_mode);
    setup_popstate_handler(ctx);
    setup_autoscroll_effect(ctx.terminal.history, output_ref);
    setup_wallet_events(ctx);

    // Derived signals
    let prompt = Signal::derive(move || ctx.get_prompt());

    // Callbacks
    let on_submit = create_submit_callback(ctx);
    let on_history_nav = create_history_nav_callback(ctx);
    let on_autocomplete = create_autocomplete_callback(ctx);
    let on_get_hint = create_hint_callback(ctx);

    let on_reader_close = Callback::new(move |_: ()| {
        ctx.terminal.screen_mode.set(ScreenMode::Terminal);
    });

    let handle_click = move |_| focus_input();

    view! {
        <div class=css::screen>
            <div class=css::crtOverlay></div>
            <div class=css::scanline></div>

            <Status />

            <div class=css::main>
                {move || {
                    let mode = ctx.terminal.screen_mode.get();
                    match mode {
                        ScreenMode::Booting | ScreenMode::Terminal => {
                            let history_signal = ctx.terminal.history;
                            view! {
                                <div class=css::container on:click=handle_click>
                                    <div
                                        node_ref=output_ref
                                        class=css::output
                                    >
                                        <For
                                            each=move || history_signal.get().to_vec()
                                            key=|line| line.id
                                            children=|line| view! { <Output line=line /> }
                                        />
                                    </div>

                                    <Show
                                        when=move || matches!(ctx.terminal.screen_mode.get(), ScreenMode::Terminal)
                                        fallback=|| ()
                                    >
                                        <div class=css::inputArea>
                                            <Input
                                                prompt=prompt
                                                on_submit=on_submit
                                                on_history_nav=on_history_nav
                                                on_autocomplete=on_autocomplete
                                                on_get_hint=on_get_hint
                                            />
                                        </div>
                                    </Show>
                                </div>
                            }.into_any()
                        }
                        ScreenMode::Reader { content, title } => {
                            view! {
                                <Reader
                                    content_path=content
                                    title=title
                                    on_close=on_reader_close
                                />
                            }.into_any()
                        }
                    }
                }}
            </div>
        </div>
    }
}

// ============================================================================
// Callback Factories
// ============================================================================

fn create_submit_callback(ctx: AppContext) -> Callback<String> {
    Callback::new(move |input: String| {
        let prompt = ctx.get_prompt();

        if !input.is_empty() {
            ctx.terminal.push_output(OutputLine::command(prompt, &input));
            ctx.terminal.add_to_command_history(&input);
        }

        // Parse with new parser (supports variables, history, pipes)
        let pipeline = ctx.terminal.command_history.with(|history| {
            parse_input(&input, history)
        });

        // Check for special async commands (only when single command, no pipes)
        if pipeline.commands.len() == 1 {
            let cmd_name = pipeline.first_command_name().unwrap_or("");
            match cmd_name.to_lowercase().as_str() {
                "login" => {
                    handle_login(ctx);
                    return;
                }
                "logout" => {
                    handle_logout(&ctx);
                    return;
                }
                _ => {}
            }
        }

        // Execute pipeline
        let current_fs = ctx.fs.get();
        let wallet_state = ctx.wallet.get();
        let output = execute_pipeline(&pipeline, &ctx.terminal, &wallet_state, &current_fs);
        ctx.terminal.push_lines(output);
    })
}

fn create_history_nav_callback(ctx: AppContext) -> Callback<i32, Option<String>> {
    Callback::new(move |direction: i32| ctx.terminal.navigate_history(direction))
}

fn create_autocomplete_callback(ctx: AppContext) -> Callback<String, crate::core::AutocompleteResult> {
    Callback::new(move |input: String| {
        ctx.terminal.current_path.with(|current_path| {
            ctx.fs.with(|current_fs| {
                autocomplete(&input, current_path, current_fs)
            })
        })
    })
}

fn create_hint_callback(ctx: AppContext) -> Callback<String, Option<String>> {
    Callback::new(move |input: String| {
        ctx.terminal.current_path.with(|current_path| {
            ctx.fs.with(|current_fs| {
                get_hint(&input, current_path, current_fs)
            })
        })
    })
}
