use crate::domain::{ChangeEntry, ChangeSet, ChangeType, RuntimeMount, VirtualPath, WalletState};
use crate::engine::shell::{
    AccessPolicy, AuthAction, CommandResult, OutputLine, SideEffect, SyncSubcommand,
};

use super::{mount_for_path, require_write_access};

/// Execute `sync <sub>` — status / commit / refresh / auth.
pub(super) fn execute_sync(
    sub: SyncSubcommand,
    wallet_state: &WalletState,
    access_policy: &AccessPolicy,
    runtime_mounts: &[RuntimeMount],
    cwd: &VirtualPath,
    changes: &ChangeSet,
    remote_head: Option<&str>,
) -> CommandResult {
    match sub {
        SyncSubcommand::Status => execute_sync_status(changes, remote_head),
        SyncSubcommand::Commit { message } => execute_sync_commit(
            message,
            wallet_state,
            access_policy,
            runtime_mounts,
            cwd,
            changes,
        ),
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

    let mut staged: Vec<(&VirtualPath, &ChangeEntry)> = changes.iter_staged().collect();
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

    let mut unstaged: Vec<(&VirtualPath, &ChangeEntry)> = changes.iter_unstaged().collect();
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

fn change_tag(change: &ChangeType) -> &'static str {
    match change {
        ChangeType::CreateFile { .. }
        | ChangeType::CreateBinary { .. }
        | ChangeType::CreateDirectory { .. } => "A",
        ChangeType::UpdateFile { .. } => "M",
        ChangeType::DeleteFile | ChangeType::DeleteDirectory => "D",
    }
}

fn execute_sync_commit(
    message: String,
    wallet_state: &WalletState,
    access_policy: &AccessPolicy,
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

    if let Err(e) = require_write_access(
        "sync commit",
        wallet_state,
        access_policy,
        runtime_mounts,
        &mount_root,
    ) {
        return e;
    }

    CommandResult {
        output: vec![],
        exit_code: 0,
        side_effects: vec![SideEffect::Commit {
            message,
            mount_root,
        }],
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
        side_effects: vec![SideEffect::ReloadRuntimeMount { mount_root }],
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
                side_effects: vec![SideEffect::SetAuthToken { token }],
            }
        }
        AuthAction::Clear => CommandResult {
            output: vec![],
            exit_code: 0,
            side_effects: vec![SideEffect::ClearAuthToken],
        },
    }
}

#[allow(clippy::result_large_err)]
pub(super) fn sync_mount_root(
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
