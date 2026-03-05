use eth2077_types::container_deploy::{
    compute_container_deploy_commitment, compute_container_deploy_stats,
    default_container_deploy_config, validate_container_deploy_config, ContainerNode,
    DeployStrategy, HealthProbe, ImagePolicy, Orchestrator,
};
use std::collections::HashMap;

#[test]
fn enum_variants_are_complete() {
    assert_eq!(
        [
            Orchestrator::Kubernetes,
            Orchestrator::DockerSwarm,
            Orchestrator::Nomad,
            Orchestrator::Custom,
            Orchestrator::BareMetal,
            Orchestrator::CloudRun
        ]
        .len(),
        6
    );
    assert_eq!(
        [
            DeployStrategy::RollingUpdate,
            DeployStrategy::BlueGreen,
            DeployStrategy::Canary,
            DeployStrategy::Recreate,
            DeployStrategy::Staged,
            DeployStrategy::Manual
        ]
        .len(),
        6
    );
    assert_eq!(
        [
            HealthProbe::HttpGet,
            HealthProbe::TcpSocket,
            HealthProbe::GrpcHealth,
            HealthProbe::ExecCommand,
            HealthProbe::PeerCount,
            HealthProbe::SyncStatus
        ]
        .len(),
        6
    );
    assert_eq!(
        [
            ImagePolicy::AlwaysPull,
            ImagePolicy::IfNotPresent,
            ImagePolicy::Pinned,
            ImagePolicy::Digest,
            ImagePolicy::Signed,
            ImagePolicy::Attested
        ]
        .len(),
        6
    );
}

#[test]
fn default_config_is_valid() {
    let config = default_container_deploy_config();
    assert_eq!(config.target_replicas, 48);
    assert_eq!(validate_container_deploy_config(&config), Ok(()));
}

#[test]
fn validation_reports_multiple_issues() {
    let mut config = default_container_deploy_config();
    config.target_replicas = 32;
    config.strategy = DeployStrategy::BlueGreen;
    config.max_surge_pct = 20.0;
    config.health_check_interval_s = 2;
    config.metadata.insert(" ".to_string(), "bad".to_string());
    config
        .metadata
        .insert("image_digest".to_string(), " ".to_string());

    let errors = validate_container_deploy_config(&config).unwrap_err();
    for field in [
        "target_replicas",
        "max_surge_pct",
        "health_check_interval_s",
        "image_policy",
        "metadata",
    ] {
        assert!(errors.iter().any(|error| error.field == field));
    }
}

fn mk_node(id: &str, healthy: bool) -> ContainerNode {
    let metadata = HashMap::from([
        ("healthy".to_string(), healthy.to_string()),
        ("cpu_usage_pct".to_string(), "50".to_string()),
        ("memory_usage_pct".to_string(), "62.5".to_string()),
    ]);
    ContainerNode {
        id: id.to_string(),
        image: "ghcr.io/eth2077/node:stable".to_string(),
        orchestrator: Orchestrator::Kubernetes,
        strategy: DeployStrategy::RollingUpdate,
        health_probe: HealthProbe::HttpGet,
        cpu_limit_milli: 2000,
        memory_limit_mb: 4096,
        metadata,
    }
}

#[test]
fn stats_and_commitment_behave_as_expected() {
    let mut nodes = Vec::new();
    for i in 0..48 {
        nodes.push(mk_node(&format!("node-{i}"), true));
    }
    let stats = compute_container_deploy_stats(&nodes);
    assert_eq!(stats.total_containers, 48);
    assert_eq!(stats.healthy, 48);
    assert_eq!(stats.unhealthy, 0);
    assert!((stats.avg_cpu_usage_pct - 50.0).abs() < 1e-9);
    assert!((stats.avg_memory_usage_pct - 62.5).abs() < 1e-9);
    assert!(stats.rollout_complete);

    let config = default_container_deploy_config();
    let baseline = compute_container_deploy_commitment(&config);
    assert_eq!(baseline, compute_container_deploy_commitment(&config));

    let mut reordered = config.clone();
    reordered.metadata = HashMap::from([
        ("observability".to_string(), "enabled".to_string()),
        ("network".to_string(), "ETH2077-testnet".to_string()),
        ("rollout_guard".to_string(), "strict".to_string()),
        ("stage".to_string(), "containerized".to_string()),
        ("image_digest".to_string(), "sha256:pending".to_string()),
    ]);
    assert_eq!(baseline, compute_container_deploy_commitment(&reordered));

    let mut changed = config;
    changed.max_surge_pct = 10.0;
    assert_ne!(baseline, compute_container_deploy_commitment(&changed));
}
