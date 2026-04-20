//! Terminal view component.
//!
//! The terminal interface with output history and command input.

use leptos::prelude::*;

use crate::app::AppContext;
use crate::components::terminal::{Input, Output, RouteContext};
use crate::core::{SideEffect, autocomplete, execute_pipeline, get_hint, parse_input, wallet};
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

        let wallet_state = ctx.wallet.get();
        let remote_head = ctx.remote_head.get_value();
        let result = ctx.changes.with_untracked(|changes| {
            ctx.fs.with(|current_fs| {
                execute_pipeline(
                    &pipeline,
                    &ctx.terminal,
                    &wallet_state,
                    current_fs,
                    &current_route,
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
fn dispatch_side_effect(ctx: &AppContext, effect: SideEffect) {
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
            let home = crate::config::mounts().home();
            let backend = crate::core::storage::boot::build_backend_for_mount(home, Some(&token));
            ctx.backend.set_value(backend);
        }
        SideEffect::ClearAuthToken => {
            crate::utils::session::clear_gh_token();
            ctx.backend.set_value(None);
        }
        SideEffect::OpenEditor { path } => {
            // Phase 5 wires this to the EditModal. For 3a, emit an info line.
            ctx.terminal.push_output(crate::models::OutputLine::info(
                format!("edit: opening {}", path.as_str())
            ));
        }
        SideEffect::Commit { message, expected_head } => {
            let Some(backend) = ctx.backend.get_value() else {
                ctx.terminal.push_output(crate::models::OutputLine::error(
                    "sync: no backend (not authenticated?)".to_string()
                ));
                return;
            };
            let changes_signal = ctx.changes;
            let fs_signal = ctx.fs;
            let head_store = ctx.remote_head;
            let terminal = ctx.terminal;
            let home = crate::config::mounts().home();
            let mount_id = home.alias().to_string();

            wasm_bindgen_futures::spawn_local(async move {
                let staged_snapshot = changes_signal.with_untracked(|cs| cs.clone());
                let merged = fs_signal.with_untracked(|base| {
                    crate::core::merge::merge_view(base, &staged_snapshot)
                });
                let new_manifest = merged.serialize_manifest();
                let manifest_body = serde_json::to_string_pretty(&new_manifest).unwrap_or_default();

                let mut snapshot_with_manifest = staged_snapshot.clone();
                let manifest_path = crate::models::VirtualPath::from_absolute("/manifest.json")
                    .expect("valid path");
                snapshot_with_manifest.upsert(
                    manifest_path.clone(),
                    crate::core::changes::ChangeType::UpdateFile {
                        content: manifest_body,
                        description: None,
                    },
                );

                match backend.commit(&snapshot_with_manifest, &message, expected_head.as_deref()).await {
                    Ok(outcome) => {
                        match backend.fetch_manifest().await {
                            Ok(manifest) => fs_signal.set(
                                crate::core::VirtualFs::from_manifest(&manifest)
                            ),
                            Err(e) => terminal.push_output(crate::models::OutputLine::info(
                                format!("sync: commit ok, refresh failed: {e}")
                            )),
                        }

                        let committed = outcome.committed_paths.clone();
                        changes_signal.update(|cs| {
                            for p in committed.iter() {
                                cs.discard(p);
                            }
                            cs.discard(&manifest_path);
                        });

                        head_store.set_value(Some(outcome.new_head.clone()));
                        let head_val = outcome.new_head.clone();
                        let mid = mount_id.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            if let Ok(db) = crate::core::storage::idb::open_db().await {
                                let _ = crate::core::storage::idb::save_metadata(
                                    &db, &format!("remote_head.{mid}"), &head_val,
                                ).await;
                            }
                        });

                        terminal.push_output(crate::models::OutputLine::info(format!(
                            "sync: committed {} files (HEAD now {}).",
                            outcome.committed_paths.len(),
                            &outcome.new_head[..outcome.new_head.len().min(8)]
                        )));
                    }
                    Err(e) => {
                        terminal.push_output(crate::models::OutputLine::error(format!("sync: {e}")));
                    }
                }
            });
        }
        SideEffect::RefreshManifest => {
            let Some(backend) = ctx.backend.get_value() else {
                ctx.terminal.push_output(crate::models::OutputLine::error(
                    "sync refresh: no backend".to_string()
                ));
                return;
            };
            let fs_signal = ctx.fs;
            let terminal = ctx.terminal;
            wasm_bindgen_futures::spawn_local(async move {
                match backend.fetch_manifest().await {
                    Ok(manifest) => {
                        fs_signal.set(crate::core::VirtualFs::from_manifest(&manifest));
                        terminal.push_output(crate::models::OutputLine::info(
                            "sync: manifest refreshed.".to_string()
                        ));
                    }
                    Err(e) => terminal.push_output(crate::models::OutputLine::error(
                        format!("sync refresh: {e}")
                    )),
                }
            });
        }
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
        let current_route = route_ctx.0.get();
        ctx.fs
            .with(|current_fs| autocomplete(&input, &current_route, current_fs))
    })
}

fn create_hint_callback(
    ctx: AppContext,
    route_ctx: RouteContext,
) -> Callback<String, Option<String>> {
    Callback::new(move |input: String| {
        let current_route = route_ctx.0.get();
        ctx.fs
            .with(|current_fs| get_hint(&input, &current_route, current_fs))
    })
}

