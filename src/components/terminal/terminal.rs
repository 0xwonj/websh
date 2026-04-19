//! Terminal view component.
//!
//! The terminal interface with output history and command input.

use leptos::prelude::*;

use crate::app::AppContext;
use crate::components::terminal::{Input, Output, RouteContext};
use crate::core::{autocomplete, execute_pipeline, get_hint, parse_input, wallet};
use crate::models::{AppRoute, OutputLine, ViewMode, WalletState};
use crate::utils::dom::focus_terminal_input;

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
// Commit Handler
// ============================================================================

use crate::core::admin::is_admin;
use crate::core::storage::{GitHubBackend, PendingChanges, StorageBackend, local};

/// Execute commit command asynchronously.
fn handle_commit(ctx: AppContext, message: Option<String>) {
    wasm_bindgen_futures::spawn_local(async move {
        // Check admin permission
        let wallet_state = ctx.wallet.get();
        if !is_admin(&wallet_state) {
            ctx.terminal.push_output(OutputLine::error(
                "Permission denied: admin access required. Use 'login' to connect wallet.",
            ));
            return;
        }

        // Get pending changes
        let pending = ctx.fs.pending().get();
        if pending.is_empty() {
            ctx.terminal.push_output(OutputLine::info(
                "Nothing to commit. Working directory clean.",
            ));
            return;
        }

        // Check GitHub token
        if !local::has_github_token() {
            ctx.terminal.push_output(OutputLine::error(
                "No GitHub token configured.",
            ));
            ctx.terminal.push_output(OutputLine::text(
                "Use 'auth github <token>' to set your Personal Access Token.",
            ));
            return;
        }

        // Get GitHub backend
        let backend = ctx.get_github_backend();
        let Some(backend) = backend else {
            ctx.terminal.push_output(OutputLine::error(
                "No writable GitHub mount configured.",
            ));
            return;
        };

        let commit_message = message.unwrap_or_else(|| "Update via websh".to_string());
        let summary = pending.summary();

        ctx.terminal.push_output(OutputLine::info(format!(
            "Committing {} changes: \"{}\"",
            summary.total(),
            commit_message
        )));

        // Execute commit
        match backend.commit(&pending, &commit_message).await {
            Ok(new_manifest) => {
                // Clear pending changes
                ctx.fs.pending().set(PendingChanges::default());
                let _ = local::save_pending_changes(&PendingChanges::default());

                // Update VirtualFs with new manifest
                let new_fs = crate::core::VirtualFs::from_manifest(&new_manifest);
                ctx.fs.set_base(new_fs);

                ctx.terminal.push_output(OutputLine::success(format!(
                    "Successfully committed {} changes.",
                    summary.total()
                )));
                ctx.terminal.push_output(OutputLine::info(format!(
                    "{} additions, {} modifications, {} deletions",
                    summary.creates, summary.updates, summary.deletes
                )));
            }
            Err(e) => {
                ctx.terminal.push_output(OutputLine::error(format!(
                    "Commit failed: {}",
                    e
                )));
            }
        }
    });
}

// ============================================================================
// Terminal Component
// ============================================================================

#[component]
pub fn Terminal(output_ref: NodeRef<leptos::html::Div>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided at root");
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    // Derived signals
    let prompt = Signal::derive(move || {
        let route = route_ctx.0.get();
        ctx.get_prompt(&route)
    });

    // Callbacks need route access
    let on_submit = create_submit_callback(ctx, route_ctx);
    let on_history_nav = create_history_nav_callback(ctx);
    let on_autocomplete = create_autocomplete_callback(ctx, route_ctx);
    let on_get_hint = create_hint_callback(ctx, route_ctx);

    let handle_click = move |_| focus_terminal_input();
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

            <div class=css::inputArea>
                <Input
                    prompt=prompt
                    on_submit=on_submit
                    on_history_nav=on_history_nav
                    on_autocomplete=on_autocomplete
                    on_get_hint=on_get_hint
                />
            </div>
        </div>
    }
}

// ============================================================================
// Callback Factories
// ============================================================================

