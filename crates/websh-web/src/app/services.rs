//! Application-owned runtime services.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use websh_core::attestation::ledger::CONTENT_LEDGER_CONTENT_PATH;
use websh_core::domain::{VirtualPath, WalletState};
use websh_core::runtime::RuntimeStateSnapshot;

use crate::render::theme;
use crate::runtime::{loader, state, wallet};

use super::{AppContext, RuntimeServiceError, RuntimeServiceResult, ThemeError};
use crate::runtime::EnvironmentError;
use crate::runtime::loader::RuntimeLoad;
use crate::runtime::mounts::{MountLoadStatus, MountScanJob};

#[derive(Clone, Copy)]
pub struct RuntimeServices {
    ctx: AppContext,
}

impl RuntimeServices {
    pub fn new(ctx: AppContext) -> Self {
        Self { ctx }
    }

    pub fn install_browser_persistence() {
        state::install_browser_persistence();
    }

    pub fn runtime_state_snapshot() -> RuntimeStateSnapshot {
        state::snapshot()
    }

    pub fn bootstrap_runtime_load() -> RuntimeLoad {
        loader::bootstrap_runtime_load()
    }

    pub fn refresh_runtime_state(&self) {
        self.ctx.runtime_state.set(state::snapshot());
    }

    pub fn init_default_env(&self) {
        state::install_browser_persistence();
        state::init_default_env();
        self.refresh_runtime_state();
    }

    pub fn set_env_var(&self, key: &str, value: &str) -> Result<(), EnvironmentError> {
        let snapshot = state::set_env_var(key, value)?;
        self.ctx.runtime_state.set(snapshot);
        Ok(())
    }

    pub fn unset_env_var(&self, key: &str) -> Result<(), EnvironmentError> {
        let snapshot = state::unset_env_var(key)?;
        self.ctx.runtime_state.set(snapshot);
        Ok(())
    }

