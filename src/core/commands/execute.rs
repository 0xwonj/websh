//! Command execution logic.
//!
//! Contains the `execute_command` function that runs parsed commands
//! against the virtual filesystem and returns results.

use crate::app::TerminalState;
use crate::config::{ASCII_PROFILE, HELP_TEXT, PROFILE_FILE, mounts};
use crate::core::admin::can_write_to;
use crate::core::changes::{ChangeSet, ChangeType};
use crate::core::{VirtualFs, env, wallet};
use crate::models::{AppRoute, FileMetadata, Mount, OutputLine, VirtualPath, WalletState};
use crate::utils::sysinfo;

use super::{AuthAction, Command, CommandResult, PathArg, SideEffect, SyncSubcommand};

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
/// * `fs` - Virtual filesystem
/// * `current_route` - Current route (for resolving relative paths)
/// * `changes` - The current set of pending changes
/// * `remote_head` - Last-known remote HEAD SHA (for CAS-protected commits)
pub fn execute_command(
    cmd: Command,
    state: &TerminalState,
    wallet_state: &WalletState,
    fs: &VirtualFs,
    current_route: &AppRoute,
    changes: &ChangeSet,
    remote_head: Option<&str>,
) -> CommandResult {
    // Get the filesystem path (relative, e.g., "blog" or "")
    let current_path = current_route.fs_path();

    match cmd {
        Command::Ls { path, long } => execute_ls(path, long, wallet_state, fs, current_route),
        Command::Cd(path) => execute_cd(path, fs, current_route),
        Command::Pwd => CommandResult::output(vec![OutputLine::text(current_route.display_path())]),
        Command::Cat(file) => match file {
            Some(f) => execute_cat(f, fs, current_path, current_route),
            None => CommandResult::error_line("cat: missing file operand"),
        },
        Command::Whoami => {
            CommandResult::output(vec![OutputLine::ascii(ASCII_PROFILE.to_string())])
        }
        Command::Id => execute_id(wallet_state),
        Command::Help => CommandResult::output(HELP_TEXT.lines().map(OutputLine::text).collect()),
        Command::Clear => {
            state.clear_history();
            CommandResult::empty()
        }
        Command::Echo(text) => CommandResult::output(vec![OutputLine::text(text)]),
        Command::Export(assignments) => execute_export(assignments),
        Command::Unset(key) => match key {
            Some(k) => execute_unset(k),
            None => CommandResult::error_line("unset: missing variable name"),
        },
        Command::Login => CommandResult::login(),
        Command::Logout => CommandResult::logout(),
        Command::Explorer(path) => execute_explorer(path, fs, current_route),
        Command::Touch { path } => execute_touch(path, wallet_state, fs, current_route),
        Command::Mkdir { path } => execute_mkdir(path, wallet_state, fs, current_route),
        Command::Rm { path, recursive } => {
            execute_rm(path, recursive, wallet_state, fs, current_route)
        }
        Command::Rmdir { path } => execute_rmdir(path, wallet_state, fs, current_route),
        Command::Edit { path } => execute_edit(path, wallet_state, fs, current_route),
        Command::EchoRedirect { body, path } => {
            execute_echo_redirect(body, path, wallet_state, fs, current_route)
        }
        Command::Sync(sub) => execute_sync(sub, wallet_state, current_route, changes, remote_head),
        Command::Unknown(cmd) => CommandResult::error_line(format!(
            "Command not found: {}. Type 'help' for available commands.",
            cmd
        ))
        .with_exit_code(127),
    }
}

/// Execute `ls` command.
fn execute_ls(
    path: Option<super::PathArg>,
    long: bool,
    wallet_state: &WalletState,
    fs: &VirtualFs,
    current_route: &AppRoute,
) -> CommandResult {
    let target = path.as_ref().map(|p| p.as_str()).unwrap_or(".");

    // Check if we're at Root or targeting Root
    let at_root = matches!(current_route, AppRoute::Root);
    let target_is_current = target == "." || target.is_empty();
    let target_is_root = target == "/" || target == "..";

    // If at Root and listing current directory, show mounts
    if at_root && (target_is_current || target_is_root) {
        return list_mounts(long);
    }

    // If at Root and targeting a mount alias, resolve it
    if at_root {
        if resolve_mount_alias(target).is_some() {
            // List the mount's root directory
            if let Some(entries) = fs.list_dir("") {
                let output = format_ls_output(&entries, "", long, wallet_state, fs);
                return CommandResult::output(output);
            }
        }
        return CommandResult::error_line(format!(
            "ls: cannot access '{}': No such file or directory",
            target
        ));
    }

    // Normal filesystem ls
    let current_path = current_route.fs_path();
    let resolved = fs.resolve_path(current_path, target);

    match resolved {
        Some(resolved_path) => {
            if let Some(entries) = fs.list_dir(&resolved_path) {
                let output = format_ls_output(&entries, &resolved_path, long, wallet_state, fs);
                CommandResult::output(output)
            } else {
                CommandResult::error_line(format!(
                    "ls: cannot access '{}': Not a directory",
                    target
                ))
            }
        }
        None => CommandResult::error_line(format!(
            "ls: cannot access '{}': No such file or directory",
            target
        )),
    }
}

