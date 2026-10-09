use leptos::prelude::*;

use crate::app::AppContext;
use crate::app::RuntimeServices;
use crate::platform::dom::push_route;
use crate::runtime::shell_execution_context;
use websh_core::domain::VirtualPath;
use websh_core::filesystem::route_cwd;
use websh_core::shell::OutputLine;
use websh_core::shell::{
    Command, SideEffect, autocomplete, execute_pipeline_with_context, get_hint,
    parse_input_with_env,
};

use super::RouteContext;

fn handle_login(ctx: AppContext) {
    wasm_bindgen_futures::spawn_local(async move {
        ctx.terminal
            .push_output(OutputLine::info("Connecting to wallet..."));

        match ctx.wallet.connect().await {
            Ok(Some(outcome)) => {
                if let Some(error) = outcome.persistence_error {
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
            Ok(None) => {}
            Err(e) => ctx
                .terminal
                .push_output(OutputLine::error(format!("Connection failed: {}", e))),
        }
    });
}

fn handle_logout(ctx: &AppContext) {
    if ctx
        .wallet
        .state
        .with(|state| !matches!(state, websh_core::domain::WalletState::Disconnected))
    {
        match ctx.wallet.disconnect() {
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
        let runtime_state = ctx.preferences.snapshot.get();
        let pipeline = ctx
            .terminal
            .command_history
            .with(|history| parse_input_with_env(&input, history, &runtime_state.env));
        // Only the current command grammar may enter the visible session history.
        if !pipeline.is_supported() {
            ctx.terminal.history_index.set(None);
            ctx.terminal
                .push_output(OutputLine::error("Unsupported command or syntax."));
            return;
        }
        let current_frame = route_ctx.0.get();
        let cwd = route_cwd(&current_frame);
        let prompt = ctx.get_prompt(&cwd);

        if !input.is_empty() {
            ctx.terminal
                .push_output(OutputLine::command(prompt, &input));
            ctx.terminal
                .add_to_command_history(&pipeline.command_line());
        }

        let needs_profile = pipeline.commands.first().is_some_and(|command| {
            matches!(
                Command::parse(&command.name, &command.args),
                Command::Whoami
            )
        });
        let execute = move |profile| {
            let wallet_state = ctx.wallet.state.get();
            let runtime_mounts = ctx.content.runtime_mounts_snapshot();
            let execution_context = shell_execution_context(&runtime_state, profile);
            let result = ctx.with_fs(|current_fs| {
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
        };
        if needs_profile {
            let output_epoch = ctx.terminal.output_epoch();
            wasm_bindgen_futures::spawn_local(async move {
                let path = VirtualPath::from_absolute("/.site/profile.txt").expect("profile path");
                let profile = ctx.read_text(&path).await;
                if ctx.terminal.output_epoch() != output_epoch {
                    return;
                }
                match profile {
                    Ok(profile) => execute(profile),
                    Err(error) => ctx
                        .terminal
                        .push_output(OutputLine::error(format!("whoami: {error}"))),
                }
            });
        } else {
            execute(String::new());
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

pub(super) fn create_history_nav_callback(ctx: AppContext) -> Callback<i32, Option<String>> {
    Callback::new(move |direction: i32| ctx.terminal.navigate_history(direction))
}

pub(super) fn create_autocomplete_callback(
    ctx: AppContext,
    route_ctx: RouteContext,
) -> Callback<String, websh_core::shell::AutocompleteResult> {
    Callback::new(move |input: String| {
        let cwd = route_cwd(&route_ctx.0.get());
        ctx.with_fs(|current_fs| autocomplete(&input, &cwd, current_fs))
    })
}

pub(super) fn create_hint_callback(
    ctx: AppContext,
    route_ctx: RouteContext,
) -> Callback<String, Option<String>> {
    Callback::new(move |input: String| {
        let cwd = route_cwd(&route_ctx.0.get());
        ctx.with_fs(|current_fs| get_hint(&input, &cwd, current_fs))
    })
}
