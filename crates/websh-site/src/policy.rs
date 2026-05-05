//! Deployed admin policy.

use websh_core::shell::AccessPolicy;

pub const ADMIN_ADDRESSES: &[&str] = &["0x2c4b04a4aeb6e18c2f8a5c8b4a3f62c0cf33795a"];
pub const ACCESS_POLICY: AccessPolicy = AccessPolicy::new(ADMIN_ADDRESSES);
