//! Boot sequence logic
//!
//! Handles the initial boot animation and system initialization.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::app::AppContext;
use crate::config::{APP_NAME, APP_TAGLINE, APP_VERSION, ASCII_BANNER, boot_delays, cache};
use crate::core::{VirtualFs, env, wallet};
use crate::models::{Manifest, OutputLine, ViewMode, WalletState};
use crate::utils::dom::is_mobile_or_tablet;
use crate::utils::fetch_json_cached;
use crate::utils::format::{format_elapsed, format_eth_address};

/// Delay helper using setTimeout
async fn delay(window: &web_sys::Window, ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Run the boot sequence
///
/// Initializes the application by:
/// 1. Booting the kernel and WASM runtime
/// 2. Fetching and mounting the virtual filesystem
/// 3. Restoring wallet session if available
/// 4. Displaying the welcome banner
/// 5. Setting the initial view mode based on device type
pub fn run(ctx: AppContext) {
    spawn_local(async move {
        let window = web_sys::window().expect("Boot sequence requires browser environment");
        let start = js_sys::Date::now();
        let elapsed = || js_sys::Date::now() - start;

        // Initialize default environment variables
        env::init_defaults();

        // Kernel init
        ctx.terminal.push_output(OutputLine::info(format!(
            "{} Booting websh kernel v{}",
            format_elapsed(elapsed()),
            APP_VERSION
        )));
        delay(&window, boot_delays::KERNEL_INIT).await;

        // WASM runtime
        ctx.terminal.push_output(OutputLine::success(format!(
            "{} WASM runtime initialized",
            format_elapsed(elapsed())
        )));
        delay(&window, boot_delays::WASM_RUNTIME).await;

        // Mount filesystems from registry
        ctx.terminal.push_output(OutputLine::text(format!(
            "{} Mounting filesystems...",
            format_elapsed(elapsed())
        )));

        // Fetch manifests for all configured mounts
        let mounts = ctx.mounts.get_value();
        let mut combined_manifest = Manifest {
            files: Vec::new(),
            directories: Vec::new(),
        };
        let mut mount_errors = Vec::new();

        for mount in mounts.all() {
            let manifest_url = mount.manifest_url();
            let cache_key = format!("{}_{}", cache::MANIFEST_KEY, mount.alias());

            match fetch_json_cached::<Manifest>(&manifest_url, &cache_key).await {
                Ok(manifest) => {
                    let file_count = manifest.files.len();
                    combined_manifest.files.extend(manifest.files);
                    combined_manifest.directories.extend(manifest.directories);
                    ctx.terminal.push_output(OutputLine::success(format!(
                        "{} Mounted '{}' ({} files)",
                        format_elapsed(elapsed()),
                        mount.alias(),
                        file_count
                    )));
                }
                Err(e) => {
                    mount_errors.push((mount.alias().to_string(), e.to_string()));
                    ctx.terminal.push_output(OutputLine::error(format!(
                        "{} Failed to mount '{}': {}",
                        format_elapsed(elapsed()),
                        mount.alias(),
                        e
                    )));
                }
            }
        }

        // Build filesystem from manifest
        if !combined_manifest.files.is_empty() {
            let total_files = combined_manifest.files.len();
            ctx.fs.set(VirtualFs::from_manifest(&combined_manifest));
            ctx.terminal.push_output(OutputLine::success(format!(
                "{} Total: {} files mounted",
                format_elapsed(elapsed()),
                total_files
            )));
        } else if mount_errors.is_empty() {
            ctx.terminal.push_output(OutputLine::text(format!(
                "{} No mounts configured",
                format_elapsed(elapsed())
            )));
            ctx.fs.set(VirtualFs::empty());
        } else {
            ctx.fs.set(VirtualFs::empty());
        }

        // Check wallet connection (only if previously logged in)
        if wallet::is_available() && wallet::has_session() {
            ctx.terminal.push_output(OutputLine::text(format!(
                "{} Restoring wallet session...",
                format_elapsed(elapsed())
            )));

            match wallet::get_account().await {
                Some(address) => {
                    let short_addr = format_eth_address(&address);
                    ctx.terminal.push_output(OutputLine::success(format!(
                        "{} Connected: {}",
                        format_elapsed(elapsed()),
                        short_addr
                    )));

                    // Get chain ID
                    let chain_id = wallet::get_chain_id().await;
                    if let Some(id) = chain_id {
                        ctx.terminal.push_output(OutputLine::info(format!(
                            "{} Network: {} (chain_id={})",
                            format_elapsed(elapsed()),
                            wallet::chain_name(id),
                            id
                        )));
                    }

                    // Resolve ENS
                    let ens_name = wallet::resolve_ens(&address).await;
                    if let Some(ref name) = ens_name {
                        ctx.terminal.push_output(OutputLine::success(format!(
                            "{} ENS resolved: {}",
                            format_elapsed(elapsed()),
                            name
                        )));
                    }

                    ctx.wallet.set(WalletState::Connected {
                        address,
                        ens_name,
                        chain_id,
                    });
                }
                None => {
                    // Session exists but wallet not connected, clear stale session
                    wallet::clear_session();
                    ctx.terminal.push_output(OutputLine::text(format!(
                        "{} Wallet session expired",
                        format_elapsed(elapsed())
                    )));
                }
            }
        }

        // Detect device type and set initial view mode
        let is_mobile = is_mobile_or_tablet();
        if is_mobile {
            ctx.view_mode.set(ViewMode::Explorer);
            ctx.terminal.push_output(OutputLine::info(format!(
                "{} Mobile device detected, switching to Explorer mode",
                format_elapsed(elapsed())
            )));
        } else {
            ctx.view_mode.set(ViewMode::Terminal);
            ctx.terminal.push_output(OutputLine::info(format!(
                "{} Desktop detected, initializing Terminal mode",
                format_elapsed(elapsed())
            )));
        }
        delay(&window, boot_delays::BOOT_COMPLETE).await;

        // Boot complete
        ctx.terminal.push_output(OutputLine::success(format!(
            "{} Boot complete. Welcome to {}",
            format_elapsed(elapsed()),
            APP_NAME
        )));

        // Banner and info
        ctx.terminal.push_output(OutputLine::empty());
        ctx.terminal.push_output(OutputLine::ascii(ASCII_BANNER));
        ctx.terminal.push_output(OutputLine::empty());
        ctx.terminal.push_output(OutputLine::info(APP_TAGLINE));
        ctx.terminal.push_output(OutputLine::empty());
        ctx.terminal.push_output(OutputLine::text("Tips:"));
        ctx.terminal
            .push_output(OutputLine::text("  - Type 'help' for available commands"));
        ctx.terminal.push_output(OutputLine::text(
            "  - Use the view toggle (top-right) to switch to Explorer mode",
        ));
        ctx.terminal.push_output(OutputLine::empty());
    });
}
