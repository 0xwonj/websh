//! Root application module.
//!
//! Contains the main App component, AppContext definition, TerminalState,
//! and application-level setup logic following Leptos conventions.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::components::RouterView;
use crate::config::{APP_NAME, MAX_COMMAND_HISTORY, MAX_TERMINAL_HISTORY};
use crate::core::changes::ChangeSet;
use crate::core::engine::{GlobalFs, display_path_for};
use crate::core::merge;
use crate::core::storage::StorageBackend;
use crate::core::storage::persist::DraftPersister;
use crate::models::{
    ExplorerViewType, OutputLine, RuntimeMount, Selection, ViewMode, VirtualPath, WalletState,
};
use crate::utils::RingBuffer;

stylance::import_crate_style!(err_css, "src/components/error_boundary.module.css");

// ============================================================================
// Convention: `#[derive(Clone, Copy)]` on signal containers
// ============================================================================
//
// The state container structs in this module — `AppContext`, `TerminalState`,
// and `ExplorerState` — all derive `Clone` and `Copy`. This is intentional:
// every field is a Leptos reactive handle (`RwSignal`, `Memo`, `StoredValue`),
// which itself is a cheap `Copy` pointer into Leptos' reactive arena. Copying
// one of these containers therefore duplicates a handful of pointers, not the
// underlying state — near free.
//
// The payoff: closures (effects, event handlers, callbacks) can capture these
// containers by move without an explicit `.clone()` at every call site, and
// any number of closures can each own their own copy while still observing
// and mutating the same reactive state.
//
// Rule of thumb: only derive `Copy` on a container here if every field is
// itself `Copy` (i.e. another signal-like handle). The moment a non-signal
// field (e.g. an owned `String`, `Vec`, or `Rc`) is added, drop the `Copy`
// derive — otherwise you silently duplicate owned data per closure capture.

// ============================================================================
// TerminalState
// ============================================================================

/// Terminal state managed with Leptos signals.
///
/// Handles terminal-specific state including output history and command history.
/// Terminal output uses a [`RingBuffer`] for O(1) push operations, avoiding
/// the O(n) cost of `Vec::drain()` when limiting history size.
///
/// # Note
///
/// This struct is `Copy` because all fields are Leptos signals, which are
/// cheap to copy (they're just pointers to the underlying reactive state).
/// See the module-level convention note on signal containers.
///
/// The current path is now derived from the URL via `RouteContext`, not stored here.
#[derive(Clone, Copy)]
pub struct TerminalState {
    /// Terminal output history (bounded by `MAX_TERMINAL_HISTORY`).
    pub history: RwSignal<RingBuffer<OutputLine>>,
    /// Command history for up/down navigation.
    pub command_history: RwSignal<Vec<String>>,
    /// Current position in command history (for navigation).
    pub history_index: RwSignal<Option<usize>>,
}

impl TerminalState {
    /// Creates a new terminal state with default values.
    ///
    /// Initializes:
    /// - Empty output history (capacity: `MAX_TERMINAL_HISTORY`)
    /// - Empty command history
    pub fn new() -> Self {
        Self {
            history: RwSignal::new(RingBuffer::new(MAX_TERMINAL_HISTORY)),
            command_history: RwSignal::new(Vec::new()),
            history_index: RwSignal::new(None),
        }
    }

    /// Appends a single output line to the terminal history.
    ///
    /// This is an O(1) operation thanks to the ring buffer implementation.
    /// When history exceeds capacity, the oldest entries are automatically
    /// overwritten.
    pub fn push_output(&self, line: OutputLine) {
        self.history.update(|h| h.push(line));
    }

    /// Appends multiple output lines to the terminal history.
    ///
    /// Each line is pushed individually, maintaining O(1) per element.
    /// An empty line is automatically added after the output.
    pub fn push_lines(&self, lines: Vec<OutputLine>) {
        if lines.is_empty() {
            return;
        }
        self.history.update(|h| {
            h.extend(lines);
            h.push(OutputLine::empty());
        });
    }

    /// Clears all terminal output history.
    pub fn clear_history(&self) {
        self.history.update(|h| h.clear());
    }

    pub fn add_to_command_history(&self, cmd: &str) {
        if !cmd.trim().is_empty() {
            self.command_history.update(|h| {
                if h.last().map(|s| s.as_str()) != Some(cmd) {
                    h.push(cmd.to_string());
                    // Limit command history size
                    if h.len() > MAX_COMMAND_HISTORY {
                        h.remove(0);
                    }
                }
            });
        }
        self.history_index.set(None);
    }

    pub fn navigate_history(&self, direction: i32) -> Option<String> {
        let current_index = self.history_index.get();
        let (new_index, result) = self.command_history.with(|history| {
            if history.is_empty() {
                return (None, None);
            }
            let new_index = match current_index {
                None if direction < 0 => Some(history.len() - 1),
                Some(i) if direction < 0 && i > 0 => Some(i - 1),
                Some(i) if direction > 0 && i < history.len() - 1 => Some(i + 1),
                Some(_) if direction > 0 => None,
                _ => current_index,
            };
            let result = new_index.map(|i| history[i].clone());
            (new_index, result)
        });
        self.history_index.set(new_index);
        result
    }
}

