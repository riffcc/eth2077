use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthorizationEntry {
    pub chain_id: u64,
    pub address: [u8; 20],
    pub nonce: u64,
    pub y_parity: u8,
    pub r: [u8; 32],
    pub s: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationError {
    EmptyAuthorizationList,
    InvalidChainId {
        index: usize,
        chain_id: u64,
        expected: u64,
    },
    InvalidYParity { index: usize, value: u8 },
    ZeroAddress { index: usize },
    DuplicateAuthorization { index: usize, address: [u8; 20] },
    SignatureRTooLarge { index: usize },
    SignatureSInUpperHalf { index: usize },
}

/// The secp256k1 curve order n
const SECP256K1_N: [u8; 32] = [
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFE, 0xBA, 0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B, 0xBF, 0xD2, 0x5E, 0x8C, 0xD0, 0x36,
    0x41, 0x41,
];

/// Half of secp256k1 curve order (for low-s check)
const SECP256K1_N_DIV_2: [u8; 32] = [
    0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0x5D, 0x57, 0x6E, 0x73, 0x57, 0xA4, 0x50, 0x1D, 0xDF, 0xE9, 0x2F, 0x46, 0x68, 0x1B,
    0x20, 0xA0,
];

/// Validate an EIP-7702 authorization list structurally.
/// chain_id 0 means "any chain" and is always valid.
/// Non-zero chain_id must match expected_chain_id.
pub fn validate_authorization_list(
    authorizations: &[AuthorizationEntry],
    expected_chain_id: u64,
) -> Result<(), Vec<AuthorizationError>> {
    let mut errors = Vec::new();
    let mut seen_addresses = HashSet::new();

    if authorizations.is_empty() {
        errors.push(AuthorizationError::EmptyAuthorizationList);
    }

    for (index, authorization) in authorizations.iter().enumerate() {
        if authorization.chain_id != 0 && authorization.chain_id != expected_chain_id {
            errors.push(AuthorizationError::InvalidChainId {
                index,
                chain_id: authorization.chain_id,
                expected: expected_chain_id,
            });
        }

        if authorization.y_parity != 0 && authorization.y_parity != 1 {
            errors.push(AuthorizationError::InvalidYParity {
                index,
                value: authorization.y_parity,
            });
        }

        if authorization.address == [0u8; 20] {
            errors.push(AuthorizationError::ZeroAddress { index });
        }

        if !seen_addresses.insert(authorization.address) {
            errors.push(AuthorizationError::DuplicateAuthorization {
                index,
                address: authorization.address,
            });
        }

        if !bytes32_less_than(&authorization.r, &SECP256K1_N) {
            errors.push(AuthorizationError::SignatureRTooLarge { index });
        }

        if !bytes32_less_than_or_equal(&authorization.s, &SECP256K1_N_DIV_2) {
            errors.push(AuthorizationError::SignatureSInUpperHalf { index });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Helper: compare 32-byte big-endian integers
fn bytes32_less_than(a: &[u8; 32], b: &[u8; 32]) -> bool {
    for (&left, &right) in a.iter().zip(b.iter()) {
        if left < right {
            return true;
        }
        if left > right {
            return false;
        }
    }
    false
}

fn bytes32_less_than_or_equal(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a == b || bytes32_less_than(a, b)
}