/// Format ls output for directory entries.
fn format_ls_output(
    entries: &[crate::core::DirEntry],
    resolved_path: &str,
    long: bool,
    wallet_state: &WalletState,
    fs: &VirtualFs,
) -> Vec<OutputLine> {
    if long {
        entries
            .iter()
            .map(|entry| {
                let entry_path = if resolved_path.is_empty() {
                    entry.name.clone()
                } else {
                    format!("{}/{}", resolved_path, &entry.name)
                };
                let fs_entry = fs.get_entry(&entry_path);
                let perms = fs_entry
                    .map(|e| fs.get_permissions(e, wallet_state))
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
                        .file_meta
                        .as_ref()
                        .map(|m| m.is_restricted())
                        .unwrap_or(false);
                    OutputLine::file_entry(&entry.name, &entry.title, is_restricted)
                }
            })
            .collect()
    }
}

/// List available mounts as directory entries.
fn list_mounts(long: bool) -> CommandResult {
    let registry = mounts();

    let output: Vec<OutputLine> = if long {
        registry
            .all()
            .map(|mount| {
                let perms = crate::models::DisplayPermissions {
                    is_dir: true,
                    read: true,
                    write: false,
                    execute: true,
                };
                let entry = crate::core::DirEntry {
                    name: mount.alias().to_string(),
                    is_dir: true,
                    title: mount.description(),
                    file_meta: None,
                };
                OutputLine::long_entry(&entry, &perms)
            })
            .collect()
    } else {
        registry
            .all()
            .map(|mount| OutputLine::dir_entry(mount.alias(), mount.description()))
            .collect()
    };

    CommandResult::output(output)
}

/// Resolve a mount alias to a Mount.
fn resolve_mount_alias(alias: &str) -> Option<Mount> {
    mounts().resolve(alias).cloned()
}

/// Execute `cd` command.
fn execute_cd(path: super::PathArg, fs: &VirtualFs, current_route: &AppRoute) -> CommandResult {
    let target = path.as_str();
    let at_root = matches!(current_route, AppRoute::Root);

    // Handle special paths
    match target {
        // cd "" — POSIX: error (bash prints "cd: : No such file or directory")
        "" => {
            return CommandResult::error_line("cd: : No such file or directory");
        }

        // cd / always goes to Root
        "/" => return CommandResult::navigate(AppRoute::Root),

        // cd ~ always goes to home mount root
        "~" => return CommandResult::navigate(AppRoute::home()),

        // cd .. from Root stays at Root
        ".." if at_root => return CommandResult::navigate(AppRoute::Root),

        // cd .. from mount root goes to Root
        ".." if current_route.fs_path().is_empty() => {
            return CommandResult::navigate(AppRoute::Root);
        }

        _ => {}
    }

    // If at Root, target should be a mount alias
    if at_root {
        if let Some(mount) = resolve_mount_alias(target) {
            return CommandResult::navigate(AppRoute::Browse {
                mount,
                path: String::new(),
            });
        }
        return CommandResult::error_line(format!(
            "cd: no such file or directory: {}",
            target
        ));
    }

    // Normal filesystem cd within a mount
    let current_path = current_route.fs_path();
    let current_mount = current_route
        .mount()
        .cloned()
        .unwrap_or_else(|| mounts().home().clone());

    match fs.resolve_path(current_path, target) {
        Some(new_path) if fs.is_directory(&new_path) => CommandResult::navigate(AppRoute::Browse {
            mount: current_mount,
            path: new_path,
        }),
        Some(_) => CommandResult::error_line(format!("cd: not a directory: {}", path)),
        None => CommandResult::error_line(format!("cd: no such file or directory: {}", path)),
    }
}

