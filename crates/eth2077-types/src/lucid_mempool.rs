use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EncryptionScheme {
    ThresholdBLS,
    IdentityBased,
    CommitReveal,
    ShutterStyle,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DecryptionTrigger {
    BlockInclusion,
    SlotBoundary,
    ThresholdReached,
    TimeLock,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedTransaction {
    pub envelope_hash: [u8; 32],
    pub encrypted_payload: Vec<u8>,
    pub scheme: EncryptionScheme,
    pub decryption_trigger: DecryptionTrigger,
    pub sender_commitment: [u8; 32],
    pub gas_ceiling: u64,
    pub priority_fee_ceiling: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecryptionShare {
    pub committee_member: [u8; 32],
    pub share_data: Vec<u8>,
    pub slot_number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LucidConfig {
    pub committee_size: usize,
    pub threshold: usize,
    pub default_scheme: EncryptionScheme,
    pub default_trigger: DecryptionTrigger,
    pub max_encrypted_size: usize,
    pub decryption_timeout_slots: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LucidValidationError {
    EmptyPayload,
    PayloadTooLarge { size: usize, max: usize },
    InvalidCommitment,
    InsufficientShares { provided: usize, required: usize },
    DuplicateCommitteeMember { member: [u8; 32] },
    GasCeilingZero,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LucidImpactStats {
    pub total_encrypted_txs: usize,
    pub total_payload_bytes: usize,
    pub avg_gas_ceiling: f64,
    pub scheme_distribution: Vec<(String, usize)>,
    pub mev_protection_ratio: f64,
    pub latency_overhead_ms: f64,
}

fn scheme_discriminant(scheme: EncryptionScheme) -> u8 {
    match scheme {
        EncryptionScheme::ThresholdBLS => 0,
        EncryptionScheme::IdentityBased => 1,
        EncryptionScheme::CommitReveal => 2,
        EncryptionScheme::ShutterStyle => 3,
    }
}

fn trigger_discriminant(trigger: DecryptionTrigger) -> u8 {
    match trigger {
        DecryptionTrigger::BlockInclusion => 0,
        DecryptionTrigger::SlotBoundary => 1,
        DecryptionTrigger::ThresholdReached => 2,
        DecryptionTrigger::TimeLock => 3,
    }
}

fn latency_ms_for_scheme(scheme: EncryptionScheme) -> f64 {
    match scheme {
        EncryptionScheme::ThresholdBLS => 2.0,
        EncryptionScheme::CommitReveal => 0.0,
        EncryptionScheme::IdentityBased => 5.0,
        EncryptionScheme::ShutterStyle => 3.0,
    }
}

pub fn default_lucid_config() -> LucidConfig {
    LucidConfig {
        committee_size: 21,
        threshold: 14,
        default_scheme: EncryptionScheme::ThresholdBLS,
        default_trigger: DecryptionTrigger::BlockInclusion,
        max_encrypted_size: 131_072,
        decryption_timeout_slots: 32,
    }
}

pub fn validate_encrypted_transaction(
    tx: &EncryptedTransaction,
    config: &LucidConfig,
) -> Result<(), Vec<LucidValidationError>> {
    let mut errors = Vec::new();

    if tx.encrypted_payload.is_empty() {
        errors.push(LucidValidationError::EmptyPayload);
    }

    if tx.encrypted_payload.len() > config.max_encrypted_size {
        errors.push(LucidValidationError::PayloadTooLarge {
            size: tx.encrypted_payload.len(),
            max: config.max_encrypted_size,
        });
    }

    if tx.sender_commitment == [0u8; 32] {
        errors.push(LucidValidationError::InvalidCommitment);
    }

    if tx.gas_ceiling == 0 {
        errors.push(LucidValidationError::GasCeilingZero);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn validate_decryption_shares(
    shares: &[DecryptionShare],
    config: &LucidConfig,
) -> Result<(), Vec<LucidValidationError>> {
    let mut errors = Vec::new();
    let mut members = HashSet::new();

    for share in shares {
        if !members.insert(share.committee_member) {
            errors.push(LucidValidationError::DuplicateCommitteeMember {
                member: share.committee_member,
            });
        }
    }

    if members.len() < config.threshold {
        errors.push(LucidValidationError::InsufficientShares {
            provided: members.len(),
            required: config.threshold,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_envelope_hash(tx: &EncryptedTransaction) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([scheme_discriminant(tx.scheme)]);
    hasher.update([trigger_discriminant(tx.decryption_trigger)]);
    hasher.update(&tx.encrypted_payload);
    hasher.update(tx.gas_ceiling.to_be_bytes());
    hasher.update(tx.priority_fee_ceiling.to_be_bytes());

    let digest = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

pub fn compute_lucid_stats(txs: &[EncryptedTransaction], config: &LucidConfig) -> LucidImpactStats {
    let total_encrypted_txs = txs.len();
    let total_payload_bytes = txs.iter().map(|tx| tx.encrypted_payload.len()).sum();
    let total_gas: u128 = txs.iter().map(|tx| tx.gas_ceiling as u128).sum();

    let avg_gas_ceiling = if total_encrypted_txs == 0 {
        0.0
    } else {
        total_gas as f64 / total_encrypted_txs as f64
    };

    let threshold_bls_count = txs
        .iter()
        .filter(|tx| tx.scheme == EncryptionScheme::ThresholdBLS)
        .count();
    let identity_based_count = txs
        .iter()
        .filter(|tx| tx.scheme == EncryptionScheme::IdentityBased)
        .count();
    let commit_reveal_count = txs
        .iter()
        .filter(|tx| tx.scheme == EncryptionScheme::CommitReveal)
        .count();
    let shutter_style_count = txs
        .iter()
        .filter(|tx| tx.scheme == EncryptionScheme::ShutterStyle)
        .count();

    let scheme_distribution = vec![
        ("ThresholdBLS".to_string(), threshold_bls_count),
        ("IdentityBased".to_string(), identity_based_count),
        ("CommitReveal".to_string(), commit_reveal_count),
        ("ShutterStyle".to_string(), shutter_style_count),
    ];

    let latency_overhead_ms = txs
        .iter()
        .map(|tx| latency_ms_for_scheme(tx.scheme))
        .sum::<f64>();

    let mev_protection_ratio = estimate_mev_protection(total_encrypted_txs, total_encrypted_txs);

    let _ = config;

    LucidImpactStats {
        total_encrypted_txs,
        total_payload_bytes,
        avg_gas_ceiling,
        scheme_distribution,
        mev_protection_ratio,
        latency_overhead_ms,
    }
}

pub fn estimate_mev_protection(encrypted_count: usize, total_count: usize) -> f64 {
    if total_count == 0 {
        0.0
    } else {
        encrypted_count as f64 / total_count as f64
    }
}
