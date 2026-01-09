//! Root application module.
//!
//! Contains the main App component, AppContext definition, TerminalState,
//! and application-level setup logic following Leptos conventions.

use leptos::prelude::*;

use crate::components::AppRouter;
use crate::config::{APP_NAME, MAX_COMMAND_HISTORY, MAX_TERMINAL_HISTORY};
use crate::core::VirtualFs;
use crate::models::{
    AppRoute, ExplorerViewType, MountRegistry, OutputLine, Selection, ViewMode, WalletState,
};
use crate::utils::RingBuffer;

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
        let history = self.command_history.get();
        if history.is_empty() {
            return None;
        }

        let current_index = self.history_index.get();
        let new_index = match current_index {
            None if direction < 0 => Some(history.len() - 1),
            Some(i) if direction < 0 && i > 0 => Some(i - 1),
            Some(i) if direction > 0 && i < history.len() - 1 => Some(i + 1),
            Some(_) if direction > 0 => None,
            _ => current_index,
        };

        self.history_index.set(new_index);
        new_index.map(|i| history[i].clone())
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
/// This struct is `Copy` because all fields are Leptos signals.
#[derive(Clone, Copy)]
pub struct ExplorerState {
    /// Currently selected item (file or directory).
    pub selection: RwSignal<Option<Selection>>,
    /// Current view type (list or grid).
    pub view_type: RwSignal<ExplorerViewType>,
    /// Forward navigation stack (stores routes to go forward to after going back/up).
    pub forward_stack: RwSignal<Vec<AppRoute>>,
}

impl ExplorerState {
    /// Creates a new explorer state with default values.
    pub fn new() -> Self {
        Self {
            selection: RwSignal::new(None),
            view_type: RwSignal::new(ExplorerViewType::default()),
            forward_stack: RwSignal::new(Vec::new()),
        }
    }

    /// Selects an item (file or directory).
    pub fn select(&self, path: String, is_dir: bool) {
        self.selection.set(Some(Selection { path, is_dir }));
    }

    /// Clears the selection.
    pub fn clear_selection(&self) {
        self.selection.set(None);
    }

    /// Push current route to forward stack (called when navigating back/up).
    pub fn push_forward(&self, route: AppRoute) {
        self.forward_stack.update(|stack| stack.push(route));
    }

    /// Pop from forward stack (called when navigating forward).
    pub fn pop_forward(&self) -> Option<AppRoute> {
        let mut result = None;
        self.forward_stack.update(|stack| {
            result = stack.pop();
        });
        result
    }

    /// Clear forward stack (called when navigating to a new location, not back/forward).
    pub fn clear_forward(&self) {
        self.forward_stack.update(|stack| stack.clear());
    }

    /// Check if forward navigation is available.
    pub fn can_go_forward(&self) -> bool {
        self.forward_stack.with(|stack| !stack.is_empty())
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
/// - **Mounts**: Registry of mounted filesystems
/// - **Filesystem**: Virtual filesystem for file operations
/// - **Terminal state**: Command history, output
/// - **Explorer state**: File browser UI state
/// - **Wallet state**: Connection status, address, ENS name
/// - **View mode**: Terminal or Explorer view
#[derive(Clone, Copy)]
pub struct AppContext {
    // === Shared State ===
    /// Mount registry for managing multiple filesystem backends.
    /// This is not a signal because mounts are configured once at startup
    /// and never change during the application lifecycle.
    pub mounts: StoredValue<MountRegistry>,
    /// Virtual filesystem for file navigation.
    pub fs: RwSignal<VirtualFs>,
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
}

impl AppContext {
    /// Creates a new application context with default state.
    ///
    /// All signals are initialized to their default values:
    /// - Mounts: Registry from configured mounts
    /// - Terminal: Empty history
    /// - Explorer: No selection, list view
    /// - Wallet: Disconnected
    /// - Filesystem: Empty
    /// - View: Terminal mode
    pub fn new() -> Self {
        use crate::config::configured_mounts;

        Self {
            // Shared state
            mounts: StoredValue::new(MountRegistry::from_mounts(configured_mounts())),
            fs: RwSignal::new(VirtualFs::empty()),
            wallet: RwSignal::new(WalletState::default()),

            // View management
            view_mode: RwSignal::new(ViewMode::default()),

            // View-specific state
            terminal: TerminalState::new(),
            explorer: ExplorerState::new(),
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
    pub fn get_prompt(&self, route: &AppRoute) -> String {
        let display_path = route.display_path();
        let username = self.wallet.get().display_name();
        format!("{}@{}:{}", username, APP_NAME, display_path)
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

    view! {
        <ErrorBoundary
            fallback=|errors| view! {
                <div style="
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    justify-content: center;
                    height: 100vh;
                    padding: 2rem;
                    background: #0a0e27;
                    color: #e0e0e0;
                    font-family: 'Courier New', monospace;
                ">
                    <div style="
                        max-width: 600px;
                        text-align: center;
                    ">
                        <h1 style="color: #ff6b6b; margin-bottom: 1rem;">
                            "Something went wrong"
                        </h1>
                        <p style="color: #a0a0a0; margin-bottom: 2rem;">
                            "An unexpected error occurred. Please try reloading the page."
                        </p>
                        <details style="
                            text-align: left;
                            background: #151a35;
                            padding: 1rem;
                            border-radius: 4px;
                            margin-bottom: 1rem;
                        ">
                            <summary style="cursor: pointer; color: #6c7a89;">
                                "Error details"
                            </summary>
                            <ul style="
                                margin: 1rem 0 0 0;
                                padding-left: 1.5rem;
                                color: #ff6b6b;
                                font-size: 0.9rem;
                            ">
                                {move || errors.get()
                                    .into_iter()
                                    .map(|(_, e)| view! { <li>{e.to_string()}</li> })
                                    .collect::<Vec<_>>()
                                }
                            </ul>
                        </details>
                        <button
                            on:click=move |_| {
                                if let Some(window) = web_sys::window() {
                                    let _ = window.location().reload();
                                }
                            }
                            style="
                                background: #4a90e2;
                                color: white;
                                border: none;
                                padding: 0.75rem 2rem;
                                border-radius: 4px;
                                cursor: pointer;
                                font-family: 'Courier New', monospace;
                                font-size: 1rem;
                            "
                        >
                            "Reload Page"
                        </button>
                    </div>
                </div>
            }
        >
            <AppRouter />
        </ErrorBoundary>
    }
}
