//! Command execution logic.
//!
//! Contains the `execute_command` function that runs parsed commands
//! against the virtual filesystem and returns results.

use crate::app::TerminalState;
use crate::config::{ASCII_PROFILE, HELP_TEXT, PROFILE_FILE, configured_mounts};
use crate::core::storage::{PendingChanges, StagedChanges};
use crate::core::{MergedFs, env, wallet};
use crate::models::{AppRoute, Mount, OutputLine, WalletState};
use crate::utils::sysinfo;

use super::{Command, CommandResult};

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
/// * `fs` - Merged filesystem (base + pending changes)
/// * `current_route` - Current route (for resolving relative paths)
/// * `pending` - Current pending changes (for admin commands)
/// * `staged` - Current staged changes (for sync commands)
pub fn execute_command(
    cmd: Command,
    state: &TerminalState,
    wallet_state: &WalletState,
    fs: &MergedFs,
    current_route: &AppRoute,
    pending: &PendingChanges,
    staged: &StagedChanges,
) -> CommandResult {
    // Get the filesystem path (relative, e.g., "blog" or "")
    let current_path = current_route.fs_path();

    match cmd {
        Command::Ls { path, long } => execute_ls(path, long, wallet_state, fs, current_route),
        Command::Cd(path) => execute_cd(path, fs, current_route),
        Command::Pwd => CommandResult::output(vec![OutputLine::text(current_route.display_path())]),
        Command::Cat(file) => execute_cat(file, fs, current_path, current_route),
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
        Command::Export(arg) => execute_export(arg),
        Command::Unset(key) => execute_unset(key),
        Command::Unknown(cmd) => CommandResult::output(vec![OutputLine::error(format!(
            "Command not found: {}. Type 'help' for available commands.",
            cmd
        ))]),
        // Login/Logout/Explorer are handled in terminal.rs
        Command::Login | Command::Logout | Command::Explorer(_) => CommandResult::empty(),

        // Admin commands
        Command::Touch(path) => execute_touch(path, wallet_state, fs, current_route, pending),
        Command::Mkdir(path) => execute_mkdir(path, wallet_state, fs, current_route, pending),
        Command::Rm(path) => execute_rm(path, wallet_state, fs, current_route, pending),
        Command::Rmdir(path) => execute_rmdir(path, wallet_state, fs, current_route, pending),

        // Sync commands
        Command::SyncStatus => execute_sync_status(wallet_state, pending, staged),
        Command::SyncAdd(path) => execute_sync_add(path, wallet_state, fs, current_route, pending, staged),
        Command::SyncReset(path) => execute_sync_reset(path, wallet_state, staged),
        Command::SyncCommit(msg) => execute_sync_commit(msg, wallet_state, pending, staged),
        Command::SyncDiscard(path) => execute_sync_discard(path, wallet_state, pending),
        Command::SyncAuth { provider, token } => execute_sync_auth(provider, token),
    }
}

/// Execute `ls` command.
fn execute_ls(
    path: Option<super::PathArg>,
    long: bool,
    wallet_state: &WalletState,
    fs: &MergedFs,
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
        return CommandResult::output(vec![OutputLine::error(format!(
            "ls: cannot access '{}': No such file or directory",
            target
        ))]);
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
                CommandResult::output(vec![OutputLine::error(format!(
                    "ls: cannot access '{}': Not a directory",
                    target
                ))])
            }
        }
        None => CommandResult::output(vec![OutputLine::error(format!(
            "ls: cannot access '{}': No such file or directory",
            target
        ))]),
    }
}

/// Format ls output for directory entries.
fn format_ls_output(
    entries: &[crate::core::DirEntry],
    resolved_path: &str,
    long: bool,
    wallet_state: &WalletState,
    fs: &MergedFs,
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
                    .as_ref()
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
                    let is_encrypted = entry
                        .file_meta
                        .as_ref()
                        .map(|m| m.is_encrypted())
                        .unwrap_or(false);
                    OutputLine::file_entry(&entry.name, &entry.title, is_encrypted)
                }
            })
            .collect()
    }
}

