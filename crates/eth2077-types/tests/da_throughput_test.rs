use eth2077_types::da_throughput::{
    compare_scaling_strategies, compute_da_commitment, compute_da_throughput_stats,
    compute_verification_time, default_da_throughput_config, estimate_tps_from_da,
    validate_da_throughput_config, DaScalingStrategy, DaThroughputConfig,
    DaThroughputValidationError, VerificationMode,
};
use std::collections::HashMap;

#[test]
fn default_config_is_valid_and_has_expected_fields() {
    let config = default_da_throughput_config();

    assert_eq!(config.strategy, DaScalingStrategy::PipelinedStreaming);
    assert_eq!(config.verification_mode, VerificationMode::BatchedKZG);
    assert_eq!(config.blobs_per_block, 6);
    assert_eq!(config.blob_size_bytes, 128 * 1024);
    assert_eq!(config.verification_parallelism, 8);
    assert!((config.target_da_bandwidth_mbps - 25.0).abs() < f64::EPSILON);
    assert!((config.max_verification_time_ms - 120.0).abs() < f64::EPSILON);
    assert_eq!(validate_da_throughput_config(&config), Ok(()));
}

#[test]
fn validation_rejects_zero_blobs_and_zero_parallelism() {
    let mut config = default_da_throughput_config();
    config.blobs_per_block = 0;
    config.verification_parallelism = 0;

    let errors = validate_da_throughput_config(&config).unwrap_err();
    assert!(errors.contains(&DaThroughputValidationError::ZeroBlobs));
    assert!(errors.contains(&DaThroughputValidationError::ZeroParallelism));
}

#[test]
fn validation_rejects_blob_size_above_maximum() {
    let mut config = default_da_throughput_config();
    config.blob_size_bytes = 3 * 1024 * 1024;

    let errors = validate_da_throughput_config(&config).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| matches!(error, DaThroughputValidationError::BlobSizeTooLarge { .. })));
}

#[test]
fn validation_rejects_low_bandwidth_and_high_verification_budget() {
    let config = DaThroughputConfig {
        strategy: DaScalingStrategy::IncreaseBlobCount,
        verification_mode: VerificationMode::FullKZG,
        blobs_per_block: 24,
        blob_size_bytes: 512 * 1024,
        verification_parallelism: 1,
        target_da_bandwidth_mbps: 0.1,
        max_verification_time_ms: 5.0,
    };

    let errors = validate_da_throughput_config(&config).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| matches!(error, DaThroughputValidationError::BandwidthTooLow { .. })));
    assert!(errors.iter().any(|error| matches!(
        error,
        DaThroughputValidationError::VerificationTimeTooHigh { .. }
    )));
}

#[test]
fn verification_time_decreases_with_parallelism() {
    let mode = VerificationMode::FullKZG;
    let blob_count = 16;
    let serial = compute_verification_time(mode, blob_count, 1);
    let parallel = compute_verification_time(mode, blob_count, 8);

    assert!(parallel < serial);
}

#[test]
fn tps_estimation_matches_expected_conversion() {
    let tps = estimate_tps_from_da(8.0, 200);
    let expected = 8_000_000.0 / 8.0 / 200.0;
    assert!((tps - expected).abs() < 1e-9);
    assert_eq!(estimate_tps_from_da(0.0, 200), 0.0);
    assert_eq!(estimate_tps_from_da(8.0, 0), 0.0);
}

#[test]
fn throughput_stats_are_positive_and_scaling_factor_is_nonzero() {
    let config = default_da_throughput_config();
    let stats = compute_da_throughput_stats(&config);

    assert!(stats.effective_da_bandwidth_mbps > 0.0);
    assert!(stats.blobs_per_second > 0.0);
    assert!(stats.verification_time_ms > 0.0);
    assert!(stats.tps_contribution > 0.0);
    assert!(!stats.bottleneck.is_empty());
    assert!(stats.scaling_factor > 0.0);
}

#[test]
fn compare_scaling_strategies_includes_all_strategies() {
    let config = default_da_throughput_config();
    let comparison = compare_scaling_strategies(&config);
    assert_eq!(comparison.len(), 6);

    let map: HashMap<String, f64> = comparison
        .into_iter()
        .map(|(name, stats)| (name, stats.tps_contribution))
        .collect();
    assert!(map.contains_key("IncreaseBlobCount"));
    assert!(map.contains_key("IncreaseBlobSize"));
    assert!(map.contains_key("ParallelVerification"));
    assert!(map.contains_key("PipelinedStreaming"));
    assert!(map.contains_key("CompressedBlobs"));
    assert!(map.contains_key("ShardedDA"));
}

#[test]
fn sharded_da_outperforms_blob_size_scaling_for_default_profile() {
    let config = default_da_throughput_config();
    let comparison = compare_scaling_strategies(&config);
    let map: HashMap<String, f64> = comparison
        .into_iter()
        .map(|(name, stats)| (name, stats.tps_contribution))
        .collect();

    let sharded = map.get("ShardedDA").copied().unwrap_or(0.0);
    let larger_blobs = map.get("IncreaseBlobSize").copied().unwrap_or(0.0);
    assert!(sharded > larger_blobs);
}

#[test]
fn da_commitment_is_order_invariant_and_config_sensitive() {
    let config = default_da_throughput_config();
    let roots_a = vec![[1_u8; 32], [2_u8; 32], [3_u8; 32]];
    let roots_b = vec![[3_u8; 32], [1_u8; 32], [2_u8; 32]];

    let commitment_a = compute_da_commitment(&config, &roots_a);
    let commitment_b = compute_da_commitment(&config, &roots_b);
    assert_eq!(commitment_a, commitment_b);

    let mut different_config = config.clone();
    different_config.verification_mode = VerificationMode::FullKZG;
    let commitment_c = compute_da_commitment(&different_config, &roots_a);
    assert_ne!(commitment_a, commitment_c);
}
