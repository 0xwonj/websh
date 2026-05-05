use crate::domain::WalletState;
use crate::engine::shell::{CommandResult, ExecutionContext, OutputLine, SideEffect};

pub(super) fn execute_whoami(context: &ExecutionContext) -> CommandResult {
    CommandResult::output(vec![OutputLine::ascii(
        context.shell_text.profile.to_string(),
    )])
}

/// Execute `id` command.
pub(super) fn execute_id(wallet_state: &WalletState, context: &ExecutionContext) -> CommandResult {
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
            crate::domain::chain_name(chain_id)
        )));
        lines.push(OutputLine::text(format!("chain_id={}", chain_id)));
    } else {
        lines.push(OutputLine::text("network=none"));
    }

    if let Some(uptime) = &context.system_info.uptime {
        lines.push(OutputLine::text(format!("uptime={}", uptime)));
    }

    if let Some(user_agent) = &context.system_info.user_agent {
        lines.push(OutputLine::text(format!("user_agent={}", user_agent)));
    }

    lines.push(OutputLine::empty());
    CommandResult::output(lines)
}

pub(super) fn execute_theme(requested: Option<String>) -> CommandResult {
    match requested {
        Some(theme) => CommandResult::empty().with_side_effect(SideEffect::SetTheme { theme }),
        None => CommandResult::empty().with_side_effect(SideEffect::ListThemes),
    }
}
