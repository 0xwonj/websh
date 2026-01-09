//! Command execution logic.
//!
//! Contains the `execute_command` function that runs parsed commands
//! against the virtual filesystem and returns results.

use crate::app::TerminalState;
use crate::config::{ASCII_PROFILE, HELP_TEXT, PROFILE_FILE, configured_mounts};
use crate::core::{VirtualFs, env, wallet};
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
/// * `fs` - Virtual filesystem
/// * `current_route` - Current route (for resolving relative paths)
pub fn execute_command(
    cmd: Command,
    state: &TerminalState,
    wallet_state: &WalletState,
    fs: &VirtualFs,
    current_route: &AppRoute,
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
fn execute_cd(path: super::PathArg, fs: &VirtualFs, current_route: &AppRoute) -> CommandResult {
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
    fs: &VirtualFs,
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
