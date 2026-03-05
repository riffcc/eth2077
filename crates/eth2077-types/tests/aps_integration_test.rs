use eth2077_types::aps_integration::{
    compare_proposer_modes, compute_aps_commitment, compute_aps_stats,
    default_aps_integration_config, validate_aps_config, ApsValidationError, ProposerMode,
};

#[test]
fn default_config_is_valid() {
    let config = default_aps_integration_config();
    assert_eq!(validate_aps_config(&config), Ok(()));
}

#[test]
fn validation_rejects_small_validator_set() {
    let mut config = default_aps_integration_config();
    config.validator_set_size = 100;

    let errors =
        validate_aps_config(&config).expect_err("validator_set_size below 1024 should fail");
    assert!(errors.contains(&ApsValidationError::ValidatorSetTooSmall { size: 100 }));
}

#[test]
fn validation_rejects_oversized_committee() {
    let mut config = default_aps_integration_config();
    config.validator_set_size = 2048;
    config.committee_size = 600;

    let errors =
        validate_aps_config(&config).expect_err("committee above validator_set/4 should fail");
    assert!(errors.contains(&ApsValidationError::CommitteeTooLarge {
        size: 600,
        max: 512,
    }));
}

#[test]
fn validation_rejects_bad_participation() {
    let mut config = default_aps_integration_config();
    config.min_attester_participation = 1.5;

    let errors = validate_aps_config(&config).expect_err("participation above 1.0 should fail");
    assert!(errors.contains(&ApsValidationError::ParticipationOutOfRange { value: 1.5 }));
}

#[test]
fn stats_have_positive_values() {
    let config = default_aps_integration_config();
    let stats = compute_aps_stats(&config);

    assert!(stats.effective_attestation_rate > 0.0);
    assert!(stats.proposer_utilization > 0.0);
    assert!(stats.separation_completeness > 0.0);
    assert!(stats.safety_coverage > 0.0);
    assert!(stats.validator_overhead > 0.0);
    assert!(stats.migration_risk_score > 0.0);
    assert!(!stats.bottleneck.is_empty());
    assert!(!stats.milestones.is_empty());
}

#[test]
fn compare_modes_returns_all_variants() {
    let config = default_aps_integration_config();
    let modes = compare_proposer_modes(&config);

    assert_eq!(modes.len(), 6);

    let expected = [
        "SoloProposer",
        "RotatingProposer",
        "AuctionedProposer",
        "CommitteeProposer",
        "DelegatedProposer",
        "HybridMode",
    ];

    for name in expected {
        assert!(
            modes.iter().any(|(mode_name, _)| mode_name == name),
            "missing mode name: {name}"
        );
    }
}

#[test]
fn commitment_is_deterministic() {
    let config = default_aps_integration_config();
    let first = compute_aps_commitment(&config);
    let second = compute_aps_commitment(&config);

    assert_eq!(first, second);
}

#[test]
fn commitment_is_config_sensitive() {
    let config = default_aps_integration_config();
    let mut changed = config.clone();
    changed.proposer_mode = ProposerMode::AuctionedProposer;

    let baseline = compute_aps_commitment(&config);
    let modified = compute_aps_commitment(&changed);

    assert_ne!(baseline, modified);
}
