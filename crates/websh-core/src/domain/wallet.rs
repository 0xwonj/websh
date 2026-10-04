use crate::support::format::format_eth_address;
use serde::{Deserialize, Serialize};

/// Convert an EIP-155 chain id to its network name.
pub fn chain_name(chain_id: u64) -> &'static str {
    match chain_id {
        1 => "Ethereum",
        11155111 => "Sepolia",
        17000 => "Holesky",
        42161 => "Arbitrum",
        10 => "Optimism",
        8453 => "Base",
        137 => "Polygon",
        56 => "BNB Chain",
        43114 => "Avalanche",
        324 => "zkSync Era",
        59144 => "Linea",
        534352 => "Scroll",
        _ => "Unknown",
    }
}

/// Wallet connection state
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
            WalletState::Connected { address, .. } => format_eth_address(address),
            WalletState::Connecting => "connecting...".to_string(),
            WalletState::Disconnected => "guest".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_tracks_connection_and_prefers_ens() {
        for (state, expected) in [
            (WalletState::Disconnected, "guest"),
            (WalletState::Connecting, "connecting..."),
            (
                WalletState::Connected {
                    address: "0x1234567890123456789012345678901234567890".into(),
                    ens_name: None,
                    chain_id: Some(1),
                },
                "0x1234...7890",
            ),
            (
                WalletState::Connected {
                    address: "0x1234567890123456789012345678901234567890".into(),
                    ens_name: Some("reader.eth".into()),
                    chain_id: Some(1),
                },
                "reader.eth",
            ),
        ] {
            assert_eq!(state.display_name(), expected, "{state:?}");
        }
    }
}
