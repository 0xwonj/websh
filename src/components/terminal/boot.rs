//! Boot sequence logic
//!
//! Handles the initial boot animation and system initialization.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::app::AppContext;
use crate::config::{
    APP_NAME, APP_TAGLINE, APP_VERSION, ASCII_BANNER, CONTENT_BASE_URL, MS_PER_SECOND,
    boot_delays, cache, eth_address,
};
use crate::core::{VirtualFs, env, wallet};
use crate::models::{ManifestEntry, OutputLine, Route, ScreenMode, WalletState};
use crate::utils::fetch_json_cached;

/// Format an Ethereum address for display (0x1234...5678)
fn format_short_address(address: &str) -> String {
    if address.len() >= eth_address::FULL_LEN {
        format!(
            "{}...{}",
            &address[..eth_address::PREFIX_LEN],
            &address[eth_address::SUFFIX_START..]
        )
    } else {
        address.to_string()
    }
}

/// Delay helper using setTimeout
async fn delay(window: &web_sys::Window, ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Format elapsed time for boot messages
fn format_time(ms: f64) -> String {
    format!("[{:8.3}]", ms / MS_PER_SECOND)
}

/// Run the boot sequence
///
/// Initializes the application by:
/// 1. Booting the kernel and WASM runtime
/// 2. Fetching and mounting the virtual filesystem
/// 3. Restoring wallet session if available
/// 4. Displaying the welcome banner
/// 5. Navigating to the initial route
pub fn run(ctx: AppContext, initial_route: Route) {
    spawn_local(async move {
        let window = web_sys::window().expect("Boot sequence requires browser environment");
        let start = js_sys::Date::now();
        let elapsed = || js_sys::Date::now() - start;

        // Initialize default environment variables
        env::init_defaults();

        // Kernel init
        ctx.terminal.push_output(OutputLine::info(format!(
            "{} Booting {} kernel v{}",
            format_time(elapsed()),
            APP_NAME,
            APP_VERSION
        )));
        delay(&window, boot_delays::KERNEL_INIT).await;

        // WASM runtime
        ctx.terminal.push_output(OutputLine::success(format!(
            "{} WASM runtime initialized",
            format_time(elapsed())
        )));
        delay(&window, boot_delays::WASM_RUNTIME).await;

        // Mount filesystem
        ctx.terminal.push_output(OutputLine::text(format!(
            "{} Mounting filesystem...",
            format_time(elapsed())
        )));

        let manifest_url = format!("{}/manifest.json", CONTENT_BASE_URL);
        match fetch_json_cached::<Vec<ManifestEntry>>(&manifest_url, cache::MANIFEST_KEY).await {
            Ok(entries) => {
                ctx.terminal.push_output(OutputLine::success(format!(
                    "{} Mounted {} file entries",
                    format_time(elapsed()),
                    entries.len()
                )));
                ctx.fs.set(VirtualFs::from_manifest(&entries));
            }
            Err(e) => {
                ctx.terminal.push_output(OutputLine::error(format!(
                    "{} Mount failed: {}",
                    format_time(elapsed()),
                    e
                )));
                ctx.fs.set(VirtualFs::empty());
            }
        }

        // Check wallet connection (only if previously logged in)
        if wallet::is_available() && wallet::has_session() {
            ctx.terminal.push_output(OutputLine::text(format!(
                "{} Restoring wallet session...",
                format_time(elapsed())
            )));

            match wallet::get_account().await {
                Some(address) => {
                    let short_addr = format_short_address(&address);
                    ctx.terminal.push_output(OutputLine::success(format!(
                        "{} Connected: {}",
                        format_time(elapsed()),
                        short_addr
                    )));

                    // Get chain ID
                    let chain_id = wallet::get_chain_id().await;
                    if let Some(id) = chain_id {
                        ctx.terminal.push_output(OutputLine::info(format!(
                            "{} Network: {} (chain_id={})",
                            format_time(elapsed()),
                            wallet::chain_name(id),
                            id
                        )));
                    }

                    // Resolve ENS
                    let ens_name = wallet::resolve_ens(&address).await;
                    if let Some(ref name) = ens_name {
                        ctx.terminal.push_output(OutputLine::success(format!(
                            "{} ENS resolved: {}",
                            format_time(elapsed()),
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
                        format_time(elapsed())
                    )));
                }
            }
        }

        // Boot complete
        ctx.terminal.push_output(OutputLine::success(format!(
            "{} Boot complete. Welcome to {}",
            format_time(elapsed()),
            APP_NAME
        )));
        delay(&window, boot_delays::BOOT_COMPLETE).await;

        // Banner and info
        ctx.terminal.push_output(OutputLine::empty());
        ctx.terminal.push_output(OutputLine::ascii(ASCII_BANNER));
        ctx.terminal.push_output(OutputLine::empty());
        ctx.terminal.push_output(OutputLine::info(APP_TAGLINE));
        ctx.terminal.push_output(OutputLine::empty());
        ctx.terminal
            .push_output(OutputLine::text("Type 'help' for available commands."));
        ctx.terminal.push_output(OutputLine::empty());

        // Navigate based on initial URL
        match initial_route {
            Route::Home => {
                ctx.terminal.screen_mode.set(ScreenMode::Terminal);
            }
            Route::Read { path } => {
                let title = path
                    .rsplit('/')
                    .next()
                    .and_then(|f| f.rsplit_once('.'))
                    .map(|(name, _)| name.to_string())
                    .unwrap_or_else(|| path.clone());
                ctx.terminal.screen_mode.set(ScreenMode::Reader {
                    content: path,
                    title,
                });
            }
        }
    });
}
