use eth2077_types::teragas_l2::*;
use std::collections::HashSet;

fn assert_close(left: f64, right: f64, eps: f64) {
    assert!(
        (left - right).abs() <= eps,
        "left={left}, right={right}, eps={eps}"
    );
}

fn high_capacity_config() -> TeragasConfig {
    TeragasConfig {
        strategy: L2ScalingStrategy::DataSharding,
        data_lane: DataLaneType::BlobData,
        compression: CompressionScheme::StateProof,
        target_throughput_gbps: 1.2,
        current_throughput_mbps: 260.0,
        rollup_count: 20,
        batch_size_kb: 1_024,
        compression_ratio: 5.5,
        l1_blob_capacity_mbps: 1_500.0,
    }
}

#[test]
fn default_config_is_valid() {
    let config = default_teragas_config();
    assert_eq!(validate_teragas_config(&config), Ok(()));
}

#[test]
fn validation_rejects_target_below_current() {
    let mut config = default_teragas_config();
    config.current_throughput_mbps = 900.0;
    config.target_throughput_gbps = 0.2;

    let errors = validate_teragas_config(&config).unwrap_err();
    assert!(errors.contains(&TeragasValidationError::TargetBelowCurrent));
}

#[test]
fn validation_rejects_zero_rollups() {
    let mut config = default_teragas_config();
    config.rollup_count = 0;

    let errors = validate_teragas_config(&config).unwrap_err();
    assert!(errors.contains(&TeragasValidationError::ZeroRollups));
}

#[test]
fn validation_rejects_invalid_compression_ratio() {
    let mut config = default_teragas_config();
    config.compression_ratio = 0.75;

    let errors = validate_teragas_config(&config).unwrap_err();
    assert!(errors.iter().any(|error| matches!(
        error,
        TeragasValidationError::CompressionRatioInvalid { value: 0.75 }
    )));
}

#[test]
fn validation_rejects_oversized_batch() {
    let mut config = default_teragas_config();
    config.batch_size_kb = 20_000;

    let errors = validate_teragas_config(&config).unwrap_err();
    assert!(errors.iter().any(|error| matches!(
        error,
        TeragasValidationError::BatchSizeTooLarge {
            size_kb: 20_000,
            max_kb: 16_384
        }
    )));
}

#[test]
fn validation_rejects_insufficient_l1_capacity() {
    let mut config = default_teragas_config();
    config.target_throughput_gbps = 2.0;
    config.data_lane = DataLaneType::CallData;
    config.compression_ratio = 1.0;
    config.l1_blob_capacity_mbps = 400.0;

    let errors = validate_teragas_config(&config).unwrap_err();
    assert!(errors.contains(&TeragasValidationError::InsufficientL1Capacity));
}

#[test]
fn stats_show_positive_scaling() {
    let config = default_teragas_config();
    let stats = compute_teragas_stats(&config);

    assert!(stats.scaling_factor > 1.0);
    assert!(stats.projected_throughput_gbps > config.current_throughput_mbps / 1_000.0);
    assert!(stats.effective_compression >= 1.0);
    assert!(stats.equivalent_tps > 0.0);
}

#[test]
fn stats_gap_is_zero_when_target_met() {
    let config = high_capacity_config();
    let stats = compute_teragas_stats(&config);

    assert!(stats.meets_target);
    assert_close(stats.gap_pct, 0.0, 1e-9);
    assert!(stats.projected_throughput_gbps >= config.target_throughput_gbps);
}

#[test]
fn stats_gap_is_positive_when_target_missed() {
    let mut config = default_teragas_config();
    config.strategy = L2ScalingStrategy::BaseLayerDA;
    config.data_lane = DataLaneType::CallData;
    config.compression = CompressionScheme::None;
    config.target_throughput_gbps = 3.0;
    config.current_throughput_mbps = 100.0;
    config.rollup_count = 4;
    config.batch_size_kb = 64;
    config.l1_blob_capacity_mbps = 120.0;

    let stats = compute_teragas_stats(&config);

    assert!(!stats.meets_target);
    assert!(stats.gap_pct > 0.0);
    assert!(stats.bottleneck == "L1DataCapacity" || stats.bottleneck == "CompressionLimits");
}

#[test]
fn compare_strategies_returns_all_variants() {
    let config = default_teragas_config();
    let comparisons = compare_l2_strategies(&config);

    assert_eq!(comparisons.len(), 6);

    let names: HashSet<String> = comparisons.into_iter().map(|(name, _)| name).collect();
    let expected: HashSet<String> = vec![
        "RollupStacking".to_string(),
        "ValidiumHybrid".to_string(),
        "DataSharding".to_string(),
        "ParallelRollups".to_string(),
        "BaseLayerDA".to_string(),
        "InterleavedBatching".to_string(),
    ]
    .into_iter()
    .collect();

    assert_eq!(names, expected);
}

#[test]
fn rollup_efficiency_decreases_with_count() {
    let batch = 512;
    let low_count_eff = estimate_rollup_efficiency(4, batch);
    let medium_count_eff = estimate_rollup_efficiency(16, batch);
    let high_count_eff = estimate_rollup_efficiency(48, batch);

    assert!(low_count_eff > medium_count_eff);
    assert!(medium_count_eff > high_count_eff);
    assert!(high_count_eff >= 0.10);
}

#[test]
fn rollup_efficiency_benefits_from_larger_batches() {
    let rollups = 24;
    let small_batch_eff = estimate_rollup_efficiency(rollups, 64);
    let large_batch_eff = estimate_rollup_efficiency(rollups, 2_048);

    assert!(large_batch_eff > small_batch_eff);
}

#[test]
fn commitment_is_deterministic_and_sensitive() {
    let config = default_teragas_config();
    let first = compute_teragas_commitment(&config);
    let second = compute_teragas_commitment(&config);
    assert_eq!(first, second);

    let mut changed = config.clone();
    changed.data_lane = DataLaneType::BlobData;
    let third = compute_teragas_commitment(&changed);
    assert_ne!(first, third);

    let mut changed_again = config.clone();
    changed_again.compression_ratio += 0.01;
    let fourth = compute_teragas_commitment(&changed_again);
    assert_ne!(first, fourth);
}
