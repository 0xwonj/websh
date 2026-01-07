//! Terminal view component.
//!
//! The terminal interface with output history and command input.

use leptos::prelude::*;

use crate::app::AppContext;
use crate::components::terminal::{Input, Output};
use crate::core::{autocomplete, execute_pipeline, get_hint, parse_input, wallet};
use crate::models::{OutputLine, ScreenMode, WalletState};

stylance::import_crate_style!(css, "src/components/terminal/terminal.module.css");

// ============================================================================
// Wallet Handlers
// ============================================================================

/// Execute wallet login command asynchronously.
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
// Helper Functions
// ============================================================================

/// Focus the terminal input element.
fn focus_input() {
    use wasm_bindgen::JsCast;
    if let Some(window) = web_sys::window()
        && let Some(document) = window.document()
        && let Some(input) = document.query_selector("input").ok().flatten()
        && let Ok(element) = input.dyn_into::<web_sys::HtmlElement>()
    {
        let _ = element.focus();
    }
}

// ============================================================================
// Terminal Component
// ============================================================================

#[component]
pub fn Terminal(output_ref: NodeRef<leptos::html::Div>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided at root");

    // Derived signals
    let prompt = Signal::derive(move || ctx.get_prompt());

    // Callbacks
    let on_submit = create_submit_callback(ctx);
    let on_history_nav = create_history_nav_callback(ctx);
    let on_autocomplete = create_autocomplete_callback(ctx);
    let on_get_hint = create_hint_callback(ctx);

    let handle_click = move |_| focus_input();
    let history_signal = ctx.terminal.history;

    view! {
        <div class=css::container on:click=handle_click>
            <div node_ref=output_ref class=css::output>
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
    }
}

// ============================================================================
// Callback Factories
// ============================================================================

fn create_submit_callback(ctx: AppContext) -> Callback<String> {
    Callback::new(move |input: String| {
        let prompt = ctx.get_prompt();

        if !input.is_empty() {
            ctx.terminal
                .push_output(OutputLine::command(prompt, &input));
            ctx.terminal.add_to_command_history(&input);
        }

        let pipeline = ctx
            .terminal
            .command_history
            .with(|history| parse_input(&input, history));

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

        let current_fs = ctx.fs.get();
        let wallet_state = ctx.wallet.get();
        let output = execute_pipeline(&pipeline, &ctx.terminal, &wallet_state, &current_fs);
        ctx.terminal.push_lines(output);
    })
}

fn create_history_nav_callback(ctx: AppContext) -> Callback<i32, Option<String>> {
    Callback::new(move |direction: i32| ctx.terminal.navigate_history(direction))
}

fn create_autocomplete_callback(
    ctx: AppContext,
) -> Callback<String, crate::core::AutocompleteResult> {
    Callback::new(move |input: String| {
        ctx.terminal.current_path.with(|current_path| {
            ctx.fs
                .with(|current_fs| autocomplete(&input, current_path, current_fs))
        })
    })
}

fn create_hint_callback(ctx: AppContext) -> Callback<String, Option<String>> {
    Callback::new(move |input: String| {
        ctx.terminal.current_path.with(|current_path| {
            ctx.fs
                .with(|current_fs| get_hint(&input, current_path, current_fs))
        })
    })
}