/// Execute `cat` command.
fn execute_cat(
    file: super::PathArg,
    fs: &VirtualFs,
    current_path: &str,
    current_route: &AppRoute,
) -> CommandResult {
    // cat doesn't work at Root (no files there)
    if matches!(current_route, AppRoute::Root) {
        return CommandResult::error_line(format!("cat: {}: No such file or directory", file));
    }

    let current_mount = current_route
        .mount()
        .cloned()
        .unwrap_or_else(|| mounts().home().clone());

    let resolved = fs.resolve_path(current_path, file.as_str());

    match resolved {
        Some(resolved_path) => {
            if fs.is_directory(&resolved_path) {
                CommandResult::error_line(format!("cat: {}: Is a directory", file))
            } else if resolved_path == PROFILE_FILE {
                // Dynamic .profile from environment variables
                let content = env::generate_profile();
                let mut lines = vec![OutputLine::empty()];
                for line in content.lines() {
                    lines.push(OutputLine::text(line));
                }
                lines.push(OutputLine::empty());
                CommandResult::output(lines)
            } else if fs.get_file_content_path(&resolved_path).is_some() {
                // Navigate to file route (opens reader overlay)
                CommandResult::navigate(AppRoute::Read {
                    mount: current_mount,
                    path: resolved_path,
                })
            } else {
                CommandResult::error_line(format!("cat: {}: No content available", file))
            }
        }
        None => CommandResult::error_line(format!("cat: {}: No such file or directory", file)),
    }
}

/// Execute `id` command.
fn execute_id(wallet_state: &WalletState) -> CommandResult {
    let mut lines = vec![OutputLine::empty()];

    // User identity
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

    // Network info
    if let Some(chain_id) = wallet_state.chain_id() {
        lines.push(OutputLine::text(format!(
            "network={}",
            wallet::chain_name(chain_id)
        )));
        lines.push(OutputLine::text(format!("chain_id={}", chain_id)));
    } else {
        lines.push(OutputLine::text("network=none"));
    }

    // Session uptime
    if let Some(uptime) = sysinfo::get_uptime() {
        lines.push(OutputLine::text(format!("uptime={}", uptime)));
    }

    // Browser info
    if let Some(window) = web_sys::window()
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
/// An empty list prints all user variables. When any assignment fails,
/// an error line is emitted and the first failure sets exit_code=1;
/// subsequent assignments are still attempted (bash-compatible).
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
    for arg in assignments {
        if let Some((key, value)) = arg.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if let Err(e) = env::set_user_var(key, value) {
                output.push(OutputLine::error(format!("export: {}", e)));
                if exit_code == 0 {
                    exit_code = 1;
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

    CommandResult::output(output).with_exit_code(exit_code)
}

/// Execute `unset` command.
fn execute_unset(key: String) -> CommandResult {
    if env::get_user_var(&key).is_some() {
        match env::unset_user_var(&key) {
            Ok(()) => CommandResult::empty(),
            Err(e) => CommandResult::error_line(format!("unset: {}", e)),
        }
    } else {
        CommandResult::empty() // Silently succeed if variable doesn't exist
    }
}

/// Execute `explorer` command.
fn execute_explorer(
    path: Option<super::PathArg>,
    fs: &VirtualFs,
    current_route: &AppRoute,
) -> CommandResult {
    use crate::models::ViewMode;

    let Some(path_arg) = path else {
        return CommandResult::switch_view(ViewMode::Explorer);
    };

    let current_path = current_route.fs_path();
    match fs.resolve_path(current_path, path_arg.as_str()) {
        Some(new_path) if fs.is_directory(&new_path) => {
            let mount = current_route
                .mount()
                .cloned()
                .unwrap_or_else(|| mounts().home().clone());
            CommandResult::open_explorer(AppRoute::Browse {
                mount,
                path: new_path,
            })
        }
        Some(_) => CommandResult::error_line(format!(
            "explorer: not a directory: {}",
            path_arg
        )),
        None => CommandResult::error_line(format!(
            "explorer: no such file or directory: {}",
            path_arg
        )),
    }
}

// =============================================================================
// Phase 4 — Write / Sync execution arms
// =============================================================================

/// Resolve an admin + mount preflight for write commands. Returns the write
/// target mount when the caller may write to `current_route`, or a
/// `CommandResult` error otherwise.
///
/// Centralising this lets every write arm emit the same error string and keeps
/// admin gating in one place.
fn require_write_access(
    cmd_label: &str,
    wallet_state: &WalletState,
    current_route: &AppRoute,
) -> Result<Mount, CommandResult> {
    let mount = current_route
        .mount()
        .cloned()
        .unwrap_or_else(|| mounts().home().clone());
    if can_write_to(wallet_state, &mount) {
        Ok(mount)
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
fn resolve_abs_path(
    cmd_label: &str,
    path: &PathArg,
    current_route: &AppRoute,
) -> Result<(VirtualPath, String), CommandResult> {
    let rel = VirtualFs::resolve_path_string(current_route.fs_path(), path.as_str());
    let abs_str = if rel.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", rel)
    };
    match VirtualPath::from_absolute(&abs_str) {
        Ok(vp) => Ok((vp, rel)),
        Err(e) => Err(CommandResult::error_line(format!(
            "{}: invalid path '{}': {}",
            cmd_label, path, e
        ))),
    }
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
    fs: &VirtualFs,
    current_route: &AppRoute,
) -> CommandResult {
    if let Err(e) = require_write_access("touch", wallet_state, current_route) {
        return e;
    }

    let (vp, rel) = match resolve_abs_path("touch", &path, current_route) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if fs.get_entry(&rel).is_some() {
        return CommandResult::error_line(format!("touch: {}: path already exists", path));
    }

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::ApplyChange {
            path: vp,
            change: ChangeType::CreateFile {
                content: String::new(),
                meta: FileMetadata::default(),
            },
        }),
    }
}

/// Execute `mkdir` — create a directory.
fn execute_mkdir(
    path: PathArg,
    wallet_state: &WalletState,
    fs: &VirtualFs,
    current_route: &AppRoute,
) -> CommandResult {
    if let Err(e) = require_write_access("mkdir", wallet_state, current_route) {
        return e;
    }

    let (vp, rel) = match resolve_abs_path("mkdir", &path, current_route) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if fs.get_entry(&rel).is_some() {
        return CommandResult::error_line(format!("mkdir: {}: path already exists", path));
    }

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::ApplyChange {
            path: vp,
            change: ChangeType::CreateDirectory {
                meta: crate::models::DirectoryMetadata::default(),
            },
        }),
    }
}

