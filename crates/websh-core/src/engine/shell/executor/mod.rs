//! Command execution logic.
//!
//! Contains the `execute_command` function that runs parsed commands
//! against the canonical filesystem and returns results.

use crate::domain::{ChangeSet, RuntimeMount, VirtualPath, WalletState, is_runtime_overlay_path};
use crate::engine::filesystem::{GlobalFs, canonicalize_user_path};

use super::{AccessPolicy, Command, CommandResult, ExecutionContext, OutputLine, SideEffect};

mod env_cmd;
mod info;
mod read;
mod sync;
mod write;

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
/// * `changes` - The current set of pending changes
/// * `remote_head` - Last-known remote HEAD SHA displayed by `sync status`
#[allow(clippy::too_many_arguments)]
pub fn execute_command(
    cmd: Command,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: &GlobalFs,
    cwd: &VirtualPath,
    changes: &ChangeSet,
    remote_head: Option<&str>,
) -> CommandResult {
    execute_command_with_context(
        cmd,
        wallet_state,
        runtime_mounts,
        fs,
        cwd,
        changes,
        remote_head,
        &ExecutionContext::default(),
    )
}

/// Execute a parsed command with target-provided context.
#[allow(clippy::too_many_arguments)]
pub fn execute_command_with_context(
    cmd: Command,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: &GlobalFs,
    cwd: &VirtualPath,
    changes: &ChangeSet,
    remote_head: Option<&str>,
    context: &ExecutionContext,
) -> CommandResult {
    match cmd {
        Command::Ls { path, long } => read::execute_ls(
            path,
            long,
            wallet_state,
            &context.access_policy,
            runtime_mounts,
            fs,
            cwd,
        ),
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
        Command::Touch { path } => write::execute_touch(
            path,
            wallet_state,
            &context.access_policy,
            runtime_mounts,
            fs,
            cwd,
        ),
        Command::Mkdir { path } => write::execute_mkdir(
            path,
            wallet_state,
            &context.access_policy,
            runtime_mounts,
            fs,
            cwd,
        ),
        Command::Rm { path, recursive } => write::execute_rm(
            path,
            recursive,
            write::WriteCommandContext {
                wallet_state,
                access_policy: &context.access_policy,
                runtime_mounts,
                fs,
                cwd,
                changes,
            },
        ),
        Command::Rmdir { path } => write::execute_rmdir(
            path,
            wallet_state,
            &context.access_policy,
            runtime_mounts,
            fs,
            cwd,
            changes,
        ),
        Command::Edit { path } => write::execute_edit(
            path,
            wallet_state,
            &context.access_policy,
            runtime_mounts,
            fs,
            cwd,
        ),
        Command::EchoRedirect { body, path } => write::execute_echo_redirect(
            body,
            path,
            wallet_state,
            &context.access_policy,
            runtime_mounts,
            fs,
            cwd,
        ),
        Command::Sync(sub) => sync::execute_sync(
            sub,
            wallet_state,
            &context.access_policy,
            runtime_mounts,
            cwd,
            changes,
            remote_head,
        ),
        Command::Unknown(cmd) => CommandResult::error_line(format!(
            "Command not found: {}. Type 'help' for available commands.",
            cmd
        ))
        .with_exit_code(127),
    }
}

/// Resolve an admin + mount preflight for write commands. Returns the write
/// target mount when the caller may write to `current_route`, or a
/// `CommandResult` error otherwise.
///
/// Centralising this lets every write arm emit the same error string and keeps
/// admin gating in one place.
#[allow(clippy::result_large_err)]
pub(super) fn require_write_access(
    cmd_label: &str,
    wallet_state: &WalletState,
    access_policy: &AccessPolicy,
    runtime_mounts: &[RuntimeMount],
    path: &VirtualPath,
) -> Result<(), CommandResult> {
    if is_runtime_overlay_path(path) {
        return Err(CommandResult::error_line(format!(
            "{}: read-only filesystem",
            cmd_label
        )));
    }

    let Some(mount) = mount_for_path(runtime_mounts, path) else {
        return Err(CommandResult::error_line(format!(
            "{}: permission denied (admin login required)",
            cmd_label
        )));
    };

    if access_policy.can_write_to(wallet_state, mount.writable) {
        Ok(())
    } else {
        Err(CommandResult::error_line(format!(
            "{}: permission denied (admin login required)",
            cmd_label
        )))
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

pub(super) fn can_write_path(
    wallet_state: &WalletState,
    access_policy: &AccessPolicy,
    runtime_mounts: &[RuntimeMount],
    path: &VirtualPath,
) -> bool {
    if is_runtime_overlay_path(path) {
        return false;
    }

    mount_for_path(runtime_mounts, path)
        .as_ref()
        .is_some_and(|mount| access_policy.can_write_to(wallet_state, mount.writable))
}

#[cfg(test)]
mod tests;
