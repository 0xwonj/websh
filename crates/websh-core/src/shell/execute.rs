//! Command execution logic.
//!
//! Contains the `execute_command` function that runs parsed commands
//! against the canonical filesystem and returns results.

use crate::admin::can_write_to;
use crate::config::{ASCII_PROFILE, HELP_TEXT};
use crate::domain::changes::{ChangeSet, ChangeType};
use crate::domain::{
    EntryExtensions, Fields, NodeKind, NodeMetadata, OutputLine, RuntimeMount, SCHEMA_VERSION,
    VirtualPath, WalletState,
};
use crate::filesystem::{
    GlobalFs, RouteRequest, RouteSurface, canonicalize_user_path, request_path_for_canonical_path,
};
use crate::runtime::{env, wallet};
use crate::theme::{THEMES, normalize_theme_id, theme_ids, theme_label};
use crate::utils::sysinfo;

use super::{AuthAction, Command, CommandResult, PathArg, SideEffect, SyncSubcommand};

fn blank_file_meta(kind: NodeKind) -> NodeMetadata {
    NodeMetadata {
        schema: SCHEMA_VERSION,
        kind,
        authored: Fields::default(),
        derived: Fields::default(),
    }
}

fn blank_dir_meta() -> NodeMetadata {
    NodeMetadata {
        schema: SCHEMA_VERSION,
        kind: NodeKind::Directory,
        authored: Fields::default(),
        derived: Fields::default(),
    }
}

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
    match cmd {
        Command::Ls { path, long } => execute_ls(path, long, wallet_state, runtime_mounts, fs, cwd),
        Command::Cd(path) => execute_cd(path, fs, cwd),
        Command::Pwd => CommandResult::output(vec![OutputLine::text(cwd.as_str())]),
        Command::Cat(file) => match file {
            Some(f) => execute_cat(f, fs, cwd),
            None => CommandResult::error_line("cat: missing file operand"),
        },
        Command::Whoami => {
            CommandResult::output(vec![OutputLine::ascii(ASCII_PROFILE.to_string())])
        }
        Command::Id => execute_id(wallet_state),
        Command::Help => CommandResult::output(HELP_TEXT.lines().map(OutputLine::text).collect()),
        Command::Theme(requested) => execute_theme(requested),
        Command::Clear => CommandResult {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::ClearHistory),
        },
        Command::Echo(text) => CommandResult::output(vec![OutputLine::text(text)]),
        Command::Export(assignments) => execute_export(assignments),
        Command::Unset(key) => match key {
            Some(k) => execute_unset(k),
            None => CommandResult::error_line("unset: missing variable name"),
        },
        Command::Login => CommandResult::login(),
        Command::Logout => CommandResult::logout(),
        Command::Explorer(path) => execute_explorer(path, fs, cwd),
        Command::Touch { path } => execute_touch(path, wallet_state, runtime_mounts, fs, cwd),
        Command::Mkdir { path } => execute_mkdir(path, wallet_state, runtime_mounts, fs, cwd),
        Command::Rm { path, recursive } => execute_rm(
            path,
            recursive,
            wallet_state,
            runtime_mounts,
            fs,
            cwd,
            changes,
        ),
        Command::Rmdir { path } => {
            execute_rmdir(path, wallet_state, runtime_mounts, fs, cwd, changes)
        }
        Command::Edit { path } => execute_edit(path, wallet_state, runtime_mounts, fs, cwd),
        Command::EchoRedirect { body, path } => {
            execute_echo_redirect(body, path, wallet_state, runtime_mounts, fs, cwd)
        }
        Command::Sync(sub) => {
            execute_sync(sub, wallet_state, runtime_mounts, cwd, changes, remote_head)
        }
        Command::Unknown(cmd) => CommandResult::error_line(format!(
            "Command not found: {}. Type 'help' for available commands.",
            cmd
        ))
        .with_exit_code(127),
    }
}

fn execute_theme(requested: Option<String>) -> CommandResult {
    let Some(requested) = requested else {
        let mut lines = vec![OutputLine::text("available themes:")];
        lines.extend(
            THEMES
                .iter()
                .map(|theme| OutputLine::text(format!("  {:<18} {}", theme.id, theme.label))),
        );
        return CommandResult::output(lines);
    };

    let Some(theme) = normalize_theme_id(&requested) else {
        return CommandResult::error_line(format!(
            "theme: unknown theme '{}'. available: {}",
            requested,
            theme_ids().collect::<Vec<_>>().join(", ")
        ));
    };

    let label = theme_label(theme).unwrap_or(theme);
    CommandResult {
        output: vec![OutputLine::success(format!("theme: {theme} ({label})"))],
        exit_code: 0,
        side_effect: Some(SideEffect::SetTheme {
            theme: theme.to_string(),
        }),
    }
}

/// Execute `ls` command.
fn execute_ls(
    path: Option<super::PathArg>,
    long: bool,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: &GlobalFs,
    cwd: &VirtualPath,
) -> CommandResult {
    let target = path.as_ref().map(|p| p.as_str()).unwrap_or(".");
    let resolved = match resolve_path_arg("ls", target, cwd) {
        Ok(path) => path,
        Err(e) => return e,
    };

    if let Some(entries) = fs.list_dir(&resolved) {
        return CommandResult::output(format_ls_output(
            &entries,
            long,
            wallet_state,
            runtime_mounts,
            fs,
        ));
    }

    if fs.exists(&resolved) {
        CommandResult::error_line(format!("ls: cannot access '{}': Not a directory", target))
    } else {
        CommandResult::error_line(format!(
            "ls: cannot access '{}': No such file or directory",
            target
        ))
    }
}

/// Format ls output for directory entries.
fn format_ls_output(
    entries: &[crate::domain::DirEntry],
    long: bool,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: &GlobalFs,
) -> Vec<OutputLine> {
    if long {
        entries
            .iter()
            .map(|entry| {
                let fs_entry = fs.get_entry(&entry.path);
                let writable = can_write_path(wallet_state, runtime_mounts, &entry.path);
                let perms = fs_entry
                    .map(|e| fs.get_permissions(e, wallet_state, writable))
                    .unwrap_or_default();
                OutputLine::long_entry(entry, &perms)
            })
            .collect()
    } else {
        entries
            .iter()
            .map(|entry| {
                if entry.is_dir {
                    OutputLine::dir_entry(&entry.name, &entry.title)
                } else {
                    let is_restricted = entry
                        .meta
                        .as_ref()
                        .map(|m| m.is_restricted())
                        .unwrap_or(false);
                    OutputLine::file_entry(&entry.name, &entry.title, is_restricted)
                }
            })
            .collect()
    }
}

