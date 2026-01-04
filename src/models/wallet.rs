use crate::config::eth_address;

/// Wallet connection state
#[derive(Clone, Debug, Default, PartialEq)]
pub enum WalletState {
    #[default]
    Disconnected,
    Connecting,
    Connected {
        address: String,
        ens_name: Option<String>,
        chain_id: Option<u64>,
    },
}

impl WalletState {
    /// Check if wallet is connected
    pub fn is_connected(&self) -> bool {
        matches!(self, WalletState::Connected { .. })
    }

    /// Get chain ID if connected
    pub fn chain_id(&self) -> Option<u64> {
        match self {
            WalletState::Connected { chain_id, .. } => *chain_id,
            _ => None,
        }
    }

    /// Format address for display (ENS name or 0x1234...5678)
    pub fn display_name(&self) -> String {
        match self {
            WalletState::Connected {
                ens_name: Some(name),
                ..
            } => name.clone(),
            WalletState::Connected { address, .. } if address.len() >= eth_address::FULL_LEN => {
                format!(
                    "{}...{}",
                    &address[..eth_address::PREFIX_LEN],
                    &address[eth_address::SUFFIX_START..]
                )
            }
            WalletState::Connected { address, .. } => address.clone(),
            WalletState::Connecting => "connecting...".to_string(),
            WalletState::Disconnected => "guest".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disconnected_state() {
        let state = WalletState::Disconnected;
        assert!(!state.is_connected());
        assert_eq!(state.chain_id(), None);
        assert_eq!(state.display_name(), "guest");
    }

    #[test]
    fn test_connecting_state() {
        let state = WalletState::Connecting;
        assert!(!state.is_connected());
        assert_eq!(state.chain_id(), None);
        assert_eq!(state.display_name(), "connecting...");
    }

    #[test]
    fn test_connected_with_ens() {
        let state = WalletState::Connected {
            address: "0x1234567890123456789012345678901234567890".to_string(),
            ens_name: Some("vitalik.eth".to_string()),
            chain_id: Some(1),
        };
        assert!(state.is_connected());
        assert_eq!(state.chain_id(), Some(1));
        assert_eq!(state.display_name(), "vitalik.eth");
    }

    #[test]
    fn test_connected_without_ens() {
        let state = WalletState::Connected {
            address: "0x1234567890123456789012345678901234567890".to_string(),
            ens_name: None,
            chain_id: Some(137),
        };
        assert!(state.is_connected());
        assert_eq!(state.chain_id(), Some(137));
        assert_eq!(state.display_name(), "0x1234...7890");
    }

    #[test]
    fn test_connected_short_address() {
        let state = WalletState::Connected {
            address: "0x1234".to_string(),
            ens_name: None,
            chain_id: None,
        };
        assert!(state.is_connected());
        assert_eq!(state.chain_id(), None);
        assert_eq!(state.display_name(), "0x1234");
    }

    #[test]
    fn test_default() {
        let state = WalletState::default();
        assert_eq!(state, WalletState::Disconnected);
    }
}