/// Execute `rm` — delete a file or directory (with `-r` for directories).
fn execute_rm(
    path: PathArg,
    recursive: bool,
    wallet_state: &WalletState,
    fs: &VirtualFs,
    current_route: &AppRoute,
) -> CommandResult {
    if let Err(e) = require_write_access("rm", wallet_state, current_route) {
        return e;
    }

    let (vp, rel) = match resolve_abs_path("rm", &path, current_route) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let Some(entry) = fs.get_entry(&rel) else {
        return CommandResult::error_line(format!("rm: {}: no such file or directory", path));
    };

    let change = if entry.is_directory() {
        if !recursive {
            return CommandResult::error_line(format!(
                "rm: {}: is a directory (use -r)",
                path
            ));
        }
        ChangeType::DeleteDirectory
    } else {
        ChangeType::DeleteFile
    };

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::ApplyChange { path: vp, change }),
    }
}

/// Execute `rmdir` — delete an empty directory.
fn execute_rmdir(
    path: PathArg,
    wallet_state: &WalletState,
    fs: &VirtualFs,
    current_route: &AppRoute,
) -> CommandResult {
    if let Err(e) = require_write_access("rmdir", wallet_state, current_route) {
        return e;
    }

    let (vp, rel) = match resolve_abs_path("rmdir", &path, current_route) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let Some(entry) = fs.get_entry(&rel) else {
        return CommandResult::error_line(format!("rmdir: {}: no such file or directory", path));
    };

    if !entry.is_directory() {
        return CommandResult::error_line(format!("rmdir: {}: not a directory", path));
    }

    if fs.has_children(&rel) {
        return CommandResult::error_line(format!("rmdir: {}: directory not empty", path));
    }

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::ApplyChange {
            path: vp,
            change: ChangeType::DeleteDirectory,
        }),
    }
}

