//! Root application module.
//!
//! Contains the main App component, AppContext definition, TerminalState,
//! and application-level setup logic following Leptos conventions.

use leptos::prelude::*;

use crate::components::Shell;
use crate::config::{APP_NAME, MAX_COMMAND_HISTORY, MAX_NAV_HISTORY, MAX_TERMINAL_HISTORY};
use crate::core::VirtualFs;
use crate::models::{
    ContentOverlay, ExplorerViewType, OutputLine, ScreenMode, SheetState, ViewMode, VirtualPath,
    WalletState,
};
use crate::utils::RingBuffer;

// ============================================================================
// TerminalState
// ============================================================================

/// Terminal state managed with Leptos signals.
///
/// Handles terminal-specific state including output history, navigation,
/// and command history. Terminal output uses a [`RingBuffer`] for O(1)
/// push operations, avoiding the O(n) cost of `Vec::drain()` when limiting
/// history size.
///
/// # Note
///
/// This struct is `Copy` because all fields are Leptos signals, which are
/// cheap to copy (they're just pointers to the underlying reactive state).
///
/// The `current_path` signal is shared with AppContext - changes from either
/// Terminal or Explorer will be reflected in both views.
#[derive(Clone, Copy)]
pub struct TerminalState {
    /// Terminal output history (bounded by `MAX_TERMINAL_HISTORY`).
    pub history: RwSignal<RingBuffer<OutputLine>>,
    /// Current working directory path (shared with AppContext).
    pub current_path: RwSignal<VirtualPath>,
    /// Current screen mode (terminal, reader, etc.).
    pub screen_mode: RwSignal<ScreenMode>,
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
    /// - Current path: home directory
    /// - Screen mode: Booting
    /// - Empty command history
    pub fn new() -> Self {
        Self {
            history: RwSignal::new(RingBuffer::new(MAX_TERMINAL_HISTORY)),
            current_path: RwSignal::new(VirtualPath::home()),
            screen_mode: RwSignal::new(ScreenMode::Booting),
            command_history: RwSignal::new(Vec::new()),
            history_index: RwSignal::new(None),
        }
    }