/// Execute `cd` command.
fn execute_cd(path: super::PathArg, fs: &GlobalFs, cwd: &VirtualPath) -> CommandResult {
    let target = path.as_str();
    if target.is_empty() {
        return CommandResult::error_line("cd: : No such file or directory");
    }

    let resolved = match resolve_path_arg("cd", target, cwd) {
        Ok(path) => path,
        Err(e) => return e,
    };

    if !fs.exists(&resolved) {
        return CommandResult::error_line(format!("cd: no such file or directory: {}", path));
    }

    if !fs.is_directory(&resolved) {
        return CommandResult::error_line(format!("cd: not a directory: {}", path));
    }

    CommandResult::navigate(RouteRequest::new(request_path_for_canonical_path(
        &resolved,
        RouteSurface::Shell,
    )))
}

/// Execute `cat` command.
fn execute_cat(file: super::PathArg, fs: &GlobalFs, cwd: &VirtualPath) -> CommandResult {
    let resolved = match resolve_path_arg("cat", file.as_str(), cwd) {
        Ok(path) => path,
        Err(e) => return e,
    };

    if !fs.exists(&resolved) {
        return CommandResult::error_line(format!("cat: {}: No such file or directory", file));
    }

    if fs.is_directory(&resolved) {
        return CommandResult::error_line(format!("cat: {}: Is a directory", file));
    }

    CommandResult::navigate(RouteRequest::new(request_path_for_canonical_path(
        &resolved,
        RouteSurface::Content,
    )))
}

/// Execute `id` command.
fn execute_id(wallet_state: &WalletState) -> CommandResult {
    let mut lines = vec![OutputLine::empty()];

    match wallet_state {
        WalletState::Connected {
            address, ens_name, ..
        } => {
            if let Some(ens) = ens_name {
                lines.push(OutputLine::text(format!("uid={} ({})", address, ens)));
            } else {
                lines.push(OutputLine::text(format!("uid={}", address)));
            }
            lines.push(OutputLine::text("gid=visitor"));
            lines.push(OutputLine::text("status=connected"));
        }
        WalletState::Disconnected => {
            lines.push(OutputLine::text("uid=guest"));
            lines.push(OutputLine::text("gid=anonymous"));
            lines.push(OutputLine::text("status=disconnected"));
        }
        WalletState::Connecting => {
            lines.push(OutputLine::text("uid=..."));
            lines.push(OutputLine::text("status=connecting"));
        }
    }

    if let Some(chain_id) = wallet_state.chain_id() {
        lines.push(OutputLine::text(format!(
            "network={}",
            wallet::chain_name(chain_id)
        )));
        lines.push(OutputLine::text(format!("chain_id={}", chain_id)));
    } else {
        lines.push(OutputLine::text("network=none"));
    }

    if let Some(uptime) = sysinfo::get_uptime() {
        lines.push(OutputLine::text(format!("uptime={}", uptime)));
    }

    if let Some(window) = crate::utils::dom::window()
        && let Ok(ua) = window.navigator().user_agent()
    {
        lines.push(OutputLine::text(format!("user_agent={}", ua)));
    }

    lines.push(OutputLine::empty());
    CommandResult::output(lines)
}

/// Execute `export` command.
///
/// Each element of `assignments` is processed independently:
///   - `KEY=value` → set the variable
///   - `KEY` alone → print `KEY=<value>` if set (silent otherwise)
///
/// An empty list prints all user variables. When any assignment fails,
/// an error line is emitted and the first failure sets exit_code=1;
/// subsequent assignments are still attempted (bash-style behavior).
fn execute_export(assignments: Vec<String>) -> CommandResult {
    if assignments.is_empty() {
        // No args: show all variables
        let lines = env::format_export_output();
        let mut output = vec![OutputLine::empty()];
        for line in lines {
            output.push(OutputLine::text(line));
        }
        output.push(OutputLine::empty());
        return CommandResult::output(output);
    }

    let mut output: Vec<OutputLine> = Vec::new();
    let mut exit_code = 0;
    let mut state_changed = false;
    for arg in assignments {
        if let Some((key, value)) = arg.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"').trim_matches('\'');
            match env::set_user_var(key, value) {
                Ok(_) => state_changed = true,
                Err(e) => {
                    output.push(OutputLine::error(format!("export: {}", e)));
                    if exit_code == 0 {
                        exit_code = 1;
                    }
                }
            }
        } else {
            // Just a key without value — show current value if set.
            let key = arg.trim();
            if let Some(value) = env::get_user_var(key) {
                output.push(OutputLine::text(format!("{}={}", key, value)));
            }
            // silent if not set (matches prior behavior)
        }
    }

    let mut result = CommandResult::output(output).with_exit_code(exit_code);
    if state_changed {
        result.side_effect = Some(SideEffect::InvalidateRuntimeState);
    }
    result
}

/// Execute `unset` command.
fn execute_unset(key: String) -> CommandResult {
    if env::get_user_var(&key).is_some() {
        match env::unset_user_var(&key) {
            Ok(_) => CommandResult {
                output: vec![],
                exit_code: 0,
                side_effect: Some(SideEffect::InvalidateRuntimeState),
            },
            Err(e) => CommandResult::error_line(format!("unset: {}", e)),
        }
    } else {
        CommandResult::empty() // Silently succeed if variable doesn't exist
    }
}

/// Execute `explorer` command.
fn execute_explorer(
    path: Option<super::PathArg>,
    fs: &GlobalFs,
    cwd: &VirtualPath,
) -> CommandResult {
    let Some(path_arg) = path else {
        return CommandResult::navigate(RouteRequest::new(request_path_for_canonical_path(
            cwd,
            RouteSurface::Explorer,
        )));
    };

    let resolved = match resolve_path_arg("explorer", path_arg.as_str(), cwd) {
        Ok(path) => path,
        Err(e) => return e,
    };

    if !fs.exists(&resolved) {
        return CommandResult::error_line(format!(
            "explorer: no such file or directory: {}",
            path_arg
        ));
    }

    if !fs.is_directory(&resolved) {
        return CommandResult::error_line(format!("explorer: not a directory: {}", path_arg));
    }

    CommandResult::navigate(RouteRequest::new(request_path_for_canonical_path(
        &resolved,
        RouteSurface::Explorer,
    )))
}