/// Execute `edit` — request the editor UI open for a file.
fn execute_edit(
    path: PathArg,
    wallet_state: &WalletState,
    fs: &VirtualFs,
    current_route: &AppRoute,
) -> CommandResult {
    if let Err(e) = require_write_access("edit", wallet_state, current_route) {
        return e;
    }

    let (vp, rel) = match resolve_abs_path("edit", &path, current_route) {
        Ok(v) => v,
        Err(e) => return e,
    };

    // Path must either not exist yet (create-on-save) or be a file.
    if let Some(entry) = fs.get_entry(&rel) {
        if entry.is_directory() {
            return CommandResult::error_line(format!("edit: {}: is a directory", path));
        }
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
    fs: &VirtualFs,
    current_route: &AppRoute,
) -> CommandResult {
    if let Err(e) = require_write_access("echo", wallet_state, current_route) {
        return e;
    }

    let (vp, rel) = match resolve_abs_path("echo", &path, current_route) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let change = match fs.get_entry(&rel) {
        Some(entry) if entry.is_directory() => {
            return CommandResult::error_line(format!("echo: {}: is a directory", path));
        }
        Some(_) => ChangeType::UpdateFile {
            content: body,
            description: None,
        },
        None => ChangeType::CreateFile {
            content: body,
            meta: FileMetadata::default(),
        },
    };

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::ApplyChange { path: vp, change }),
    }
}

