use eth2077_types::epbs_integration::{
    compare_separation_models, compute_epbs_commitment, compute_epbs_stats,
    default_epbs_integration_config, validate_epbs_config, EpbsValidationError, MEVPolicy,
};

#[test]
fn default_config_is_valid() {
    let config = default_epbs_integration_config();
    assert_eq!(validate_epbs_config(&config), Ok(()));
}

#[test]
fn validation_rejects_zero_slots() {
    let mut config = default_epbs_integration_config();
    config.max_builder_slots = 0;

    let errors = validate_epbs_config(&config).expect_err("zero max_builder_slots should fail");
    assert!(errors.contains(&EpbsValidationError::InvalidSlotCount));
}

#[test]
fn validation_rejects_bad_collateral_and_censorship() {
    let mut config = default_epbs_integration_config();
    config.builder_collateral_eth = 0.5;
    config.censorship_resistance_target = 1.5;

    let errors =
        validate_epbs_config(&config).expect_err("invalid collateral and censorship should fail");

    assert!(errors.contains(&EpbsValidationError::CollateralTooLow { value: 0.5 }));
    assert!(errors.contains(&EpbsValidationError::CensorshipTargetOutOfRange { value: 1.5 }));
}

#[test]
fn validation_rejects_oversized_inclusion_list() {
    let mut config = default_epbs_integration_config();
    config.inclusion_list_size = 5000;

    let errors = validate_epbs_config(&config).expect_err("oversized inclusion list should fail");
    assert!(
        errors.contains(&EpbsValidationError::InclusionListTooLarge {
            size: 5000,
            max: 2048,
        })
    );
}

#[test]
fn stats_have_positive_values() {
    let config = default_epbs_integration_config();
    let stats = compute_epbs_stats(&config);

    assert!(stats.effective_block_value_gwei > 0.0);
    assert!(stats.censorship_resistance_score > 0.0);
    assert!(stats.builder_diversity_index > 0.0);
    assert!(stats.mev_leakage_fraction > 0.0);
    assert!(stats.ordering_fairness_score > 0.0);
    assert!(stats.inclusion_guarantee_rate > 0.0);
    assert!(!stats.bottleneck.is_empty());
}

#[test]
fn compare_models_returns_all_variants() {
    let config = default_epbs_integration_config();
    let models = compare_separation_models(&config);

    assert_eq!(models.len(), 6);

    let expected = [
        "ProposerBuilderSplit",
        "ExecutionTickets",
        "AttesterProposerSplit",
        "SlotAuction",
        "CombinedPBS",
        "HybridModel",
    ];

    for name in expected {
        assert!(
            models.iter().any(|(model_name, _)| model_name == name),
            "missing model name: {name}"
        );
    }
}

#[test]
fn commitment_is_deterministic() {
    let config = default_epbs_integration_config();
    let first = compute_epbs_commitment(&config);
    let second = compute_epbs_commitment(&config);

    assert_eq!(first, second);
}

#[test]
fn commitment_is_config_sensitive() {
    let config = default_epbs_integration_config();
    let mut changed = config.clone();
    changed.mev_policy = MEVPolicy::NoMEVIntervention;

    let baseline = compute_epbs_commitment(&config);
    let modified = compute_epbs_commitment(&changed);

    assert_ne!(baseline, modified);
}