/// Resolve an admin + mount preflight for write commands. Returns the write
/// target mount when the caller may write to `current_route`, or a
/// `CommandResult` error otherwise.
///
/// Centralising this lets every write arm emit the same error string and keeps
/// admin gating in one place.
#[allow(clippy::result_large_err)]
fn require_write_access(
    cmd_label: &str,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    path: &VirtualPath,
) -> Result<(), CommandResult> {
    if is_synthetic_runtime_state_path(path) {
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

    if can_write_to(wallet_state, mount.writable) {
        Ok(())
    } else {
        Err(CommandResult::error_line(format!(
            "{}: permission denied (admin login required)",
            cmd_label
        )))
    }
}

/// Resolve `path` (possibly relative) against `current_route` into an absolute
/// `VirtualPath` (with a leading `/`). Returns an error `CommandResult` with
/// the given `cmd_label` if the absolute form cannot be constructed.
#[allow(clippy::result_large_err)]
fn resolve_abs_path(
    cmd_label: &str,
    path: &PathArg,
    cwd: &VirtualPath,
) -> Result<VirtualPath, CommandResult> {
    resolve_path_arg(cmd_label, path.as_str(), cwd)
}

/// Single-letter status tag for a ChangeType (A/M/D).
fn change_tag(change: &ChangeType) -> &'static str {
    match change {
        ChangeType::CreateFile { .. }
        | ChangeType::CreateBinary { .. }
        | ChangeType::CreateDirectory { .. } => "A",
        ChangeType::UpdateFile { .. } => "M",
        ChangeType::DeleteFile | ChangeType::DeleteDirectory => "D",
    }
}

/// Execute `touch` — create an empty file.
fn execute_touch(
    path: PathArg,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: &GlobalFs,
    cwd: &VirtualPath,
) -> CommandResult {
    let vp = match resolve_abs_path("touch", &path, cwd) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if let Err(e) = require_write_access("touch", wallet_state, runtime_mounts, &vp) {
        return e;
    }

    if fs.exists(&vp) {
        return CommandResult::error_line(format!("touch: {}: path already exists", path));
    }

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::ApplyChange {
            path: vp,
            change: Box::new(ChangeType::CreateFile {
                content: String::new(),
                meta: blank_file_meta(NodeKind::Asset),
                extensions: EntryExtensions::default(),
            }),
        }),
    }
}

/// Execute `mkdir` — create a directory.
fn execute_mkdir(
    path: PathArg,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: &GlobalFs,
    cwd: &VirtualPath,
) -> CommandResult {
    let vp = match resolve_abs_path("mkdir", &path, cwd) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if let Err(e) = require_write_access("mkdir", wallet_state, runtime_mounts, &vp) {
        return e;
    }

    if fs.exists(&vp) {
        return CommandResult::error_line(format!("mkdir: {}: path already exists", path));
    }

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::ApplyChange {
            path: vp,
            change: Box::new(ChangeType::CreateDirectory {
                meta: blank_dir_meta(),
            }),
        }),
    }
}

/// Execute `rm` — delete a file or directory (with `-r` for directories).
///
/// When the target exists only as a pending `Create*` in `changes` (not in the
/// base `GlobalFs`), `rm` emits `SideEffect::DiscardChange` to drop the pending create
/// entirely instead of stacking a `DeleteFile`/`DeleteDirectory` on top. For
/// `rm -r` on a pending `CreateDirectory` with pending children, only the
/// target's create is discarded — any orphan pending children are left for the
/// user to clean up via `sync status`.
fn execute_rm(
    path: PathArg,
    recursive: bool,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: &GlobalFs,
    cwd: &VirtualPath,
    changes: &ChangeSet,
) -> CommandResult {
    let vp = match resolve_abs_path("rm", &path, cwd) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if let Err(e) = require_write_access("rm", wallet_state, runtime_mounts, &vp) {
        return e;
    }

    let Some(entry) = fs.get_entry(&vp) else {
        return CommandResult::error_line(format!("rm: {}: no such file or directory", path));
    };

    if entry.is_directory() && !recursive {
        return CommandResult::error_line(format!("rm: {}: is a directory (use -r)", path));
    }

    // If the target exists only as a pending create in the ChangeSet, discard
    // the create rather than emitting a Delete change on top.
    if is_pending_create(changes, &vp) {
        return CommandResult {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::DiscardChange { path: vp }),
        };
    }

    let change = if entry.is_directory() {
        ChangeType::DeleteDirectory
    } else {
        ChangeType::DeleteFile
    };

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::ApplyChange {
            path: vp,
            change: Box::new(change),
        }),
    }
}

/// Execute `rmdir` — delete an empty directory.
fn execute_rmdir(
    path: PathArg,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: &GlobalFs,
    cwd: &VirtualPath,
    changes: &ChangeSet,
) -> CommandResult {
    let vp = match resolve_abs_path("rmdir", &path, cwd) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if let Err(e) = require_write_access("rmdir", wallet_state, runtime_mounts, &vp) {
        return e;
    }

    let Some(entry) = fs.get_entry(&vp) else {
        return CommandResult::error_line(format!("rmdir: {}: no such file or directory", path));
    };

    if !entry.is_directory() {
        return CommandResult::error_line(format!("rmdir: {}: not a directory", path));
    }

    if fs.has_children(&vp) {
        return CommandResult::error_line(format!("rmdir: {}: directory not empty", path));
    }

    // Pending-create on an empty dir → discard the create.
    if is_pending_create(changes, &vp) {
        return CommandResult {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::DiscardChange { path: vp }),
        };
    }

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::ApplyChange {
            path: vp,
            change: Box::new(ChangeType::DeleteDirectory),
        }),
    }
}

/// True iff `changes` has an entry at `path` whose `ChangeType` is one of the
/// `Create*` variants (i.e., the path exists only as a pending create, not in
/// the base `GlobalFs`).
fn is_pending_create(changes: &ChangeSet, path: &VirtualPath) -> bool {
    matches!(
        changes.get(path).map(|e| &e.change),
        Some(
            ChangeType::CreateFile { .. }
                | ChangeType::CreateBinary { .. }
                | ChangeType::CreateDirectory { .. }
        )
    )
}

