//! Pipeline execution for parsed shell commands.

use crate::domain::{RuntimeMount, VirtualPath, WalletState};
use crate::engine::filesystem::FsView;
use crate::engine::shell::parser::Pipeline;

use super::{Command, CommandResult, ExecutionContext, apply_filter, execute_command_with_context};

/// Execute a pipeline of commands with pipe filtering.
///
/// A pipeline consists of a main command followed by optional filter commands
/// separated by `|`. For example: `ls | grep foo | head -5`
pub fn execute_pipeline(
    pipeline: &Pipeline,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: FsView<'_>,
    cwd: &VirtualPath,
) -> CommandResult {
    execute_pipeline_with_context(
        pipeline,
        wallet_state,
        runtime_mounts,
        fs,
        cwd,
        &ExecutionContext::default(),
    )
}

/// Execute a pipeline of commands with target-provided context.
pub fn execute_pipeline_with_context(
    pipeline: &Pipeline,
    wallet_state: &WalletState,
    runtime_mounts: &[RuntimeMount],
    fs: FsView<'_>,
    cwd: &VirtualPath,
    context: &ExecutionContext,
) -> CommandResult {
    if let Some(ref err) = pipeline.error {
        return CommandResult::error_line(err.to_string()).with_exit_code(2);
    }

    if pipeline.is_empty() {
        return CommandResult::empty();
    }

    // Execute first command.
    let first = &pipeline.commands[0];
    let cmd = Command::parse(&first.name, &first.args);
    let mut result =
        execute_command_with_context(cmd, wallet_state, runtime_mounts, fs, cwd, context);

    if pipeline.commands.len() == 1 {
        return result;
    }

    // Pipeline mode: side effects are discarded (cannot navigate or mutate mid-pipe).
    result.side_effects.clear();
    let mut current_lines = result.output;
    let mut current_exit = result.exit_code;

    for filter_cmd in pipeline.commands.iter().skip(1) {
        let stage = apply_filter(&filter_cmd.name, &filter_cmd.args, current_lines);
        current_lines = stage.output;
        current_exit = stage.exit_code;
    }

    CommandResult::output(current_lines).with_exit_code(current_exit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filesystem::GlobalFs;
    use crate::shell::{SideEffect, parse_input};

    fn execute(input: &str) -> CommandResult {
        execute_pipeline(
            &parse_input(input, &[]),
            &WalletState::Disconnected,
            &[],
            FsView::content(&GlobalFs::empty()),
            &VirtualPath::root(),
        )
    }

    #[test]
    fn only_unpiped_commands_apply_side_effects() {
        assert_eq!(execute("login").side_effects, vec![SideEffect::Login]);
        assert!(execute("login | head -1").side_effects.is_empty());
    }

    #[test]
    fn exit_status_reports_parse_failure_or_the_last_stage() {
        for (input, expected) in [("ls |", 2), ("help | grep xyzzy", 1), ("help | head -1", 0)] {
            assert_eq!(execute(input).exit_code, expected, "{input}");
        }
    }
}