impl Default for TerminalState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ExplorerState
// ============================================================================

/// Explorer state for the file browser UI.
///
/// # Note
///
/// This struct is `Copy` because all fields are Leptos signals. See the
/// module-level convention note on signal containers.
///
/// Back/forward navigation is delegated to the browser's own history
/// (`window.history().back()` / `.forward()`), so no in-app forward stack
/// is needed. The browser's history is the single source of truth, which
/// eliminates desync with the URL hash when users click the browser's
/// native back/forward buttons.
#[derive(Clone, Copy)]
pub struct ExplorerState {
    /// Currently selected item (file or directory).
    pub selection: RwSignal<Option<Selection>>,
    /// Current view type (list or grid).
    pub view_type: RwSignal<ExplorerViewType>,
}

impl ExplorerState {
    /// Creates a new explorer state with default values.
    pub fn new() -> Self {
        Self {
            selection: RwSignal::new(None),
            view_type: RwSignal::new(ExplorerViewType::default()),
        }
    }

    /// Selects an item (file or directory).
    pub fn select(&self, path: VirtualPath, is_dir: bool) {
        self.selection.set(Some(Selection { path, is_dir }));
    }

    /// Clears the selection.
    pub fn clear_selection(&self) {
        self.selection.set(None);
    }

    /// Toggle between list and grid view.
    pub fn toggle_view_type(&self) {
        self.view_type.update(|vt| {
            *vt = match *vt {
                ExplorerViewType::List => ExplorerViewType::Grid,
                ExplorerViewType::Grid => ExplorerViewType::List,
            };
        });
    }
}

impl Default for ExplorerState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// AppContext
// ============================================================================

/// Application-wide reactive context.
///
/// This context is provided at the root of the component tree and can be
/// accessed from any child component using `use_context::<AppContext>()`.
///
/// # Architecture
///
/// The URL is the single source of truth for navigation state.
/// AppContext only manages non-navigation state:
/// - **Filesystem**: Virtual filesystem for file operations
/// - **Terminal state**: Command history, output
/// - **Explorer state**: File browser UI state
/// - **Wallet state**: Connection status, address, ENS name
/// - **View mode**: Terminal or Explorer view
///
/// `Clone + Copy` because every field is a signal handle or a nested
/// signal-container struct — see the module-level convention note.
#[derive(Clone, Copy)]
pub struct AppContext {
    // === Shared State ===
    /// Global canonical filesystem tree (`/site`, `/mnt/*`, `/state/*`).
    pub global_fs: RwSignal<GlobalFs>,
    /// Current canonical working directory for shell/explorer surfaces.
    pub cwd: RwSignal<VirtualPath>,
    /// Wallet connection state.
    pub wallet: RwSignal<WalletState>,

    // === View Management ===
    /// Current view mode (Terminal or Explorer).
    pub view_mode: RwSignal<ViewMode>,

    // === View-Specific State ===
    /// Terminal state (history, commands).
    pub terminal: TerminalState,
    /// Explorer state (selection, view type, sheet).
    pub explorer: ExplorerState,

    // === Runtime filesystem/write state ===
    /// Staged + working-tree edits awaiting commit.
    pub changes: RwSignal<ChangeSet>,
    /// Merged global canonical filesystem with local `changes` overlaid.
    pub view_global_fs: Signal<Rc<GlobalFs>, LocalStorage>,
    /// Backend registry keyed by canonical mount roots.
    pub backends: StoredValue<BTreeMap<VirtualPath, Arc<dyn StorageBackend>>, LocalStorage>,
    /// Runtime mount ownership keyed by canonical roots.
    pub runtime_mounts: RwSignal<Vec<RuntimeMount>>,
    /// Remote HEAD registry keyed by canonical mount roots.
    pub remote_heads: RwSignal<BTreeMap<VirtualPath, String>>,
    /// Version counter for runtime state-backed `/state` nodes.
    pub runtime_state_rev: RwSignal<u64>,

    // === Editor modal ===
    /// When `Some(path)`, the `EditModal` is open editing that path. `None` = closed.
    pub editor_open: RwSignal<Option<crate::models::VirtualPath>>,
}