/// List available mounts as directory entries.
fn list_mounts(long: bool) -> CommandResult {
    let mounts = configured_mounts();

    let output: Vec<OutputLine> = if long {
        mounts
            .iter()
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
        mounts
            .iter()
            .map(|mount| OutputLine::dir_entry(mount.alias(), mount.description()))
            .collect()
    };

    CommandResult::output(output)
}

/// Resolve a mount alias to a Mount.
fn resolve_mount_alias(alias: &str) -> Option<Mount> {
    configured_mounts().into_iter().find(|m| m.alias() == alias)
}

/// Execute `cd` command.
fn execute_cd(path: super::PathArg, fs: &MergedFs, current_route: &AppRoute) -> CommandResult {
    let target = path.as_str();
    let at_root = matches!(current_route, AppRoute::Root);

    // Handle special paths
    match target {
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
        return CommandResult::output(vec![OutputLine::error(format!(
            "cd: no such file or directory: {}",
            target
        ))]);
    }

    // Normal filesystem cd within a mount
    let current_path = current_route.fs_path();
    let current_mount = current_route.mount().cloned().unwrap_or_else(|| {
        configured_mounts()
            .into_iter()
            .next()
            .expect("At least one mount must be configured")
    });

    match fs.resolve_path(current_path, target) {
        Some(new_path) if fs.is_directory(&new_path) => CommandResult::navigate(AppRoute::Browse {
            mount: current_mount,
            path: new_path,
        }),
        Some(_) => CommandResult::output(vec![OutputLine::error(format!(
            "cd: not a directory: {}",
            path
        ))]),
        None => CommandResult::output(vec![OutputLine::error(format!(
            "cd: no such file or directory: {}",
            path
        ))]),
    }
}

