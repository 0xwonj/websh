//! Terminal view component.
//!
//! The terminal interface with output history and command input.

use leptos::prelude::*;

use crate::app::AppContext;
use crate::components::terminal::{Input, Output, RouteContext};
use websh_core::filesystem::route_cwd;
use websh_core::runtime;
use websh_core::shell::{
    SideEffect, autocomplete, execute_pipeline, get_hint, parse_input,
};
use websh_core::domain::OutputLine;
use crate::utils::dom::focus_terminal_input;

stylance::import_crate_style!(css, "src/components/terminal/terminal.module.css");

/// Execute wallet login command asynchronously.
fn handle_login(ctx: AppContext) {
    wasm_bindgen_futures::spawn_local(async move {
        ctx.terminal
            .push_output(OutputLine::info("Connecting to wallet..."));

        match crate::components::wallet::connect_with_session(&ctx).await {
            Ok(outcome) => {
                if let Some(error) = outcome.session_persist_error {
                    ctx.terminal.push_output(OutputLine::error(format!(
                        "login: failed to persist session: {error}"
                    )));
                }
                ctx.terminal.push_output(OutputLine::success(format!(
                    "Connected: {}",
                    outcome.address
                )));
                if let Some(id) = outcome.chain_id {
                    ctx.terminal.push_output(OutputLine::info(format!(
                        "Network: {} (chain_id={})",
                        websh_core::domain::chain_name(id),
                        id
                    )));
                }
                if let Some(ens) = outcome.ens_name {
                    ctx.terminal
                        .push_output(OutputLine::success(format!("ENS: {}", ens)));
                }
            }
            Err(e) => ctx
                .terminal
                .push_output(OutputLine::error(format!("Connection failed: {}", e))),
        }
    });
}

/// Execute wallet logout command.
fn handle_logout(ctx: &AppContext) {
    if ctx.wallet.with(|w| w.is_connected()) {
        match crate::components::wallet::disconnect(ctx) {
            Ok(()) => ctx
                .terminal
                .push_output(OutputLine::success("Disconnected from wallet.")),
            Err(error) => ctx.terminal.push_output(OutputLine::error(format!(
                "logout: failed to clear session: {error}"
            ))),
        }
    } else {
        ctx.terminal
            .push_output(OutputLine::info("No wallet connected."));
    }
}

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

