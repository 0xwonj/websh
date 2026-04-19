//! Admin identification and authorization.
//!
//! Provides functions for checking if a wallet address has admin privileges.

use crate::config::ADMIN_ADDRESSES;
use crate::models::WalletState;

/// Check if the current wallet is an admin.
///
/// Compares the connected wallet address against the configured admin addresses
/// in a case-insensitive manner.
pub fn is_admin(wallet: &WalletState) -> bool {
    match wallet {
        WalletState::Connected { address, .. } => {
            let addr_lower = address.to_lowercase();
            ADMIN_ADDRESSES
                .iter()
                .any(|admin| admin.to_lowercase() == addr_lower)
        }
        _ => false,
    }
}

/// Admin status with details.
#[derive(Clone, Debug)]
pub enum AdminStatus {
    /// User is an admin with connected wallet.
    Admin {
        address: String,
        ens_name: Option<String>,
    },
    /// User has wallet connected but is not an admin.
    NotAdmin { address: String },
    /// Wallet is currently connecting.
    Connecting,
    /// No wallet connected.
    Disconnected,
}

impl AdminStatus {
    /// Check if this status represents an admin.
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Admin { .. })
    }

    /// Get the display name if available.
    pub fn display_name(&self) -> Option<String> {
        match self {
            Self::Admin {
                address, ens_name, ..
            } => Some(ens_name.clone().unwrap_or_else(|| format_address(address))),
            Self::NotAdmin { address } => Some(format_address(address)),
            _ => None,
        }
    }
}

/// Get detailed admin status from wallet state.
pub fn admin_status(wallet: &WalletState) -> AdminStatus {
    match wallet {
        WalletState::Connected {
            address, ens_name, ..
        } => {
            if is_admin(wallet) {
                AdminStatus::Admin {
                    address: address.clone(),
                    ens_name: ens_name.clone(),
                }
            } else {
                AdminStatus::NotAdmin {
                    address: address.clone(),
                }
            }
        }
        WalletState::Connecting => AdminStatus::Connecting,
        WalletState::Disconnected => AdminStatus::Disconnected,
    }
}

/// Format address as shortened form (0x1234...5678).
fn format_address(address: &str) -> String {
    if address.len() > 10 {
        format!("{}...{}", &address[..6], &address[address.len() - 4..])
    } else {
        address.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_admin_disconnected() {
        assert!(!is_admin(&WalletState::Disconnected));
    }

    #[test]
    fn test_is_admin_connecting() {
        assert!(!is_admin(&WalletState::Connecting));
    }

    #[test]
    fn test_is_admin_not_in_list() {
        let wallet = WalletState::Connected {
            address: "0xnotanadmin".to_string(),
            ens_name: None,
            chain_id: Some(1),
        };
        assert!(!is_admin(&wallet));
    }

    #[test]
    fn test_admin_status_disconnected() {
        let status = admin_status(&WalletState::Disconnected);
        assert!(matches!(status, AdminStatus::Disconnected));
        assert!(!status.is_admin());
    }

    #[test]
    fn test_format_address() {
        assert_eq!(
            format_address("0x1234567890abcdef1234567890abcdef12345678"),
            "0x1234...5678"
        );
        assert_eq!(format_address("0x123"), "0x123");
    }
}
