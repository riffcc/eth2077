use eth2077_types::vops::{
    classify_witness_need, compute_vops_stats, compute_witness_commitment, default_vops_config,
    estimate_bandwidth_savings, validate_witness_requirement, StateCategory, VopsValidationError,
    WitnessRequirement,
};
use std::collections::HashSet;

fn hash32(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn requirement(
    tx_seed: u8,
    keys: Vec<[u8; 32]>,
    witness_size_bytes: usize,
    categories_accessed: HashSet<StateCategory>,
) -> WitnessRequirement {
    WitnessRequirement {
        tx_hash: hash32(tx_seed),
        needed_state_keys: keys,
        witness_size_bytes,
        categories_accessed,
        requires_witness: true,
    }
}

#[test]
fn default_vops_config_has_expected_categories() {
    let config = default_vops_config();
    let expected = HashSet::from([
        StateCategory::AccountBalance,
        StateCategory::AccountNonce,
        StateCategory::ContractCode,
        StateCategory::PaymasterState,
    ]);

    assert_eq!(config.retained_categories, expected);
    assert_eq!(config.witness_size_budget_bytes, 1024 * 1024);
    assert_eq!(config.partial_state_size_mb, 16 * 1024);
    assert_eq!(config.full_state_size_mb, 256 * 1024);
    assert_eq!(config.max_witness_items_per_tx, 64);
}

#[test]
fn witness_within_budget_passes_validation() {
    let config = default_vops_config();
    let req = requirement(
        1,
        vec![hash32(10), hash32(11)],
        4_096,
        HashSet::from([StateCategory::AccountBalance]),
    );

    assert_eq!(validate_witness_requirement(&req, &config), Ok(()));
}

#[test]
fn witness_too_large_is_rejected() {
    let config = default_vops_config();
    let req = requirement(
        2,
        vec![hash32(10)],
        config.witness_size_budget_bytes + 1,
        HashSet::from([StateCategory::StorageSlot]),
    );

    let errors = validate_witness_requirement(&req, &config).unwrap_err();
    assert!(errors.contains(&VopsValidationError::WitnessTooLarge {
        size: config.witness_size_budget_bytes + 1,
        max: config.witness_size_budget_bytes,
    }));
}

#[test]
fn classify_need_all_retained_returns_false() {
    let config = default_vops_config();
    let categories = HashSet::from([StateCategory::AccountBalance, StateCategory::ContractCode]);

    assert!(!classify_witness_need(&categories, &config));
}

#[test]
fn classify_need_when_storage_slot_accessed_returns_true() {
    let config = default_vops_config();
    let categories = HashSet::from([StateCategory::AccountBalance, StateCategory::StorageSlot]);

    assert!(classify_witness_need(&categories, &config));
}

#[test]
fn stats_computation_is_correct() {
    let config = default_vops_config();
    let requirements = vec![
        requirement(
            1,
            vec![hash32(1)],
            100,
            HashSet::from([StateCategory::AccountBalance]),
        ),
        requirement(
            2,
            vec![hash32(2), hash32(3)],
            300,
            HashSet::from([StateCategory::StorageSlot]),
        ),
        requirement(
            3,
            vec![hash32(4)],
            700,
            HashSet::from([StateCategory::ValidationCode]),
        ),
    ];

    let stats = compute_vops_stats(&requirements, &config);
    assert_eq!(stats.total_transactions, 3);
    assert_eq!(stats.witness_required_count, 2);
    assert_eq!(stats.witness_free_count, 1);
    assert_eq!(stats.total_witness_bytes, 1_000);
    assert!((stats.avg_witness_bytes_per_tx - (1_000.0 / 3.0)).abs() < 1e-12);
    assert!((stats.bandwidth_reduction_percent - (100.0 / 11.0)).abs() < 1e-12);
    assert!((stats.state_storage_reduction_percent - 93.75).abs() < 1e-12);
}

#[test]
fn witness_commitment_is_deterministic() {
    let categories = HashSet::from([StateCategory::StorageSlot]);
    let req_a = requirement(
        9,
        vec![hash32(20), hash32(10), hash32(30)],
        128,
        categories.clone(),
    );
    let req_b = requirement(9, vec![hash32(30), hash32(20), hash32(10)], 128, categories);

    let commitment_a = compute_witness_commitment(&req_a);
    let commitment_b = compute_witness_commitment(&req_b);
    assert_eq!(commitment_a, commitment_b);
}

#[test]
fn bandwidth_savings_calculation_is_correct() {
    let savings = estimate_bandwidth_savings(1_000, 250);
    assert!((savings - 75.0).abs() < f64::EPSILON);
}
