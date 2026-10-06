//! UI context: independent content, wallet, and preferences owners plus terminal state.
use super::TerminalState;
use crate::config::APP_NAME;
use crate::runtime::{content::Content, state::Preferences, wallet::Wallet};
use leptos::prelude::*;
use websh_core::domain::{VirtualPath, is_runtime_overlay_path, runtime_state_root};
use websh_core::filesystem::{ContentReadError, FsView, GlobalFs, display_path_for};

#[derive(Clone, Copy)]
pub struct AppContext {
    pub content: Content,
    pub wallet: Wallet,
    pub preferences: Preferences,
    pub cwd: RwSignal<VirtualPath>,
    pub theme: Memo<&'static str>,
    pub terminal: TerminalState,
    pub runtime_overlay: Memo<GlobalFs>,
}

impl AppContext {
    pub fn new() -> Self {
        let content = Content::new(crate::runtime::loader::bootstrap_runtime_load());
        let preferences = Preferences::new();
        let wallet = Wallet::new(preferences);
        // Runtime projection never copies or subscribes to the content tree.
        let runtime_overlay = Memo::new_with_compare(
            move |_| {
                wallet.state.with(|wallet| {
                    preferences
                        .snapshot
                        .with(|state| websh_core::runtime::build_runtime_overlay(wallet, state))
                })
            },
            |_, _| true,
        );
        Self {
            content,
            wallet,
            preferences,
            cwd: RwSignal::new(VirtualPath::root()),
            theme: Memo::new(move |_| {
                preferences.snapshot.with(|state| {
                    state
                        .env
                        .get("THEME")
                        .and_then(|value| crate::render::theme::normalize_theme_id(value))
                        .unwrap_or(crate::render::theme::DEFAULT_THEME)
                })
            }),
            terminal: TerminalState::new(),
            runtime_overlay,
        }
    }

    pub fn with_fs<T>(&self, f: impl FnOnce(FsView<'_>) -> T) -> T {
        self.content.with_fs(|content| {
            self.runtime_overlay
                .with(|runtime| f(FsView::with_runtime(content, runtime)))
        })
    }

    /// Content-only paths do not subscribe to wallet or preference changes.
    pub fn with_fs_at<T>(&self, path: &VirtualPath, f: impl FnOnce(FsView<'_>) -> T) -> T {
        if is_runtime_overlay_path(path) || runtime_state_root().starts_with(path) {
            self.with_fs(f)
        } else {
            self.content.with_fs(|fs| f(FsView::content(fs)))
        }
    }

    pub fn get_prompt(&self, cwd: &VirtualPath) -> String {
        format!(
            "{}@{}:{}",
            self.wallet.state.get().display_name(),
            APP_NAME,
            display_path_for(cwd)
        )
    }

    pub async fn read_text(&self, path: &VirtualPath) -> Result<String, ContentReadError> {
        if is_runtime_overlay_path(path) {
            return self
                .runtime_overlay
                .with(|fs| fs.read_inline_text(path))
                .ok_or_else(|| ContentReadError::NoBackend { path: path.clone() });
        }
        self.content.read_text(path).await
    }

    pub async fn read_bytes(&self, path: &VirtualPath) -> Result<Vec<u8>, ContentReadError> {
        if is_runtime_overlay_path(path) {
            return self.read_text(path).await.map(String::into_bytes);
        }
        self.content.read_bytes(path).await
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}
