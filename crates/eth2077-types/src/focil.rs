use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A validator-submitted transaction summary for FOCIL inclusion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InclusionListEntry {
    pub validator_index: u64,
    pub tx_hash: [u8; 32],
    pub gas_limit: u64,
    pub max_fee_per_gas: u128,
    pub inclusion_deadline_slot: u64,
}

/// Aggregated inclusion list for a beacon slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InclusionList {
    pub slot: u64,
    pub entries: Vec<InclusionListEntry>,
    pub aggregate_signature: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InclusionListValidationError {
    EmptyList,
    DuplicateTxHashes {
        tx_hash: [u8; 32],
    },
    ExpiredDeadline {
        tx_hash: [u8; 32],
        deadline_slot: u64,
        current_slot: u64,
    },
    GasLimitExceeded {
        total_requested_gas: u128,
        block_gas_limit: u64,
    },
    TooManyEntries {
        entry_count: usize,
        max_entries: usize,
    },
}

/// Validate structural and protocol-level constraints for a FOCIL inclusion list.
pub fn validate_inclusion_list(
    list: &InclusionList,
    current_slot: u64,
    max_entries: usize,
    block_gas_limit: u64,
) -> Result<(), Vec<InclusionListValidationError>> {
    let mut errors = Vec::new();
    let entry_count = list.entries.len();

    if entry_count == 0 {
        errors.push(InclusionListValidationError::EmptyList);
    }

    if entry_count > max_entries {
        errors.push(InclusionListValidationError::TooManyEntries {
            entry_count,
            max_entries,
        });
    }

    let mut seen_hashes = HashSet::new();
    let mut duplicate_reported = HashSet::new();
    let mut total_requested_gas = 0_u128;

    for entry in &list.entries {
        if !seen_hashes.insert(entry.tx_hash) && duplicate_reported.insert(entry.tx_hash) {
            errors.push(InclusionListValidationError::DuplicateTxHashes {
                tx_hash: entry.tx_hash,
            });
        }

        if current_slot > entry.inclusion_deadline_slot {
            errors.push(InclusionListValidationError::ExpiredDeadline {
                tx_hash: entry.tx_hash,
                deadline_slot: entry.inclusion_deadline_slot,
                current_slot,
            });
        }

        total_requested_gas = total_requested_gas.saturating_add(entry.gas_limit as u128);
    }

    if total_requested_gas > block_gas_limit as u128 {
        errors.push(InclusionListValidationError::GasLimitExceeded {
            total_requested_gas,
            block_gas_limit,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Check whether a block's transaction set satisfies the required inclusion list.
/// Returns the required transaction hashes that are missing from `included_tx_hashes`.
pub fn check_block_satisfies_inclusion_list(
    included_tx_hashes: &[[u8; 32]],
    required: &InclusionList,
) -> Vec<[u8; 32]> {
    let included: HashSet<[u8; 32]> = included_tx_hashes.iter().copied().collect();
    let mut seen_required = HashSet::new();
    let mut missing = Vec::new();

    for entry in &required.entries {
        if seen_required.insert(entry.tx_hash) && !included.contains(&entry.tx_hash) {
            missing.push(entry.tx_hash);
        }
    }

    missing
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InclusionListStats {
    pub slot: u64,
    pub entry_count: usize,
    pub unique_validator_count: usize,
    pub unique_tx_count: usize,
    pub total_gas_limit: u128,
    pub min_deadline_slot: Option<u64>,
    pub max_deadline_slot: Option<u64>,
    pub average_max_fee_per_gas: u128,
    pub has_aggregate_signature: bool,
}

/// Compute summary statistics for an inclusion list.
pub fn compute_inclusion_stats(list: &InclusionList) -> InclusionListStats {
    let mut validators = HashSet::new();
    let mut transactions = HashSet::new();
    let mut total_gas_limit = 0_u128;
    let mut total_max_fee_per_gas = 0_u128;

    for entry in &list.entries {
        validators.insert(entry.validator_index);
        transactions.insert(entry.tx_hash);
        total_gas_limit = total_gas_limit.saturating_add(entry.gas_limit as u128);
        total_max_fee_per_gas = total_max_fee_per_gas.saturating_add(entry.max_fee_per_gas);
    }

    let entry_count = list.entries.len();
    let average_max_fee_per_gas = if entry_count == 0 {
        0
    } else {
        total_max_fee_per_gas / entry_count as u128
    };

    InclusionListStats {
        slot: list.slot,
        entry_count,
        unique_validator_count: validators.len(),
        unique_tx_count: transactions.len(),
        total_gas_limit,
        min_deadline_slot: list
            .entries
            .iter()
            .map(|entry| entry.inclusion_deadline_slot)
            .min(),
        max_deadline_slot: list
            .entries
            .iter()
            .map(|entry| entry.inclusion_deadline_slot)
            .max(),
        average_max_fee_per_gas,
        has_aggregate_signature: list.aggregate_signature.is_some(),
    }
}
