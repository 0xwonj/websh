use super::super::SideEffect;
use super::*;
use crate::domain::{EntryExtensions, NodeKind, WalletState};
use crate::engine::filesystem::{GlobalFs, RouteRequest};
use crate::engine::shell::PathArg;

use crate::domain::BootstrapSiteSource;

fn empty_state() -> (WalletState, GlobalFs) {
    (WalletState::Disconnected, GlobalFs::empty())
}

fn bootstrap_source() -> BootstrapSiteSource {
    BootstrapSiteSource {
        repo_with_owner: "example/site",
        branch: "main",
        content_root: "content",
        gateway: "self",
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
) -> CommandResult {
    let runtime_mounts = [crate::engine::runtime::boot::bootstrap_runtime_mount(
        &bootstrap_source(),
    )];
    super::execute_command_with_context(
        cmd,
        wallet_state,
        &runtime_mounts,
        fs,
        cwd,
        &ExecutionContext::default(),
    )
}

#[test]
fn test_login_returns_login_side_effect() {
    let (ws, fs) = empty_state();
    let result = execute_command(Command::Login, &ws, &fs, &root_cwd());
    assert_eq!(
        result.side_effects.first().cloned(),
        Some(SideEffect::Login)
    );
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_logout_returns_logout_side_effect() {
    let (ws, fs) = empty_state();
    let result = execute_command(Command::Logout, &ws, &fs, &root_cwd());
    assert_eq!(
        result.side_effects.first().cloned(),
        Some(SideEffect::Logout)
    );
}

#[test]
fn test_theme_lists_available_palettes() {
    let (ws, fs) = empty_state();
    let result = execute_command(Command::Theme(None), &ws, &fs, &root_cwd());
    assert!(result.output.is_empty());
    assert_eq!(
        result.side_effects.first().cloned(),
        Some(SideEffect::ListThemes)
    );
}

#[test]
fn test_theme_sets_known_palette() {
    let (ws, fs) = empty_state();
    let result = execute_command(
        Command::Theme(Some("black-ink".to_string())),
        &ws,
        &fs,
        &root_cwd(),
    );
    assert_eq!(
        result.side_effects.first().cloned(),
        Some(SideEffect::SetTheme {
            theme: "black-ink".to_string()
        })
    );
}

#[test]
fn test_cd_navigates_shell_surface() {
    let mut fs = GlobalFs::empty();
    fs.upsert_directory(VirtualPath::from_absolute("/db").unwrap(), blank_dir_meta());
    let ws = WalletState::Disconnected;

    let result = execute_command(Command::Cd(PathArg::new("/db")), &ws, &fs, &root_cwd());

    assert_eq!(
        result.side_effects.first().cloned(),
        Some(SideEffect::Navigate(RouteRequest::new("/websh/db")))
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

    let result = execute_command(
        Command::Cat(Some(PathArg::new("/blog/hello.md"))),
        &ws,
        &fs,
        &root_cwd(),
    );

    assert_eq!(
        result.side_effects.first().cloned(),
        Some(SideEffect::Navigate(RouteRequest::new("/blog/hello")))
    );
}

#[test]
fn test_unknown_command_exit_127() {
    let (ws, fs) = empty_state();
    let result = execute_command(Command::Unknown("foobar".into()), &ws, &fs, &root_cwd());
    assert_eq!(result.exit_code, 127);
}

#[test]
fn test_ls_nonexistent_exit_1() {
    let (ws, fs) = empty_state();
    let result = execute_command(
        Command::Ls {
            path: Some(super::super::PathArg::new("nonexistent")),
            long: false,
        },
        &ws,
        &fs,
        &root_cwd(),
    );
    assert_eq!(result.exit_code, 1);
    assert!(!result.output.is_empty());
}

#[test]
fn test_cat_missing_operand_exit_1() {
    let (ws, fs) = empty_state();
    let result = execute_command(Command::Cat(None), &ws, &fs, &root_cwd());
    assert_eq!(result.exit_code, 1);
    assert!(
            result
                .output
                .iter()
                .any(|l| matches!(&l.data, crate::engine::shell::OutputLineData::Error(s) if s == "cat: missing file operand"))
        );
}

#[test]
fn test_unset_missing_operand_exit_1() {
    let (ws, fs) = empty_state();
    let result = execute_command(Command::Unset(None), &ws, &fs, &root_cwd());
    assert_eq!(result.exit_code, 1);
}

#[test]
fn test_execute_export_multi_processes_each_assignment() {
    let (ws, fs) = empty_state();
    let result = execute_command(
        Command::Export(vec![
            "FOO_P2_A=alpha".to_string(),
            "BAR_P2_A=beta".to_string(),
        ]),
        &ws,
        &fs,
        &root_cwd(),
    );
    assert_eq!(result.exit_code, 0);
    assert!(result.output.is_empty());
    assert_eq!(
        result.side_effects,
        vec![
            SideEffect::SetEnvVar {
                key: "FOO_P2_A".to_string(),
                value: "alpha".to_string(),
            },
            SideEffect::SetEnvVar {
                key: "BAR_P2_A".to_string(),
                value: "beta".to_string(),
            },
        ]
    );
}

#[test]
fn test_cd_empty_string_exit_1() {
    // POSIX bash: `cd ""` errors with "cd: : No such file or directory".
    // Must exercise a non-Root route so the early `at_root` branch doesn't
    // short-circuit to the generic mount-alias error.
    let (ws, fs) = empty_state();
    let browse_route = home_cwd("");
    let result = execute_command(
        Command::Cd(super::super::PathArg::new("")),
        &ws,
        &fs,
        &browse_route,
    );
    assert_eq!(result.exit_code, 1);
    assert!(result.side_effects.first().cloned().is_none());
    assert!(
        result.output.iter().any(|l| matches!(
            &l.data,
            crate::engine::shell::OutputLineData::Error(s) if s == "cd: : No such file or directory"
        )),
        "expected POSIX cd error; got: {:?}",
        result.output
    );
}

fn blank_file_meta(kind: NodeKind) -> crate::domain::NodeMetadata {
    crate::domain::NodeMetadata {
        kind,
        ..Default::default()
    }
}

fn blank_dir_meta() -> crate::domain::NodeMetadata {
    blank_file_meta(NodeKind::Directory)
}

#[test]
fn refresh_selects_owner_without_requiring_a_listed_path() {
    let mounts = [
        crate::domain::RuntimeMount::new(VirtualPath::root(), "root"),
        crate::domain::RuntimeMount::new(home_vpath("mempool"), "external"),
    ];
    let fs = GlobalFs::empty();
    for raw in ["/mempool", "/mempool/not-loaded.md"] {
        let result = super::execute_command(
            Command::Refresh(Some(PathArg::new(raw))),
            &WalletState::Disconnected,
            &mounts,
            &fs,
            &root_cwd(),
        );
        assert_eq!(
            result.side_effects,
            vec![SideEffect::ReloadRuntimeMount {
                mount_root: home_vpath("mempool")
            }]
        );
    }
    let result = super::execute_command(
        Command::Refresh(None),
        &WalletState::Disconnected,
        &mounts,
        &fs,
        &home_vpath(".websh/state"),
    );
    assert_ne!(result.exit_code, 0);
    assert!(result.side_effects.is_empty());
}
