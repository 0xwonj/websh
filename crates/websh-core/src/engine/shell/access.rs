//! Admin eligibility supplied by the embedding target.

use crate::domain::WalletState;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdminStatus {
    NotConnected,
    Connected { address: String },
    Admin { address: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessPolicy {
    admin_addresses: &'static [&'static str],
}

impl AccessPolicy {
    pub const fn new(admin_addresses: &'static [&'static str]) -> Self {
        Self { admin_addresses }
    }

    pub const fn empty() -> Self {
        Self::new(&[])
    }

    pub fn admin_status(&self, wallet: &WalletState) -> AdminStatus {
        match wallet {
            WalletState::Connected { address, .. } => {
                if self
                    .admin_addresses
                    .iter()
                    .any(|admin| admin.eq_ignore_ascii_case(address))
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

    pub fn can_write_to(&self, wallet: &WalletState, writable: bool) -> bool {
        matches!(self.admin_status(wallet), AdminStatus::Admin { .. }) && writable
    }
}

impl Default for AccessPolicy {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub(crate) const ADMIN_ADDRESS: &str = "0x0000000000000000000000000000000000000001";
    pub(crate) const ACCESS_POLICY: AccessPolicy = AccessPolicy::new(&[ADMIN_ADDRESS]);
}

#[cfg(test)]
mod tests {
    use super::test_support::{ACCESS_POLICY, ADMIN_ADDRESS};
    use super::*;

    #[test]
    fn disconnected_is_not_admin() {
        assert_eq!(
            ACCESS_POLICY.admin_status(&WalletState::Disconnected),
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
        assert!(matches!(
            ACCESS_POLICY.admin_status(&w),
            AdminStatus::Connected { .. }
        ));
    }

    #[test]
    fn allowlisted_is_admin() {
        let w = WalletState::Connected {
            address: ADMIN_ADDRESS.to_ascii_uppercase(),
            ens_name: None,
            chain_id: Some(1),
        };
        assert!(matches!(
            ACCESS_POLICY.admin_status(&w),
            AdminStatus::Admin { .. }
        ));
    }

    #[test]
    fn can_write_requires_both() {
        let admin = WalletState::Connected {
            address: ADMIN_ADDRESS.to_string(),
            ens_name: None,
            chain_id: Some(1),
        };
        assert!(ACCESS_POLICY.can_write_to(&admin, true));
        assert!(!ACCESS_POLICY.can_write_to(&admin, false));
    }

    #[test]
    fn default_policy_has_no_admins() {
        let admin = WalletState::Connected {
            address: ADMIN_ADDRESS.to_string(),
            ens_name: None,
            chain_id: Some(1),
        };
        assert!(matches!(
            AccessPolicy::default().admin_status(&admin),
            AdminStatus::Connected { .. }
        ));
    }
}