/// Execute `sync <sub>` — status / commit / refresh / auth.
fn execute_sync(
    sub: SyncSubcommand,
    wallet_state: &WalletState,
    current_route: &AppRoute,
    changes: &ChangeSet,
    remote_head: Option<&str>,
) -> CommandResult {
    match sub {
        SyncSubcommand::Status => execute_sync_status(changes, remote_head),
        SyncSubcommand::Commit { message } => {
            execute_sync_commit(message, wallet_state, current_route, changes, remote_head)
        }
        SyncSubcommand::Refresh => CommandResult {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::RefreshManifest),
        },
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

    let mut staged: Vec<(&crate::models::VirtualPath, &crate::core::changes::Entry)> =
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

    let mut unstaged: Vec<(&crate::models::VirtualPath, &crate::core::changes::Entry)> =
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
    current_route: &AppRoute,
    changes: &ChangeSet,
    remote_head: Option<&str>,
) -> CommandResult {
    if let Err(e) = require_write_access("sync commit", wallet_state, current_route) {
        return e;
    }

    if message.trim().is_empty() {
        return CommandResult::error_line("sync commit: empty commit message");
    }

    let staged = changes.summary().total_staged();
    if staged == 0 {
        return CommandResult::error_line("sync commit: no staged changes");
    }

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effect: Some(SideEffect::Commit {
            message,
            expected_head: remote_head.map(|s| s.to_string()),
        }),
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::SideEffect;
    use crate::app::TerminalState;
    use crate::core::VirtualFs;
    use crate::core::changes::ChangeSet;
    use crate::models::{AppRoute, ViewMode, WalletState};

    fn empty_state() -> (TerminalState, WalletState, VirtualFs) {
        (
            TerminalState::new(),
            WalletState::Disconnected,
            VirtualFs::empty(),
        )
    }

    /// Admin wallet constructor for write-path tests.
    ///
    /// Mirrors the placeholder in `crate::core::admin::ADMIN_ADDRESSES`.
    fn admin_wallet() -> WalletState {
        WalletState::Connected {
            address: "0x0000000000000000000000000000000000000000".to_string(),
            ens_name: None,
            chain_id: Some(1),
        }
    }

    fn home_browse(path: &str) -> AppRoute {
        AppRoute::Browse {
            mount: crate::config::mounts().home().clone(),
            path: path.to_string(),
        }
    }

    #[test]
    fn test_login_returns_login_side_effect() {
        let (ts, ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(Command::Login, &ts, &ws, &fs, &AppRoute::Root, &cs, None);
        assert_eq!(result.side_effect, Some(SideEffect::Login));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_logout_returns_logout_side_effect() {
        let (ts, ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(Command::Logout, &ts, &ws, &fs, &AppRoute::Root, &cs, None);
        assert_eq!(result.side_effect, Some(SideEffect::Logout));
    }

    #[test]
    fn test_explorer_no_arg_switches_view() {
        let (ts, ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Explorer(None),
            &ts,
            &ws,
            &fs,
            &AppRoute::Root,
            &cs,
            None,
        );
        assert_eq!(
            result.side_effect,
            Some(SideEffect::SwitchView(ViewMode::Explorer))
        );
    }

    #[test]
    fn test_unknown_command_exit_127() {
        let (ts, ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Unknown("foobar".into()),
            &ts,
            &ws,
            &fs,
            &AppRoute::Root,
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 127);
    }

    #[test]
    fn test_ls_nonexistent_exit_1() {
        let (ts, ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Ls {
                path: Some(super::super::PathArg::new("nonexistent")),
                long: false,
            },
            &ts,
            &ws,
            &fs,
            &AppRoute::Root,
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
        assert!(!result.output.is_empty());
    }

    #[test]
    fn test_cat_missing_operand_exit_1() {
        let (ts, ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(Command::Cat(None), &ts, &ws, &fs, &AppRoute::Root, &cs, None);
        assert_eq!(result.exit_code, 1);
        assert!(
            result
                .output
                .iter()
                .any(|l| matches!(&l.data, crate::models::OutputLineData::Error(s) if s == "cat: missing file operand"))
        );
    }

    #[test]
    fn test_unset_missing_operand_exit_1() {
        let (ts, ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(Command::Unset(None), &ts, &ws, &fs, &AppRoute::Root, &cs, None);
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_execute_export_multi_processes_each_assignment() {
        // On native (no localStorage), each assignment triggers an
        // EnvironmentError::StorageUnavailable. We verify:
        //   - exit code is non-zero (at least one assignment errored)
        //   - there is one error line per assignment
        // This confirms the loop iterates per arg rather than joining them.
        let (ts, ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Export(vec![
                "FOO_P2_A=alpha".to_string(),
                "BAR_P2_A=beta".to_string(),
            ]),
            &ts,
            &ws,
            &fs,
            &AppRoute::Root,
            &cs,
            None,
        );
        let error_count = result
            .output
            .iter()
            .filter(|l| matches!(&l.data, crate::models::OutputLineData::Error(_)))
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
        let _ = crate::core::env::unset_user_var("FOO_P2_A");
        let _ = crate::core::env::unset_user_var("BAR_P2_A");
    }

    #[test]
    fn test_cd_empty_string_exit_1() {
        // POSIX bash: `cd ""` errors with "cd: : No such file or directory".
        // Must exercise a non-Root route so the early `at_root` branch doesn't
        // short-circuit to the generic mount-alias error.
        let (ts, ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let browse_route = home_browse("");
        let result = execute_command(
            Command::Cd(super::super::PathArg::new("")),
            &ts,
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
                crate::models::OutputLineData::Error(s) if s == "cd: : No such file or directory"
            )),
            "expected POSIX cd error; got: {:?}",
            result.output
        );
    }

    // ======================================================================
    // Phase 4 write-command tests
    // ======================================================================

    #[test]
    fn test_touch_requires_admin() {
        let (ts, ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Touch { path: PathArg::new("new.md") },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
        assert!(result.side_effect.is_none());
    }

    #[test]
    fn test_touch_creates_apply_change_side_effect() {
        let (ts, _ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Touch { path: PathArg::new("new.md") },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange { ref path, ref change }) => {
                assert_eq!(path.as_str(), "/new.md");
                assert!(matches!(change, ChangeType::CreateFile { .. }));
            }
            other => panic!("expected ApplyChange, got {:?}", other),
        }
    }

    #[test]
    fn test_touch_errors_when_path_exists_in_fs() {
        // Build an fs with a file at "new.md"
        let mut fs = VirtualFs::empty();
        fs.upsert_file(
            VirtualPath::from_absolute("/new.md").unwrap(),
            String::new(),
            FileMetadata::default(),
        );
        let ts = TerminalState::new();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Touch { path: PathArg::new("new.md") },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_mkdir_creates_apply_change_side_effect() {
        let (ts, _ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Mkdir { path: PathArg::new("newdir") },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange { ref path, ref change }) => {
                assert_eq!(path.as_str(), "/newdir");
                assert!(matches!(change, ChangeType::CreateDirectory { .. }));
            }
            other => panic!("expected ApplyChange, got {:?}", other),
        }
    }

    #[test]
    fn test_mkdir_errors_when_path_exists() {
        let mut fs = VirtualFs::empty();
        fs.upsert_directory(
            VirtualPath::from_absolute("/dir").unwrap(),
            crate::models::DirectoryMetadata::default(),
        );
        let ts = TerminalState::new();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Mkdir { path: PathArg::new("dir") },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_rm_file_side_effect() {
        let mut fs = VirtualFs::empty();
        fs.upsert_file(
            VirtualPath::from_absolute("/doomed.md").unwrap(),
            String::new(),
            FileMetadata::default(),
        );
        let ts = TerminalState::new();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rm { path: PathArg::new("doomed.md"), recursive: false },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange { ref path, change: ChangeType::DeleteFile }) => {
                assert_eq!(path.as_str(), "/doomed.md");
            }
            other => panic!("expected DeleteFile ApplyChange, got {:?}", other),
        }
    }

    #[test]
    fn test_rm_directory_without_r_errors() {
        let mut fs = VirtualFs::empty();
        fs.upsert_directory(
            VirtualPath::from_absolute("/dir").unwrap(),
            crate::models::DirectoryMetadata::default(),
        );
        let ts = TerminalState::new();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rm { path: PathArg::new("dir"), recursive: false },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_rm_directory_recursive_side_effect() {
        let mut fs = VirtualFs::empty();
        fs.upsert_directory(
            VirtualPath::from_absolute("/dir").unwrap(),
            crate::models::DirectoryMetadata::default(),
        );
        let ts = TerminalState::new();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rm { path: PathArg::new("dir"), recursive: true },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange { ref path, change: ChangeType::DeleteDirectory }) => {
                assert_eq!(path.as_str(), "/dir");
            }
            other => panic!("expected DeleteDirectory ApplyChange, got {:?}", other),
        }
    }

    #[test]
    fn test_rm_nonexistent_path_errors() {
        let (ts, _ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rm { path: PathArg::new("ghost.md"), recursive: false },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_rmdir_empty_directory_side_effect() {
        let mut fs = VirtualFs::empty();
        fs.upsert_directory(
            VirtualPath::from_absolute("/empty").unwrap(),
            crate::models::DirectoryMetadata::default(),
        );
        let ts = TerminalState::new();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rmdir { path: PathArg::new("empty") },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange { change: ChangeType::DeleteDirectory, .. }) => {}
            other => panic!("expected DeleteDirectory, got {:?}", other),
        }
    }

    #[test]
    fn test_rmdir_nonempty_directory_errors() {
        let mut fs = VirtualFs::empty();
        fs.upsert_directory(
            VirtualPath::from_absolute("/dir").unwrap(),
            crate::models::DirectoryMetadata::default(),
        );
        fs.upsert_file(
            VirtualPath::from_absolute("/dir/child.md").unwrap(),
            String::new(),
            FileMetadata::default(),
        );
        let ts = TerminalState::new();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rmdir { path: PathArg::new("dir") },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_rmdir_on_file_errors() {
        let mut fs = VirtualFs::empty();
        fs.upsert_file(
            VirtualPath::from_absolute("/file.md").unwrap(),
            String::new(),
            FileMetadata::default(),
        );
        let ts = TerminalState::new();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Rmdir { path: PathArg::new("file.md") },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_edit_opens_editor_for_existing_file() {
        let mut fs = VirtualFs::empty();
        fs.upsert_file(
            VirtualPath::from_absolute("/note.md").unwrap(),
            "hi".to_string(),
            FileMetadata::default(),
        );
        let ts = TerminalState::new();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Edit { path: PathArg::new("note.md") },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
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
        let (ts, _ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Edit { path: PathArg::new("fresh.md") },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
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
        let mut fs = VirtualFs::empty();
        fs.upsert_directory(
            VirtualPath::from_absolute("/dir").unwrap(),
            crate::models::DirectoryMetadata::default(),
        );
        let ts = TerminalState::new();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Edit { path: PathArg::new("dir") },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_echo_redirect_writes_content() {
        let (ts, _ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::EchoRedirect {
                body: "hello".to_string(),
                path: PathArg::new("greeting.md"),
            },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange { ref path, ref change }) => {
                assert_eq!(path.as_str(), "/greeting.md");
                match change {
                    ChangeType::CreateFile { content, .. } => assert_eq!(content, "hello"),
                    other => panic!("expected CreateFile, got {:?}", other),
                }
            }
            other => panic!("expected ApplyChange, got {:?}", other),
        }
    }

    #[test]
    fn test_echo_redirect_updates_existing_file() {
        let mut fs = VirtualFs::empty();
        fs.upsert_file(
            VirtualPath::from_absolute("/greet.md").unwrap(),
            "old".to_string(),
            FileMetadata::default(),
        );
        let ts = TerminalState::new();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::EchoRedirect {
                body: "new".to_string(),
                path: PathArg::new("greet.md"),
            },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::ApplyChange { change: ChangeType::UpdateFile { ref content, .. }, .. }) => {
                assert_eq!(content, "new");
            }
            other => panic!("expected UpdateFile, got {:?}", other),
        }
    }

    #[test]
    fn test_echo_redirect_requires_admin() {
        let (ts, ws, fs) = empty_state();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::EchoRedirect {
                body: "x".to_string(),
                path: PathArg::new("a.md"),
            },
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_sync_status_clean_tree() {
        let (ts, _ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Sync(SyncSubcommand::Status),
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        let rendered: String = result
            .output
            .iter()
            .filter_map(|l| match &l.data {
                crate::models::OutputLineData::Text(s) => Some(s.clone()),
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
        let (ts, _ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Sync(SyncSubcommand::Status),
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            Some("abcdef1234567890"),
        );
        assert_eq!(result.exit_code, 0);
        let has_head = result.output.iter().any(|l| {
            matches!(&l.data, crate::models::OutputLineData::Text(s) if s.contains("abcdef12"))
        });
        assert!(has_head, "expected remote HEAD prefix in output");
    }

    #[test]
    fn test_sync_status_reports_entries() {
        let (ts, _ws, fs) = empty_state();
        let ws = admin_wallet();
        let mut cs = ChangeSet::new();
        cs.upsert(
            VirtualPath::from_absolute("/new.md").unwrap(),
            ChangeType::CreateFile { content: "x".to_string(), meta: FileMetadata::default() },
        );
        cs.upsert(
            VirtualPath::from_absolute("/del.md").unwrap(),
            ChangeType::DeleteFile,
        );
        cs.unstage(&VirtualPath::from_absolute("/del.md").unwrap());
        let result = execute_command(
            Command::Sync(SyncSubcommand::Status),
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        let rendered: String = result
            .output
            .iter()
            .filter_map(|l| match &l.data {
                crate::models::OutputLineData::Text(s) => Some(s.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("/new.md"), "missing /new.md: {}", rendered);
        assert!(rendered.contains("/del.md"), "missing /del.md: {}", rendered);
    }

    #[test]
    fn test_sync_commit_side_effect() {
        let (ts, _ws, fs) = empty_state();
        let ws = admin_wallet();
        let mut cs = ChangeSet::new();
        cs.upsert(
            VirtualPath::from_absolute("/a.md").unwrap(),
            ChangeType::CreateFile { content: "x".to_string(), meta: FileMetadata::default() },
        );
        let result = execute_command(
            Command::Sync(SyncSubcommand::Commit { message: "feat: x".to_string() }),
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            Some("deadbeef"),
        );
        assert_eq!(result.exit_code, 0);
        match result.side_effect {
            Some(SideEffect::Commit { ref message, ref expected_head }) => {
                assert_eq!(message, "feat: x");
                assert_eq!(expected_head.as_deref(), Some("deadbeef"));
            }
            other => panic!("expected Commit, got {:?}", other),
        }
    }

    #[test]
    fn test_sync_commit_requires_staged_changes() {
        let (ts, _ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Sync(SyncSubcommand::Commit { message: "msg".to_string() }),
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_sync_refresh_side_effect() {
        let (ts, _ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Sync(SyncSubcommand::Refresh),
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.side_effect, Some(SideEffect::RefreshManifest));
    }

    #[test]
    fn test_sync_auth_set_side_effect() {
        let (ts, _ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Sync(SyncSubcommand::Auth(AuthAction::Set {
                token: "ghp_abc".to_string(),
            })),
            &ts,
            &ws,
            &fs,
            &home_browse(""),
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
        let (ts, _ws, fs) = empty_state();
        let ws = admin_wallet();
        let cs = ChangeSet::new();
        let result = execute_command(
            Command::Sync(SyncSubcommand::Auth(AuthAction::Clear)),
            &ts,
            &ws,
            &fs,
            &home_browse(""),
            &cs,
            None,
        );
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.side_effect, Some(SideEffect::ClearAuthToken));
    }

    #[test]
    fn test_has_children_empty_dir_is_false() {
        let mut fs = VirtualFs::empty();
        fs.upsert_directory(
            VirtualPath::from_absolute("/empty").unwrap(),
            crate::models::DirectoryMetadata::default(),
        );
        assert!(!fs.has_children("empty"));
    }

    #[test]
    fn test_has_children_with_child_is_true() {
        let mut fs = VirtualFs::empty();
        fs.upsert_directory(
            VirtualPath::from_absolute("/dir").unwrap(),
            crate::models::DirectoryMetadata::default(),
        );
        fs.upsert_file(
            VirtualPath::from_absolute("/dir/child.md").unwrap(),
            String::new(),
            FileMetadata::default(),
        );
        assert!(fs.has_children("dir"));
    }

    #[test]
    fn test_has_children_nonexistent_is_false() {
        let fs = VirtualFs::empty();
        assert!(!fs.has_children("ghost"));
    }

    #[test]
    fn test_has_children_file_is_false() {
        let mut fs = VirtualFs::empty();
        fs.upsert_file(
            VirtualPath::from_absolute("/file.md").unwrap(),
            String::new(),
            FileMetadata::default(),
        );
        assert!(!fs.has_children("file.md"));
    }
}
