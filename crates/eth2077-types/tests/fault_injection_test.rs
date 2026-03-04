use eth2077_types::fault_injection::{
    compute_fault_commitment, compute_fault_stats, default_fault_injection_config,
    validate_fault_injection, FaultInjectionValidationError, FaultScenario, FaultType,
    InjectionTiming, RecoveryStrategy, SeverityLevel,
};

#[test]
fn default_config_is_valid() {
    let config = default_fault_injection_config();
    assert!(validate_fault_injection(&config).is_ok());
}

#[test]
fn validation_rejects_empty_scenarios() {
    let mut config = default_fault_injection_config();
    config.scenarios.clear();

    let errors =
        validate_fault_injection(&config).expect_err("empty scenarios should fail validation");
    assert!(errors.contains(&FaultInjectionValidationError::EmptyScenarios));
}

#[test]
fn validation_rejects_too_few_nodes() {
    let mut config = default_fault_injection_config();
    config.total_nodes = 0;

    let errors = validate_fault_injection(&config).expect_err("zero nodes should fail validation");
    assert!(errors.contains(&FaultInjectionValidationError::NodesTooFew { count: 0 }));
}

#[test]
fn validation_rejects_bad_byzantine_threshold() {
    let mut config = default_fault_injection_config();
    config.byzantine_threshold = 1.5;

    let errors = validate_fault_injection(&config)
        .expect_err("byzantine threshold outside [0, 1] should fail validation");
    assert!(errors
        .contains(&FaultInjectionValidationError::ByzantineThresholdOutOfRange { value: 1.5 }));
}

#[test]
fn stats_count_scenarios() {
    let config = default_fault_injection_config();
    let stats = compute_fault_stats(&config);
    assert_eq!(stats.total_scenarios, 6);
}

#[test]
fn stats_have_coverage() {
    let config = default_fault_injection_config();
    let stats = compute_fault_stats(&config);
    assert!(stats.coverage_score > 0.0);
}

#[test]
fn commitment_is_deterministic() {
    let config = default_fault_injection_config();
    let first = compute_fault_commitment(&config);
    let second = compute_fault_commitment(&config);
    assert_eq!(first, second);
}

#[test]
fn commitment_is_config_sensitive() {
    let config = default_fault_injection_config();
    let original = compute_fault_commitment(&config);

    let mut modified = config.clone();
    modified.scenarios.push(FaultScenario {
        name: "Synthetic delay-and-drop campaign".to_string(),
        fault_type: FaultType::MessageDrop,
        timing: InjectionTiming::Continuous,
        severity: SeverityLevel::Major,
        duration_slots: 18,
        affected_nodes_fraction: 0.27,
        expected_recovery: RecoveryStrategy::Rollback,
        expected_impact: "Sustained drops trigger reorg pressure until rollback coordination."
            .to_string(),
    });

    let changed = compute_fault_commitment(&modified);
    assert_ne!(original, changed);
}
