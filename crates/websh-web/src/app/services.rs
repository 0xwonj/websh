//! Application-owned runtime services.

use wasm_bindgen_futures::spawn_local;
use websh_core::attestation::ledger::CONTENT_LEDGER_CONTENT_PATH;
use websh_core::domain::VirtualPath;

use crate::render::theme;
use crate::runtime::loader;

use super::{AppContext, ThemeError};
use crate::runtime::EnvironmentError;
use crate::runtime::loader::RuntimeLoad;
use crate::runtime::mounts::{MountLoadStatus, MountScanJob};
use crate::runtime::{RuntimeError, RuntimeResult};

#[derive(Clone, Copy)]
pub struct RuntimeServices {
    ctx: AppContext,
}

impl RuntimeServices {
    pub fn new(ctx: AppContext) -> Self {
        Self { ctx }
    }

    pub fn init_default_env(&self) {
        self.ctx.preferences.init_language();
    }

    pub fn set_env_var(&self, key: &str, value: &str) -> Result<(), EnvironmentError> {
        self.ctx.preferences.set_env(key, value)
    }

    pub fn unset_env_var(&self, key: &str) -> Result<(), EnvironmentError> {
        self.ctx.preferences.unset_env(key)
    }

    pub fn set_theme(&self, raw_theme: &str) -> Result<&'static str, ThemeError> {
        let Some(theme_id) = theme::normalize_theme_id(raw_theme) else {
            return Err(ThemeError::unknown(raw_theme));
        };
        self.ctx
            .preferences
            .set_env("THEME", theme_id)
            .map_err(|source| ThemeError::Persist { theme_id, source })?;
        Ok(theme_id)
    }

    pub async fn reload_runtime(&self) -> RuntimeResult {
        let sequence = self.ctx.content.begin_root_request();
        let result = loader::load_runtime().await;
        self.finish_root_load(sequence, result)
    }

    fn finish_root_load(
        &self,
        sequence: u64,
        result: Result<RuntimeLoad, crate::runtime::RuntimeLoadError>,
    ) -> RuntimeResult {
        // Both successful and failed older requests are obsolete at the same boundary.
        if !self.ctx.content.accepts_root_request(sequence) {
            return Ok(());
        }
        match result {
            Ok(load) => {
                let jobs = load.mounts.scan_jobs.clone();
                let generation = self.ctx.content.apply_runtime_load(load);
                self.start_ledger_prefetch(generation);
                self.start_mount_scans(generation, jobs);
                Ok(())
            }
            Err(error) => {
                let _ = self
                    .ctx
                    .content
                    .mark_mount_failed(&VirtualPath::root(), error.to_string());
                Err(error.into())
            }
        }
    }

    pub async fn reload_runtime_mount(&self, mount_root: VirtualPath) -> RuntimeResult {
        if mount_root.is_root() {
            return self.reload_runtime().await;
        }

        let backend = self
            .ctx
            .content
            .backend_for_mount_root(&mount_root)
            .ok_or_else(|| RuntimeError::NoBackend {
                mount_root: mount_root.clone(),
            })?;
        let generation = self.ctx.content.runtime_generation();
        let (declared_mount, epoch) = self.ctx.content.mark_mount_loading(&mount_root)?;
        crate::runtime::mount_refresh::refresh_mount(
            self.ctx.content,
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
                    crate::runtime::mount_refresh::refresh_mount(ctx.content, generation, job).await
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
            ctx.content.mount_status_for(&root),
            Some(MountLoadStatus::Available { .. })
        ) {
            return;
        }
        if !ctx.content.with_fs_untracked(|fs| fs.exists(&path)) {
            return;
        }

        spawn_local(async move {
            if ctx.content.runtime_generation() != generation {
                return;
            }

            let _ = ctx.read_text(&path).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use leptos::prelude::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    async fn set_theme_persists_user_theme_and_runtime_snapshot() {
        super::super::boot::init_test_renderer();
        let owner = Owner::new();
        let ctx = owner.with(|| {
            let storage = web_sys::window()
                .and_then(|window| window.local_storage().ok().flatten())
                .expect("localStorage should be available");
            let _ = storage.remove_item(theme::STORAGE_KEY);

            let ctx = AppContext::new();
            super::super::boot::install_theme_effect(ctx);
            let services = RuntimeServices::new(ctx);
            let theme_id = services.set_theme("dracula").expect("theme should apply");

            assert_eq!(theme_id, "dracula");
            assert_eq!(ctx.theme.get_untracked(), "dracula");
            assert_eq!(
                storage.get_item(theme::STORAGE_KEY).unwrap().as_deref(),
                Some("dracula")
            );
            assert_eq!(
                ctx.preferences
                    .snapshot
                    .get_untracked()
                    .env
                    .get("THEME")
                    .map(String::as_str),
                Some("dracula")
            );

            ctx
        });
        gloo_timers::future::TimeoutFuture::new(0).await;
        let document = web_sys::window().unwrap().document().unwrap();
        assert_eq!(
            document
                .document_element()
                .unwrap()
                .get_attribute("data-theme")
                .as_deref(),
            Some("dracula")
        );
        RuntimeServices::new(ctx)
            .set_env_var("THEME", "nord")
            .unwrap();
        gloo_timers::future::TimeoutFuture::new(0).await;
        assert_eq!(ctx.theme.get_untracked(), "nord");
        assert_eq!(
            document
                .document_element()
                .unwrap()
                .get_attribute("data-theme")
                .as_deref(),
            Some("nord")
        );
        RuntimeServices::new(ctx).unset_env_var("THEME").unwrap();
        gloo_timers::future::TimeoutFuture::new(0).await;
        assert_eq!(ctx.theme.get_untracked(), theme::DEFAULT_THEME);
        owner.cleanup();
    }
    #[wasm_bindgen_test]
    fn latest_root_dispatch_controls_both_success_and_failure() {
        let owner = Owner::new();
        owner.with(|| {
            let ctx = AppContext::new();
            let services = RuntimeServices::new(ctx);
            let stale = ctx.content.begin_root_request();
            let current = ctx.content.begin_root_request();
            let mut load = loader::bootstrap_runtime_load();
            let root = VirtualPath::root();
            let declared = load.mounts.declared(&root).unwrap();
            load.mounts.insert_loaded(declared, 7);
            services
                .finish_root_load(current, Ok(load.clone()))
                .unwrap();
            let generation = ctx.content.runtime_generation();
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
            assert_eq!(ctx.content.runtime_generation(), generation);
            assert!(matches!(
                ctx.content.mount_status_for(&root),
                Some(MountLoadStatus::Available {
                    total_files: 7,
                    refresh: crate::runtime::mounts::RefreshState::Idle,
                    ..
                })
            ));
            let latest = ctx.content.begin_root_request();
            assert!(services.finish_root_load(latest, Err(failure())).is_err());
            assert_eq!(ctx.content.runtime_generation(), generation);
            assert!(ctx.content.mount_is_loaded(&root));
        });
    }
}
