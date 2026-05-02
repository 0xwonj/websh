//! UI-side wallet orchestration. Wraps the pure wallet primitives in
//! [`websh_core::runtime::wallet`] with Leptos signal updates that
//! mirror connection state into [`AppContext`].

use leptos::prelude::Set;

use crate::app::AppContext;
use websh_core::runtime::state::EnvironmentError;
use websh_core::runtime::wallet::{
    ConnectOutcome, WalletError, connect, get_chain_id, is_available, resolve_ens, save_session,
};
use websh_core::domain::WalletState;

/// Disconnect the wallet: clear the stored session and reset
/// `AppContext.wallet` to `Disconnected`.
pub fn disconnect(ctx: &AppContext) -> Result<(), EnvironmentError> {
    let snapshot = websh_core::runtime::wallet::clear_session()?;
    ctx.wallet.set(WalletState::Disconnected);
    ctx.runtime_state.set(snapshot);
    Ok(())
}

/// Run the canonical wallet connection flow and reflect each stage on
/// `AppContext.wallet`. UI surfaces (terminal, site chrome) call this and
/// then format their own user-facing feedback from the returned outcome.
///
/// State transitions:
/// - `Disconnected` → `Connecting` (popup shown)
/// - `Connecting` → `Connected { address, chain_id, ens_name: None }`
/// - then `Connected { ens_name: Some(..) }` if ENS resolves
/// - any error path → `Disconnected`
pub async fn connect_with_session(ctx: &AppContext) -> Result<ConnectOutcome, WalletError> {
    if !is_available() {
        return Err(WalletError::NotInstalled);
    }
    ctx.wallet.set(WalletState::Connecting);

    let address = match connect().await {
        Ok(addr) => addr,
        Err(err) => {
            ctx.wallet.set(WalletState::Disconnected);
            return Err(err);
        }
    };

    let session_persist_error = match save_session() {
        Ok(snapshot) => {
            ctx.runtime_state.set(snapshot);
            None
        }
        Err(err) => Some(err),
    };

    let chain_id = get_chain_id().await;
    ctx.wallet.set(WalletState::Connected {
        address: address.clone(),
        ens_name: None,
        chain_id,
    });

    let ens_name = resolve_ens(&address).await;
    if ens_name.is_some() {
        ctx.wallet.set(WalletState::Connected {
            address: address.clone(),
            ens_name: ens_name.clone(),
            chain_id,
        });
    }

    Ok(ConnectOutcome {
        address,
        chain_id,
        ens_name,
        session_persist_error,
    })
}