impl AppContext {
    /// Creates a new application context with default state.
    ///
    /// All signals are initialized to their default values:
    /// - Terminal: Empty history
    /// - Explorer: No selection, list view
    /// - Wallet: Disconnected
    /// - Filesystem: Empty
    /// - View: Terminal mode
    pub fn new() -> Self {
        let global_fs = RwSignal::new(crate::core::storage::boot::bootstrap_global_fs());
        let changes = RwSignal::new(ChangeSet::new());
        let wallet = RwSignal::new(WalletState::default());
        let runtime_state_rev = RwSignal::new(0_u64);
        let view_global_fs = Signal::derive_local(move || {
            runtime_state_rev.track();
            Rc::new(global_fs.with(|base| {
                changes.with(|cs| wallet.with(|ws| merge::merge_global_view(base, cs, ws)))
            }))
        });

        let backends: StoredValue<BTreeMap<VirtualPath, Arc<dyn StorageBackend>>, LocalStorage> =
            StoredValue::new_local(crate::core::runtime::bootstrap_backends());
        let runtime_mounts = RwSignal::new(crate::core::runtime::bootstrap_runtime_mounts());
        let remote_heads = RwSignal::new(BTreeMap::new());

        let editor_open = RwSignal::new(None);

        Self {
            // Shared state
            global_fs,
            cwd: RwSignal::new(VirtualPath::from_absolute("/site").expect("valid cwd")),
            wallet,

            // View management
            view_mode: RwSignal::new(ViewMode::default()),

            // View-specific state
            terminal: TerminalState::new(),
            explorer: ExplorerState::new(),

            // Runtime filesystem/write state
            changes,
            view_global_fs,
            backends,
            runtime_mounts,
            remote_heads,
            runtime_state_rev,

            // Editor state
            editor_open,
        }
    }

    /// Gets the current prompt string for display.
    ///
    /// Format: `{username}@{app_name}:{path}`
    ///
    /// The username is derived from the wallet state:
    /// - ENS name if available
    /// - Shortened address (0x1234...5678) if connected
    /// - "guest" if disconnected
    pub fn get_prompt(&self, cwd: &VirtualPath) -> String {
        let display_path = display_path_for(cwd);
        let username = self.wallet.get().display_name();
        format!("{}@{}:{}", username, APP_NAME, display_path)
    }

    /// Best-effort lookup for the backend responsible for a canonical path.
    pub fn backend_for_path(&self, path: &VirtualPath) -> Option<Arc<dyn StorageBackend>> {
        self.backends.with_value(|map| {
            map.iter()
                .filter(|(root, _)| path.starts_with(root))
                .max_by_key(|(root, _)| root.as_str().len())
                .map(|(_, backend)| backend.clone())
        })
    }

    pub fn runtime_mount_for_path(&self, path: &VirtualPath) -> Option<RuntimeMount> {
        self.runtime_mounts.with(|mounts| {
            mounts
                .iter()
                .filter(|mount| mount.contains(path))
                .max_by_key(|mount| mount.root.as_str().len())
                .cloned()
        })
    }

    /// Best-effort lookup for the last known remote HEAD responsible for a
    /// canonical path.
    pub fn remote_head_for_path(&self, path: &VirtualPath) -> Option<String> {
        self.remote_heads.with(|map| {
            map.iter()
                .filter(|(root, _)| path.starts_with(root))
                .max_by_key(|(root, _)| root.as_str().len())
                .map(|(_, head)| head.clone())
        })
    }

    /// Toggles between Terminal and Explorer view modes.
    #[allow(dead_code)]
    pub fn toggle_view_mode(&self) {
        self.view_mode.update(|mode| {
            *mode = match *mode {
                ViewMode::Terminal => ViewMode::Explorer,
                ViewMode::Explorer => ViewMode::Terminal,
            };
        });
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Root application component with error boundary.
///
/// This component:
/// - Creates and provides the global AppContext
/// - Wraps the app in an ErrorBoundary for graceful error handling
/// - Renders the main Shell component
#[component]
pub fn App() -> impl IntoView {
    // Create and provide application context
    let ctx = AppContext::new();
    provide_context(ctx);
    let changes_signal = ctx.changes;
    spawn_local(async move {
        match crate::core::storage::boot::hydrate_drafts("~").await {
            Ok(cs) if !cs.is_empty() => changes_signal.set(cs),
            Ok(_) => {}
            Err(e) => web_sys::console::error_1(&format!("hydrate drafts: {e}").into()),
        }
    });

    let persister = Rc::new(DraftPersister::new("~"));
    let persister_for_effect = persister.clone();
    Effect::new(move |_| {
        let snapshot = ctx.changes.get();
        persister_for_effect.schedule(snapshot);
    });

    let boot_started = StoredValue::new(false);
    Effect::new(move |_| {
        if !boot_started.get_value() {
            boot_started.set_value(true);
            crate::components::terminal::boot::run(ctx);
        }
    });

    view! {
        <ErrorBoundary
            fallback=|errors| view! {
                <div class=err_css::container>
                    <div class=err_css::inner>
                        <h1 class=err_css::title>
                            "Something went wrong"
                        </h1>
                        <p class=err_css::message>
                            "An unexpected error occurred. Please try reloading the page."
                        </p>
                        <details class=err_css::details>
                            <summary class=err_css::summary>
                                "Error details"
                            </summary>
                            <ul class=err_css::detailsList>
                                {move || errors.get()
                                    .into_iter()
                                    .map(|(_, e)| view! { <li>{e.to_string()}</li> })
                                    .collect::<Vec<_>>()
                                }
                            </ul>
                        </details>
                        <button
                            class=err_css::reloadButton
                            on:click=move |_| {
                                if let Some(window) = web_sys::window() {
                                    let _ = window.location().reload();
                                }
                            }
                        >
                            "Reload Page"
                        </button>
                    </div>
                </div>
            }
        >
            <RouterView />
            <crate::components::editor::EditModal />
        </ErrorBoundary>
    }
}
