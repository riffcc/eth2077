use eth2077_types::bals_integration::{
    compare_bals_features, compute_bals_commitment, compute_bals_stats,
    default_bals_integration_config, validate_bals_config, BalsValidationError,
};

#[test]
fn default_config_is_valid() {
    let config = default_bals_integration_config();
    assert_eq!(validate_bals_config(&config), Ok(()));
}

#[test]
fn validation_rejects_empty_features() {
    let mut config = default_bals_integration_config();
    config.target_features.clear();

    let errors = validate_bals_config(&config).expect_err("expected validation failure");
    assert!(errors.contains(&BalsValidationError::EmptyFeatures));
}

#[test]
fn validation_rejects_small_validator_set() {
    let mut config = default_bals_integration_config();
    config.validator_set_size = 100;

    let errors = validate_bals_config(&config).expect_err("expected validation failure");
    assert!(errors.contains(&BalsValidationError::ValidatorSetTooSmall { size: 100 }));
}

#[test]
fn validation_rejects_zero_requests_and_queue() {
    let mut config = default_bals_integration_config();
    config.max_requests_per_block = 0;
    config.queue_capacity = 0;

    let errors = validate_bals_config(&config).expect_err("expected validation failure");
    assert!(errors.contains(&BalsValidationError::RequestsPerBlockZero));
    assert!(errors.contains(&BalsValidationError::QueueCapacityZero));
}

#[test]
fn stats_have_positive_values() {
    let config = default_bals_integration_config();
    let stats = compute_bals_stats(&config);

    assert!(stats.total_implementation_effort > 0.0);
    assert!(stats.feature_coverage > 0.0);
}

#[test]
fn compare_features_returns_all() {
    let config = default_bals_integration_config();
    let compared = compare_bals_features(&config);

    assert_eq!(compared.len(), 6);
    assert!(compared
        .iter()
        .any(|(name, _)| name == "ValidatorTriggeredExits"));
    assert!(compared
        .iter()
        .any(|(name, _)| name == "ExecutionLayerRequests"));
    assert!(compared
        .iter()
        .any(|(name, _)| name == "ConsolidationRequests"));
    assert!(compared.iter().any(|(name, _)| name == "DepositProcessing"));
    assert!(compared.iter().any(|(name, _)| name == "WithdrawalQueue"));
    assert!(compared
        .iter()
        .any(|(name, _)| name == "MaxEffectiveBalance"));
}

#[test]
fn commitment_is_deterministic() {
    let config = default_bals_integration_config();
    let first = compute_bals_commitment(&config);
    let second = compute_bals_commitment(&config);

    assert_eq!(first, second);
}

#[test]
fn commitment_is_config_sensitive() {
    let config = default_bals_integration_config();
    let baseline = compute_bals_commitment(&config);

    let mut changed = config.clone();
    changed.activation_epoch_delay += 1;
    let changed_hash = compute_bals_commitment(&changed);

    assert_ne!(baseline, changed_hash);
}
