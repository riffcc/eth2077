use eth2077_types::ec_broadcast::{
    compare_topologies, compute_broadcast_commitment, compute_ec_broadcast_stats,
    default_ec_broadcast_config, estimate_coding_overhead, validate_ec_broadcast_config,
    BroadcastTopology, EcBroadcastConfig, EcBroadcastValidationError, ErasureCodingScheme,
};
use std::collections::HashMap;

#[test]
fn default_config_matches_expected_values() {
    let config = default_ec_broadcast_config();

    assert_eq!(config.coding_scheme, ErasureCodingScheme::ReedSolomon);
    assert_eq!(config.topology, BroadcastTopology::ErasureGossip);
    assert_eq!(config.shard_count, 64);
    assert!((config.redundancy_factor - 1.5).abs() < f64::EPSILON);
    assert_eq!(config.max_payload_bytes, 1_048_576);
    assert_eq!(config.fanout, 6);
    assert!((config.target_latency_ms - 240.0).abs() < f64::EPSILON);
    assert_eq!(config.node_count, 128);
    assert_eq!(validate_ec_broadcast_config(&config), Ok(()));
}

#[test]
fn validation_rejects_zero_shards() {
    let mut config = default_ec_broadcast_config();
    config.shard_count = 0;

    let errors = validate_ec_broadcast_config(&config).unwrap_err();
    assert!(errors.contains(&EcBroadcastValidationError::ZeroShards));
}

#[test]
fn validation_rejects_redundancy_outside_bounds() {
    let mut low = default_ec_broadcast_config();
    low.redundancy_factor = 0.8;
    let low_errors = validate_ec_broadcast_config(&low).unwrap_err();
    assert!(low_errors.contains(&EcBroadcastValidationError::RedundancyTooLow { value: 0.8 }));

    let mut high = default_ec_broadcast_config();
    high.redundancy_factor = 4.5;
    let high_errors = validate_ec_broadcast_config(&high).unwrap_err();
    assert!(high_errors.contains(&EcBroadcastValidationError::RedundancyTooHigh { value: 4.5 }));
}

#[test]
fn validation_rejects_payload_fanout_and_insufficient_nodes() {
    let config = EcBroadcastConfig {
        coding_scheme: ErasureCodingScheme::LDPCCode,
        topology: BroadcastTopology::GossipSub,
        shard_count: 16,
        redundancy_factor: 1.2,
        max_payload_bytes: 32 * 1024 * 1024,
        fanout: 12,
        target_latency_ms: 300.0,
        node_count: 8,
    };

    let errors = validate_ec_broadcast_config(&config).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| matches!(error, EcBroadcastValidationError::PayloadTooLarge { .. })));
    assert!(errors
        .iter()
        .any(|error| matches!(error, EcBroadcastValidationError::FanoutExceedsNodes { .. })));
    assert!(errors.contains(&EcBroadcastValidationError::InsufficientNodes));
}

#[test]
fn stats_match_expected_core_calculations() {
    let config = EcBroadcastConfig {
        coding_scheme: ErasureCodingScheme::ReedSolomon,
        topology: BroadcastTopology::TreeBroadcast,
        shard_count: 8,
        redundancy_factor: 1.5,
        max_payload_bytes: 800_000,
        fanout: 4,
        target_latency_ms: 160.0,
        node_count: 32,
    };

    let stats = compute_ec_broadcast_stats(&config);
    assert_eq!(stats.bandwidth_per_node_bytes, 150_000);
    assert_eq!(stats.total_network_bytes, 4_800_000);
    assert_eq!(stats.propagation_rounds, 3);
    assert_eq!(stats.recovery_threshold, 6);
    assert!((stats.redundancy_overhead - 0.5).abs() < 1e-12);
    assert!(stats.estimated_latency_ms > 0.0);
    assert!(stats.coding_efficiency > 0.0);
}

#[test]
fn propagation_rounds_drop_when_fanout_increases() {
    let mut low_fanout = default_ec_broadcast_config();
    low_fanout.topology = BroadcastTopology::GossipSub;
    low_fanout.node_count = 256;
    low_fanout.fanout = 2;

    let mut high_fanout = low_fanout.clone();
    high_fanout.fanout = 8;

    let low_stats = compute_ec_broadcast_stats(&low_fanout);
    let high_stats = compute_ec_broadcast_stats(&high_fanout);
    assert!(low_stats.propagation_rounds > high_stats.propagation_rounds);
}

#[test]
fn topology_comparison_returns_all_variants() {
    let config = default_ec_broadcast_config();
    let compared = compare_topologies(&config);
    assert_eq!(compared.len(), 5);

    let map: HashMap<String, f64> = compared
        .iter()
        .map(|(name, stats)| (name.clone(), stats.estimated_latency_ms))
        .collect();
    assert!(map.contains_key("FullMesh"));
    assert!(map.contains_key("GossipSub"));
    assert!(map.contains_key("TreeBroadcast"));
    assert!(map.contains_key("HybridPushPull"));
    assert!(map.contains_key("ErasureGossip"));

    let full_mesh = map.get("FullMesh").copied().unwrap_or(0.0);
    let erasure_gossip = map.get("ErasureGossip").copied().unwrap_or(0.0);
    assert!(full_mesh > erasure_gossip);
}

#[test]
fn coding_overhead_reflects_scheme_efficiency() {
    let redundancy = 2.0;
    let reed = estimate_coding_overhead(ErasureCodingScheme::ReedSolomon, redundancy);
    let ldpc = estimate_coding_overhead(ErasureCodingScheme::LDPCCode, redundancy);
    let fountain = estimate_coding_overhead(ErasureCodingScheme::FountainCode, redundancy);
    let raptor = estimate_coding_overhead(ErasureCodingScheme::RaptorCode, redundancy);

    assert!(reed > ldpc);
    assert!(ldpc > fountain);
    assert!(fountain > raptor);
}

#[test]
fn broadcast_commitment_is_deterministic_order_invariant_and_sensitive() {
    let config = default_ec_broadcast_config();
    let shard_hashes_a = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
    let shard_hashes_b = vec![[3u8; 32], [1u8; 32], [2u8; 32]];
    let shard_hashes_c = vec![[1u8; 32], [2u8; 32], [9u8; 32]];

    let hash_a = compute_broadcast_commitment(&config, &shard_hashes_a);
    let hash_b = compute_broadcast_commitment(&config, &shard_hashes_b);
    let hash_c = compute_broadcast_commitment(&config, &shard_hashes_c);

    let mut changed_config = config.clone();
    changed_config.fanout = config.fanout + 1;
    let hash_changed_cfg = compute_broadcast_commitment(&changed_config, &shard_hashes_a);

    assert_eq!(hash_a, hash_b);
    assert_ne!(hash_a, hash_c);
    assert_ne!(hash_a, hash_changed_cfg);
}
