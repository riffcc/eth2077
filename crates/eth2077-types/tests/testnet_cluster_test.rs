use eth2077_types::testnet_cluster::{
    compute_testnet_cluster_commitment, compute_testnet_cluster_stats,
    default_testnet_cluster_config, validate_testnet_cluster_config, ClusterHealth, NodeRole,
    ReadinessGate, TestnetClusterConfig, TestnetNode, TopologyKind,
};
use std::collections::HashMap;

#[test]
fn enum_variant_sets_are_complete() {
    let roles = [
        NodeRole::Validator,
        NodeRole::FullNode,
        NodeRole::BootNode,
        NodeRole::Archive,
        NodeRole::LightClient,
        NodeRole::Bridge,
    ];
    let health = [
        ClusterHealth::Healthy,
        ClusterHealth::Degraded,
        ClusterHealth::Partitioned,
        ClusterHealth::Recovering,
        ClusterHealth::Launching,
        ClusterHealth::Stopped,
    ];
    let topologies = [
        TopologyKind::FullMesh,
        TopologyKind::Ring,
        TopologyKind::Star,
        TopologyKind::Random,
        TopologyKind::Geographic,
        TopologyKind::Hierarchical,
    ];
    let gates = [
        ReadinessGate::SyncComplete,
        ReadinessGate::ValidatorQuorum,
        ReadinessGate::GenesisLocked,
        ReadinessGate::ForkScheduleSet,
        ReadinessGate::MonitoringUp,
        ReadinessGate::FaucetLive,
    ];

    assert_eq!(roles.len(), 6);
    assert_eq!(health.len(), 6);
    assert_eq!(topologies.len(), 6);
    assert_eq!(gates.len(), 6);
}

#[test]
fn default_config_is_valid() {
    let config = default_testnet_cluster_config();
    assert_eq!(config.target_node_count, 48);
    assert_eq!(config.min_validators, 32);
    assert_eq!(validate_testnet_cluster_config(&config), Ok(()));
}

#[test]
fn validation_reports_multiple_issues() {
    let mut config = default_testnet_cluster_config();
    config.target_node_count = 50;
    config.min_validators = 64;
    config.max_partition_tolerance = 0.9;
    config.health_check_interval_s = 1;
    config.launch_gates = vec![ReadinessGate::SyncComplete, ReadinessGate::SyncComplete];
    config.metadata.insert(" ".to_string(), "bad".to_string());

    let errors = validate_testnet_cluster_config(&config).unwrap_err();
    for field in [
        "target_node_count",
        "min_validators",
        "max_partition_tolerance",
        "health_check_interval_s",
        "launch_gates",
        "metadata",
    ] {
        assert!(errors.iter().any(|error| error.field == field));
    }
}

fn mk_node(id: &str, role: NodeRole, peers: usize, sync_pct: f64, healthy: bool) -> TestnetNode {
    TestnetNode {
        id: id.to_string(),
        role,
        region: "us-east".to_string(),
        peer_count: peers,
        sync_pct,
        is_healthy: healthy,
        metadata: HashMap::from([("monitoring".to_string(), "up".to_string())]),
    }
}

#[test]
fn stats_compute_expected_and_gate_health() {
    let mut config: TestnetClusterConfig = default_testnet_cluster_config();
    config.min_validators = 4;

    let nodes = vec![
        mk_node("v1", NodeRole::Validator, 5, 100.0, true),
        mk_node("v2", NodeRole::Validator, 5, 99.8, true),
        mk_node("v3", NodeRole::Validator, 5, 99.6, true),
        mk_node("v4", NodeRole::Validator, 5, 99.5, true),
        mk_node("f1", NodeRole::FullNode, 5, 99.4, true),
        mk_node("f2", NodeRole::FullNode, 5, 99.3, true),
    ];

    let stats = compute_testnet_cluster_stats(&nodes, &config);

    assert_eq!(stats.total_nodes, 6);
    assert_eq!(stats.validators, 4);
    assert_eq!(stats.full_nodes, 2);
    assert_eq!(stats.gates_passed, config.launch_gates.len());
    assert_eq!(stats.health, ClusterHealth::Healthy);
    assert!((stats.avg_sync_pct - 99.6).abs() < 1e-9);
    assert!(stats.peer_density > 0.95);
}

#[test]
fn stats_detect_partitioned_cluster() {
    let mut config = default_testnet_cluster_config();
    config.topology = TopologyKind::Geographic;
    config.max_partition_tolerance = 0.10;

    let nodes = vec![
        mk_node("v1", NodeRole::Validator, 0, 95.0, true),
        mk_node("v2", NodeRole::Validator, 0, 94.0, true),
        mk_node("v3", NodeRole::Validator, 1, 93.0, false),
        mk_node("f1", NodeRole::FullNode, 0, 92.0, true),
    ];

    let stats = compute_testnet_cluster_stats(&nodes, &config);
    assert_eq!(stats.health, ClusterHealth::Partitioned);
}

#[test]
fn commitment_is_deterministic_and_sensitive() {
    let config = default_testnet_cluster_config();
    let baseline = compute_testnet_cluster_commitment(&config);
    assert_eq!(baseline, compute_testnet_cluster_commitment(&config));

    let mut reordered = config.clone();
    reordered.launch_gates.reverse();
    reordered.metadata = HashMap::from([
        ("monitoring_up".to_string(), "true".to_string()),
        ("network".to_string(), "ETH2077-testnet".to_string()),
        ("faucet_live".to_string(), "true".to_string()),
        ("fork_schedule_set".to_string(), "true".to_string()),
        ("genesis_locked".to_string(), "true".to_string()),
        ("stage".to_string(), "public".to_string()),
    ]);
    assert_eq!(baseline, compute_testnet_cluster_commitment(&reordered));

    let mut changed = config.clone();
    changed.min_validators = 31;
    assert_ne!(baseline, compute_testnet_cluster_commitment(&changed));
}
