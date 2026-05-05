use leptos::prelude::*;

use crate::app::AppContext;
use crate::app::RuntimeServices;
use crate::platform::dom::push_route;
use crate::runtime::shell_execution_context;
use websh_core::filesystem::route_cwd;
use websh_core::shell::OutputLine;
use websh_core::shell::{
    SideEffect, autocomplete, execute_pipeline_with_context, get_hint, parse_input_with_env,
};

use super::RouteContext;

fn handle_login(ctx: AppContext) {
    wasm_bindgen_futures::spawn_local(async move {
        ctx.terminal
            .push_output(OutputLine::info("Connecting to wallet..."));

        match RuntimeServices::new(ctx)
            .connect_wallet_with_session()
            .await
        {
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

fn handle_logout(ctx: &AppContext) {
    if ctx.wallet.with(|w| w.is_connected()) {
        match RuntimeServices::new(*ctx).disconnect_wallet() {
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

pub(super) fn create_submit_callback(ctx: AppContext, route_ctx: RouteContext) -> Callback<String> {
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

        let runtime_state = ctx.runtime_state.get();
        let pipeline = ctx
            .terminal
            .command_history
            .with(|history| parse_input_with_env(&input, history, &runtime_state.env));

        let wallet_state = ctx.wallet.get();
        let remote_head = ctx.remote_head_for_path(&cwd);
        let runtime_mounts = ctx.runtime_mounts_snapshot();
        let execution_context = shell_execution_context(&runtime_state);
        let result = ctx.changes.with_untracked(|changes| {
            ctx.system_global_fs.with(|current_fs| {
                execute_pipeline_with_context(
                    &pipeline,
                    &wallet_state,
                    &runtime_mounts,
                    current_fs,
                    &cwd,
                    changes,
                    remote_head.as_deref(),
                    &execution_context,
                )
            })
        });

        ctx.terminal.push_lines(result.output);

        for effect in result.side_effects {
            dispatch_side_effect(&ctx, effect);
        }
    })
}

pub(crate) fn dispatch_side_effect(ctx: &AppContext, effect: SideEffect) {
    match effect {
        SideEffect::Navigate(route) => push_route(&route),
        SideEffect::Login => handle_login(*ctx),
        SideEffect::Logout => handle_logout(ctx),
        SideEffect::SwitchView(_) => {}
        SideEffect::SwitchViewAndNavigate(_, route) => push_route(&route),
        SideEffect::ClearHistory => ctx.terminal.clear_history(),
        SideEffect::ListThemes => {
            ctx.terminal
                .push_lines(crate::render::theme::theme_output_lines());
        }
        SideEffect::SetTheme { theme } => match RuntimeServices::new(*ctx).set_theme(&theme) {
            Ok(theme_id) => {
                let label = crate::render::theme::theme_label(theme_id).unwrap_or(theme_id);
                ctx.terminal
                    .push_output(OutputLine::success(format!("theme: {theme_id} ({label})")));
            }
            Err(error) => ctx
                .terminal
                .push_output(OutputLine::error(format!("theme: {error}"))),
        },
        SideEffect::SetEnvVar { key, value } => {
            match RuntimeServices::new(*ctx).set_env_var(&key, &value) {
                Ok(()) => {}
                Err(error) => ctx.terminal.push_output(OutputLine::error(format!(
                    "export: failed to persist {key}: {error}"
                ))),
            }
        }
        SideEffect::UnsetEnvVar { key } => match RuntimeServices::new(*ctx).unset_env_var(&key) {
            Ok(()) => {}
            Err(error) => ctx.terminal.push_output(OutputLine::error(format!(
                "unset: failed to remove {key}: {error}"
            ))),
        },
        SideEffect::ApplyChange { path, change } => {
            let timestamp_ms = crate::platform::current_timestamp();
            ctx.evict_text_cache_path(&path);
            ctx.changes
                .update(|cs| cs.upsert_at(path, *change, timestamp_ms));
        }
        SideEffect::StageChange { path } => {
            ctx.changes.update(|cs| cs.stage(&path));
        }
        SideEffect::UnstageChange { path } => {
            ctx.changes.update(|cs| cs.unstage(&path));
        }
        SideEffect::DiscardChange { path } => {
            ctx.evict_text_cache_path(&path);
            ctx.changes.update(|cs| cs.discard(&path));
        }
        SideEffect::StageAll => {
            ctx.changes.update(|cs| cs.stage_all());
        }
        SideEffect::UnstageAll => {
            ctx.changes.update(|cs| cs.unstage_all());
        }
        SideEffect::SetAuthToken { token } => {
            match RuntimeServices::new(*ctx).set_github_token(&token) {
                Ok(()) => {}
                Err(error) => ctx.terminal.push_output(OutputLine::error(format!(
                    "sync auth: failed to persist token: {error}"
                ))),
            }
        }
        SideEffect::ClearAuthToken => match RuntimeServices::new(*ctx).clear_github_token() {
            Ok(()) => {}
            Err(error) => ctx.terminal.push_output(OutputLine::error(format!(
                "sync auth: failed to clear token: {error}"
            ))),
        },
        SideEffect::InvalidateRuntimeState => {}
        SideEffect::OpenEditor { path } => {
            ctx.editor_open.set(Some(path));
        }
        SideEffect::Commit {
            message,
            mount_root,
        } => {
            let changes_signal = ctx.changes;
            let terminal = ctx.terminal;
            let services = RuntimeServices::new(*ctx);

            wasm_bindgen_futures::spawn_local(async move {
                match services.commit_staged(mount_root.clone(), message).await {
                    Ok(outcome) => {
                        let reload = if mount_root.is_root() {
                            services.reload_runtime().await
                        } else {
                            services.reload_runtime_mount(mount_root.clone()).await
                        };
                        match reload {
                            Ok(()) => {}
                            Err(error) => {
                                terminal.push_output(websh_core::shell::OutputLine::info(format!(
                                    "sync: commit ok, runtime reload failed: {error}"
                                )))
                            }
                        }

                        let committed = outcome.committed_paths.clone();
                        changes_signal.update(|cs| {
                            for p in committed.iter() {
                                cs.discard(p);
                            }
                        });

                        terminal.push_output(websh_core::shell::OutputLine::info(format!(
                            "sync: committed {} files (HEAD now {}).",
                            outcome.committed_paths.len(),
                            &outcome.new_head[..outcome.new_head.len().min(8)]
                        )));
                    }
                    Err(e) => {
                        terminal.push_output(websh_core::shell::OutputLine::error(format!(
                            "sync: {e}"
                        )));
                    }
                }
            });
        }
        SideEffect::ReloadRuntimeMount { mount_root } => {
            let terminal = ctx.terminal;
            let services = RuntimeServices::new(*ctx);
            wasm_bindgen_futures::spawn_local(async move {
                match services.reload_runtime_mount(mount_root.clone()).await {
                    Ok(()) => {
                        terminal.push_output(websh_core::shell::OutputLine::info(format!(
                            "sync: {} reloaded.",
                            mount_root.as_str()
                        )));
                    }
                    Err(error) => terminal.push_output(websh_core::shell::OutputLine::error(
                        format!("sync refresh: {error}"),
                    )),
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

pub(super) fn create_history_nav_callback(ctx: AppContext) -> Callback<i32, Option<String>> {
    Callback::new(move |direction: i32| ctx.terminal.navigate_history(direction))
}

pub(super) fn create_autocomplete_callback(
    ctx: AppContext,
    route_ctx: RouteContext,
) -> Callback<String, websh_core::shell::AutocompleteResult> {
    Callback::new(move |input: String| {
        let cwd = route_cwd(&route_ctx.0.get());
        ctx.system_global_fs
            .with(|current_fs| autocomplete(&input, &cwd, current_fs))
    })
}

pub(super) fn create_hint_callback(
    ctx: AppContext,
    route_ctx: RouteContext,
) -> Callback<String, Option<String>> {
    Callback::new(move |input: String| {
        let cwd = route_cwd(&route_ctx.0.get());
        ctx.system_global_fs
            .with(|current_fs| get_hint(&input, &cwd, current_fs))
    })
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
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

    #[wasm_bindgen_test]
    fn auth_set_command_is_not_stored_in_history() {
        assert!(!should_store_command_history("sync auth set ghp_secret"));
        assert!(should_store_command_history("sync auth clear"));
    }
}