/// Execute `cat` command.
fn execute_cat(
    file: super::PathArg,
    fs: &MergedFs,
    current_path: &str,
    current_route: &AppRoute,
) -> CommandResult {
    // cat doesn't work at Root (no files there)
    if matches!(current_route, AppRoute::Root) {
        return CommandResult::output(vec![OutputLine::error(format!(
            "cat: {}: No such file or directory",
            file
        ))]);
    }

    let current_mount = current_route.mount().cloned().unwrap_or_else(|| {
        configured_mounts()
            .into_iter()
            .next()
            .expect("At least one mount must be configured")
    });

    let resolved = fs.resolve_path(current_path, file.as_str());

    match resolved {
        Some(resolved_path) => {
            if fs.is_directory(&resolved_path) {
                CommandResult::output(vec![OutputLine::error(format!(
                    "cat: {}: Is a directory",
                    file
                ))])
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
                CommandResult::output(vec![OutputLine::error(format!(
                    "cat: {}: No content available",
                    file
                ))])
            }
        }
        None => CommandResult::output(vec![OutputLine::error(format!(
            "cat: {}: No such file or directory",
            file
        ))]),
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
fn execute_export(arg: Option<String>) -> CommandResult {
    match arg {
        None => {
            // No argument: show all variables
            let lines = env::format_export_output();
            let mut output = vec![OutputLine::empty()];
            for line in lines {
                output.push(OutputLine::text(line));
            }
            output.push(OutputLine::empty());
            CommandResult::output(output)
        }
        Some(assignment) => {
            // Parse KEY=value
            if let Some((key, value)) = assignment.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"').trim_matches('\'');

                match env::set_user_var(key, value) {
                    Ok(()) => CommandResult::empty(),
                    Err(e) => {
                        CommandResult::output(vec![OutputLine::error(format!("export: {}", e))])
                    }
                }
            } else {
                // Just a key without value - show current value
                let key = assignment.trim();
                if let Some(value) = env::get_user_var(key) {
                    CommandResult::output(vec![OutputLine::text(format!("{}={}", key, value))])
                } else {
                    CommandResult::empty()
                }
            }
        }
    }
}

/// Execute `unset` command.
fn execute_unset(key: String) -> CommandResult {
    if env::get_user_var(&key).is_some() {
        match env::unset_user_var(&key) {
            Ok(()) => CommandResult::empty(),
            Err(e) => CommandResult::output(vec![OutputLine::error(format!("unset: {}", e))]),
        }
    } else {
        CommandResult::empty() // Silently succeed if variable doesn't exist
    }
}

// =============================================================================
// Admin Commands
// =============================================================================

use crate::core::admin::is_admin;
use crate::core::storage::{ChangeType, local};
use crate::utils::current_timestamp;

/// Check admin permission and return error if not admin.
fn check_admin(wallet_state: &WalletState) -> Option<CommandResult> {
    if !is_admin(wallet_state) {
        Some(CommandResult::output(vec![OutputLine::error(
            "Permission denied: admin access required. Use 'login' to connect wallet.",
        )]))
    } else {
        None
    }
}

/// Execute `touch` command - create a new file.
fn execute_touch(
    path: super::PathArg,
    wallet_state: &WalletState,
    fs: &MergedFs,
    current_route: &AppRoute,
    pending: &PendingChanges,
) -> CommandResult {
    if let Some(err) = check_admin(wallet_state) {
        return err;
    }

    if matches!(current_route, AppRoute::Root) {
        return CommandResult::output(vec![OutputLine::error(
            "touch: cannot create file at root level",
        )]);
    }

    let current_path = current_route.fs_path();
    let resolved = MergedFs::resolve_path_string(current_path, path.as_str());

    // Check if file already exists
    if fs.get_entry(&resolved).is_some() {
        return CommandResult::output(vec![OutputLine::error(format!(
            "touch: cannot create '{}': File exists",
            path
        ))]);
    }

    // Get file name for description
    let filename = resolved.rsplit('/').next().unwrap_or(&resolved);

    // Clone pending and add change
    let mut new_pending = pending.clone();
    new_pending.add(
        resolved.clone(),
        ChangeType::CreateFile {
            content: String::new(),
            description: filename.to_string(),
            meta: crate::models::FileMetadata {
                size: Some(0),
                modified: Some(current_timestamp()),
                encryption: None,
            },
        },
    );

    CommandResult::with_pending(
        vec![OutputLine::success(format!(
            "Created file '{}' (pending commit)",
            resolved
        ))],
        new_pending,
    )
}

/// Execute `mkdir` command - create a new directory.
fn execute_mkdir(
    path: super::PathArg,
    wallet_state: &WalletState,
    fs: &MergedFs,
    current_route: &AppRoute,
    pending: &PendingChanges,
) -> CommandResult {
    if let Some(err) = check_admin(wallet_state) {
        return err;
    }

    if matches!(current_route, AppRoute::Root) {
        return CommandResult::output(vec![OutputLine::error(
            "mkdir: cannot create directory at root level",
        )]);
    }

    let current_path = current_route.fs_path();
    let resolved = MergedFs::resolve_path_string(current_path, path.as_str());

    // Check if already exists
    if fs.get_entry(&resolved).is_some() {
        return CommandResult::output(vec![OutputLine::error(format!(
            "mkdir: cannot create '{}': File exists",
            path
        ))]);
    }

    // Get dir name for title
    let dirname = resolved.rsplit('/').next().unwrap_or(&resolved);

    let mut new_pending = pending.clone();
    new_pending.add(
        resolved.clone(),
        ChangeType::CreateDirectory {
            meta: crate::models::DirectoryMetadata {
                title: dirname.to_string(),
                ..Default::default()
            },
        },
    );

    CommandResult::with_pending(
        vec![OutputLine::success(format!(
            "Created directory '{}' (pending commit)",
            resolved
        ))],
        new_pending,
    )
}

/// Execute `rm` command - remove a file.
fn execute_rm(
    path: super::PathArg,
    wallet_state: &WalletState,
    fs: &MergedFs,
    current_route: &AppRoute,
    pending: &PendingChanges,
) -> CommandResult {
    if let Some(err) = check_admin(wallet_state) {
        return err;
    }

    if matches!(current_route, AppRoute::Root) {
        return CommandResult::output(vec![OutputLine::error("rm: cannot remove at root level")]);
    }

    let current_path = current_route.fs_path();
    let resolved = match fs.resolve_path(current_path, path.as_str()) {
        Some(p) => p,
        None => {
            return CommandResult::output(vec![OutputLine::error(format!(
                "rm: cannot remove '{}': No such file or directory",
                path
            ))]);
        }
    };

    // Check if it's a directory
    if fs.is_directory(&resolved) {
        return CommandResult::output(vec![OutputLine::error(format!(
            "rm: cannot remove '{}': Is a directory (use rmdir)",
            path
        ))]);
    }

    let mut new_pending = pending.clone();
    new_pending.add(resolved.clone(), ChangeType::DeleteFile);

    CommandResult::with_pending(
        vec![OutputLine::success(format!(
            "Removed file '{}' (pending commit)",
            resolved
        ))],
        new_pending,
    )
}

/// Execute `rmdir` command - remove a directory.
fn execute_rmdir(
    path: super::PathArg,
    wallet_state: &WalletState,
    fs: &MergedFs,
    current_route: &AppRoute,
    pending: &PendingChanges,
) -> CommandResult {
    if let Some(err) = check_admin(wallet_state) {
        return err;
    }

    if matches!(current_route, AppRoute::Root) {
        return CommandResult::output(vec![OutputLine::error(
            "rmdir: cannot remove at root level",
        )]);
    }

    let current_path = current_route.fs_path();
    let resolved = match fs.resolve_path(current_path, path.as_str()) {
        Some(p) => p,
        None => {
            return CommandResult::output(vec![OutputLine::error(format!(
                "rmdir: cannot remove '{}': No such file or directory",
                path
            ))]);
        }
    };

    // Check if it's actually a directory
    if !fs.is_directory(&resolved) {
        return CommandResult::output(vec![OutputLine::error(format!(
            "rmdir: cannot remove '{}': Not a directory",
            path
        ))]);
    }

    // Check if directory is empty
    if let Some(entries) = fs.list_dir(&resolved)
        && !entries.is_empty()
    {
        return CommandResult::output(vec![OutputLine::error(format!(
            "rmdir: failed to remove '{}': Directory not empty",
            path
        ))]);
    }

    let mut new_pending = pending.clone();
    new_pending.add(resolved.clone(), ChangeType::DeleteDirectory);

    CommandResult::with_pending(
        vec![OutputLine::success(format!(
            "Removed directory '{}' (pending commit)",
            resolved
        ))],
        new_pending,
    )
}

// =============================================================================
// Sync Commands
// =============================================================================

/// Execute `sync status` - show pending and staged changes.
fn execute_sync_status(
    wallet_state: &WalletState,
    pending: &PendingChanges,
    staged: &StagedChanges,
) -> CommandResult {
    if let Some(err) = check_admin(wallet_state) {
        return err;
    }

    if pending.is_empty() {
        return CommandResult::output(vec![OutputLine::info(
            "No pending changes. Working directory clean.",
        )]);
    }

    let summary = pending.summary();
    let mut lines = vec![OutputLine::empty()];

    // Show staged changes first
    let staged_count = staged.len();
    if staged_count > 0 {
        lines.push(OutputLine::text(format!(
            "Changes staged for commit ({}):",
            staged_count
        )));
        lines.push(OutputLine::empty());

        for change in pending.iter() {
            if staged.is_staged(&change.path) {
                let status = match &change.change_type {
                    ChangeType::CreateFile { .. } => "new file:   ",
                    ChangeType::CreateBinaryFile { .. } => "new file:   ",
                    ChangeType::CreateDirectory { .. } => "new dir:    ",
                    ChangeType::UpdateFile { .. } => "modified:   ",
                    ChangeType::DeleteFile => "deleted:    ",
                    ChangeType::DeleteDirectory => "deleted:    ",
                };
                lines.push(OutputLine::success(format!(
                    "        {}{}",
                    status, change.path
                )));
            }
        }
        lines.push(OutputLine::empty());
    }

    // Show unstaged changes
    let unstaged: Vec<_> = pending
        .iter()
        .filter(|c| !staged.is_staged(&c.path))
        .collect();

    if !unstaged.is_empty() {
        lines.push(OutputLine::text(format!(
            "Changes not staged for commit ({}):",
            unstaged.len()
        )));
        lines.push(OutputLine::empty());

        for change in unstaged {
            let status = match &change.change_type {
                ChangeType::CreateFile { .. } => "new file:   ",
                ChangeType::CreateBinaryFile { .. } => "new file:   ",
                ChangeType::CreateDirectory { .. } => "new dir:    ",
                ChangeType::UpdateFile { .. } => "modified:   ",
                ChangeType::DeleteFile => "deleted:    ",
                ChangeType::DeleteDirectory => "deleted:    ",
            };
            lines.push(OutputLine::text(format!(
                "        {}{}",
                status, change.path
            )));
        }
        lines.push(OutputLine::empty());
    }

    lines.push(OutputLine::info(format!(
        "{} additions, {} modifications, {} deletions",
        summary.creates, summary.updates, summary.deletes
    )));
    lines.push(OutputLine::empty());
    lines.push(OutputLine::text(
        "Use 'sync add <path>' to stage, 'sync commit' to commit staged changes.",
    ));

    CommandResult::output(lines)
}

/// Execute `sync add` - stage changes for commit.
fn execute_sync_add(
    path: Option<super::PathArg>,
    wallet_state: &WalletState,
    fs: &MergedFs,
    current_route: &AppRoute,
    pending: &PendingChanges,
    staged: &StagedChanges,
) -> CommandResult {
    if let Some(err) = check_admin(wallet_state) {
        return err;
    }

    if pending.is_empty() {
        return CommandResult::output(vec![OutputLine::info("No pending changes to stage.")]);
    }

    match path {
        Some(p) => {
            let target = p.as_str();

            // Handle "." to stage all
            if target == "." {
                let mut new_staged = staged.clone();
                new_staged.add_all(pending.paths().map(|s| s.to_string()));
                let count = pending.len();
                return CommandResult::with_staged(
                    vec![OutputLine::success(format!("Staged all {} changes", count))],
                    new_staged,
                );
            }

            // Resolve path
            let current_path = current_route.fs_path();
            let resolved = MergedFs::resolve_path_string(current_path, target);

            if pending.has_change(&resolved) {
                let mut new_staged = staged.clone();
                new_staged.add(resolved.clone());
                CommandResult::with_staged(
                    vec![OutputLine::success(format!("Staged '{}'", resolved))],
                    new_staged,
                )
            } else {
                CommandResult::output(vec![OutputLine::error(format!(
                    "sync add: no pending changes for '{}'",
                    target
                ))])
            }
        }
        None => {
            // No path - show usage
            CommandResult::output(vec![
                OutputLine::text("Usage: sync add <path>"),
                OutputLine::text("       sync add .        Stage all pending changes"),
            ])
        }
    }
}

/// Execute `sync reset` - unstage changes.
fn execute_sync_reset(
    path: Option<super::PathArg>,
    wallet_state: &WalletState,
    staged: &StagedChanges,
) -> CommandResult {
    if let Some(err) = check_admin(wallet_state) {
        return err;
    }

    if staged.is_empty() {
        return CommandResult::output(vec![OutputLine::info("No staged changes to reset.")]);
    }

    match path {
        Some(p) => {
            let target = p.as_str();
            if staged.is_staged(target) {
                let mut new_staged = staged.clone();
                new_staged.remove(target);
                CommandResult::with_staged(
                    vec![OutputLine::success(format!("Unstaged '{}'", target))],
                    new_staged,
                )
            } else {
                CommandResult::output(vec![OutputLine::error(format!(
                    "sync reset: '{}' is not staged",
                    target
                ))])
            }
        }
        None => {
            // Reset all
            let count = staged.len();
            CommandResult::with_staged(
                vec![OutputLine::success(format!(
                    "Unstaged all {} changes",
                    count
                ))],
                StagedChanges::default(),
            )
        }
    }
}

/// Execute `sync commit` - commit staged changes.
fn execute_sync_commit(
    msg: Option<String>,
    wallet_state: &WalletState,
    pending: &PendingChanges,
    staged: &StagedChanges,
) -> CommandResult {
    if let Some(err) = check_admin(wallet_state) {
        return err;
    }

    if staged.is_empty() {
        return CommandResult::output(vec![
            OutputLine::info("Nothing staged to commit."),
            OutputLine::text("Use 'sync add <path>' to stage changes first."),
        ]);
    }

    // Check if GitHub token is set
    if !local::has_github_token() {
        return CommandResult::output(vec![
            OutputLine::error("No GitHub token configured."),
            OutputLine::text("Use 'sync auth github <token>' to set your Personal Access Token."),
        ]);
    }

    let message = msg.unwrap_or_else(|| "Update via websh".to_string());
    let staged_count = staged.len();

    // Note: Actual GitHub commit is async and needs to be handled separately.
    // For now, we just show the info. The actual commit logic would be triggered
    // from a component that can handle async operations.
    CommandResult::output(vec![
        OutputLine::info(format!(
            "Ready to commit {} staged changes: \"{}\"",
            staged_count, message
        )),
        OutputLine::text("Commit will be processed asynchronously..."),
        OutputLine::text("(Note: Full async commit implementation pending)"),
    ])
}

/// Execute `sync discard` - discard pending changes.
fn execute_sync_discard(
    path: Option<super::PathArg>,
    wallet_state: &WalletState,
    pending: &PendingChanges,
) -> CommandResult {
    if let Some(err) = check_admin(wallet_state) {
        return err;
    }

    if pending.is_empty() {
        return CommandResult::output(vec![OutputLine::info("No pending changes to discard.")]);
    }

    match path {
        Some(p) => {
            let target = p.as_str();
            if pending.has_change(target) {
                let mut new_pending = pending.clone();
                new_pending.remove(target);
                CommandResult::with_pending(
                    vec![OutputLine::success(format!(
                        "Discarded changes to '{}'",
                        target
                    ))],
                    new_pending,
                )
            } else {
                CommandResult::output(vec![OutputLine::error(format!(
                    "sync discard: no pending changes for '{}'",
                    target
                ))])
            }
        }
        None => {
            // Discard all
            let count = pending.len();
            CommandResult::with_pending(
                vec![OutputLine::success(format!(
                    "Discarded all {} pending changes",
                    count
                ))],
                PendingChanges::default(),
            )
        }
    }
}

/// Execute `sync auth` - set authentication tokens.
fn execute_sync_auth(provider: String, token: Option<String>) -> CommandResult {
    match provider.to_lowercase().as_str() {
        "github" => match token {
            Some(t) => {
                if let Err(e) = local::store_github_token(&t) {
                    return CommandResult::output(vec![OutputLine::error(format!(
                        "sync auth: failed to store token: {}",
                        e
                    ))]);
                }
                CommandResult::output(vec![OutputLine::success(
                    "GitHub token stored successfully.",
                )])
            }
            None => {
                // Show status
                if local::has_github_token() {
                    CommandResult::output(vec![OutputLine::info("GitHub token is configured.")])
                } else {
                    CommandResult::output(vec![
                        OutputLine::info("GitHub token is not configured."),
                        OutputLine::text("Usage: sync auth github <personal-access-token>"),
                    ])
                }
            }
        },
        "" => CommandResult::output(vec![
            OutputLine::text("Usage: sync auth <provider> [token]"),
            OutputLine::text(""),
            OutputLine::text("Providers:"),
            OutputLine::text("  github    Set GitHub Personal Access Token for commits"),
        ]),
        _ => CommandResult::output(vec![OutputLine::error(format!(
            "sync auth: unknown provider '{}'. Supported: github",
            provider
        ))]),
    }
}