fn create_submit_callback(ctx: AppContext, route_ctx: RouteContext) -> Callback<String> {
    Callback::new(move |input: String| {
        let current_frame = route_ctx.0.get();
        let cwd = route_cwd(&current_frame);
        let prompt = ctx.get_prompt(&cwd);
        let display_input = display_command(&input);

        if !input.is_empty() {
            ctx.terminal
                .push_output(OutputLine::command(prompt, &display_input));
            if should_store_command_history(&input) {
                ctx.terminal.add_to_command_history(&input);
            } else {
                ctx.terminal.history_index.set(None);
            }
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
        SideEffect::ClearHistory => ctx.terminal.clear_history(),
        SideEffect::SetTheme { theme } => match crate::utils::theme::apply_theme(&theme) {
            Ok(theme_id) => {
                ctx.theme.set(theme_id);
                ctx.runtime_state
                    .set(websh_core::runtime::state::snapshot());
            }
            Err(error) => ctx.terminal.push_output(OutputLine::error(error)),
        },
        SideEffect::ApplyChange { path, change } => {
            ctx.changes.update(|cs| cs.upsert(path, *change));
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
            match websh_core::runtime::state::set_github_token(&token) {
                Ok(snapshot) => ctx.runtime_state.set(snapshot),
                Err(error) => ctx.terminal.push_output(OutputLine::error(format!(
                    "sync auth: failed to persist token: {error}"
                ))),
            }
        }
        SideEffect::ClearAuthToken => match websh_core::runtime::state::clear_github_token() {
            Ok(snapshot) => ctx.runtime_state.set(snapshot),
            Err(error) => ctx.terminal.push_output(OutputLine::error(format!(
                "sync auth: failed to clear token: {error}"
            ))),
        },
        SideEffect::InvalidateRuntimeState => {
            ctx.runtime_state
                .set(websh_core::runtime::state::snapshot());
        }
        SideEffect::OpenEditor { path } => {
            ctx.editor_open.set(Some(path));
        }
        SideEffect::Commit {
            message,
            mount_root,
        } => {
            let Some(backend) = ctx.backend_for_mount_root(&mount_root) else {
                ctx.terminal
                    .push_output(websh_core::domain::OutputLine::error(format!(
                        "sync: no backend registered at mount root {}",
                        mount_root.as_str()
                    )));
                return;
            };
            let changes_signal = ctx.changes;
            let runtime_mounts_signal = ctx.runtime_mounts;
            let terminal = ctx.terminal;
            let mount_root_for_commit = mount_root.clone();
            let expected_head = ctx.remote_head_for_path(&mount_root_for_commit);
            let auth_token = runtime::state::github_token_for_commit();
            let app_ctx = *ctx;

            wasm_bindgen_futures::spawn_local(async move {
                let staged_snapshot = changes_signal.with_untracked(|cs| cs.clone());

                match runtime::commit_backend(
                    backend,
                    mount_root_for_commit.clone(),
                    staged_snapshot,
                    message,
                    expected_head,
                    auth_token,
                )
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
                        if let Ok(db) = websh_core::storage::idb::open_db().await {
                            let _ = websh_core::storage::idb::save_metadata(
                                &db,
                                &format!("remote_head.{mount_storage_id}"),
                                &head_val,
                            )
                            .await;
                        }

                        match runtime::reload_runtime().await {
                            Ok(load) => {
                                app_ctx.apply_runtime_load(load);
                            }
                            Err(error) => terminal.push_output(websh_core::domain::OutputLine::info(
                                format!("sync: commit ok, runtime reload failed: {error}"),
                            )),
                        }

                        let committed = outcome.committed_paths.clone();
                        changes_signal.update(|cs| {
                            for p in committed.iter() {
                                cs.discard(p);
                            }
                        });

                        terminal.push_output(websh_core::domain::OutputLine::info(format!(
                            "sync: committed {} files (HEAD now {}).",
                            outcome.committed_paths.len(),
                            &outcome.new_head[..outcome.new_head.len().min(8)]
                        )));
                    }
                    Err(e) => {
                        terminal
                            .push_output(websh_core::domain::OutputLine::error(format!("sync: {e}")));
                    }
                }
            });
        }
        SideEffect::ReloadRuntimeMount { mount_root: _ } => {
            let app_ctx = *ctx;
            let terminal = ctx.terminal;
            wasm_bindgen_futures::spawn_local(async move {
                match runtime::reload_runtime().await {
                    Ok(load) => {
                        app_ctx.apply_runtime_load(load);
                        terminal.push_output(websh_core::domain::OutputLine::info(
                            "sync: runtime reloaded.".to_string(),
                        ));
                    }
                    Err(error) => terminal.push_output(websh_core::domain::OutputLine::error(format!(
                        "sync refresh: {error}"
                    ))),
                }
            });
        }
    }
}

fn display_command(input: &str) -> String {
    let trimmed = input.trim_start();
    if is_sync_auth_set(trimmed) {
        let leading = &input[..input.len() - trimmed.len()];
        format!("{leading}sync auth set <redacted>")
    } else {
        input.to_string()
    }
}

fn should_store_command_history(input: &str) -> bool {
    !is_sync_auth_set(input.trim_start())
}

fn is_sync_auth_set(input: &str) -> bool {
    let mut parts = input.split_whitespace();
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some("sync"), Some("auth"), Some("set"), Some(_))
    )
}

fn mount_id_for_root(root: &websh_core::domain::VirtualPath) -> String {
    if root.is_root() {
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
) -> Callback<String, websh_core::shell::AutocompleteResult> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_set_command_is_redacted_for_display() {
        assert_eq!(
            display_command("sync auth set ghp_secret"),
            "sync auth set <redacted>"
        );
        assert_eq!(
            display_command("  sync auth set ghp_secret"),
            "  sync auth set <redacted>"
        );
    }

    #[test]
    fn auth_set_command_is_not_stored_in_history() {
        assert!(!should_store_command_history("sync auth set ghp_secret"));
        assert!(should_store_command_history("sync auth clear"));
    }
}