    pub fn set_theme(&self, raw_theme: &str) -> Result<&'static str, ThemeError> {
        let Some(theme_id) = theme::normalize_theme_id(raw_theme) else {
            return Err(ThemeError::unknown(raw_theme));
        };
        let snapshot = state::set_env_var("THEME", theme_id)
            .map_err(|source| ThemeError::Persist { theme_id, source })?;
        if self.ctx.theme.get_untracked() != theme_id {
            self.ctx.theme.set(theme_id);
        }
        self.ctx.runtime_state.set(snapshot);
        theme::apply_theme_to_document(theme_id);
        Ok(theme_id)
    }

    pub async fn reload_runtime(&self) -> RuntimeServiceResult {
        let sequence = self.ctx.begin_root_request();
        let result = loader::reload_runtime().await;
        self.finish_root_load(sequence, result)
    }

    fn finish_root_load(
        &self,
        sequence: u64,
        result: Result<RuntimeLoad, crate::runtime::RuntimeLoadError>,
    ) -> RuntimeServiceResult {
        // Both successful and failed older requests are obsolete at the same boundary.
        if !self.ctx.accepts_root_request(sequence) {
            return Ok(());
        }
        match result {
            Ok(load) => {
                let jobs = load.mounts.scan_jobs.clone();
                let generation = self.ctx.apply_runtime_load(load);
                self.start_ledger_prefetch(generation);
                self.start_mount_scans(generation, jobs);
                Ok(())
            }
            Err(error) => {
                let _ = self
                    .ctx
                    .mark_mount_failed(&VirtualPath::root(), error.to_string());
                Err(error.into())
            }
        }
    }

    pub async fn reload_runtime_mount(&self, mount_root: VirtualPath) -> RuntimeServiceResult {
        if mount_root.is_root() {
            return self.reload_runtime().await;
        }

        let backend = self
            .ctx
            .backend_for_mount_root(&mount_root)
            .ok_or_else(|| RuntimeServiceError::NoBackend {
                mount_root: mount_root.clone(),
            })?;
        let generation = self.ctx.runtime_generation();
        let (declared_mount, epoch) = self.ctx.mark_mount_loading(&mount_root)?;
        crate::runtime::mount_refresh::refresh_mount(
            self.ctx,
            generation,
            MountScanJob {
                mount: declared_mount,
                backend,
                epoch,
            },
        )
        .await
    }

    pub fn start_mount_scans(&self, generation: u64, jobs: Vec<MountScanJob>) {
        for job in jobs {
            let ctx = self.ctx;
            spawn_local(async move {
                if let Err(error) =
                    crate::runtime::mount_refresh::refresh_mount(ctx, generation, job).await
                {
                    leptos::logging::warn!("runtime: mount apply failed: {error}");
                }
            });
        }
    }

    fn start_ledger_prefetch(&self, generation: u64) {
        let ctx = self.ctx;
        let path = VirtualPath::from_absolute(format!("/{CONTENT_LEDGER_CONTENT_PATH}"))
            .expect("ledger path is absolute");
        let root = VirtualPath::root();
        if !matches!(
            ctx.mount_status_for(&root),
            Some(MountLoadStatus::Available { .. })
        ) {
            return;
        }
        if !ctx.global_fs.with_untracked(|fs| fs.exists(&path)) {
            return;
        }

        spawn_local(async move {
            if ctx.runtime_generation() != generation {
                return;
            }

            let _ = ctx.read_text(&path).await;
        });
    }

    pub fn set_wallet_session(&self, active: bool) -> Result<(), EnvironmentError> {
        let snapshot = state::set_wallet_session(active)?;
        self.ctx.runtime_state.set(snapshot);
        Ok(())
    }

    pub fn wallet_available(&self) -> bool {
        wallet::is_available()
    }

    pub fn has_wallet_session(&self) -> bool {
        state::has_wallet_session()
    }

    pub async fn wallet_account(&self) -> Option<String> {
        wallet::get_account().await
    }

    pub async fn wallet_chain_id(&self) -> Option<u64> {
        wallet::get_chain_id().await
    }

    pub async fn resolve_wallet_ens(&self, address: &str) -> Option<String> {
        wallet::resolve_ens(address).await
    }

    pub async fn connect_wallet_with_session(
        &self,
    ) -> Result<wallet::ConnectOutcome, wallet::WalletError> {
        if !self.wallet_available() {
            return Err(wallet::WalletError::NotInstalled);
        }
        self.ctx.wallet.set(WalletState::Connecting);

        let address = match wallet::connect().await {
            Ok(addr) => addr,
            Err(err) => {
                self.ctx.wallet.set(WalletState::Disconnected);
                return Err(err);
            }
        };

        let session_persist_error = self.set_wallet_session(true).err();

        let chain_id = self.wallet_chain_id().await;
        self.ctx.wallet.set(WalletState::Connected {
            address: address.clone(),
            ens_name: None,
            chain_id,
        });

        let ens_name = self.resolve_wallet_ens(&address).await;
        if ens_name.is_some() {
            self.ctx.wallet.set(WalletState::Connected {
                address: address.clone(),
                ens_name: ens_name.clone(),
                chain_id,
            });
        }

        Ok(wallet::ConnectOutcome {
            address,
            chain_id,
            ens_name,
            session_persist_error,
        })
    }

    pub fn restore_wallet_session(
        &self,
        address: String,
        chain_id: Option<u64>,
        ens_name: Option<String>,
    ) -> Result<(), EnvironmentError> {
        self.ctx.wallet.set(WalletState::Connected {
            address,
            ens_name,
            chain_id,
        });
        self.set_wallet_session(true)
    }

    pub fn disconnect_wallet(&self) -> Result<(), EnvironmentError> {
        self.set_wallet_session(false)?;
        self.ctx.wallet.set(WalletState::Disconnected);
        Ok(())
    }

    pub fn install_wallet_event_listeners(&self) {
        if self.ctx.wallet_event_listeners_installed() {
            return;
        }

        let services_for_accounts = *self;
        let accounts_listener =
            match wallet::on_accounts_changed(move |account: Option<String>| match account {
                Some(new_addr) => {
                    services_for_accounts.ctx.wallet.update(|w| {
                        if let WalletState::Connected { chain_id, .. } = w {
                            *w = WalletState::Connected {
                                address: new_addr,
                                ens_name: None,
                                chain_id: *chain_id,
                            };
                        }
                    });
                }
                None => {
                    let _ = services_for_accounts.disconnect_wallet();
                }
            }) {
                Ok(listener) => listener,
                Err(error) => {
                    leptos::logging::warn!("wallet: account listener unavailable: {error}");
                    return;
                }
            };

        let services_for_chain = *self;
        let chain_listener = match wallet::on_chain_changed(move |chain_id_hex: String| {
            let new_chain_id = u64::from_str_radix(chain_id_hex.trim_start_matches("0x"), 16).ok();

            services_for_chain.ctx.wallet.update(|w| {
                if let WalletState::Connected { chain_id, .. } = w {
                    *chain_id = new_chain_id;
                }
            });
        }) {
            Ok(listener) => listener,
            Err(error) => {
                leptos::logging::warn!("wallet: chain listener unavailable: {error}");
                return;
            }
        };

        self.ctx
            .install_wallet_event_listeners(wallet::WalletEventListeners::new(
                accounts_listener,
                chain_listener,
            ));
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use leptos::prelude::Owner;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn set_theme_persists_user_theme_and_runtime_snapshot() {
        let owner = Owner::new();
        owner.with(|| {
            let storage = web_sys::window()
                .and_then(|window| window.local_storage().ok().flatten())
                .expect("localStorage should be available");
            let _ = storage.remove_item(theme::STORAGE_KEY);

            let ctx = AppContext::new();
            let services = RuntimeServices::new(ctx);
            let theme_id = services.set_theme("dracula").expect("theme should apply");
            let document = web_sys::window()
                .and_then(|window| window.document())
                .expect("document should be available");
            let root = document
                .document_element()
                .expect("documentElement should be available");
            let meta = document
                .query_selector(r#"meta[name="theme-color"]"#)
                .ok()
                .flatten();

            assert_eq!(theme_id, "dracula");
            assert_eq!(ctx.theme.get_untracked(), "dracula");
            assert_eq!(root.get_attribute("data-theme").as_deref(), Some("dracula"));
            if let Some(meta) = meta {
                assert_eq!(meta.get_attribute("content").as_deref(), Some("#282a36"));
            }
            assert_eq!(
                storage.get_item(theme::STORAGE_KEY).unwrap().as_deref(),
                Some("dracula")
            );
            assert_eq!(
                ctx.runtime_state
                    .get_untracked()
                    .env
                    .get("THEME")
                    .map(String::as_str),
                Some("dracula")
            );

            let _ = storage.remove_item(theme::STORAGE_KEY);
        });
    }
    #[wasm_bindgen_test]
    fn latest_root_dispatch_controls_both_success_and_failure() {
        let owner = Owner::new();
        owner.with(|| {
            let ctx = AppContext::new();
            let services = RuntimeServices::new(ctx);
            let stale = ctx.begin_root_request();
            let current = ctx.begin_root_request();
            let mut load = loader::bootstrap_runtime_load();
            let root = VirtualPath::root();
            let declared = load.mounts.declared(&root).unwrap();
            load.mounts.insert_loaded(declared, 7);
            services
                .finish_root_load(current, Ok(load.clone()))
                .unwrap();
            let generation = ctx.runtime_generation();
            services
                .finish_root_load(stale, Ok(loader::bootstrap_runtime_load()))
                .unwrap();
            let failure = || crate::runtime::RuntimeLoadError::BootstrapMount {
                label: "~".into(),
                source: websh_core::ports::StorageError::Network {
                    message: "offline".into(),
                },
            };
            services.finish_root_load(stale, Err(failure())).unwrap();
            assert_eq!(ctx.runtime_generation(), generation);
            assert!(matches!(
                ctx.mount_status_for(&root),
                Some(MountLoadStatus::Available {
                    total_files: 7,
                    refresh: crate::runtime::mounts::RefreshState::Idle,
                    ..
                })
            ));
            let latest = ctx.begin_root_request();
            assert!(services.finish_root_load(latest, Err(failure())).is_err());
            assert_eq!(ctx.runtime_generation(), generation);
            assert!(ctx.mount_is_loaded(&root));
        });
    }
}
