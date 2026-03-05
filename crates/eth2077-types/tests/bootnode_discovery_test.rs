use eth2077_types::bootnode_discovery::{
    compute_stats, is_healthy_network, Bootnode, BootnodeDiscoveryConfig, BootnodeHealth,
    DiscoveredPeer, DiscoveryProtocol, NodeReachability, TopologySnapshot, TopologyStrategy,
    ETH2077_TESTNET_NODE_COUNT,
};

fn mk_bootnode(id: &str, health: BootnodeHealth, peers: usize) -> Bootnode {
    Bootnode {
        id: id.to_string(),
        enode_url: format!("enode://{id}@127.0.0.1:30303"),
        enr_record: format!("enr:{id}"),
        ip_address: "127.0.0.1".to_string(),
        tcp_port: 30303,
        udp_port: 30303,
        health,
        region: "eu-west".to_string(),
        connected_peers: peers,
        uptime_seconds: 10_000,
    }
}

fn mk_peer(id: &str, reachability: NodeReachability, latency_ms: f64) -> DiscoveredPeer {
    DiscoveredPeer {
        node_id: id.to_string(),
        protocol: DiscoveryProtocol::Discv5,
        reachability,
        latency_ms,
        discovered_at_unix: 1_700_000_000,
        last_seen_unix: 1_700_000_123,
        client_version: "reth/v1.2.0".to_string(),
        capabilities: vec!["eth/68".to_string(), "snap/1".to_string()],
    }
}

#[test]
fn default_config_matches_requested_values() {
    let cfg = BootnodeDiscoveryConfig::default();
    assert_eq!(cfg.discovery_protocol, DiscoveryProtocol::Discv5);
    assert_eq!(cfg.topology_strategy, TopologyStrategy::KademliaDht);
    assert_eq!(cfg.target_peer_count, 25);
    assert_eq!(cfg.max_peer_count, 50);
    assert_eq!(cfg.bootnode_count, 4);
    assert_eq!(cfg.refresh_interval_seconds, 30);
    assert_eq!(cfg.eviction_timeout_seconds, 300);
    assert!(cfg.enable_nat_traversal);
    assert!(cfg.validate().is_empty());
}

#[test]
fn validation_reports_multiple_errors() {
    let cfg = BootnodeDiscoveryConfig {
        discovery_protocol: DiscoveryProtocol::Discv5,
        topology_strategy: TopologyStrategy::FullMesh,
        target_peer_count: 0,
        max_peer_count: 0,
        bootnode_count: ETH2077_TESTNET_NODE_COUNT + 1,
        refresh_interval_seconds: 1,
        eviction_timeout_seconds: 1,
        dns_discovery_url: Some("ftp://bad.example".to_string()),
        enable_nat_traversal: false,
    };

    let errors = cfg.validate();
    for field in [
        "target_peer_count",
        "max_peer_count",
        "bootnode_count",
        "refresh_interval_seconds",
        "eviction_timeout_seconds",
        "dns_discovery_url",
        "enable_nat_traversal",
    ] {
        assert!(errors.iter().any(|e| e.field == field));
    }
}

#[test]
fn compute_stats_without_snapshot_uses_bootnode_peer_average() {
    let bootnodes = vec![
        mk_bootnode("b1", BootnodeHealth::Online, 10),
        mk_bootnode("b2", BootnodeHealth::Degraded, 20),
        mk_bootnode("b3", BootnodeHealth::Offline, 30),
    ];
    let peers = vec![
        mk_peer("p1", NodeReachability::Public, 20.0),
        mk_peer("p2", NodeReachability::BehindNat, 40.0),
        mk_peer("p3", NodeReachability::Firewalled, -1.0),
    ];

    let stats = compute_stats(&bootnodes, &peers, None);
    assert_eq!(stats.total_bootnodes, 3);
    assert_eq!(stats.healthy_bootnodes, 2);
    assert_eq!(stats.discovered_peers, 3);
    assert_eq!(stats.reachable_peers, 2);
    assert!((stats.avg_latency_ms - 30.0).abs() < 1e-9);
    assert!((stats.avg_peer_count - 20.0).abs() < 1e-9);
    assert!(!stats.network_partitioned);
    assert_ne!(stats.commitment, [0u8; 32]);
}

#[test]
fn compute_stats_is_order_independent_for_commitment() {
    let bootnodes_a = vec![
        mk_bootnode("b2", BootnodeHealth::Online, 8),
        mk_bootnode("b1", BootnodeHealth::Online, 9),
    ];
    let peers_a = vec![
        mk_peer("p2", NodeReachability::Public, 18.0),
        mk_peer("p1", NodeReachability::Unknown, 35.0),
    ];

    let mut bootnodes_b = bootnodes_a.clone();
    bootnodes_b.reverse();
    let mut peers_b = peers_a.clone();
    peers_b.reverse();

    let stats_a = compute_stats(&bootnodes_a, &peers_a, None);
    let stats_b = compute_stats(&bootnodes_b, &peers_b, None);
    assert_eq!(stats_a.commitment, stats_b.commitment);
}

#[test]
fn healthy_network_checks_connectivity_and_partitioning() {
    let healthy = TopologySnapshot {
        timestamp_unix: 1,
        total_nodes: 48,
        connected_pairs: 780,
        avg_peer_count: 38.0,
        min_peer_count: 30,
        max_peer_count: 45,
        network_diameter: 6,
        partitioned: false,
    };
    assert!(is_healthy_network(&healthy, 0.65));

    let mut partitioned = healthy.clone();
    partitioned.partitioned = true;
    assert!(!is_healthy_network(&partitioned, 0.65));

    let mut sparse = healthy.clone();
    sparse.connected_pairs = 200;
    sparse.avg_peer_count = 5.0;
    assert!(!is_healthy_network(&sparse, 0.65));
}
