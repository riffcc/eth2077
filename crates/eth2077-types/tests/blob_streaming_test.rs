use eth2077_types::blob_streaming::{
    compare_propagation_modes, compute_blob_commitment, compute_blob_stream_stats,
    compute_das_security, default_blob_stream_config, estimate_propagation_latency,
    validate_blob_stream_config, BlobPropagationMode, BlobStreamConfig, BlobStreamValidationError,
    DASStrategy,
};
use std::collections::HashMap;

#[test]
fn default_blob_stream_config_matches_expected_values() {
    let config = default_blob_stream_config();

    assert_eq!(config.blobs_per_block, 6);
    assert_eq!(config.blob_size_bytes, 128 * 1024);
    assert_eq!(config.propagation_mode, BlobPropagationMode::Sidecar);
    assert_eq!(config.das_strategy, DASStrategy::PeerDAS);
    assert!((config.target_da_bandwidth_mbps - 10.0).abs() < f64::EPSILON);
    assert_eq!(config.max_blob_latency_ms, 2_000);
    assert!((config.erasure_coding_rate - 0.5).abs() < f64::EPSILON);
    assert_eq!(config.sample_count, 75);
    assert_eq!(validate_blob_stream_config(&config), Ok(()));
}

#[test]
fn validation_accepts_reasonable_configuration() {
    let config = BlobStreamConfig {
        blobs_per_block: 8,
        blob_size_bytes: 96 * 1024,
        propagation_mode: BlobPropagationMode::InterleavedStream,
        das_strategy: DASStrategy::RowColumn2D,
        target_da_bandwidth_mbps: 30.0,
        max_blob_latency_ms: 2_400,
        erasure_coding_rate: 0.6,
        sample_count: 80,
    };

    assert_eq!(validate_blob_stream_config(&config), Ok(()));
}

#[test]
fn validation_rejects_zero_blob_fields() {
    let mut config = default_blob_stream_config();
    config.blobs_per_block = 0;
    config.blob_size_bytes = 0;

    let errors = validate_blob_stream_config(&config).unwrap_err();
    assert!(errors.contains(&BlobStreamValidationError::ZeroBlobs));
    assert!(errors.contains(&BlobStreamValidationError::ZeroBlobSize));
}

#[test]
fn validation_rejects_invalid_erasure_rate_and_oversampling() {
    let config = BlobStreamConfig {
        blobs_per_block: 1,
        blob_size_bytes: 4 * 1024,
        propagation_mode: BlobPropagationMode::DASampled,
        das_strategy: DASStrategy::RandomSampling,
        target_da_bandwidth_mbps: 50.0,
        max_blob_latency_ms: 2_000,
        erasure_coding_rate: 0.0,
        sample_count: 10,
    };

    let errors = validate_blob_stream_config(&config).unwrap_err();
    assert!(errors.contains(&BlobStreamValidationError::InvalidErasureRate));
    assert!(errors.iter().any(|error| matches!(
        error,
        BlobStreamValidationError::SampleCountExceedsTotal { .. }
    )));
}

#[test]
fn validation_rejects_bandwidth_and_latency_budget_violations() {
    let mut config = default_blob_stream_config();
    config.target_da_bandwidth_mbps = 0.5;
    config.max_blob_latency_ms = 2_000;

    let errors = validate_blob_stream_config(&config).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| matches!(error, BlobStreamValidationError::BandwidthTooLow { .. })));
    assert!(errors.iter().any(|error| matches!(
        error,
        BlobStreamValidationError::LatencyBudgetExceeded { .. }
    )));
}

#[test]
fn stats_computation_returns_consistent_metrics() {
    let config = default_blob_stream_config();
    let stats = compute_blob_stream_stats(&config);

    assert_eq!(stats.total_da_bytes_per_block, 6 * 128 * 1024);
    assert!(stats.effective_da_throughput_mbps > 0.0);
    assert!(stats.bandwidth_utilization > 0.0);
    assert!(stats.estimated_propagation_ms > 0.0);
    assert!(stats.das_security_bits > 0.0);
    assert_eq!(stats.mode_comparison.len(), 5);
}

#[test]
fn das_security_increases_with_more_samples() {
    let mut low = default_blob_stream_config();
    low.das_strategy = DASStrategy::RandomSampling;
    low.sample_count = 24;

    let mut high = low.clone();
    high.sample_count = 120;

    assert!(compute_das_security(&high) > compute_das_security(&low));
}

#[test]
fn propagation_latency_improves_with_more_streaming_friendly_modes() {
    let base = BlobStreamConfig {
        blobs_per_block: 12,
        blob_size_bytes: 128 * 1024,
        propagation_mode: BlobPropagationMode::PostBlock,
        das_strategy: DASStrategy::FullDownload,
        target_da_bandwidth_mbps: 25.0,
        max_blob_latency_ms: 4_000,
        erasure_coding_rate: 0.5,
        sample_count: 100,
    };

    let post = estimate_propagation_latency(&base);

    let mut sidecar_cfg = base.clone();
    sidecar_cfg.propagation_mode = BlobPropagationMode::Sidecar;
    let sidecar = estimate_propagation_latency(&sidecar_cfg);

    let mut interleaved_cfg = base.clone();
    interleaved_cfg.propagation_mode = BlobPropagationMode::InterleavedStream;
    let interleaved = estimate_propagation_latency(&interleaved_cfg);

    let mut push_cfg = base.clone();
    push_cfg.propagation_mode = BlobPropagationMode::PushBased;
    let push = estimate_propagation_latency(&push_cfg);

    let mut sampled_cfg = base.clone();
    sampled_cfg.propagation_mode = BlobPropagationMode::DASampled;
    let sampled = estimate_propagation_latency(&sampled_cfg);

    assert!(post > sidecar);
    assert!(sidecar > interleaved);
    assert!(interleaved > push);
    assert!(push > sampled);
}

#[test]
fn mode_comparison_covers_all_modes_and_ranks_push_above_post_block() {
    let config = default_blob_stream_config();
    let comparison = compare_propagation_modes(&config);
    assert_eq!(comparison.len(), 5);

    let map: HashMap<String, f64> = comparison.into_iter().collect();
    assert!(map.contains_key("PostBlock"));
    assert!(map.contains_key("Sidecar"));
    assert!(map.contains_key("InterleavedStream"));
    assert!(map.contains_key("PushBased"));
    assert!(map.contains_key("DASampled"));

    let post = map.get("PostBlock").copied().unwrap_or(0.0);
    let push = map.get("PushBased").copied().unwrap_or(0.0);
    assert!(push > post);
}

#[test]
fn blob_commitment_is_order_invariant_and_data_sensitive() {
    let blobs_a = vec![
        b"blob-alpha".to_vec(),
        b"blob-beta".to_vec(),
        b"blob-gamma".to_vec(),
    ];
    let blobs_b = vec![
        b"blob-gamma".to_vec(),
        b"blob-alpha".to_vec(),
        b"blob-beta".to_vec(),
    ];
    let blobs_c = vec![
        b"blob-alpha".to_vec(),
        b"blob-beta".to_vec(),
        b"blob-gamma-mutated".to_vec(),
    ];

    let commitment_a = compute_blob_commitment(&blobs_a);
    let commitment_b = compute_blob_commitment(&blobs_b);
    let commitment_c = compute_blob_commitment(&blobs_c);

    assert_eq!(commitment_a, commitment_b);
    assert_ne!(commitment_a, commitment_c);
}
