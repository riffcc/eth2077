use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LaneId {
    Transfer,
    TokenOps,
    DeFi,
    StateHeavy,
    SystemOps,
    BlobCarrying,
    Uncategorized,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaneAssignment {
    pub tx_hash: [u8; 32],
    pub assigned_lane: LaneId,
    pub state_keys_accessed: Vec<[u8; 32]>,
    pub conflict_set: HashSet<[u8; 32]>,
    pub gas_estimate: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LaneAssignmentError {
    EmptyTransaction,
    ConflictDetected {
        tx_a: [u8; 32],
        tx_b: [u8; 32],
        conflicting_key: [u8; 32],
    },
    LaneCapacityExceeded {
        lane: LaneId,
        current: usize,
        max: usize,
    },
    InvalidStateKeys,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaneConfig {
    pub max_txs_per_lane: usize,
    pub max_total_gas_per_lane: u64,
    pub enable_conflict_detection: bool,
    pub parallel_lanes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LaneExecutionStats {
    pub total_transactions: usize,
    pub lanes_used: usize,
    pub max_lane_depth: usize,
    pub conflict_count: usize,
    pub parallelism_ratio: f64,
    pub gas_by_lane: HashMap<String, u64>,
}

pub fn default_lane_config() -> LaneConfig {
    LaneConfig {
        max_txs_per_lane: 128,
        max_total_gas_per_lane: 15_000_000,
        enable_conflict_detection: true,
        parallel_lanes: 4,
    }
}

pub fn classify_transaction(
    to_address: &[u8; 20],
    calldata: &[u8],
    value: u128,
    tx_type: u8,
) -> LaneId {
    if tx_type == 0x03 {
        return LaneId::BlobCarrying;
    }

    if is_system_address(to_address) {
        return LaneId::SystemOps;
    }

    if calldata.is_empty() && value > 0 {
        return LaneId::Transfer;
    }

    if calldata.len() > 1024 {
        return LaneId::StateHeavy;
    }

    if calldata.len() >= 4 {
        let selector = [calldata[0], calldata[1], calldata[2], calldata[3]];

        if is_token_selector(selector) {
            return LaneId::TokenOps;
        }
        if is_defi_selector(selector) {
            return LaneId::DeFi;
        }
    }

    if to_address[0] == 0xDE && to_address[1] == 0xF1 {
        return LaneId::DeFi;
    }
    if to_address[0] == 0x7E && to_address[1] == 0x20 {
        return LaneId::TokenOps;
    }

    LaneId::Uncategorized
}

pub fn assign_to_lanes(
    assignments: &[LaneAssignment],
    config: &LaneConfig,
) -> Result<HashMap<LaneId, Vec<usize>>, Vec<LaneAssignmentError>> {
    let mut grouped: HashMap<LaneId, Vec<usize>> = HashMap::new();
    let mut lane_gas: HashMap<LaneId, u64> = HashMap::new();
    let mut lane_keys: HashMap<LaneId, HashMap<[u8; 32], [u8; 32]>> = HashMap::new();
    let mut errors = Vec::new();

    for (index, assignment) in assignments.iter().enumerate() {
        if assignment.tx_hash == [0u8; 32] {
            errors.push(LaneAssignmentError::EmptyTransaction);
            continue;
        }

        let normalized_keys = match normalize_keys(assignment) {
            Ok(keys) => keys,
            Err(err) => {
                errors.push(err);
                continue;
            }
        };

        let lane_entries = grouped.entry(assignment.assigned_lane).or_default();
        let next_count = lane_entries.len().saturating_add(1);
        if next_count > config.max_txs_per_lane {
            errors.push(LaneAssignmentError::LaneCapacityExceeded {
                lane: assignment.assigned_lane,
                current: next_count,
                max: config.max_txs_per_lane,
            });
        }

        let lane_total_gas = lane_gas.entry(assignment.assigned_lane).or_default();
        let next_gas = lane_total_gas.saturating_add(assignment.gas_estimate);
        if next_gas > config.max_total_gas_per_lane {
            errors.push(LaneAssignmentError::LaneCapacityExceeded {
                lane: assignment.assigned_lane,
                current: usize::try_from(next_gas).unwrap_or(usize::MAX),
                max: usize::try_from(config.max_total_gas_per_lane).unwrap_or(usize::MAX),
            });
        }

        if config.enable_conflict_detection {
            let key_index = lane_keys.entry(assignment.assigned_lane).or_default();
            for key in normalized_keys {
                if let Some(existing_tx) = key_index.get(&key) {
                    if existing_tx != &assignment.tx_hash {
                        errors.push(LaneAssignmentError::ConflictDetected {
                            tx_a: *existing_tx,
                            tx_b: assignment.tx_hash,
                            conflicting_key: key,
                        });
                    }
                } else {
                    key_index.insert(key, assignment.tx_hash);
                }
            }
        }

        *lane_total_gas = next_gas;
        lane_entries.push(index);
    }

    if errors.is_empty() {
        Ok(grouped)
    } else {
        Err(errors)
    }
}

pub fn detect_cross_lane_conflicts(
    assignments: &[LaneAssignment],
) -> Vec<(usize, usize, [u8; 32])> {
    let mut conflicts = Vec::new();
    let mut normalized: Vec<HashSet<[u8; 32]>> = Vec::with_capacity(assignments.len());

    for assignment in assignments {
        match normalize_keys(assignment) {
            Ok(keys) => normalized.push(keys),
            Err(_) => normalized.push(HashSet::new()),
        }
    }

    for i in 0..assignments.len() {
        for j in (i + 1)..assignments.len() {
            if assignments[i].assigned_lane == assignments[j].assigned_lane {
                continue;
            }

            let mut keys = normalized[i].iter().copied().collect::<Vec<_>>();
            keys.sort_unstable();
            if let Some(key) = keys.into_iter().find(|key| normalized[j].contains(key)) {
                conflicts.push((i, j, key));
            }
        }
    }

    conflicts
}

pub fn compute_lane_stats(
    assignments: &[LaneAssignment],
    config: &LaneConfig,
) -> LaneExecutionStats {
    let mut lane_depths: HashMap<LaneId, usize> = HashMap::new();
    let mut gas_by_lane_id: HashMap<LaneId, u64> = HashMap::new();

    for assignment in assignments {
        *lane_depths.entry(assignment.assigned_lane).or_default() += 1;
        *gas_by_lane_id.entry(assignment.assigned_lane).or_default() = gas_by_lane_id
            .get(&assignment.assigned_lane)
            .copied()
            .unwrap_or(0)
            .saturating_add(assignment.gas_estimate);
    }

    let total_transactions = assignments.len();
    let lanes_used = lane_depths.len();
    let max_lane_depth = lane_depths.values().copied().max().unwrap_or(0);
    let conflict_count = if config.enable_conflict_detection {
        detect_cross_lane_conflicts(assignments).len()
    } else {
        0
    };

    let parallelism_ratio = compute_parallelism_ratio(total_transactions, &lane_depths, config)
        / (1.0 + (conflict_count as f64 / total_transactions.max(1) as f64));

    let mut gas_by_lane = HashMap::new();
    for (lane, gas) in gas_by_lane_id {
        gas_by_lane.insert(format!("{lane:?}"), gas);
    }

    LaneExecutionStats {
        total_transactions,
        lanes_used,
        max_lane_depth,
        conflict_count,
        parallelism_ratio,
        gas_by_lane,
    }
}

pub fn compute_lane_commitment(lane: LaneId, tx_hashes: &[[u8; 32]]) -> [u8; 32] {
    let mut sorted = tx_hashes.to_vec();
    sorted.sort_unstable();

    let mut hasher = Sha256::new();
    hasher.update([lane_discriminant(lane)]);
    for tx_hash in sorted {
        hasher.update(tx_hash);
    }
    hasher.finalize().into()
}

fn is_system_address(address: &[u8; 20]) -> bool {
    let prefix_is_zero = address[..19].iter().all(|byte| *byte == 0);
    prefix_is_zero && (1..=0x20).contains(&address[19])
}

fn is_token_selector(selector: [u8; 4]) -> bool {
    matches!(
        selector,
        [0x09, 0x5E, 0xA7, 0xB3]
            | [0xA9, 0x05, 0x9C, 0xBB]
            | [0x23, 0xB8, 0x72, 0xDD]
            | [0xA2, 0x2C, 0xB4, 0x65]
            | [0x42, 0x84, 0x2E, 0x0E]
            | [0xB8, 0x8D, 0x4F, 0xDE]
            | [0xF2, 0x42, 0x43, 0x2A]
    )
}

fn is_defi_selector(selector: [u8; 4]) -> bool {
    matches!(
        selector,
        [0x38, 0xED, 0x17, 0x39]
            | [0x18, 0xCB, 0xAF, 0xE5]
            | [0x7F, 0xF3, 0x6A, 0xB5]
            | [0x41, 0x4B, 0xF3, 0x89]
            | [0x5C, 0x11, 0xD7, 0x95]
            | [0xAC, 0x96, 0x50, 0xD8]
    )
}

fn normalize_keys(assignment: &LaneAssignment) -> Result<HashSet<[u8; 32]>, LaneAssignmentError> {
    if assignment
        .state_keys_accessed
        .iter()
        .chain(assignment.conflict_set.iter())
        .any(|key| *key == [0u8; 32])
    {
        return Err(LaneAssignmentError::InvalidStateKeys);
    }

    let mut keys = assignment
        .state_keys_accessed
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    keys.extend(assignment.conflict_set.iter().copied());
    Ok(keys)
}

fn compute_parallelism_ratio(
    total_transactions: usize,
    lane_depths: &HashMap<LaneId, usize>,
    config: &LaneConfig,
) -> f64 {
    if total_transactions == 0 || lane_depths.is_empty() {
        return 0.0;
    }

    let workers_len = config.parallel_lanes.max(1);
    let mut workers = vec![0usize; workers_len];

    let mut depths = lane_depths.values().copied().collect::<Vec<_>>();
    depths.sort_unstable_by(|a, b| b.cmp(a));

    for depth in depths {
        if let Some((index, _)) = workers.iter().enumerate().min_by_key(|(_, load)| **load) {
            workers[index] = workers[index].saturating_add(depth);
        }
    }

    let makespan = workers.into_iter().max().unwrap_or(0);
    if makespan == 0 {
        0.0
    } else {
        total_transactions as f64 / makespan as f64
    }
}

fn lane_discriminant(lane: LaneId) -> u8 {
    match lane {
        LaneId::Transfer => 0,
        LaneId::TokenOps => 1,
        LaneId::DeFi => 2,
        LaneId::StateHeavy => 3,
        LaneId::SystemOps => 4,
        LaneId::BlobCarrying => 5,
        LaneId::Uncategorized => 6,
    }
}
