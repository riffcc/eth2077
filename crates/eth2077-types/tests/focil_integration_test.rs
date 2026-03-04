use eth2077_types::focil_integration::{
    compare_inclusion_modes, compute_focil_commitment, compute_focil_stats,
    default_focil_integration_config, validate_focil_config, EnforcementMechanism,
    FocilValidationError,
};
use std::collections::HashSet;

#[test]
fn default_config_is_valid() {
    let config = default_focil_integration_config();
    assert_eq!(validate_focil_config(&config), Ok(()));
}

#[test]
fn validation_rejects_zero_list_size() {
    let mut config = default_focil_integration_config();
    config.max_inclusion_list_txs = 0;

    let errors = validate_focil_config(&config).unwrap_err();
    assert!(errors.contains(&FocilValidationError::ZeroInclusionListSize));
}

#[test]
fn validation_rejects_short_deadline_and_low_penalty() {
    let mut config = default_focil_integration_config();
    config.inclusion_deadline_slots = 1;
    config.enforcement_penalty_gwei = 500;

    let errors = validate_focil_config(&config).unwrap_err();
    assert!(errors.contains(&FocilValidationError::DeadlineTooShort { slots: 1 }));
    assert!(errors.contains(&FocilValidationError::PenaltyTooLow { value: 500 }));
}

#[test]
fn validation_rejects_bad_compliance_and_participation() {
    let mut config = default_focil_integration_config();
    config.min_proposer_compliance = 1.5;
    config.network_participation_threshold = -0.1;

    let errors = validate_focil_config(&config).unwrap_err();
    assert!(errors.contains(&FocilValidationError::ComplianceOutOfRange { value: 1.5 }));
    assert!(errors.contains(&FocilValidationError::ParticipationOutOfRange { value: -0.1 }));
}

#[test]
fn stats_have_positive_values() {
    let config = default_focil_integration_config();
    let stats = compute_focil_stats(&config);

    assert!(stats.effective_inclusion_rate > 0.0);
    assert!(stats.proposer_compliance_score > 0.0);
    assert!(stats.censorship_resistance_index > 0.0);
    assert!(stats.enforcement_cost_gwei > 0.0);
    assert!(stats.network_overhead_fraction > 0.0);
    assert!(stats.fork_complexity_score > 0.0);
    assert!(!stats.bottleneck.is_empty());
    assert!(!stats.deployment_caveats.is_empty());
}

#[test]
fn compare_modes_returns_all_variants() {
    let config = default_focil_integration_config();
    let results = compare_inclusion_modes(&config);
    assert_eq!(results.len(), 6);

    let names: HashSet<String> = results.into_iter().map(|(name, _)| name).collect();
    let expected: HashSet<String> = vec![
        "Mandatory".to_string(),
        "Advisory".to_string(),
        "Hybrid".to_string(),
        "ConditionalEnforcement".to_string(),
        "GradualRollout".to_string(),
        "FullEnforcement".to_string(),
    ]
    .into_iter()
    .collect();
    assert_eq!(names, expected);
}

#[test]
fn commitment_is_deterministic() {
    let config = default_focil_integration_config();
    let a = compute_focil_commitment(&config);
    let b = compute_focil_commitment(&config);
    assert_eq!(a, b);
}

#[test]
fn commitment_is_config_sensitive() {
    let config = default_focil_integration_config();
    let base = compute_focil_commitment(&config);

    let mut changed = config.clone();
    changed.enforcement_mechanism = EnforcementMechanism::SocialConsensus;
    let changed_hash = compute_focil_commitment(&changed);

    assert_ne!(base, changed_hash);
}
