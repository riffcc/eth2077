use eth2077_types::million_tps::{
    compute_million_tps_commitment, compute_million_tps_stats, default_million_tps_config,
    validate_million_tps, MillionTpsValidationError,
};

#[test]
fn default_config_is_valid() {
    let config = default_million_tps_config();
    assert_eq!(validate_million_tps(&config), Ok(()));
}

#[test]
fn validation_rejects_empty_milestones() {
    let mut config = default_million_tps_config();
    config.milestones.clear();

    let errors = validate_million_tps(&config).unwrap_err();
    assert!(errors.contains(&MillionTpsValidationError::EmptyMilestones));
}

#[test]
fn validation_rejects_target_mismatch() {
    let mut config = default_million_tps_config();
    config.l1_target_tps = 700_000;
    config.l2_target_tps = 400_000;
    config.combined_target_tps = 1_000_000;

    let errors = validate_million_tps(&config).unwrap_err();
    assert!(errors.iter().any(|error| matches!(
        error,
        MillionTpsValidationError::TargetMismatch { l1, l2, combined }
            if *l1 == 700_000 && *l2 == 400_000 && *combined == 1_000_000
    )));
}

#[test]
fn validation_rejects_bad_confidence() {
    let mut config = default_million_tps_config();
    config.min_finality_confidence = 2.0;

    let errors = validate_million_tps(&config).unwrap_err();
    assert!(errors.contains(&MillionTpsValidationError::ConfidenceOutOfRange { value: 2.0 }));
}

#[test]
fn stats_count_milestones() {
    let config = default_million_tps_config();
    let stats = compute_million_tps_stats(&config);
    assert_eq!(stats.total_milestones, 5);
}

#[test]
fn stats_progress_is_bounded() {
    let config = default_million_tps_config();
    let stats = compute_million_tps_stats(&config);

    assert!(stats.overall_progress >= 0.0);
    assert!(stats.overall_progress <= 1.0);
}

#[test]
fn commitment_is_deterministic() {
    let config = default_million_tps_config();
    let first = compute_million_tps_commitment(&config);
    let second = compute_million_tps_commitment(&config);

    assert_eq!(first, second);
}

#[test]
fn commitment_is_config_sensitive() {
    let mut config_a = default_million_tps_config();
    let mut config_b = default_million_tps_config();
    config_b.l1_target_tps = config_b.l1_target_tps.saturating_add(1);

    let commitment_a = compute_million_tps_commitment(&config_a);
    let commitment_b = compute_million_tps_commitment(&config_b);

    assert_ne!(commitment_a, commitment_b);

    config_a.l1_target_tps = config_b.l1_target_tps;
    let commitment_a_updated = compute_million_tps_commitment(&config_a);
    assert_eq!(commitment_a_updated, commitment_b);
}
