//! UI context: independent content, wallet, and preferences owners plus terminal state.
use super::TerminalState;
use crate::config::APP_NAME;
use crate::runtime::{content::Content, state::Preferences, wallet::Wallet};
use leptos::prelude::*;
use websh_core::domain::{VirtualPath, is_runtime_overlay_path};
use websh_core::filesystem::{ContentReadError, GlobalFs, display_path_for};

#[derive(Clone, Copy)]
pub struct AppContext {
    pub content: Content,
    pub wallet: Wallet,
    pub preferences: Preferences,
    pub cwd: RwSignal<VirtualPath>,
    pub theme: Memo<&'static str>,
    pub terminal: TerminalState,
    pub system_global_fs: Memo<GlobalFs>,
}

impl AppContext {
    pub fn new() -> Self {
        let content = Content::new(crate::runtime::loader::bootstrap_runtime_load());
        let preferences = Preferences::new();
        let wallet = Wallet::new(preferences);
        // Memoized once per actual content/session/preference change, not once per shell read.
        let system_global_fs = Memo::new_with_compare(
            move |_| {
                content.with_fs(|fs| {
                    wallet.state.with(|wallet| {
                        preferences.snapshot.with(|state| {
                            websh_core::runtime::build_view_global_fs(fs, wallet, state)
                        })
                    })
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
            system_global_fs,
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
                .system_global_fs
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

    pub fn public_read_url(&self, path: &VirtualPath) -> Result<Option<String>, ContentReadError> {
        if is_runtime_overlay_path(path) {
            return Ok(None);
        }
        self.content.public_read_url(path)
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}
