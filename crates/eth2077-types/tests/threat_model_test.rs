use eth2077_types::threat_model::{
    compute_threat_commitment, compute_threat_model_stats, default_threat_model_config,
    validate_threat_model, AdversaryCapability, ClaimClass, ThreatEntry,
    ThreatModelValidationError, ThreatSeverity,
};

#[test]
fn default_config_is_valid() {
    let config = default_threat_model_config();
    assert!(validate_threat_model(&config).is_ok());
}

#[test]
fn validation_rejects_empty_threats() {
    let mut config = default_threat_model_config();
    config.threats.clear();

    let errors = validate_threat_model(&config).expect_err("empty threats should fail validation");
    assert!(errors.contains(&ThreatModelValidationError::EmptyThreats));
}

#[test]
fn validation_rejects_empty_assumptions() {
    let mut config = default_threat_model_config();
    config.assumptions.clear();

    let errors =
        validate_threat_model(&config).expect_err("empty assumptions should fail validation");
    assert!(errors.contains(&ThreatModelValidationError::EmptyAssumptions));
}

#[test]
fn validation_rejects_bad_byzantine_threshold() {
    let mut config = default_threat_model_config();
    config.byzantine_threshold = 1.5;

    let errors = validate_threat_model(&config)
        .expect_err("byzantine threshold outside [0, 1] should fail validation");
    assert!(
        errors.contains(&ThreatModelValidationError::ByzantineThresholdOutOfRange { value: 1.5 })
    );
}

#[test]
fn stats_count_threats_correctly() {
    let config = default_threat_model_config();
    let stats = compute_threat_model_stats(&config);

    assert_eq!(stats.total_threats, 6);
    assert_eq!(stats.critical_threats, 2);
    assert_eq!(stats.high_threats, 3);
    assert_eq!(stats.assumption_count, 6);
}

#[test]
fn stats_compute_coverage() {
    let config = default_threat_model_config();
    let stats = compute_threat_model_stats(&config);

    assert!(stats.coverage_score > 0.0);
}

#[test]
fn commitment_is_deterministic() {
    let config = default_threat_model_config();

    let first = compute_threat_commitment(&config);
    let second = compute_threat_commitment(&config);
    assert_eq!(first, second);
}

#[test]
fn commitment_is_config_sensitive() {
    let config = default_threat_model_config();
    let original = compute_threat_commitment(&config);

    let mut modified = config.clone();
    modified.threats.push(ThreatEntry {
        name: "Added sensitivity test threat".to_string(),
        capability: AdversaryCapability::MessageReorder,
        severity: ThreatSeverity::Low,
        affected_claims: vec![ClaimClass::LivenessProperty],
        mitigations: vec!["Replay-protected sequencing".to_string()],
        residual_risk: 0.05,
    });

    let changed = compute_threat_commitment(&modified);
    assert_ne!(original, changed);
}
