use super::super::SideEffect;
use super::*;
use crate::domain::{ChangeSet, ChangeType, EntryExtensions, NodeKind, WalletState};
use crate::engine::filesystem::{GlobalFs, RouteRequest};
use crate::engine::shell::{AuthAction, OutputLineData, PathArg, SyncSubcommand};

use super::sync::sync_mount_root;
use super::write::{blank_dir_meta, blank_file_meta};
use crate::domain::BootstrapSiteSource;
use crate::engine::shell::access::test_support::{ACCESS_POLICY, ADMIN_ADDRESS};

fn empty_state() -> (WalletState, GlobalFs) {
    (WalletState::Disconnected, GlobalFs::empty())
}

fn admin_wallet() -> WalletState {
    WalletState::Connected {
        address: ADMIN_ADDRESS.to_string(),
        ens_name: None,
        chain_id: Some(1),
    }
}

fn bootstrap_source() -> BootstrapSiteSource {
    BootstrapSiteSource {
        repo_with_owner: "example/site",
        branch: "main",
        content_root: "content",
        gateway: "self",
        writable: true,
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

fn upsert(changes: &mut ChangeSet, path: VirtualPath, change: ChangeType) {
    changes.upsert_at(path, change, 1234);
}

fn execute_command(
    cmd: Command,
    wallet_state: &WalletState,
    fs: &GlobalFs,
    cwd: &VirtualPath,
    changes: &ChangeSet,
    remote_head: Option<&str>,
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
        changes,
        remote_head,
        &ExecutionContext {
            access_policy: ACCESS_POLICY,
            ..ExecutionContext::default()
        },
    )
}

#[test]
fn test_login_returns_login_side_effect() {
    let (ws, fs) = empty_state();
    let cs = ChangeSet::new();
    let result = execute_command(Command::Login, &ws, &fs, &root_cwd(), &cs, None);
    assert_eq!(
        result.side_effects.first().cloned(),
        Some(SideEffect::Login)
    );
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_logout_returns_logout_side_effect() {
    let (ws, fs) = empty_state();
    let cs = ChangeSet::new();
    let result = execute_command(Command::Logout, &ws, &fs, &root_cwd(), &cs, None);
    assert_eq!(
        result.side_effects.first().cloned(),
        Some(SideEffect::Logout)
    );
}

#[test]
fn test_theme_lists_available_palettes() {
    let (ws, fs) = empty_state();
    let cs = ChangeSet::new();
    let result = execute_command(Command::Theme(None), &ws, &fs, &root_cwd(), &cs, None);
    assert!(result.output.is_empty());
    assert_eq!(
        result.side_effects.first().cloned(),
        Some(SideEffect::ListThemes)
    );
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
        result.side_effects.first().cloned(),
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
                .any(|l| matches!(&l.data, crate::engine::shell::OutputLineData::Error(s) if s == "cat: missing file operand"))
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
    assert!(result.side_effects.first().cloned().is_none());
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
    assert!(result.side_effects.first().cloned().is_none());
    assert!(result.output.iter().any(|line| {
        matches!(
            &line.data,
            crate::engine::shell::OutputLineData::Error(message)
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
    match result.side_effects.first().cloned() {
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
fn test_touch_errors_when_parent_is_file() {
    let mut fs = GlobalFs::empty();
    fs.upsert_file(
        home_vpath("file"),
        String::new(),
        blank_file_meta(NodeKind::Asset),
        EntryExtensions::default(),
    );
    let ws = admin_wallet();
    let cs = ChangeSet::new();
    let result = execute_command(
        Command::Touch {
            path: PathArg::new("file/child.md"),
        },
        &ws,
        &fs,
        &home_cwd(""),
        &cs,
        None,
    );
    assert_eq!(result.exit_code, 1);
    assert!(result.output.iter().any(|line| {
        matches!(
            &line.data,
            crate::engine::shell::OutputLineData::Error(message)
                if message.contains("parent is not a directory")
        )
    }));
}

#[test]
fn test_touch_errors_when_parent_is_missing() {
    let (_ws, fs) = empty_state();
    let ws = admin_wallet();
    let cs = ChangeSet::new();
    let result = execute_command(
        Command::Touch {
            path: PathArg::new("missing/child.md"),
        },
        &ws,
        &fs,
        &home_cwd(""),
        &cs,
        None,
    );
    assert_eq!(result.exit_code, 1);
    assert!(result.output.iter().any(|line| {
        matches!(
            &line.data,
            crate::engine::shell::OutputLineData::Error(message)
                if message.contains("parent directory does not exist")
        )
    }));
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
    match result.side_effects.first().cloned() {
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
fn test_mkdir_errors_when_parent_is_file() {
    let mut fs = GlobalFs::empty();
    fs.upsert_file(
        home_vpath("file"),
        String::new(),
        blank_file_meta(NodeKind::Asset),
        EntryExtensions::default(),
    );
    let ws = admin_wallet();
    let cs = ChangeSet::new();
    let result = execute_command(
        Command::Mkdir {
            path: PathArg::new("file/child"),
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
    match result.side_effects.first().cloned() {
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
    match result.side_effects.first().cloned() {
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
fn test_rm_recursive_rejects_mount_root() {
    let db_root = VirtualPath::from_absolute("/db").unwrap();
    let mut fs = GlobalFs::empty();
    fs.upsert_directory(db_root.clone(), blank_dir_meta());
    let runtime_mounts = vec![
        crate::engine::runtime::boot::bootstrap_runtime_mount(&bootstrap_source()),
        crate::domain::RuntimeMount::new(
            db_root,
            "db",
            crate::domain::RuntimeBackendKind::GitHub,
            true,
        ),
    ];
    let ws = admin_wallet();
    let cs = ChangeSet::new();

    let result = super::execute_command_with_context(
        Command::Rm {
            path: PathArg::new("/db"),
            recursive: true,
        },
        &ws,
        &runtime_mounts,
        &fs,
        &root_cwd(),
        &cs,
        None,
        &ExecutionContext {
            access_policy: ACCESS_POLICY,
            ..ExecutionContext::default()
        },
    );

    assert_eq!(result.exit_code, 1);
    assert!(matches!(
        result.output.first().map(|line| &line.data),
        Some(OutputLineData::Error(message)) if message.contains("cannot remove mount root")
    ));
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
    match result.side_effects.first().cloned() {
        Some(SideEffect::ApplyChange { ref change, .. }) => {
            assert!(matches!(change.as_ref(), ChangeType::DeleteDirectory));
        }
        other => panic!("expected DeleteDirectory, got {:?}", other),
    }
}

#[test]
fn test_rmdir_rejects_mount_root() {
    let db_root = VirtualPath::from_absolute("/db").unwrap();
    let mut fs = GlobalFs::empty();
    fs.upsert_directory(db_root.clone(), blank_dir_meta());
    let runtime_mounts = vec![
        crate::engine::runtime::boot::bootstrap_runtime_mount(&bootstrap_source()),
        crate::domain::RuntimeMount::new(
            db_root,
            "db",
            crate::domain::RuntimeBackendKind::GitHub,
            true,
        ),
    ];
    let ws = admin_wallet();
    let cs = ChangeSet::new();

    let result = super::execute_command_with_context(
        Command::Rmdir {
            path: PathArg::new("/db"),
        },
        &ws,
        &runtime_mounts,
        &fs,
        &root_cwd(),
        &cs,
        None,
        &ExecutionContext {
            access_policy: ACCESS_POLICY,
            ..ExecutionContext::default()
        },
    );

    assert_eq!(result.exit_code, 1);
    assert!(matches!(
        result.output.first().map(|line| &line.data),
        Some(OutputLineData::Error(message)) if message.contains("cannot remove mount root")
    ));
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
    match result.side_effects.first().cloned() {
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
        result.side_effects.first().cloned(),
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
    match result.side_effects.first().cloned() {
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
    match result.side_effects.first().cloned() {
        Some(SideEffect::ApplyChange { ref change, .. }) => match change.as_ref() {
            ChangeType::UpdateFile { content, .. } => assert_eq!(content, "new"),
            other => panic!("expected UpdateFile, got {:?}", other),
        },
        other => panic!("expected UpdateFile, got {:?}", other),
    }
}

#[test]
fn test_echo_redirect_errors_when_parent_is_file() {
    let mut fs = GlobalFs::empty();
    fs.upsert_file(
        home_vpath("file"),
        String::new(),
        blank_file_meta(NodeKind::Asset),
        EntryExtensions::default(),
    );
    let ws = admin_wallet();
    let cs = ChangeSet::new();
    let result = execute_command(
        Command::EchoRedirect {
            body: "hello".to_string(),
            path: PathArg::new("file/child.md"),
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
            crate::engine::shell::OutputLineData::Text(s) => Some(s.clone()),
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
            |l| matches!(&l.data, crate::engine::shell::OutputLineData::Text(s) if s.contains("abcdef12")),
        );
    assert!(has_head, "expected remote HEAD prefix in output");
}

#[test]
fn test_sync_status_reports_entries() {
    let (_ws, fs) = empty_state();
    let ws = admin_wallet();
    let mut cs = ChangeSet::new();
    upsert(
        &mut cs,
        home_vpath("new.md"),
        ChangeType::CreateFile {
            content: "x".to_string(),
            meta: blank_file_meta(NodeKind::Asset),
            extensions: EntryExtensions::default(),
        },
    );
    upsert(&mut cs, home_vpath("del.md"), ChangeType::DeleteFile);
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
            crate::engine::shell::OutputLineData::Text(s) => Some(s.clone()),
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
    upsert(
        &mut cs,
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
    match result.side_effects.first().cloned() {
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
        crate::engine::runtime::boot::bootstrap_runtime_mount(&bootstrap_source()),
        crate::domain::RuntimeMount::new(
            VirtualPath::from_absolute("/db").unwrap(),
            "db",
            crate::domain::RuntimeBackendKind::GitHub,
            true,
        ),
    ];
    let mut cs = ChangeSet::new();
    upsert(
        &mut cs,
        home_vpath("a.md"),
        ChangeType::CreateFile {
            content: "site".to_string(),
            meta: blank_file_meta(NodeKind::Asset),
            extensions: EntryExtensions::default(),
        },
    );
    upsert(
        &mut cs,
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
        result.side_effects.first().cloned(),
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
    match result.side_effects.first().cloned() {
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
    assert_eq!(
        result.side_effects.first().cloned(),
        Some(SideEffect::ClearAuthToken)
    );
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
    crate::engine::runtime::build_view_global_fs(
        base,
        changes,
        &WalletState::Disconnected,
        &crate::engine::runtime::RuntimeStateSnapshot::default(),
    )
}

#[test]
fn test_rm_on_pending_create_file_emits_discard_change() {
    // Base fs empty; ChangeSet has a pending CreateFile at /a.md.
    let base = GlobalFs::empty();
    let mut cs = ChangeSet::new();
    upsert(
        &mut cs,
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
    match result.side_effects.first().cloned() {
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
    upsert(
        &mut cs,
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
    match result.side_effects.first().cloned() {
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
    upsert(
        &mut cs,
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
    match result.side_effects.first().cloned() {
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
    match result.side_effects.first().cloned() {
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
    upsert(
        &mut cs,
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
    assert!(result.side_effects.first().cloned().is_none());
}
