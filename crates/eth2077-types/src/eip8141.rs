use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FramePhase {
    Validation,
    Authorization,
    Sponsorship,
    Deployment,
    Execution,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Frame {
    pub phase: FramePhase,
    pub target: [u8; 20],
    pub calldata: Vec<u8>,
    pub gas_limit: u64,
    pub value: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrameTransaction {
    pub sender: [u8; 20],
    pub nonce: u64,
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
    pub frames: Vec<Frame>,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FrameTxError {
    EmptyFrames,
    MissingPhase {
        phase: FramePhase,
    },
    DuplicatePhase {
        phase: FramePhase,
    },
    InvalidPhaseOrder {
        expected: FramePhase,
        found: FramePhase,
    },
    ZeroGasLimit {
        frame_index: usize,
    },
    ExcessiveTotalGas {
        total: u64,
        max: u64,
    },
    EmptySignature,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrameTxStats {
    pub frame_count: usize,
    pub total_gas_limit: u64,
    pub total_value: u128,
    pub phases_present: Vec<FramePhase>,
    pub total_calldata_bytes: usize,
}

pub const MAX_TOTAL_GAS: u64 = 30_000_000;
pub const REQUIRED_PHASE_ORDER: &[FramePhase] = &[
    FramePhase::Validation,
    FramePhase::Authorization,
    FramePhase::Sponsorship,
    FramePhase::Deployment,
    FramePhase::Execution,
];

fn phase_discriminant(phase: FramePhase) -> u8 {
    match phase {
        FramePhase::Validation => 0,
        FramePhase::Authorization => 1,
        FramePhase::Sponsorship => 2,
        FramePhase::Deployment => 3,
        FramePhase::Execution => 4,
    }
}

fn phase_index(phase: FramePhase) -> usize {
    phase_discriminant(phase) as usize
}

pub fn validate_frame_transaction(tx: &FrameTransaction) -> Result<(), Vec<FrameTxError>> {
    let mut errors = Vec::new();
    let mut seen_phases = HashSet::new();
    let mut total_gas: u128 = 0;
    let mut previous_phase_index: Option<usize> = None;

    if tx.frames.is_empty() {
        errors.push(FrameTxError::EmptyFrames);
    }

    if tx.signature.is_empty() {
        errors.push(FrameTxError::EmptySignature);
    }

    for (frame_index, frame) in tx.frames.iter().enumerate() {
        if frame.gas_limit == 0 {
            errors.push(FrameTxError::ZeroGasLimit { frame_index });
        }

        total_gas = total_gas.saturating_add(frame.gas_limit as u128);

        if !seen_phases.insert(frame.phase) {
            errors.push(FrameTxError::DuplicatePhase { phase: frame.phase });
        }

        let current_index = phase_index(frame.phase);
        if let Some(prev_index) = previous_phase_index {
            if current_index < prev_index {
                errors.push(FrameTxError::InvalidPhaseOrder {
                    expected: REQUIRED_PHASE_ORDER[prev_index],
                    found: frame.phase,
                });
            }
        }
        previous_phase_index = Some(current_index);
    }

    if !seen_phases.contains(&FramePhase::Validation) {
        errors.push(FrameTxError::MissingPhase {
            phase: FramePhase::Validation,
        });
    }
    if !seen_phases.contains(&FramePhase::Execution) {
        errors.push(FrameTxError::MissingPhase {
            phase: FramePhase::Execution,
        });
    }

    if total_gas > MAX_TOTAL_GAS as u128 {
        let total = total_gas.min(u64::MAX as u128) as u64;
        errors.push(FrameTxError::ExcessiveTotalGas {
            total,
            max: MAX_TOTAL_GAS,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_frame_tx_hash(tx: &FrameTransaction) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(tx.sender);
    hasher.update(tx.nonce.to_be_bytes());
    hasher.update(tx.max_fee_per_gas.to_be_bytes());

    for frame in &tx.frames {
        hasher.update([phase_discriminant(frame.phase)]);
        hasher.update(frame.target);
        hasher.update(frame.gas_limit.to_be_bytes());
        hasher.update(frame.value.to_be_bytes());
        hasher.update(&frame.calldata);
    }

    let digest = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

pub fn compute_frame_tx_stats(tx: &FrameTransaction) -> FrameTxStats {
    let mut total_gas_limit = 0u64;
    let mut total_value = 0u128;
    let mut phases_present = Vec::new();
    let mut seen_phases = HashSet::new();
    let mut total_calldata_bytes = 0usize;

    for frame in &tx.frames {
        total_gas_limit = total_gas_limit.saturating_add(frame.gas_limit);
        total_value = total_value.saturating_add(frame.value);
        total_calldata_bytes = total_calldata_bytes.saturating_add(frame.calldata.len());

        if seen_phases.insert(frame.phase) {
            phases_present.push(frame.phase);
        }
    }

    FrameTxStats {
        frame_count: tx.frames.len(),
        total_gas_limit,
        total_value,
        phases_present,
        total_calldata_bytes,
    }
}

pub fn estimate_frame_tx_cost(tx: &FrameTransaction, base_fee: u128) -> u128 {
    tx.frames.iter().fold(0u128, |total, frame| {
        total
            .saturating_add((frame.gas_limit as u128).saturating_mul(base_fee))
            .saturating_add(frame.value)
    })
}

pub fn extract_sponsorship_target(tx: &FrameTransaction) -> Option<[u8; 20]> {
    tx.frames
        .iter()
        .find(|frame| frame.phase == FramePhase::Sponsorship)
        .map(|frame| frame.target)
}
