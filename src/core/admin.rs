//! Admin eligibility. See spec §8.2.

use crate::models::{Mount, WalletState};

/// Hard-coded allowlist. Single-admin model per design.
///
/// Store lowercased. `admin_status` compares case-insensitively.
const ADMIN_ADDRESSES: &[&str] = &["0x2c4b04a4aeb6e18c2f8a5c8b4a3f62c0cf33795a"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdminStatus {
    NotConnected,
    Connected { address: String },
    Admin { address: String },
}

pub fn admin_status(wallet: &WalletState) -> AdminStatus {
    match wallet {
        WalletState::Connected { address, .. } => {
            if ADMIN_ADDRESSES
                .iter()
                .any(|a| a.eq_ignore_ascii_case(address))
            {
                AdminStatus::Admin {
                    address: address.clone(),
                }
            } else {
                AdminStatus::Connected {
                    address: address.clone(),
                }
            }
        }
        _ => AdminStatus::NotConnected,
    }
}

pub fn can_write_to(wallet: &WalletState, mount: &Mount) -> bool {
    matches!(admin_status(wallet), AdminStatus::Admin { .. }) && mount.is_writable()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnected_is_not_admin() {
        assert_eq!(
            admin_status(&WalletState::Disconnected),
            AdminStatus::NotConnected
        );
    }

    #[test]
    fn non_allowlisted_connected_is_not_admin() {
        let w = WalletState::Connected {
            address: "0xdeadbeef".to_string(),
            ens_name: None,
            chain_id: Some(1),
        };
        assert!(matches!(admin_status(&w), AdminStatus::Connected { .. }));
    }

    #[test]
    fn allowlisted_is_admin() {
        let w = WalletState::Connected {
            address: ADMIN_ADDRESSES[0].to_ascii_uppercase(),
            ens_name: None,
            chain_id: Some(1),
        };
        assert!(matches!(admin_status(&w), AdminStatus::Admin { .. }));
    }

    #[test]
    fn can_write_requires_both() {
        let admin = WalletState::Connected {
            address: ADMIN_ADDRESSES[0].to_string(),
            ens_name: None,
            chain_id: Some(1),
        };
        let writable = Mount::github_writable("~", "https://x", "~");
        let readonly = Mount::github_with_prefix("ro", "https://y", "~");
        assert!(can_write_to(&admin, &writable));
        assert!(!can_write_to(&admin, &readonly));
    }
}
