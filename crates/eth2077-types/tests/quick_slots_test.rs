use eth2077_types::quick_slots::{
    analyze_tradeoffs, compare_configurations, default_slot_config, estimate_oob_benefit,
    slot_duration_ms, validate_slot_config, FinalityMode, QuickSlotValidationError, SlotConfig,
    SlotDuration,
};

#[test]
fn slot_duration_conversions_are_correct() {
    assert_eq!(slot_duration_ms(SlotDuration::Standard12s), 12_000);
    assert_eq!(slot_duration_ms(SlotDuration::Fast8s), 8_000);
    assert_eq!(slot_duration_ms(SlotDuration::Quick6s), 6_000);
    assert_eq!(slot_duration_ms(SlotDuration::Rapid4s), 4_000);
    assert_eq!(slot_duration_ms(SlotDuration::Ultra2s), 2_000);
}

#[test]
fn default_config_is_valid() {
    let config = default_slot_config();

    assert_eq!(config.slot_duration, SlotDuration::Standard12s);
    assert_eq!(config.finality_mode, FinalityMode::EpochBased);
    assert_eq!(config.propagation_budget_ms, 4_000);
    assert_eq!(config.attestation_deadline_ms, 4_000);
    assert_eq!(config.max_validators_per_slot, 32);
    assert_eq!(config.network_latency_p95_ms, 500);
    assert_eq!(validate_slot_config(&config), Ok(()));
}

#[test]
fn propagation_exceeds_slot_is_rejected() {
    let config = SlotConfig {
        slot_duration: SlotDuration::Ultra2s,
        finality_mode: FinalityMode::ThreeSlotFinality,
        propagation_budget_ms: 2_500,
        attestation_deadline_ms: 500,
        max_validators_per_slot: 32,
        network_latency_p95_ms: 300,
    };

    let errors = validate_slot_config(&config).unwrap_err();
    assert!(errors.contains(&QuickSlotValidationError::PropagationExceedsSlot {
        propagation_ms: 2_500,
        slot_ms: 2_000,
    }));
}

#[test]
fn tradeoff_analysis_inclusion_latency_matches_half_slot() {
    let config = default_slot_config();
    let analysis = analyze_tradeoffs(&config);

    assert!((analysis.inclusion_latency_avg_ms - 6_000.0).abs() < f64::EPSILON);
}

#[test]
fn throughput_multiplier_for_ultra_slots_is_six_x() {
    let config = SlotConfig {
        slot_duration: SlotDuration::Ultra2s,
        finality_mode: FinalityMode::SingleSlotFinality,
        propagation_budget_ms: 700,
        attestation_deadline_ms: 700,
        max_validators_per_slot: 24,
        network_latency_p95_ms: 350,
    };

    let analysis = analyze_tradeoffs(&config);
    assert!((analysis.throughput_multiplier - 6.0).abs() < f64::EPSILON);
}

#[test]
fn insufficient_security_margin_is_rejected() {
    let config = SlotConfig {
        slot_duration: SlotDuration::Rapid4s,
        finality_mode: FinalityMode::ThreeSlotFinality,
        propagation_budget_ms: 2_000,
        attestation_deadline_ms: 1_400,
        max_validators_per_slot: 40,
        network_latency_p95_ms: 450,
    };

    let errors = validate_slot_config(&config).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, QuickSlotValidationError::InsufficientSecurityMargin { .. }))
    );
}

#[test]
fn stats_comparison_picks_best_configs() {
    let baseline = default_slot_config();
    let fast_three_slot = SlotConfig {
        slot_duration: SlotDuration::Quick6s,
        finality_mode: FinalityMode::ThreeSlotFinality,
        propagation_budget_ms: 1_800,
        attestation_deadline_ms: 1_200,
        max_validators_per_slot: 48,
        network_latency_p95_ms: 450,
    };
    let ultra_oob = SlotConfig {
        slot_duration: SlotDuration::Ultra2s,
        finality_mode: FinalityMode::OobCitadel,
        propagation_budget_ms: 700,
        attestation_deadline_ms: 700,
        max_validators_per_slot: 24,
        network_latency_p95_ms: 350,
    };

    let stats = compare_configurations(&[baseline, fast_three_slot, ultra_oob]);

    assert_eq!(stats.configs_evaluated, 3);
    assert!(stats.best_throughput_config.contains("Ultra2s"));
    assert!(stats.best_finality_config.contains("Ultra2s"));
    assert!(stats.avg_security_margin > 0.0);
    assert!(stats.total_bandwidth_overhead > 0.0);
}

#[test]
fn oob_benefit_improves_with_faster_slots() {
    let standard = estimate_oob_benefit(SlotDuration::Standard12s);
    let ultra = estimate_oob_benefit(SlotDuration::Ultra2s);

    assert!(standard > 1.0);
    assert!(ultra > standard);
}