fn create_submit_callback(ctx: AppContext, route_ctx: RouteContext) -> Callback<String> {
    Callback::new(move |input: String| {
        let current_route = route_ctx.0.get();
        let prompt = ctx.get_prompt(&current_route);

        if !input.is_empty() {
            ctx.terminal
                .push_output(OutputLine::command(prompt, &input));
            ctx.terminal.add_to_command_history(&input);
        }

        let pipeline = ctx
            .terminal
            .command_history
            .with(|history| parse_input(&input, history));

        // Handle special commands that need async or view switching
        if pipeline.commands.len() == 1 {
            let first_cmd = &pipeline.commands[0];
            match first_cmd.name.to_lowercase().as_str() {
                "login" => {
                    handle_login(ctx);
                    return;
                }
                "logout" => {
                    handle_logout(&ctx);
                    return;
                }
                "commit" => {
                    let message = if first_cmd.args.is_empty() {
                        None
                    } else {
                        Some(first_cmd.args.join(" "))
                    };
                    handle_commit(ctx, message);
                    return;
                }
                "explorer" => {
                    // Switch to explorer view, optionally navigate first
                    if let Some(path_arg) = first_cmd.args.first() {
                        let current_path = current_route.fs_path();
                        let merged_fs = ctx.fs.get();

                        match merged_fs.resolve_path(current_path, path_arg) {
                            Some(new_path) if merged_fs.is_directory(&new_path) => {
                                // Navigate to the new path and switch to explorer
                                let new_route = fs_path_to_browse_route(&new_path);
                                new_route.push();
                            }
                            Some(_) => {
                                ctx.terminal.push_output(OutputLine::error(format!(
                                    "explorer: not a directory: {}",
                                    path_arg
                                )));
                                return;
                            }
                            None => {
                                ctx.terminal.push_output(OutputLine::error(format!(
                                    "explorer: no such file or directory: {}",
                                    path_arg
                                )));
                                return;
                            }
                        }
                    }
                    ctx.view_mode.set(ViewMode::Explorer);
                    return;
                }
                _ => {}
            }
        }

        let merged_fs = ctx.fs.get();
        let wallet_state = ctx.wallet.get();
        let pending = ctx.fs.pending().get();
        let staged = ctx.fs.staged().get();
        let result = execute_pipeline(
            &pipeline,
            &ctx.terminal,
            &wallet_state,
            &merged_fs,
            &current_route,
            &pending,
            &staged,
        );

        // Handle pending changes update (signal + localStorage)
        if let Some(new_pending) = result.pending {
            ctx.fs.pending().set(new_pending.clone());
            let _ = crate::core::storage::local::save_pending_changes(&new_pending);
        }

        // Handle staged changes update
        if let Some(new_staged) = result.staged {
            ctx.fs.staged().set(new_staged);
        }

        // Handle navigation if command requested it
        if let Some(new_route) = result.navigate_to {
            new_route.push();
        }

        ctx.terminal.push_lines(result.output);
    })
}

fn create_history_nav_callback(ctx: AppContext) -> Callback<i32, Option<String>> {
    Callback::new(move |direction: i32| ctx.terminal.navigate_history(direction))
}

fn create_autocomplete_callback(
    ctx: AppContext,
    route_ctx: RouteContext,
) -> Callback<String, crate::core::AutocompleteResult> {
    Callback::new(move |input: String| {
        let current_route = route_ctx.0.get();
        let merged_fs = ctx.fs.get();
        autocomplete(&input, &current_route, &merged_fs)
    })
}

fn create_hint_callback(
    ctx: AppContext,
    route_ctx: RouteContext,
) -> Callback<String, Option<String>> {
    Callback::new(move |input: String| {
        let current_route = route_ctx.0.get();
        let merged_fs = ctx.fs.get();
        get_hint(&input, &current_route, &merged_fs)
    })
}

// ============================================================================
// Path to Route Helper
// ============================================================================

/// Convert a filesystem path (relative) to a Browse route.
fn fs_path_to_browse_route(fs_path: &str) -> AppRoute {
    let mount = crate::config::configured_mounts()
        .into_iter()
        .next()
        .expect("At least one mount must be configured");

    AppRoute::Browse {
        mount,
        path: fs_path.to_string(),
    }
}