/// Execute `edit` — request the editor UI open for a file.
fn execute_edit(
    path: PathArg,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: &GlobalFs,
    cwd: &VirtualPath,
) -> CommandResult {
    let vp = match resolve_abs_path("edit", &path, cwd) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if let Err(e) = require_write_access("edit", wallet_state, runtime_mounts, &vp) {
        return e;
    }

    // Path must either not exist yet (create-on-save) or be a file.
    if let Some(entry) = fs.get_entry(&vp)
        && entry.is_directory()
    {
        return CommandResult::error_line(format!("edit: {}: is a directory", path));
    }

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::OpenEditor { path: vp }),
    }
}

/// Execute `echo "..." > path` — create or update a file with literal content.
fn execute_echo_redirect(
    body: String,
    path: PathArg,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: &GlobalFs,
    cwd: &VirtualPath,
) -> CommandResult {
    let vp = match resolve_abs_path("echo", &path, cwd) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if let Err(e) = require_write_access("echo", wallet_state, runtime_mounts, &vp) {
        return e;
    }

    let change = match fs.get_entry(&vp) {
        Some(entry) if entry.is_directory() => {
            return CommandResult::error_line(format!("echo: {}: is a directory", path));
        }
        Some(_) => ChangeType::UpdateFile {
            content: body,
            meta: None,
            extensions: None,
        },
        None => ChangeType::CreateFile {
            content: body,
            meta: blank_file_meta(NodeKind::Asset),
            extensions: EntryExtensions::default(),
        },
    };

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::ApplyChange {
            path: vp,
            change: Box::new(change),
        }),
    }
}

/// Execute `sync <sub>` — status / commit / refresh / auth.
fn execute_sync(
    sub: SyncSubcommand,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    cwd: &VirtualPath,
    changes: &ChangeSet,
    remote_head: Option<&str>,
) -> CommandResult {
    match sub {
        SyncSubcommand::Status => execute_sync_status(changes, remote_head),
        SyncSubcommand::Commit { message } => {
            execute_sync_commit(message, wallet_state, runtime_mounts, cwd, changes)
        }
        SyncSubcommand::Refresh => execute_sync_refresh(runtime_mounts, cwd),
        SyncSubcommand::Auth(action) => execute_sync_auth(action),
    }
}

fn execute_sync_status(changes: &ChangeSet, remote_head: Option<&str>) -> CommandResult {
    let mut lines: Vec<OutputLine> = Vec::new();

    if let Some(head) = remote_head {
        let short = &head[..head.len().min(8)];
        lines.push(OutputLine::text(format!("remote HEAD: {}", short)));
    }

    if changes.is_empty() {
        lines.push(OutputLine::text(
            "nothing to commit, working tree clean".to_string(),
        ));
        return CommandResult::output(lines);
    }

    let summary = changes.summary();
    lines.push(OutputLine::text(format!(
        "staged: {} / unstaged: {} / total: {}",
        summary.total_staged(),
        summary.total() - summary.total_staged(),
        summary.total()
    )));

    let mut staged: Vec<(&crate::domain::VirtualPath, &crate::domain::changes::Entry)> =
        changes.iter_staged().collect();
    staged.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    if !staged.is_empty() {
        lines.push(OutputLine::text("Changes staged for commit:".to_string()));
        for (p, e) in staged {
            lines.push(OutputLine::text(format!(
                "  {} {}",
                change_tag(&e.change),
                p.as_str()
            )));
        }
    }

    let mut unstaged: Vec<(&crate::domain::VirtualPath, &crate::domain::changes::Entry)> =
        changes.iter_unstaged().collect();
    unstaged.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    if !unstaged.is_empty() {
        lines.push(OutputLine::text("Changes not staged:".to_string()));
        for (p, e) in unstaged {
            lines.push(OutputLine::text(format!(
                "  {} {}",
                change_tag(&e.change),
                p.as_str()
            )));
        }
    }

    CommandResult::output(lines)
}

fn execute_sync_commit(
    message: String,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    cwd: &VirtualPath,
    changes: &ChangeSet,
) -> CommandResult {
    if message.trim().is_empty() {
        return CommandResult::error_line("sync commit: empty commit message");
    }

    let staged = changes.summary().total_staged();
    if staged == 0 {
        return CommandResult::error_line("sync commit: no staged changes");
    }

    let mount_root = match sync_mount_root(runtime_mounts, cwd, changes) {
        Ok(root) => root,
        Err(e) => return e,
    };

    if let Err(e) = require_write_access("sync commit", wallet_state, runtime_mounts, &mount_root) {
        return e;
    }

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::Commit {
            message,
            mount_root,
        }),
    }
}

fn execute_sync_refresh(runtime_mounts: &[RuntimeMount], cwd: &VirtualPath) -> CommandResult {
    let mount_root = match sync_mount_root(runtime_mounts, cwd, &ChangeSet::new()) {
        Ok(root) => root,
        Err(e) => return e,
    };

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::ReloadRuntimeMount { mount_root }),
    }
}

fn execute_sync_auth(action: AuthAction) -> CommandResult {
    match action {
        AuthAction::Set { token } => {
            if token.trim().is_empty() {
                return CommandResult::error_line("sync auth: empty token");
            }
            CommandResult {
                output: vec![],
                exit_code: 0,
                side_effect: Some(SideEffect::SetAuthToken { token }),
            }
        }
        AuthAction::Clear => CommandResult {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::ClearAuthToken),
        },
    }
}

#[allow(clippy::result_large_err)]
fn resolve_path_arg(
    cmd_label: &str,
    raw: &str,
    cwd: &VirtualPath,
) -> Result<VirtualPath, CommandResult> {
    canonicalize_user_path(cwd, raw)
        .ok_or_else(|| CommandResult::error_line(format!("{}: invalid path '{}'", cmd_label, raw)))
}

fn mount_for_path(runtime_mounts: &[RuntimeMount], path: &VirtualPath) -> Option<RuntimeMount> {
    runtime_mounts
        .iter()
        .filter(|mount| mount.contains(path))
        .max_by_key(|mount| mount.root.as_str().len())
        .cloned()
}

fn can_write_path(
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    path: &VirtualPath,
) -> bool {
    if is_synthetic_runtime_state_path(path) {
        return false;
    }

    mount_for_path(runtime_mounts, path)
        .as_ref()
        .is_some_and(|mount| can_write_to(wallet_state, mount.writable))
}

fn is_synthetic_runtime_state_path(path: &VirtualPath) -> bool {
    let state_root = VirtualPath::from_absolute("/.websh/state").expect("constant path");
    path.starts_with(&state_root)
}

