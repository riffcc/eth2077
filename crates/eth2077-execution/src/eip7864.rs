use serde::{Deserialize, Serialize};

const BASE_GAS_COST: u64 = 3_000;
const PROOF_BYTE_GAS_COST: u64 = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StateAccessType {
    AccountBalance,
    StorageSlot,
    CodeHash,
    AccountNonce,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateAccessRequest {
    pub access_type: StateAccessType,
    pub target_address: [u8; 20],
    pub storage_slot: Option<[u8; 32]>,
    pub block_number: u64,
    pub proof_data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateAccessResult {
    pub value: [u8; 32],
    pub verified: bool,
    pub gas_used: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateAccessError {
    InvalidProofEncoding,
    BlockTooOld { requested: u64, oldest_allowed: u64 },
    BlockInFuture { requested: u64, current: u64 },
    StorageSlotRequiredForType,
    StorageSlotNotAllowedForType,
    ProofTooLarge { size: usize, max: usize },
}

pub fn validate_state_access_request(
    request: &StateAccessRequest,
    current_block: u64,
    lookback_window: u64,
    max_proof_size: usize,
) -> Result<(), StateAccessError> {
    if request.block_number > current_block {
        return Err(StateAccessError::BlockInFuture {
            requested: request.block_number,
            current: current_block,
        });
    }

    let oldest_allowed = current_block.saturating_sub(lookback_window);
    if request.block_number < oldest_allowed {
        return Err(StateAccessError::BlockTooOld {
            requested: request.block_number,
            oldest_allowed,
        });
    }

    match request.access_type {
        StateAccessType::StorageSlot => {
            if request.storage_slot.is_none() {
                return Err(StateAccessError::StorageSlotRequiredForType);
            }
        }
        _ => {
            if request.storage_slot.is_some() {
                return Err(StateAccessError::StorageSlotNotAllowedForType);
            }
        }
    }

    let proof_size = request.proof_data.len();
    if proof_size > max_proof_size {
        return Err(StateAccessError::ProofTooLarge {
            size: proof_size,
            max: max_proof_size,
        });
    }

    // Merkle proof is encoded as 32-byte branch nodes.
    if proof_size == 0 || proof_size % 32 != 0 {
        return Err(StateAccessError::InvalidProofEncoding);
    }

    Ok(())
}

pub fn estimate_gas_cost(request: &StateAccessRequest) -> u64 {
    let proof_bytes = request.proof_data.len() as u64;
    BASE_GAS_COST.saturating_add(proof_bytes.saturating_mul(PROOF_BYTE_GAS_COST))
}