    /// Creates a new terminal state with a shared current_path signal.
    ///
    /// This constructor is used when creating a TerminalState that shares
    /// its path with AppContext for synchronization between Terminal and Explorer.
    pub fn new_with_path(current_path: RwSignal<VirtualPath>) -> Self {
        Self {
            history: RwSignal::new(RingBuffer::new(MAX_TERMINAL_HISTORY)),
            current_path,
            screen_mode: RwSignal::new(ScreenMode::Booting),
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
    pub fn push_lines(&self, lines: Vec<OutputLine>) {
        self.history.update(|h| h.extend(lines));
    }

    /// Clears all terminal output history.
    pub fn clear_history(&self) {
        self.history.update(|h| h.clear());
    }

    /// Gets the current prompt string for display.
    ///
    /// Format: `{username}@{app_name}:{path}`
    ///
    /// # Arguments
    ///
    /// * `wallet_state` - Current wallet state for username derivation
    #[allow(dead_code)]
    pub fn get_prompt(&self, wallet_state: WalletState) -> String {
        let path = self.current_path.get();
        let display_path = path.display();
        let username = wallet_state.display_name();
        format!("{}@{}:{}", username, APP_NAME, display_path)
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
    /// Currently selected file path (for preview).
    pub selected_file: RwSignal<Option<String>>,
    /// Current view type (list or grid).
    pub view_type: RwSignal<ExplorerViewType>,
    /// Bottom sheet state.
    pub sheet_state: RwSignal<SheetState>,
}

impl ExplorerState {
    /// Creates a new explorer state with default values.
    pub fn new() -> Self {
        Self {
            selected_file: RwSignal::new(None),
            view_type: RwSignal::new(ExplorerViewType::default()),
            sheet_state: RwSignal::new(SheetState::default()),
        }
    }

    /// Selects a file and opens the preview sheet.
    pub fn select_file(&self, path: String) {
        self.selected_file.set(Some(path));
        self.sheet_state.set(SheetState::Preview);
    }

    /// Clears the selection and closes the sheet.
    pub fn clear_selection(&self) {
        self.selected_file.set(None);
        self.sheet_state.set(SheetState::Closed);
    }

    /// Expands the sheet to full screen.
    #[allow(dead_code)]
    pub fn expand_sheet(&self) {
        self.sheet_state.set(SheetState::Expanded);
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
/// The [`AppContext`] separates concerns into independent domains:
/// - **Terminal state**: Command history, output, navigation
/// - **Explorer state**: File browser UI state
/// - **Wallet state**: Connection status, address, ENS name
/// - **Filesystem**: Virtual filesystem for navigation (shared)
/// - **View management**: ViewMode and ContentOverlay
#[derive(Clone, Copy)]
pub struct AppContext {
    // === Shared State ===
    /// Virtual filesystem for file navigation.
    pub fs: RwSignal<VirtualFs>,
    /// Current working directory path (shared between Terminal and Explorer).
    pub current_path: RwSignal<VirtualPath>,
    /// Wallet connection state.
    pub wallet: RwSignal<WalletState>,

    // === Navigation History ===
    /// Back navigation stack (bounded by `MAX_NAV_HISTORY`).
    pub back_stack: RwSignal<Vec<VirtualPath>>,
    /// Forward navigation stack (cleared on new navigation).
    pub forward_stack: RwSignal<Vec<VirtualPath>>,

    // === View Management ===
    /// Current view mode (Terminal or Explorer).
    pub view_mode: RwSignal<ViewMode>,
    /// Content overlay (Reader, etc.).
    pub content_overlay: RwSignal<ContentOverlay>,

    // === View-Specific State ===
    /// Terminal state (history, commands, navigation).
    pub terminal: TerminalState,
    /// Explorer state (selection, view type, sheet).
    pub explorer: ExplorerState,
}

impl AppContext {
    /// Creates a new application context with default state.
    ///
    /// All signals are initialized to their default values:
    /// - Terminal: Empty history, home directory
    /// - Explorer: No selection, list view
    /// - Wallet: Disconnected
    /// - Filesystem: Empty
    /// - View: Terminal mode
    pub fn new() -> Self {
        let current_path = RwSignal::new(VirtualPath::home());

        Self {
            // Shared state
            fs: RwSignal::new(VirtualFs::empty()),
            current_path,
            wallet: RwSignal::new(WalletState::default()),

            // Navigation history
            back_stack: RwSignal::new(Vec::new()),
            forward_stack: RwSignal::new(Vec::new()),

            // View management
            view_mode: RwSignal::new(ViewMode::default()),
            content_overlay: RwSignal::new(ContentOverlay::default()),

            // View-specific state
            terminal: TerminalState::new_with_path(current_path),
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
    pub fn get_prompt(&self) -> String {
        let path = self.current_path.get();
        let display_path = path.display();
        let username = self.wallet.get().display_name();
        format!("{}@{}:{}", username, APP_NAME, display_path)
    }

    /// Toggles between Terminal and Explorer view modes.
    pub fn toggle_view_mode(&self) {
        self.view_mode.update(|mode| {
            *mode = match *mode {
                ViewMode::Terminal => ViewMode::Explorer,
                ViewMode::Explorer => ViewMode::Terminal,
            };
        });
    }

    /// Opens the reader overlay with the specified content.
    ///
    /// # Arguments
    /// * `content_path` - Relative path to content (e.g., "blog/hello.md")
    /// * `virtual_path` - Full virtual path for breadcrumb (e.g., "/home/wonjae/blog/hello.md")
    pub fn open_reader(&self, content_path: String, virtual_path: String) {
        self.content_overlay.set(ContentOverlay::Reader {
            content_path,
            virtual_path,
        });
    }

    /// Closes the content overlay.
    pub fn close_overlay(&self) {
        self.content_overlay.set(ContentOverlay::None);
    }

    // =========================================================================
    // Navigation Methods
    // =========================================================================

    /// Navigates to a new directory, updating history stacks.
    ///
    /// This is the primary method for all directory changes. It:
    /// - Pushes the current path to the back stack
    /// - Clears the forward stack (new navigation invalidates forward history)
    /// - Updates the current path
    /// - Clears any file selection in the explorer
    ///
    /// The back stack is bounded by `MAX_NAV_HISTORY` to prevent unbounded growth.
    pub fn navigate_to(&self, path: VirtualPath) {
        let current = self.current_path.get();

        // Don't add to history if navigating to the same path
        if current == path {
            return;
        }

        // Push current path to back stack (with size limit)
        self.back_stack.update(|stack| {
            stack.push(current);
            if stack.len() > MAX_NAV_HISTORY {
                stack.remove(0);
            }
        });

        // Clear forward stack on new navigation
        self.forward_stack.update(|stack| stack.clear());

        // Update current path
        self.current_path.set(path);

        // Clear explorer selection
        self.explorer.clear_selection();
    }

    /// Navigates back in history.
    ///
    /// Returns `true` if navigation occurred, `false` if back stack was empty.
    pub fn go_back(&self) -> bool {
        let prev = self.back_stack.try_update(|stack| stack.pop()).flatten();

        if let Some(prev_path) = prev {
            let current = self.current_path.get();

            // Push current to forward stack
            self.forward_stack.update(|stack| {
                stack.push(current);
                // Forward stack doesn't need strict limit since it's cleared on new navigation
            });

            self.current_path.set(prev_path);
            self.explorer.clear_selection();
            true
        } else {
            false
        }
    }

    /// Navigates forward in history.
    ///
    /// Returns `true` if navigation occurred, `false` if forward stack was empty.
    pub fn go_forward(&self) -> bool {
        let next = self.forward_stack.try_update(|stack| stack.pop()).flatten();

        if let Some(next_path) = next {
            let current = self.current_path.get();

            // Push current to back stack
            self.back_stack.update(|stack| {
                stack.push(current);
                if stack.len() > MAX_NAV_HISTORY {
                    stack.remove(0);
                }
            });

            self.current_path.set(next_path);
            self.explorer.clear_selection();
            true
        } else {
            false
        }
    }

    /// Returns the previous path (top of back stack) without navigating.
    ///
    /// Useful for `cd -` command which needs to know the previous directory.
    #[inline]
    #[allow(dead_code)]
    pub fn previous_path(&self) -> Option<VirtualPath> {
        self.back_stack.with(|stack| stack.last().cloned())
    }

    /// Checks if back navigation is available.
    #[inline]
    pub fn can_go_back(&self) -> bool {
        self.back_stack.with(|stack| !stack.is_empty())
    }

    /// Checks if forward navigation is available.
    #[inline]
    pub fn can_go_forward(&self) -> bool {
        self.forward_stack.with(|stack| !stack.is_empty())
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
            <Shell />
        </ErrorBoundary>
    }
}
