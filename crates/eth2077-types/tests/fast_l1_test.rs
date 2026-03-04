use eth2077_types::fast_l1::*;

#[test]
fn default_config_is_valid() {
    let config = default_fast_l1_config();

    assert_eq!(config.finality_target, FinalityTarget::FewSlots);
    assert_eq!(config.inclusion_mode, InclusionMode::Immediate);
    assert!(config.slot_time_ms >= 500.0);
    assert!(config.target_inclusion_ms <= config.target_finality_ms);
    assert!(!config.latency_budgets.is_empty());
    assert!(config.validator_count > 0);
    assert!(config.network_diameter_ms > 0.0);
    assert_eq!(validate_fast_l1_config(&config), Ok(()));
}

#[test]
fn validation_rejects_low_slot_time() {
    let mut config = default_fast_l1_config();
    config.slot_time_ms = 400.0;

    let errors = validate_fast_l1_config(&config).expect_err("expected validation failure");
    assert!(errors.contains(&FastL1ValidationError::SlotTimeTooLow { value: 400.0 }));
}

#[test]
fn validation_rejects_inclusion_exceeds_finality() {
    let mut config = default_fast_l1_config();
    config.target_inclusion_ms = 12_000.0;
    config.target_finality_ms = 8_000.0;

    let errors = validate_fast_l1_config(&config).expect_err("expected validation failure");
    assert!(errors.contains(&FastL1ValidationError::InclusionExceedsFinality));
}

#[test]
fn validation_rejects_budget_exceeds_slot() {
    let mut config = default_fast_l1_config();
    config.slot_time_ms = 1_000.0;
    for budget in &mut config.latency_budgets {
        budget.budget_ms = 300.0;
    }

    let errors = validate_fast_l1_config(&config).expect_err("expected validation failure");
    assert!(errors.iter().any(|error| matches!(
        error,
        FastL1ValidationError::BudgetExceedsSlot {
            total_ms,
            slot_ms
        } if *total_ms > *slot_ms
    )));
}

#[test]
fn validation_rejects_zero_validators_and_invalid_network() {
    let mut config = default_fast_l1_config();
    config.validator_count = 0;
    config.network_diameter_ms = 0.0;

    let errors = validate_fast_l1_config(&config).expect_err("expected validation failure");
    assert!(errors.contains(&FastL1ValidationError::ValidatorCountZero));
    assert!(errors.contains(&FastL1ValidationError::NetworkDiameterInvalid));
}

#[test]
fn stats_meet_target_for_default_config() {
    let config = default_fast_l1_config();
    let stats = compute_fast_l1_stats(&config);

    assert!(stats.meets_target);
    assert!(stats.achievable_inclusion_ms <= config.target_inclusion_ms);
    assert!(stats.achievable_finality_ms <= config.target_finality_ms);
    assert!(stats.headroom_ms >= 0.0);
    assert!(stats.slot_utilization > 0.0);
    assert_eq!(stats.bottleneck_component, "FinalityVoting");
}

#[test]
fn propagation_increases_with_validators() {
    let small = estimate_propagation_delay(256, 120.0);
    let medium = estimate_propagation_delay(2_048, 120.0);
    let large = estimate_propagation_delay(16_384, 120.0);

    assert!(small > 0.0);
    assert!(medium > small);
    assert!(large > medium);
}

#[test]
fn compare_targets_returns_all_variants() {
    let config = default_fast_l1_config();
    let compared = compare_finality_targets(&config);

    assert_eq!(compared.len(), 4);
    assert_eq!(compared[0].0, "SingleSlot");
    assert_eq!(compared[1].0, "FewSlots");
    assert_eq!(compared[2].0, "SubMinute");
    assert_eq!(compared[3].0, "CurrentBaseline");
}

#[test]
fn compared_target_profiles_have_expected_ordering() {
    let config = default_fast_l1_config();
    let compared = compare_finality_targets(&config);

    let single = &compared[0].1;
    let few = &compared[1].1;
    let sub_minute = &compared[2].1;
    let baseline = &compared[3].1;

    assert!(single.achievable_finality_ms <= few.achievable_finality_ms);
    assert!(few.achievable_finality_ms <= sub_minute.achievable_finality_ms);
    assert!(sub_minute.achievable_finality_ms < baseline.achievable_finality_ms);
}

#[test]
fn commitment_is_deterministic() {
    let config = default_fast_l1_config();
    let first = compute_fast_l1_commitment(&config);
    let second = compute_fast_l1_commitment(&config);

    assert_eq!(first, second);

    let mut changed = config.clone();
    changed.inclusion_mode = InclusionMode::NextSlot;
    let changed_commitment = compute_fast_l1_commitment(&changed);
    assert_ne!(first, changed_commitment);
}

#[test]
fn commitment_ignores_latency_budget_order() {
    let mut a = default_fast_l1_config();
    let mut b = default_fast_l1_config();
    b.latency_budgets.reverse();

    let hash_a = compute_fast_l1_commitment(&a);
    let hash_b = compute_fast_l1_commitment(&b);
    assert_eq!(hash_a, hash_b);

    a.latency_budgets[0].budget_ms += 1.0;
    let hash_changed = compute_fast_l1_commitment(&a);
    assert_ne!(hash_a, hash_changed);
}

#[test]
fn feasibility_score_is_bounded() {
    let config = default_fast_l1_config();
    let stats = compute_fast_l1_stats(&config);
    assert!((0.0..=1.0).contains(&stats.feasibility_score));

    let mut stressed = config.clone();
    stressed.inclusion_mode = InclusionMode::Guaranteed;
    stressed.target_inclusion_ms = 10.0;
    stressed.target_finality_ms = 100.0;
    stressed.slot_time_ms = 2_000.0;
    let stressed_stats = compute_fast_l1_stats(&stressed);

    assert!((0.0..=1.0).contains(&stressed_stats.feasibility_score));
    assert!(!stressed_stats.meets_target);
}
