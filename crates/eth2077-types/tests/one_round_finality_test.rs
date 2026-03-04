use eth2077_types::one_round_finality::{
    compare_finality_modes, compute_finality_commitment, compute_finality_stats,
    default_one_round_finality_config, estimate_liveness, estimate_safety_margin,
    validate_finality_config, FinalityMode, FinalityValidationError, OneRoundFinalityConfig,
    SafetyProperty,
};

fn sample_key(byte: u8) -> [u8; 32] {
    [byte; 32]
}

#[test]
fn default_config_matches_expected_values() {
    let config = default_one_round_finality_config();

    assert_eq!(config.mode, FinalityMode::OneRoundFinality);
    assert_eq!(config.validator_count, 8_192);
    assert!((config.quorum_threshold - 0.67).abs() < 1e-9);
    assert_eq!(config.message_complexity_bound, 200_000);
    assert!((config.expected_finality_slots - 1.0).abs() < 1e-9);
    assert_eq!(config.safety_property, SafetyProperty::AccountableSafety);
    assert!((config.max_tolerable_byzantine_fraction - 0.33).abs() < 1e-9);
    assert!(config.signature_aggregation);
}

#[test]
fn default_config_validates_successfully() {
    let config = default_one_round_finality_config();
    assert_eq!(validate_finality_config(&config), Ok(()));
}

#[test]
fn validation_reports_multiple_errors() {
    let config = OneRoundFinalityConfig {
        mode: FinalityMode::OneRoundFinality,
        validator_count: 64,
        quorum_threshold: 0.49,
        message_complexity_bound: 10_000,
        expected_finality_slots: 3.0,
        safety_property: SafetyProperty::ByzantineFaultTolerance,
        max_tolerable_byzantine_fraction: 0.55,
        signature_aggregation: false,
    };

    let errors = validate_finality_config(&config).expect_err("expected validation errors");
    assert!(errors.contains(&FinalityValidationError::InsufficientValidators));
    assert!(errors.contains(&FinalityValidationError::QuorumTooLow { value: 0.49 }));
    assert!(errors.contains(&FinalityValidationError::ByzantineFractionInvalid { value: 0.55 }));
    assert!(errors.contains(&FinalityValidationError::FinalitySlotsTooHigh { value: 3.0 }));
}

#[test]
fn validation_rejects_quorum_above_one() {
    let config = OneRoundFinalityConfig {
        quorum_threshold: 1.2,
        ..default_one_round_finality_config()
    };

    let errors = validate_finality_config(&config).expect_err("expected validation errors");
    assert!(errors.contains(&FinalityValidationError::QuorumTooHigh { value: 1.2 }));
}

#[test]
fn safety_margin_behaves_as_expected() {
    let safe = estimate_safety_margin(0.67, 0.20);
    let unsafe_margin = estimate_safety_margin(0.60, 0.35);

    assert!(safe > 0.0);
    assert!(unsafe_margin < 0.0);
}

#[test]
fn liveness_improves_with_larger_validator_set() {
    let small = estimate_liveness(128, 0.20);
    let large = estimate_liveness(8_192, 0.20);
    let degraded = estimate_liveness(8_192, 0.40);

    assert!(large > small);
    assert!(degraded < large);
    assert!((0.0..=1.0).contains(&large));
}

#[test]
fn computed_stats_are_coherent_and_bounded() {
    let config = default_one_round_finality_config();
    let stats = compute_finality_stats(&config);

    assert_eq!(stats.finality_time_ms, 12_000.0);
    assert!(stats.message_count > 0);
    assert!(stats.message_count <= config.message_complexity_bound);
    assert!(stats.bandwidth_per_validator_bytes > 0);
    assert!(stats.safety_margin > 0.0);
    assert!((0.0..=1.0).contains(&stats.liveness_probability));
    assert!(stats.validator_overhead_factor > 0.0);
}

#[test]
fn compare_modes_lists_all_finality_modes() {
    let config = default_one_round_finality_config();
    let comparison = compare_finality_modes(&config);

    assert_eq!(comparison.len(), 5);
    assert_eq!(comparison[0].0, "CasperFFG");
    assert_eq!(comparison[1].0, "SingleSlotFinality");
    assert_eq!(comparison[2].0, "OneRoundFinality");
    assert_eq!(comparison[3].0, "OptimisticFinality");
    assert_eq!(comparison[4].0, "PipelinedFinality");
}

#[test]
fn commitment_is_order_independent_for_pubkeys() {
    let config = default_one_round_finality_config();
    let keys_a = vec![sample_key(1), sample_key(2), sample_key(3)];
    let keys_b = vec![sample_key(3), sample_key(1), sample_key(2)];

    let commitment_a = compute_finality_commitment(&config, &keys_a);
    let commitment_b = compute_finality_commitment(&config, &keys_b);
    assert_eq!(commitment_a, commitment_b);
}

#[test]
fn commitment_changes_when_config_changes() {
    let base = default_one_round_finality_config();
    let changed = OneRoundFinalityConfig {
        mode: FinalityMode::OptimisticFinality,
        ..base.clone()
    };
    let keys = vec![sample_key(7), sample_key(8), sample_key(9)];

    let base_commitment = compute_finality_commitment(&base, &keys);
    let changed_commitment = compute_finality_commitment(&changed, &keys);
    assert_ne!(base_commitment, changed_commitment);
}
