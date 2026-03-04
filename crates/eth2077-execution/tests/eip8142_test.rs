use eth2077_execution::eip8142::{
    assign_to_lanes, classify_transaction, compute_lane_commitment, compute_lane_stats,
    default_lane_config, detect_cross_lane_conflicts, LaneAssignment, LaneAssignmentError, LaneId,
};
use std::collections::HashSet;

fn key(seed: u8) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[31] = seed;
    out
}

fn set(keys: &[[u8; 32]]) -> HashSet<[u8; 32]> {
    keys.iter().copied().collect()
}

fn assignment(
    tx_seed: u8,
    lane: LaneId,
    state_keys_accessed: Vec<[u8; 32]>,
    conflict_set: HashSet<[u8; 32]>,
    gas_estimate: u64,
) -> LaneAssignment {
    LaneAssignment {
        tx_hash: [tx_seed; 32],
        assigned_lane: lane,
        state_keys_accessed,
        conflict_set,
        gas_estimate,
    }
}

#[test]
fn default_config_values() {
    let cfg = default_lane_config();
    assert_eq!(cfg.max_txs_per_lane, 128);
    assert_eq!(cfg.max_total_gas_per_lane, 15_000_000);
    assert!(cfg.enable_conflict_detection);
    assert_eq!(cfg.parallel_lanes, 4);
}

#[test]
fn transfer_classification() {
    let to = [0x11u8; 20];
    let lane = classify_transaction(&to, &[], 1_000_000_000_000_000_000u128, 0x00);
    assert_eq!(lane, LaneId::Transfer);
}

#[test]
fn blob_classification() {
    let to = [0x22u8; 20];
    let lane = classify_transaction(&to, &[0x01, 0x02, 0x03, 0x04], 0, 0x03);
    assert_eq!(lane, LaneId::BlobCarrying);
}

#[test]
fn valid_lane_assignment_groups_indices() {
    let cfg = default_lane_config();
    let assignments = vec![
        assignment(0x01, LaneId::Transfer, vec![key(1)], set(&[key(1)]), 21_000),
        assignment(0x02, LaneId::Transfer, vec![key(2)], set(&[key(2)]), 21_000),
        assignment(0x03, LaneId::DeFi, vec![key(3)], set(&[key(3)]), 80_000),
    ];

    let grouped = assign_to_lanes(&assignments, &cfg).expect("assignments should be valid");
    assert_eq!(grouped.get(&LaneId::Transfer), Some(&vec![0, 1]));
    assert_eq!(grouped.get(&LaneId::DeFi), Some(&vec![2]));
}

#[test]
fn capacity_exceeded_detection() {
    let mut cfg = default_lane_config();
    cfg.max_txs_per_lane = 1;

    let assignments = vec![
        assignment(0x01, LaneId::Transfer, vec![key(1)], set(&[key(1)]), 21_000),
        assignment(0x02, LaneId::Transfer, vec![key(2)], set(&[key(2)]), 21_000),
    ];

    let errors = assign_to_lanes(&assignments, &cfg).expect_err("lane capacity should be exceeded");
    assert!(errors.iter().any(|err| {
        matches!(
            err,
            LaneAssignmentError::LaneCapacityExceeded {
                lane: LaneId::Transfer,
                current: 2,
                max: 1
            }
        )
    }));
}

#[test]
fn cross_lane_conflict_detection() {
    let shared = key(9);
    let assignments = vec![
        assignment(
            0x01,
            LaneId::Transfer,
            vec![shared, key(1)],
            set(&[shared, key(1)]),
            21_000,
        ),
        assignment(
            0x02,
            LaneId::DeFi,
            vec![shared, key(2)],
            set(&[shared, key(2)]),
            90_000,
        ),
    ];

    let conflicts = detect_cross_lane_conflicts(&assignments);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0], (0, 1, shared));
}

#[test]
fn stats_computation() {
    let mut cfg = default_lane_config();
    cfg.parallel_lanes = 2;

    let assignments = vec![
        assignment(0x01, LaneId::Transfer, vec![key(1)], set(&[key(1)]), 21_000),
        assignment(0x02, LaneId::Transfer, vec![key(2)], set(&[key(2)]), 21_000),
        assignment(0x03, LaneId::DeFi, vec![key(3)], set(&[key(3)]), 80_000),
    ];

    let stats = compute_lane_stats(&assignments, &cfg);
    assert_eq!(stats.total_transactions, 3);
    assert_eq!(stats.lanes_used, 2);
    assert_eq!(stats.max_lane_depth, 2);
    assert_eq!(stats.conflict_count, 0);
    assert!((stats.parallelism_ratio - 1.5).abs() < f64::EPSILON);
    assert_eq!(stats.gas_by_lane.get("Transfer"), Some(&42_000));
    assert_eq!(stats.gas_by_lane.get("DeFi"), Some(&80_000));
}

#[test]
fn commitment_is_deterministic_for_ordering() {
    let hashes_a = vec![[0x10; 32], [0x01; 32], [0xFF; 32]];
    let hashes_b = vec![[0xFF; 32], [0x10; 32], [0x01; 32]];

    let a = compute_lane_commitment(LaneId::TokenOps, &hashes_a);
    let b = compute_lane_commitment(LaneId::TokenOps, &hashes_b);
    let c = compute_lane_commitment(LaneId::DeFi, &hashes_b);

    assert_eq!(a, b);
    assert_ne!(a, c);
}
