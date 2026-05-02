//! Ethereum EIP-191 personal-sign verification.

use std::str::FromStr;

use alloy_primitives::{Address, Signature};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EthVerification {
    pub expected_address: String,
    pub recovered_address: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum EthVerifyError {
    #[error("invalid address: {0}")]
    InvalidAddress(String),
    #[error("invalid signature: {0}")]
    InvalidSignature(String),
    #[error("signature recovery failed: {0}")]
    RecoveryFailed(String),
    #[error("address mismatch: expected {expected}, recovered {recovered}")]
    AddressMismatch { expected: String, recovered: String },
}

pub fn verify_personal_sign(
    expected_address: &str,
    message: &str,
    signature_hex: &str,
) -> Result<EthVerification, EthVerifyError> {
    let expected = parse_address(expected_address)?;
    let signature = Signature::from_str(signature_hex)
        .map_err(|error| EthVerifyError::InvalidSignature(error.to_string()))?;
    let recovered = signature
        .recover_address_from_msg(message.as_bytes())
        .map_err(|error| EthVerifyError::RecoveryFailed(error.to_string()))?;

    if recovered != expected {
        return Err(EthVerifyError::AddressMismatch {
            expected: expected.to_checksum(None),
            recovered: recovered.to_checksum(None),
        });
    }

    Ok(EthVerification {
        expected_address: expected.to_checksum(None),
        recovered_address: recovered.to_checksum(None),
    })
}

pub fn parse_address(address: &str) -> Result<Address, EthVerifyError> {
    Address::parse_checksummed(address, None)
        .or_else(|_| Address::from_str(address))
        .map_err(|error| EthVerifyError::InvalidAddress(error.to_string()))
}

pub fn short_hex(value: &str, head: usize, tail: usize) -> String {
    if value.len() <= head + tail {
        return value.to_string();
    }
    format!("{}…{}", &value[..head], &value[value.len() - tail..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_signature_length() {
        let result = verify_personal_sign(
            "0x742d35Cc6634C0532925a3b844Bc454e44f3A8B4",
            "websh.home.v1",
            "0x1234",
        );
        assert!(matches!(result, Err(EthVerifyError::InvalidSignature(_))));
    }
}
