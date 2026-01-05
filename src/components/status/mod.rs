use leptos::prelude::*;

use crate::app::AppContext;
use crate::core::wallet;

stylance::import_crate_style!(css, "src/components/status/status.module.css");

/// Status bar component displaying session, location, and network information.
///
/// This component uses the application context to reactively display:
/// - Session: Current wallet connection status (ENS name, address, or "guest")
/// - Location: Current directory path in the virtual filesystem
/// - Network: Connected blockchain network (if wallet is connected)
#[component]
pub fn Status() -> impl IntoView {
    // Get context - Status bar needs access to both terminal and wallet state
    let ctx = use_context::<AppContext>().expect("AppContext must be provided at root");

    let display_path = Signal::derive(move || ctx.terminal.current_path.with(|p| p.display()));
    let session_name = Signal::derive(move || ctx.wallet.with(|w| w.display_name()));
    let network_name = Signal::derive(move || {
        ctx.wallet.with(|w| {
            w.chain_id()
                .map(|id| wallet::chain_name(id).to_string())
                .unwrap_or_else(|| "—".to_string())
        })
    });

    view! {
        <header class=css::bar>
            <div class=css::section>
                <span class=css::label>
                    "Session: "
                    <span class=css::value>{session_name}</span>
                </span>
                <span class=css::labelCyan>
                    "Location: "
                    <span class=css::value>{display_path}</span>
                </span>
                <span class=css::labelPurple>
                    "Network: "
                    <span class=css::value>{network_name}</span>
                </span>
            </div>
            <div class=css::brand>
                "wonjae.eth"
            </div>
        </header>
    }
}
