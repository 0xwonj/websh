//! Terminal view component.
//!
//! The terminal interface with output history and command input.

use leptos::prelude::*;

use crate::app::AppContext;
use crate::components::terminal::{Input, Output, RouteContext};
use crate::core::engine::route_cwd;
use crate::core::{
    SideEffect, autocomplete, execute_pipeline, get_hint, parse_input, runtime, wallet,
};
use crate::models::{OutputLine, WalletState};
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
                "No EIP-1193 wallet found. Please install a browser wallet extension.",
            ));
            return;
        }

        ctx.wallet.set(WalletState::Connecting);
        ctx.terminal
            .push_output(OutputLine::info("Connecting to wallet..."));

        match wallet::connect().await {
            Ok(address) => {
                wallet::save_session();
                ctx.runtime_state_rev.update(|rev| *rev += 1);
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
        wallet::disconnect(ctx);
        ctx.terminal
            .push_output(OutputLine::success("Disconnected from wallet."));
    } else {
        ctx.terminal
            .push_output(OutputLine::info("No wallet connected."));
    }
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
        ctx.get_prompt(&route_cwd(&route))
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
                    each=move || history_signal.with(|buf| buf.iter().cloned().collect::<Vec<_>>())
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
        let current_frame = route_ctx.0.get();
        let cwd = route_cwd(&current_frame);
        let prompt = ctx.get_prompt(&cwd);

        if !input.is_empty() {
            ctx.terminal
                .push_output(OutputLine::command(prompt, &input));
            ctx.terminal.add_to_command_history(&input);
        }

        let pipeline = ctx
            .terminal
            .command_history
            .with(|history| parse_input(&input, history));

        let wallet_state = ctx.wallet.get();
        let remote_head = ctx.remote_head_for_path(&cwd);
        let runtime_mounts = ctx.runtime_mounts.get();
        let result = ctx.changes.with_untracked(|changes| {
            ctx.view_global_fs.with(|current_fs| {
                execute_pipeline(
                    &pipeline,
                    &ctx.terminal,
                    &wallet_state,
                    &runtime_mounts,
                    current_fs,
                    &cwd,
                    changes,
                    remote_head.as_deref(),
                )
            })
        });

        ctx.terminal.push_lines(result.output);

        if let Some(effect) = result.side_effect {
            dispatch_side_effect(&ctx, effect);
        }
    })
}

