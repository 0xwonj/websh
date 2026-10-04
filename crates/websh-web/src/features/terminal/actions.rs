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
        // Reject retired credential input before echo, history, or parsing.
        if contains_retired_credential_command(&input) {
            ctx.terminal.history_index.set(None);
            ctx.terminal.push_output(OutputLine::error(
                "Browser authoring is no longer supported.",
            ));
            return;
        }
        let current_frame = route_ctx.0.get();
        let cwd = route_cwd(&current_frame);
        let prompt = ctx.get_prompt(&cwd);

        if !input.is_empty() {
            ctx.terminal
                .push_output(OutputLine::command(prompt, &input));
            ctx.terminal.add_to_command_history(&input);
        }

        let runtime_state = ctx.runtime_state.get();
        let pipeline = ctx
            .terminal
            .command_history
            .with(|history| parse_input_with_env(&input, history, &runtime_state.env));

        let wallet_state = ctx.wallet.get();
        let runtime_mounts = ctx.runtime_mounts_snapshot();
        let execution_context = shell_execution_context(&runtime_state);
        let result = ctx.system_global_fs.with(|current_fs| {
            execute_pipeline_with_context(
                &pipeline,
                &wallet_state,
                &runtime_mounts,
                current_fs,
                &cwd,
                &execution_context,
            )
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
        SideEffect::ReloadRuntimeMount { mount_root } => {
            let terminal = ctx.terminal;
            let services = RuntimeServices::new(*ctx);
            wasm_bindgen_futures::spawn_local(async move {
                match services.reload_runtime_mount(mount_root.clone()).await {
                    Ok(()) => {
                        terminal.push_output(websh_core::shell::OutputLine::info(format!(
                            "refresh: {} reloaded.",
                            mount_root.as_str()
                        )));
                    }
                    Err(error) => terminal.push_output(websh_core::shell::OutputLine::error(
                        format!("refresh: {error}"),
                    )),
                }
            });
        }
    }
}

fn contains_retired_credential_command(input: &str) -> bool {
    // Inspect only each stage's leading words, accepting the old quote/escape spelling.
    // The credential payload is never parsed, copied into history, or logged.
    input.split('|').any(|segment| {
        let mut chars = segment.trim_start().chars().peekable();
        for expected in ["sync", "auth", "set"] {
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
            let mut word = String::new();
            let mut quote = None;
            while let Some(c) = chars.next() {
                if c.is_whitespace() && quote.is_none() {
                    break;
                }
                if c == '\\' && quote != Some('\'') {
                    if let Some(escaped) = chars.next() {
                        word.push(escaped);
                    }
                } else if quote == Some(c) {
                    quote = None;
                } else if quote.is_none() && matches!(c, '\'' | '"') {
                    quote = Some(c);
                } else {
                    word.push(c);
                }
            }
            if !word.eq_ignore_ascii_case(expected) {
                return false;
            }
        }
        true
    })
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
    fn retired_credential_input_is_detected_before_echo_or_history() {
        for input in [
            "sync auth set placeholder",
            "sync \"auth\" 'set' placeholder",
            "\"sync\" auth set placeholder",
            r"s\ync auth set placeholder",
            "  SYNC\tAUTH  SET placeholder",
            "echo hello | sync auth set placeholder",
            "sync auth set",
        ] {
            assert!(contains_retired_credential_command(input));
        }
        for input in ["refresh", "login", "echo sync auth set", "sync auth clear"] {
            assert!(!contains_retired_credential_command(input));
        }
    }
}