#[allow(clippy::result_large_err)]
fn sync_mount_root(
    runtime_mounts: &[RuntimeMount],
    cwd: &VirtualPath,
    changes: &ChangeSet,
) -> Result<VirtualPath, CommandResult> {
    let mut staged_root: Option<VirtualPath> = None;
    for (path, _) in changes.iter_staged() {
        let Some(mount) = mount_for_path(runtime_mounts, path) else {
            return Err(CommandResult::error_line(
                "sync: no writable mount for staged changes",
            ));
        };

        match &staged_root {
            None => staged_root = Some(mount.root),
            Some(root) if root == &mount.root => {}
            Some(_) => {
                return Err(CommandResult::error_line(
                    "sync commit: staged changes span multiple mounts",
                ));
            }
        }
    }

    if let Some(root) = staged_root {
        return Ok(root);
    }

    if let Some(mount) = mount_for_path(runtime_mounts, cwd) {
        return Ok(mount.root);
    }

    runtime_mounts
        .iter()
        .find(|mount| mount.root.is_root())
        .map(|mount| mount.root.clone())
        .ok_or_else(|| CommandResult::error_line("sync: no writable mount available"))
}

#[cfg(test)]
mod tests {
    use super::super::SideEffect;
    use super::*;
    use crate::domain::WalletState;
    use crate::domain::changes::ChangeSet;
    use crate::filesystem::GlobalFs;

    fn empty_state() -> (WalletState, GlobalFs) {
        (WalletState::Disconnected, GlobalFs::empty())
    }

    /// Admin wallet constructor for write-path tests.
    ///
    /// Mirrors the single entry in `crate::admin::ADMIN_ADDRESSES`.
    fn admin_wallet() -> WalletState {
        WalletState::Connected {
            address: "0x2c4b04a4aeb6e18c2f8a5c8b4a3f62c0cf33795a".to_string(),
            ens_name: None,
            chain_id: Some(1),
        }
    }

    fn root_cwd() -> VirtualPath {
        VirtualPath::root()
    }

    fn home_cwd(path: &str) -> VirtualPath {
        VirtualPath::root().join(path)
    }

    fn home_vpath(path: &str) -> VirtualPath {
        home_cwd(path)
    }

    fn execute_command(
        cmd: Command,
        wallet_state: &WalletState,
        fs: &GlobalFs,
        cwd: &VirtualPath,
        changes: &ChangeSet,
        remote_head: Option<&str>,
    ) -> CommandResult {
        super::execute_command(
            cmd,
            wallet_state,
            &[crate::storage::boot::bootstrap_runtime_mount()],
            fs,
            cwd,
            changes,
            remote_head,
        )
    }

