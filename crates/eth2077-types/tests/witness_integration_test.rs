use eth2077_types::witness_integration::{
    compare_sync_modes, compute_witness_commitment, compute_witness_stats,
    default_witness_integration_config, validate_witness_config, DiffStrategy, SyncMode,
    WitnessValidationError,
};
use std::collections::HashSet;

#[test]
fn default_config_is_valid() {
    let config = default_witness_integration_config();
    assert_eq!(validate_witness_config(&config), Ok(()));
}

#[test]
fn validation_rejects_empty_witness_types() {
    let mut config = default_witness_integration_config();
    config.witness_types = vec![];

    let errors = validate_witness_config(&config).unwrap_err();
    assert!(errors.contains(&WitnessValidationError::EmptyWitnessTypes));
}

#[test]
fn validation_rejects_zero_cache() {
    let mut config = default_witness_integration_config();
    config.witness_cache_size_mb = 0;

    let errors = validate_witness_config(&config).unwrap_err();
    assert!(errors.contains(&WitnessValidationError::CacheSizeZero));
}

#[test]
fn validation_rejects_bad_compression() {
    let mut config = default_witness_integration_config();
    config.target_compression_ratio = 0.0;

    let errors = validate_witness_config(&config).unwrap_err();
    assert!(errors.contains(&WitnessValidationError::CompressionRatioInvalid { value: 0.0 }));
}

#[test]
fn stats_have_positive_throughput() {
    let config = default_witness_integration_config();
    let stats = compute_witness_stats(&config);

    assert!(stats.sync_throughput_mbps > 0.0);
    assert!(stats.avg_witness_size_bytes > 0.0);
    assert!(stats.avg_generation_time_ms > 0.0);
    assert!(stats.avg_verification_time_ms > 0.0);
    assert!(stats.propagation_efficiency > 0.0);
}

#[test]
fn compare_sync_modes_returns_all() {
    let config = default_witness_integration_config();
    let results = compare_sync_modes(&config);
    assert_eq!(results.len(), 6);

    let names: HashSet<String> = results.into_iter().map(|(name, _)| name).collect();
    let expected: HashSet<String> = vec![
        format!("{:?}", SyncMode::FullSync),
        format!("{:?}", SyncMode::SnapSync),
        format!("{:?}", SyncMode::WitnessSync),
        format!("{:?}", SyncMode::BeamSync),
        format!("{:?}", SyncMode::DiffBasedSync),
        format!("{:?}", SyncMode::HybridSync),
    ]
    .into_iter()
    .collect();

    assert_eq!(names, expected);
}

#[test]
fn commitment_is_deterministic() {
    let config = default_witness_integration_config();
    let left = compute_witness_commitment(&config);
    let right = compute_witness_commitment(&config);
    assert_eq!(left, right);
}

#[test]
fn commitment_is_config_sensitive() {
    let config = default_witness_integration_config();
    let base = compute_witness_commitment(&config);

    let mut changed = config.clone();
    changed.diff_strategy = DiffStrategy::BatchedDiff;
    let changed_hash = compute_witness_commitment(&changed);

    assert_ne!(base, changed_hash);
}
