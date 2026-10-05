//! Command execution logic.
//!
//! Contains the `execute_command` function that runs parsed commands
//! against the canonical filesystem and returns results.

use crate::domain::{RuntimeMount, VirtualPath, WalletState, is_runtime_overlay_path};
use crate::engine::filesystem::{FsView, canonicalize_user_path};

use super::{Command, CommandResult, ExecutionContext, OutputLine, PathArg, SideEffect};

mod env_cmd;
mod info;
mod read;

/// Execute a parsed command and return output lines.
///
/// This function may have side effects on the terminal state (e.g., clearing
/// history). Navigation is returned as a route, not directly applied.
///
/// # Arguments
///
/// * `cmd` - The parsed command to execute
/// * `state` - Terminal state (for clearing history)
/// * `wallet_state` - Current wallet connection state
/// * `fs` - Global canonical filesystem
/// * `cwd` - Current canonical working directory
pub fn execute_command(
    cmd: Command,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: FsView<'_>,
    cwd: &VirtualPath,
) -> CommandResult {
    execute_command_with_context(
        cmd,
        wallet_state,
        runtime_mounts,
        fs,
        cwd,
        &ExecutionContext::default(),
    )
}

/// Execute a parsed command with target-provided context.
pub fn execute_command_with_context(
    cmd: Command,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: FsView<'_>,
    cwd: &VirtualPath,
    context: &ExecutionContext,
) -> CommandResult {
    match cmd {
        Command::Ls { path, long } => read::execute_ls(path, long, wallet_state, fs, cwd),
        Command::Cd(path) => read::execute_cd(path, fs, cwd),
        Command::Pwd => CommandResult::output(vec![OutputLine::text(cwd.as_str())]),
        Command::Cat(file) => match file {
            Some(f) => read::execute_cat(f, fs, cwd),
            None => CommandResult::error_line("cat: missing file operand"),
        },
        Command::Whoami => info::execute_whoami(context),
        Command::Id => info::execute_id(wallet_state, context),
        Command::Help => CommandResult::output(
            context
                .shell_text
                .help
                .lines()
                .map(OutputLine::text)
                .collect(),
        ),
        Command::Theme(requested) => info::execute_theme(requested),
        Command::Clear => CommandResult {
            output: vec![],
            exit_code: 0,
            side_effects: vec![SideEffect::ClearHistory],
        },
        Command::Echo(text) => CommandResult::output(vec![OutputLine::text(text)]),
        Command::Export(assignments) => env_cmd::execute_export(assignments, &context.env),
        Command::Unset(key) => match key {
            Some(k) => env_cmd::execute_unset(k, &context.env),
            None => CommandResult::error_line("unset: missing variable name"),
        },
        Command::Login => CommandResult::login(),
        Command::Logout => CommandResult::logout(),
        Command::Refresh(path) => {
            let target = match resolve_path_arg(
                "refresh",
                path.as_ref().map_or(".", PathArg::as_str),
                cwd,
            ) {
                Ok(path) => path,
                Err(error) => return error,
            };
            if is_runtime_overlay_path(&target) {
                return CommandResult::error_line("refresh: runtime state has no remote source");
            }
            match mount_for_path(runtime_mounts, &target) {
                Some(mount) => {
                    CommandResult::empty().with_side_effect(SideEffect::ReloadRuntimeMount {
                        mount_root: mount.root,
                    })
                }
                None => CommandResult::error_line("refresh: no accepted mount owns this path"),
            }
        }
        Command::Unknown(cmd) => CommandResult::error_line(format!(
            "Command not found: {}. Type 'help' for available commands.",
            cmd
        ))
        .with_exit_code(127),
    }
}

#[allow(clippy::result_large_err)]
pub(super) fn resolve_path_arg(
    cmd_label: &str,
    raw: &str,
    cwd: &VirtualPath,
) -> Result<VirtualPath, CommandResult> {
    canonicalize_user_path(cwd, raw)
        .ok_or_else(|| CommandResult::error_line(format!("{}: invalid path '{}'", cmd_label, raw)))
}

pub(super) fn mount_for_path(
    runtime_mounts: &[RuntimeMount],
    path: &VirtualPath,
) -> Option<RuntimeMount> {
    runtime_mounts
        .iter()
        .filter(|mount| mount.contains(path))
        .max_by_key(|mount| mount.root.as_str().len())
        .cloned()
}

#[cfg(test)]
mod tests;