/// Perform a side effect requested by a command.
pub fn dispatch_side_effect(ctx: &AppContext, effect: SideEffect) {
    match effect {
        SideEffect::Navigate(route) => route.push(),
        SideEffect::Login => handle_login(*ctx),
        SideEffect::Logout => handle_logout(ctx),
        SideEffect::SwitchView(mode) => ctx.view_mode.set(mode),
        SideEffect::SwitchViewAndNavigate(mode, route) => {
            route.push();
            ctx.view_mode.set(mode);
        }
        SideEffect::ApplyChange { path, change } => {
            ctx.changes.update(|cs| cs.upsert(path, change));
        }
        SideEffect::StageChange { path } => {
            ctx.changes.update(|cs| cs.stage(&path));
        }
        SideEffect::UnstageChange { path } => {
            ctx.changes.update(|cs| cs.unstage(&path));
        }
        SideEffect::DiscardChange { path } => {
            ctx.changes.update(|cs| cs.discard(&path));
        }
        SideEffect::StageAll => {
            ctx.changes.update(|cs| cs.stage_all());
        }
        SideEffect::UnstageAll => {
            ctx.changes.update(|cs| cs.unstage_all());
        }
        SideEffect::SetAuthToken { token } => {
            crate::utils::session::set_gh_token(&token);
            ctx.runtime_state_rev.update(|rev| *rev += 1);
        }
        SideEffect::ClearAuthToken => {
            crate::utils::session::clear_gh_token();
            ctx.runtime_state_rev.update(|rev| *rev += 1);
        }
        SideEffect::InvalidateRuntimeState => {
            ctx.runtime_state_rev.update(|rev| *rev += 1);
        }
        SideEffect::OpenEditor { path } => {
            ctx.editor_open.set(Some(path));
        }
        SideEffect::Commit {
            message,
            expected_head,
            mount_root,
        } => {
            let Some(backend) = ctx.backend_for_path(&mount_root) else {
                ctx.terminal.push_output(crate::models::OutputLine::error(
                    "sync: no backend for path".to_string(),
                ));
                return;
            };
            let changes_signal = ctx.changes;
            let global_fs_signal = ctx.global_fs;
            let heads_signal = ctx.remote_heads;
            let backends_store = ctx.backends;
            let runtime_mounts_signal = ctx.runtime_mounts;
            let terminal = ctx.terminal;
            let mount_root_for_commit = mount_root.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let staged_snapshot = changes_signal.with_untracked(|cs| cs.clone());

                match backend
                    .commit(&staged_snapshot, &message, expected_head.as_deref())
                    .await
                {
                    Ok(outcome) => {
                        let mount_storage_id = runtime_mounts_signal
                            .get_untracked()
                            .into_iter()
                            .find(|mount| mount.root == mount_root_for_commit)
                            .map(|mount| mount.storage_id())
                            .unwrap_or_else(|| mount_id_for_root(&mount_root_for_commit));
                        let head_val = outcome.new_head.clone();
                        if let Ok(db) = crate::core::storage::idb::open_db().await {
                            let _ = crate::core::storage::idb::save_metadata(
                                &db,
                                &format!("remote_head.{mount_storage_id}"),
                                &head_val,
                            )
                            .await;
                        }

                        match runtime::reload_runtime().await {
                            Ok(load) => {
                                global_fs_signal.set(load.global_fs);
                                backends_store.set_value(load.backends);
                                runtime_mounts_signal.set(load.runtime_mounts);
                                heads_signal.set(load.remote_heads);
                            }
                            Err(error) => terminal.push_output(crate::models::OutputLine::info(
                                format!("sync: commit ok, runtime reload failed: {error}"),
                            )),
                        }

                        let committed = outcome.committed_paths.clone();
                        changes_signal.update(|cs| {
                            for p in committed.iter() {
                                cs.discard(p);
                            }
                        });

                        terminal.push_output(crate::models::OutputLine::info(format!(
                            "sync: committed {} files (HEAD now {}).",
                            outcome.committed_paths.len(),
                            &outcome.new_head[..outcome.new_head.len().min(8)]
                        )));
                    }
                    Err(e) => {
                        terminal
                            .push_output(crate::models::OutputLine::error(format!("sync: {e}")));
                    }
                }
            });
        }
        SideEffect::ReloadRuntimeMount { mount_root } => {
            let Some(backend) = ctx.backend_for_path(&mount_root) else {
                ctx.terminal.push_output(crate::models::OutputLine::error(
                    "sync refresh: no backend".to_string(),
                ));
                return;
            };
            let global_fs_signal = ctx.global_fs;
            let backends_store = ctx.backends;
            let runtime_mounts_signal = ctx.runtime_mounts;
            let heads_signal = ctx.remote_heads;
            let terminal = ctx.terminal;
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = backend.scan().await {
                    terminal.push_output(crate::models::OutputLine::error(format!(
                        "sync refresh: {e}"
                    )));
                    return;
                }

                match runtime::reload_runtime().await {
                    Ok(load) => {
                        global_fs_signal.set(load.global_fs);
                        backends_store.set_value(load.backends);
                        runtime_mounts_signal.set(load.runtime_mounts);
                        heads_signal.set(load.remote_heads);
                        terminal.push_output(crate::models::OutputLine::info(
                            "sync: runtime reloaded.".to_string(),
                        ));
                    }
                    Err(error) => terminal.push_output(crate::models::OutputLine::error(format!(
                        "sync refresh: {error}"
                    ))),
                }
            });
        }
    }
}

fn mount_id_for_root(root: &crate::models::VirtualPath) -> String {
    if root.as_str() == "/site" {
        "~".to_string()
    } else {
        root.file_name()
            .map(str::to_string)
            .unwrap_or_else(|| root.as_str().to_string())
    }
}

fn create_history_nav_callback(ctx: AppContext) -> Callback<i32, Option<String>> {
    Callback::new(move |direction: i32| ctx.terminal.navigate_history(direction))
}

fn create_autocomplete_callback(
    ctx: AppContext,
    route_ctx: RouteContext,
) -> Callback<String, crate::core::AutocompleteResult> {
    Callback::new(move |input: String| {
        let cwd = route_cwd(&route_ctx.0.get());
        ctx.view_global_fs
            .with(|current_fs| autocomplete(&input, &cwd, current_fs))
    })
}

fn create_hint_callback(
    ctx: AppContext,
    route_ctx: RouteContext,
) -> Callback<String, Option<String>> {
    Callback::new(move |input: String| {
        let cwd = route_cwd(&route_ctx.0.get());
        ctx.view_global_fs
            .with(|current_fs| get_hint(&input, &cwd, current_fs))
    })
}
