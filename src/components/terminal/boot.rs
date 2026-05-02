//! Boot sequence logic
//!
//! Handles the initial terminal animation and applies the pure runtime loader.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::app::AppContext;
use crate::config::{APP_NAME, APP_TAGLINE, APP_VERSION, ASCII_BANNER, boot_delays};
use crate::core::{env, runtime, wallet};
use crate::models::{OutputLine, ViewMode, WalletState};
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
/// 2. Fetching and mounting the runtime filesystem
/// 3. Restoring wallet session if available
/// 4. Displaying the welcome banner
/// 5. Setting the initial view mode based on device type
pub fn run(ctx: AppContext) {
    spawn_local(async move {
        let window = web_sys::window().expect("Boot sequence requires browser environment");
        let start = js_sys::Date::now();
        let elapsed = || js_sys::Date::now() - start;

        env::init_defaults();
        ctx.runtime_state.set(runtime::state::snapshot());

        ctx.terminal.push_output(OutputLine::info(format!(
            "{} Booting websh kernel v{}",
            format_elapsed(elapsed()),
            APP_VERSION
        )));
        delay(&window, boot_delays::KERNEL_INIT).await;

        ctx.terminal.push_output(OutputLine::success(format!(
            "{} WASM runtime initialized",
            format_elapsed(elapsed())
        )));
        delay(&window, boot_delays::WASM_RUNTIME).await;

        ctx.terminal.push_output(OutputLine::text(format!(
            "{} Mounting filesystems...",
            format_elapsed(elapsed())
        )));

        match runtime::reload_runtime().await {
            Ok(load) => {
                let total_files = load.total_files;
                let mount_errors = load.mount_errors.clone();
                ctx.apply_runtime_load(load);
                ctx.terminal.push_output(OutputLine::success(format!(
                    "{} Total: {} files mounted",
                    format_elapsed(elapsed()),
                    total_files
                )));
                for failure in mount_errors {
                    ctx.terminal.push_output(OutputLine::error(format!(
                        "{} mount {} unavailable: {}",
                        format_elapsed(elapsed()),
                        failure.label,
                        failure.error
                    )));
                }
            }
            Err(error) => {
                ctx.apply_runtime_load(runtime::bootstrap_runtime_load());
                ctx.terminal.push_output(OutputLine::error(format!(
                    "{} Failed to mount filesystems: {}",
                    format_elapsed(elapsed()),
                    error
                )));
            }
        }

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

                    let chain_id = wallet::get_chain_id().await;
                    if let Some(id) = chain_id {
                        ctx.terminal.push_output(OutputLine::info(format!(
                            "{} Network: {} (chain_id={})",
                            format_elapsed(elapsed()),
                            wallet::chain_name(id),
                            id
                        )));
                    }

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
                    match runtime::state::set_wallet_session(true) {
                        Ok(snapshot) => ctx.runtime_state.set(snapshot),
                        Err(error) => ctx.terminal.push_output(OutputLine::error(format!(
                            "wallet: failed to persist session: {error}"
                        ))),
                    }
                }
                None => {
                    match wallet::clear_session() {
                        Ok(snapshot) => ctx.runtime_state.set(snapshot),
                        Err(error) => ctx.terminal.push_output(OutputLine::error(format!(
                            "wallet: failed to clear session: {error}"
                        ))),
                    }
                    ctx.terminal.push_output(OutputLine::text(format!(
                        "{} Wallet session expired",
                        format_elapsed(elapsed())
                    )));
                }
            }
        }

        ctx.view_mode.set(ViewMode::Terminal);
        ctx.terminal.push_output(OutputLine::info(format!(
            "{} Initializing Terminal mode",
            format_elapsed(elapsed())
        )));
        delay(&window, boot_delays::BOOT_COMPLETE).await;

        ctx.terminal.push_output(OutputLine::success(format!(
            "{} Boot complete. Welcome to {}",
            format_elapsed(elapsed()),
            APP_NAME
        )));

        ctx.terminal.push_output(OutputLine::empty());
        ctx.terminal.push_output(OutputLine::ascii(ASCII_BANNER));
        ctx.terminal.push_output(OutputLine::empty());
        ctx.terminal.push_output(OutputLine::info(APP_TAGLINE));
        ctx.terminal.push_output(OutputLine::empty());
        ctx.terminal.push_output(OutputLine::text("Tips:"));
        ctx.terminal
            .push_output(OutputLine::text("  - Type 'help' for available commands"));
        ctx.terminal.push_output(OutputLine::text(
            "  - Use the archive bar to switch between websh and explorer",
        ));
        ctx.terminal.push_output(OutputLine::empty());
    });
}