    #[test]
    fn test_login_returns_login_side_effect() {
        let (ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(Command::Login, &ws, &fs, &root_cwd(), &cs, None);
        assert_eq!(result.side_effect, Some(SideEffect::Login));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_logout_returns_logout_side_effect() {
        let (ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(Command::Logout, &ws, &fs, &root_cwd(), &cs, None);
        assert_eq!(result.side_effect, Some(SideEffect::Logout));
    }

    #[test]
    fn test_theme_lists_available_palettes() {
        let (ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(Command::Theme(None), &ws, &fs, &root_cwd(), &cs, None);
        let rendered = result
            .output
            .iter()
            .map(|line| format!("{:?}", line.data))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("sepia-dark"));
        assert!(rendered.contains("black-ink"));
        assert!(result.side_effect.is_none());
    }

    #[test]
    fn test_theme_sets_known_palette() {
        let (ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Theme(Some("black-ink".to_string())),
            &ws,
            &fs,
            &root_cwd(),
            &cs,
            None,
        );
        assert_eq!(
            result.side_effect,
            Some(SideEffect::SetTheme {
                theme: "black-ink".to_string()
            })
        );
    }

    #[test]
    fn test_explorer_no_arg_switches_view() {
        let (ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(Command::Explorer(None), &ws, &fs, &root_cwd(), &cs, None);
        assert_eq!(
            result.side_effect,
            Some(SideEffect::Navigate(RouteRequest::new("/explorer")))
        );
    }

    #[test]
    fn test_cd_navigates_shell_surface() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(VirtualPath::from_absolute("/db").unwrap(), blank_dir_meta());
        let ws = WalletState::Disconnected;
        let cs = ChangeSet::new();

        let result = execute_command(
            Command::Cd(PathArg::new("/db")),
            &ws,
            &fs,
            &root_cwd(),
            &cs,
            None,
        );

        assert_eq!(
            result.side_effect,
            Some(SideEffect::Navigate(RouteRequest::new("/websh/db")))
        );
    }

    #[test]
    fn test_explorer_path_navigates_explorer_surface() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(VirtualPath::from_absolute("/db").unwrap(), blank_dir_meta());
        let ws = WalletState::Disconnected;
        let cs = ChangeSet::new();

        let result = execute_command(
            Command::Explorer(Some(PathArg::new("/db"))),
            &ws,
            &fs,
            &root_cwd(),
            &cs,
            None,
        );

        assert_eq!(
            result.side_effect,
            Some(SideEffect::Navigate(RouteRequest::new("/explorer/db")))
        );
    }

    #[test]
    fn test_cat_navigates_content_surface() {
        let mut fs = GlobalFs::empty();
        fs.upsert_file(
            VirtualPath::from_absolute("/blog/hello.md").unwrap(),
            "hello".into(),
            blank_file_meta(NodeKind::Asset),
            EntryExtensions::default(),
        );
        let ws = WalletState::Disconnected;
        let cs = ChangeSet::new();

        let result = execute_command(
            Command::Cat(Some(PathArg::new("/blog/hello.md"))),
            &ws,
            &fs,
            &root_cwd(),
            &cs,
            None,
        );

        assert_eq!(
            result.side_effect,
            Some(SideEffect::Navigate(RouteRequest::new("/blog/hello.md")))
        );
    }

    #[test]
    fn test_unknown_command_exit_127() {
        let (ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Unknown("foobar".into()),
            &ws,
            &fs,
            &root_cwd(),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 127);
    }

    #[test]
    fn test_ls_nonexistent_exit_1() {
        let (ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Ls {
                path: Some(super::super::PathArg::new("nonexistent")),
                long: false,
            },
            &ws,
            &fs,
            &root_cwd(),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
        assert!(!result.output.is_empty());
    }

    #[test]
    fn test_cat_missing_operand_exit_1() {
        let (ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(Command::Cat(None), &ws, &fs, &root_cwd(), &cs, None);
        assert_eq!(result.exit_code, 1);
        assert!(
            result
                .output
                .iter()
                .any(|l| matches!(&l.data, crate::domain::OutputLineData::Error(s) if s == "cat: missing file operand"))
        );
    }

    #[test]
    fn test_unset_missing_operand_exit_1() {
        let (ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(Command::Unset(None), &ws, &fs, &root_cwd(), &cs, None);
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_execute_export_multi_processes_each_assignment() {
        // On native (no localStorage), each assignment triggers an
        // EnvironmentError::StorageUnavailable. We verify:
        //   - exit code is non-zero (at least one assignment errored)
        //   - there is one error line per assignment
        // This confirms the loop iterates per arg rather than joining them.
        let (ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Export(vec![
                "FOO_P2_A=alpha".to_string(),
                "BAR_P2_A=beta".to_string(),
            ]),
            &ws,
            &fs,
            &root_cwd(),
            &cs,
            None,
        );
        let error_count = result
            .output
            .iter()
            .filter(|l| matches!(&l.data, crate::domain::OutputLineData::Error(_)))
            .count();
        // On native: both fail → 2 errors, exit 1.
        // On wasm (if ever run): both succeed → 0 errors, exit 0.
        // Either way, the count matches the number of actual outcomes
        // (we cannot get 1 error unless the loop malfunctioned).
        assert!(
            error_count == 0 || error_count == 2,
            "expected 0 or 2 errors, got {}",
            error_count
        );
        if error_count == 2 {
            assert_eq!(result.exit_code, 1);
        }

        // Cleanup (best effort — no-op on native, succeeds on wasm)
        let _ = crate::runtime::env::unset_user_var("FOO_P2_A");
        let _ = crate::runtime::env::unset_user_var("BAR_P2_A");
    }

    #[test]
    fn test_cd_empty_string_exit_1() {
        // POSIX bash: `cd ""` errors with "cd: : No such file or directory".
        // Must exercise a non-Root route so the early `at_root` branch doesn't
        // short-circuit to the generic mount-alias error.
        let (ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let browse_route = home_cwd("");
        let result = execute_command(
            Command::Cd(super::super::PathArg::new("")),
            &ws,
            &fs,
            &browse_route,
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
        assert!(result.side_effect.is_none());
        assert!(
            result.output.iter().any(|l| matches!(
                &l.data,
                crate::domain::OutputLineData::Error(s) if s == "cd: : No such file or directory"
            )),
            "expected POSIX cd error; got: {:?}",
            result.output
        );
    }

    #[test]
    fn test_touch_requires_admin() {
        let (ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Touch {
                path: PathArg::new("new.md"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
        assert!(result.side_effect.is_none());
    }

    #[test]
    fn test_write_rejects_runtime_state_tree() {
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Touch {
                path: PathArg::new("/.websh/state/new.md"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );

        assert_eq!(result.exit_code, 1);
        assert!(result.side_effect.is_none());
        assert!(result.output.iter().any(|line| {
            matches!(
                &line.data,
                crate::domain::OutputLineData::Error(message)
                    if message.contains("read-only filesystem")
            )
        }));
    }

    #[test]
    fn test_touch_creates_apply_change_side_effect() {
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Touch {
                path: PathArg::new("new.md"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange {
                ref path,
                ref change,
            }) => {
                assert_eq!(path.as_str(), "/new.md");
                assert!(matches!(change.as_ref(), ChangeType::CreateFile { .. }));
            }
            other => panic!("expected ApplyChange, got {:?}", other),
        }
    }

    #[test]
    fn test_touch_errors_when_path_exists_in_fs() {
        // Build an fs with a file at "new.md"
        let mut fs = GlobalFs::empty();
        fs.upsert_file(
            home_vpath("new.md"),
            String::new(),
            blank_file_meta(NodeKind::Asset),
            EntryExtensions::default(),
        );
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Touch {
                path: PathArg::new("new.md"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_mkdir_creates_apply_change_side_effect() {
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Mkdir {
                path: PathArg::new("newdir"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange {
                ref path,
                ref change,
            }) => {
                assert_eq!(path.as_str(), "/newdir");
                assert!(matches!(
                    change.as_ref(),
                    ChangeType::CreateDirectory { .. }
                ));
            }
            other => panic!("expected ApplyChange, got {:?}", other),
        }
    }

    #[test]
    fn test_mkdir_errors_when_path_exists() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(home_vpath("dir"), blank_dir_meta());
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Mkdir {
                path: PathArg::new("dir"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_rm_file_side_effect() {
        let mut fs = GlobalFs::empty();
        fs.upsert_file(
            home_vpath("doomed.md"),
            String::new(),
            blank_file_meta(NodeKind::Asset),
            EntryExtensions::default(),
        );
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rm {
                path: PathArg::new("doomed.md"),
                recursive: false,
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange {
                ref path,
                ref change,
            }) => {
                assert_eq!(path.as_str(), "/doomed.md");
                assert!(matches!(change.as_ref(), ChangeType::DeleteFile));
            }
            other => panic!("expected DeleteFile ApplyChange, got {:?}", other),
        }
    }

    #[test]
    fn test_rm_directory_without_r_errors() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(home_vpath("dir"), blank_dir_meta());
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rm {
                path: PathArg::new("dir"),
                recursive: false,
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_rm_directory_recursive_side_effect() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(home_vpath("dir"), blank_dir_meta());
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rm {
                path: PathArg::new("dir"),
                recursive: true,
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange {
                ref path,
                ref change,
            }) => {
                assert_eq!(path.as_str(), "/dir");
                assert!(matches!(change.as_ref(), ChangeType::DeleteDirectory));
            }
            other => panic!("expected DeleteDirectory ApplyChange, got {:?}", other),
        }
    }

    #[test]
    fn test_rm_nonexistent_path_errors() {
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rm {
                path: PathArg::new("ghost.md"),
                recursive: false,
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_rmdir_empty_directory_side_effect() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(home_vpath("empty"), blank_dir_meta());
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rmdir {
                path: PathArg::new("empty"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange { ref change, .. }) => {
                assert!(matches!(change.as_ref(), ChangeType::DeleteDirectory));
            }
            other => panic!("expected DeleteDirectory, got {:?}", other),
        }
    }

    #[test]
    fn test_rmdir_nonempty_directory_errors() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(home_vpath("dir"), blank_dir_meta());
        fs.upsert_file(
            home_vpath("dir/child.md"),
            String::new(),
            blank_file_meta(NodeKind::Asset),
            EntryExtensions::default(),
        );
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rmdir {
                path: PathArg::new("dir"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_rmdir_on_file_errors() {
        let mut fs = GlobalFs::empty();
        fs.upsert_file(
            home_vpath("file.md"),
            String::new(),
            blank_file_meta(NodeKind::Asset),
            EntryExtensions::default(),
        );
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rmdir {
                path: PathArg::new("file.md"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_edit_opens_editor_for_existing_file() {
        let mut fs = GlobalFs::empty();
        fs.upsert_file(
            home_vpath("note.md"),
            "hi".to_string(),
            blank_file_meta(NodeKind::Asset),
            EntryExtensions::default(),
        );
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Edit {
                path: PathArg::new("note.md"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::OpenEditor { ref path }) => {
                assert_eq!(path.as_str(), "/note.md");
            }
            other => panic!("expected OpenEditor, got {:?}", other),
        }
    }

    #[test]
    fn test_edit_on_missing_file_opens_editor() {
        // Create-on-save: `edit` on a non-existent path still yields OpenEditor.
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Edit {
                path: PathArg::new("fresh.md"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        assert!(matches!(
            result.side_effect,
            Some(SideEffect::OpenEditor { .. })
        ));
    }

    #[test]
    fn test_edit_on_directory_errors() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(home_vpath("dir"), blank_dir_meta());
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Edit {
                path: PathArg::new("dir"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_echo_redirect_writes_content() {
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::EchoRedirect {
                body: "hello".to_string(),
                path: PathArg::new("greeting.md"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange {
                ref path,
                ref change,
            }) => {
                assert_eq!(path.as_str(), "/greeting.md");
                match change.as_ref() {
                    ChangeType::CreateFile { content, .. } => assert_eq!(content, "hello"),
                    other => panic!("expected CreateFile, got {:?}", other),
                }
            }
            other => panic!("expected ApplyChange, got {:?}", other),
        }
    }

    #[test]
    fn test_echo_redirect_updates_existing_file() {
        let mut fs = GlobalFs::empty();
        fs.upsert_file(
            home_vpath("greet.md"),
            "old".to_string(),
            blank_file_meta(NodeKind::Asset),
            EntryExtensions::default(),
        );
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::EchoRedirect {
                body: "new".to_string(),
                path: PathArg::new("greet.md"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange { ref change, .. }) => match change.as_ref() {
                ChangeType::UpdateFile { content, .. } => assert_eq!(content, "new"),
                other => panic!("expected UpdateFile, got {:?}", other),
            },
            other => panic!("expected UpdateFile, got {:?}", other),
        }
    }

    #[test]
    fn test_echo_redirect_requires_admin() {
        let (ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::EchoRedirect {
                body: "x".to_string(),
                path: PathArg::new("a.md"),
            },
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_sync_status_clean_tree() {
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Sync(SyncSubcommand::Status),
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        let rendered: String = result
            .output
            .iter()
            .filter_map(|l| match &l.data {
                crate::domain::OutputLineData::Text(s) => Some(s.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rendered.contains("clean") || rendered.contains("nothing to commit"),
            "expected clean-tree message, got:\n{}",
            rendered
        );
    }

    #[test]
    fn test_sync_status_with_remote_head() {
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Sync(SyncSubcommand::Status),
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            Some("abcdef1234567890"),
        );
        assert_eq!(result.exit_code, 0);
        let has_head = result.output.iter().any(
            |l| matches!(&l.data, crate::domain::OutputLineData::Text(s) if s.contains("abcdef12")),
        );
        assert!(has_head, "expected remote HEAD prefix in output");
    }

    #[test]
    fn test_sync_status_reports_entries() {
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let mut cs = ChangeSet::new();
        cs.upsert(
            home_vpath("new.md"),
            ChangeType::CreateFile {
                content: "x".to_string(),
                meta: blank_file_meta(NodeKind::Asset),
                extensions: EntryExtensions::default(),
            },
        );
        cs.upsert(home_vpath("del.md"), ChangeType::DeleteFile);
        cs.unstage(&home_vpath("del.md"));
        let result = execute_command(
            Command::Sync(SyncSubcommand::Status),
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        let rendered: String = result
            .output
            .iter()
            .filter_map(|l| match &l.data {
                crate::domain::OutputLineData::Text(s) => Some(s.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rendered.contains("/new.md"),
            "missing /new.md: {}",
            rendered
        );
        assert!(
            rendered.contains("/del.md"),
            "missing /del.md: {}",
            rendered
        );
    }

    #[test]
    fn test_sync_commit_side_effect() {
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let mut cs = ChangeSet::new();
        cs.upsert(
            home_vpath("a.md"),
            ChangeType::CreateFile {
                content: "x".to_string(),
                meta: blank_file_meta(NodeKind::Asset),
                extensions: EntryExtensions::default(),
            },
        );
        let result = execute_command(
            Command::Sync(SyncSubcommand::Commit {
                message: "feat: x".to_string(),
            }),
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            Some("deadbeef"),
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::Commit {
                ref message,
                ref mount_root,
            }) => {
                assert_eq!(message, "feat: x");
                assert_eq!(mount_root.as_str(), "/");
            }
            other => panic!("expected Commit, got {:?}", other),
        }
    }

    #[test]
    fn test_sync_commit_requires_staged_changes() {
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Sync(SyncSubcommand::Commit {
                message: "msg".to_string(),
            }),
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_sync_commit_rejects_changes_across_multiple_mounts() {
        let runtime_mounts = vec![
            crate::storage::boot::bootstrap_runtime_mount(),
            crate::domain::RuntimeMount::new(
                VirtualPath::from_absolute("/db").unwrap(),
                "db",
                crate::domain::RuntimeBackendKind::GitHub,
                true,
            ),
        ];
        let mut cs = ChangeSet::new();
        cs.upsert(
            home_vpath("a.md"),
            ChangeType::CreateFile {
                content: "site".to_string(),
                meta: blank_file_meta(NodeKind::Asset),
                extensions: EntryExtensions::default(),
            },
        );
        cs.upsert(
            VirtualPath::from_absolute("/db/b.md").unwrap(),
            ChangeType::CreateFile {
                content: "db".to_string(),
                meta: blank_file_meta(NodeKind::Asset),
                extensions: EntryExtensions::default(),
            },
        );

        let err = sync_mount_root(&runtime_mounts, &home_cwd(""), &cs)
            .expect_err("mixed mount changes must not select a single backend");
        assert_eq!(err.exit_code, 1);
    }

    #[test]
    fn test_sync_refresh_side_effect() {
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Sync(SyncSubcommand::Refresh),
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        assert_eq!(
            result.side_effect,
            Some(SideEffect::ReloadRuntimeMount {
                mount_root: VirtualPath::root(),
            })
        );
    }

    #[test]
    fn test_sync_auth_set_side_effect() {
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Sync(SyncSubcommand::Auth(AuthAction::Set {
                token: "ghp_abc".to_string(),
            })),
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::SetAuthToken { ref token }) => assert_eq!(token, "ghp_abc"),
            other => panic!("expected SetAuthToken, got {:?}", other),
        }
    }

    #[test]
    fn test_sync_auth_clear_side_effect() {
        let (_ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Sync(SyncSubcommand::Auth(AuthAction::Clear)),
            &ws,
            &fs,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.side_effect, Some(SideEffect::ClearAuthToken));
    }

    #[test]
    fn test_has_children_empty_dir_is_false() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(home_vpath("empty"), blank_dir_meta());
        assert!(!fs.has_children(&home_vpath("empty")));
    }

    #[test]
    fn test_has_children_with_child_is_true() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(home_vpath("dir"), blank_dir_meta());
        fs.upsert_file(
            home_vpath("dir/child.md"),
            String::new(),
            blank_file_meta(NodeKind::Asset),
            EntryExtensions::default(),
        );
        assert!(fs.has_children(&home_vpath("dir")));
    }

    #[test]
    fn test_has_children_nonexistent_is_false() {
        let fs = GlobalFs::empty();
        assert!(!fs.has_children(&home_vpath("ghost")));
    }

    #[test]
    fn test_has_children_file_is_false() {
        let mut fs = GlobalFs::empty();
        fs.upsert_file(
            home_vpath("file.md"),
            String::new(),
            blank_file_meta(NodeKind::Asset),
            EntryExtensions::default(),
        );
        assert!(!fs.has_children(&home_vpath("file.md")));
    }

    /// Build the merged "current view" that the terminal dispatcher sees.
    fn view(base: &GlobalFs, changes: &ChangeSet) -> GlobalFs {
        crate::runtime::build_view_global_fs(
            base,
            changes,
            &WalletState::Disconnected,
            &crate::runtime::RuntimeStateSnapshot::default(),
        )
    }

    #[test]
    fn test_rm_on_pending_create_file_emits_discard_change() {
        // Base fs empty; ChangeSet has a pending CreateFile at /a.md.
        let base = GlobalFs::empty();
        let mut cs = ChangeSet::new();
        cs.upsert(
            home_vpath("a.md"),
            ChangeType::CreateFile {
                content: String::new(),
                meta: blank_file_meta(NodeKind::Asset),
                extensions: EntryExtensions::default(),
            },
        );
        let merged = view(&base, &cs);

        let ws = admin_wallet();
        let result = execute_command(
            Command::Rm {
                path: PathArg::new("a.md"),
                recursive: false,
            },
            &ws,
            &merged,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::DiscardChange { ref path }) => {
                assert_eq!(path.as_str(), "/a.md");
            }
            other => panic!("expected DiscardChange, got {:?}", other),
        }
    }

    #[test]
    fn test_rm_recursive_on_pending_create_directory_emits_discard_change() {
        let base = GlobalFs::empty();
        let mut cs = ChangeSet::new();
        cs.upsert(
            home_vpath("d"),
            ChangeType::CreateDirectory {
                meta: blank_dir_meta(),
            },
        );
        let merged = view(&base, &cs);

        let ws = admin_wallet();
        let result = execute_command(
            Command::Rm {
                path: PathArg::new("d"),
                recursive: true,
            },
            &ws,
            &merged,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::DiscardChange { ref path }) => {
                assert_eq!(path.as_str(), "/d");
            }
            other => panic!("expected DiscardChange, got {:?}", other),
        }
    }

    #[test]
    fn test_rmdir_on_pending_create_directory_emits_discard_change() {
        let base = GlobalFs::empty();
        let mut cs = ChangeSet::new();
        cs.upsert(
            home_vpath("d"),
            ChangeType::CreateDirectory {
                meta: blank_dir_meta(),
            },
        );
        let merged = view(&base, &cs);

        let ws = admin_wallet();
        let result = execute_command(
            Command::Rmdir {
                path: PathArg::new("d"),
            },
            &ws,
            &merged,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::DiscardChange { ref path }) => {
                assert_eq!(path.as_str(), "/d");
            }
            other => panic!("expected DiscardChange, got {:?}", other),
        }
    }

    #[test]
    fn test_rm_on_base_file_still_emits_apply_change_delete() {
        // File is in base fs, NOT in ChangeSet -> Delete, not Discard.
        let mut base = GlobalFs::empty();
        base.upsert_file(
            home_vpath("existing.md"),
            "hi".into(),
            blank_file_meta(NodeKind::Asset),
            EntryExtensions::default(),
        );
        let cs = ChangeSet::new();
        let merged = view(&base, &cs);

        let ws = admin_wallet();
        let result = execute_command(
            Command::Rm {
                path: PathArg::new("existing.md"),
                recursive: false,
            },
            &ws,
            &merged,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange {
                ref path,
                ref change,
            }) => {
                assert_eq!(path.as_str(), "/existing.md");
                assert!(matches!(change.as_ref(), ChangeType::DeleteFile));
            }
            other => panic!("expected ApplyChange(DeleteFile), got {:?}", other),
        }
    }

    #[test]
    fn test_touch_errors_when_path_is_pending_create_in_merged_view() {
        // Base does not contain /a.md, but the ChangeSet does as CreateFile.
        // After the merged runtime view is computed and passed to execute, the
        // existing `fs.get_entry(...).is_some()` guard must fire.
        let base = GlobalFs::empty();
        let mut cs = ChangeSet::new();
        cs.upsert(
            home_vpath("a.md"),
            ChangeType::CreateFile {
                content: String::new(),
                meta: blank_file_meta(NodeKind::Asset),
                extensions: EntryExtensions::default(),
            },
        );
        let merged = view(&base, &cs);

        let ws = admin_wallet();
        let result = execute_command(
            Command::Touch {
                path: PathArg::new("a.md"),
            },
            &ws,
            &merged,
            &home_cwd(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
        assert!(result.side_effect.is_none());
    }
}
