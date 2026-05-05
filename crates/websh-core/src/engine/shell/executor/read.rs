use crate::domain::{DirEntry, RuntimeMount, VirtualPath, WalletState};
use crate::engine::filesystem::{
    GlobalFs, RouteRequest, RouteSurface, request_path_for_canonical_path,
};
use crate::engine::shell::{AccessPolicy, CommandResult, OutputLine, PathArg};

use super::{can_write_path, resolve_path_arg};

/// Execute `ls` command.
pub(super) fn execute_ls(
    path: Option<PathArg>,
    long: bool,
    wallet_state: &WalletState,
    access_policy: &AccessPolicy,
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
            access_policy,
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

fn format_ls_output(
    entries: &[DirEntry],
    long: bool,
    wallet_state: &WalletState,
    access_policy: &AccessPolicy,
    runtime_mounts: &[RuntimeMount],
    fs: &GlobalFs,
) -> Vec<OutputLine> {
    if long {
        entries
            .iter()
            .map(|entry| {
                let fs_entry = fs.get_entry(&entry.path);
                let writable =
                    can_write_path(wallet_state, access_policy, runtime_mounts, &entry.path);
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
pub(super) fn execute_cd(path: PathArg, fs: &GlobalFs, cwd: &VirtualPath) -> CommandResult {
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
pub(super) fn execute_cat(file: PathArg, fs: &GlobalFs, cwd: &VirtualPath) -> CommandResult {
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
