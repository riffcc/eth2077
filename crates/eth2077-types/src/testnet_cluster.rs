//! Public testnet cluster modeling types for ETH2077.
//!
//! This module models deployment and launch operations for a fixed-size,
//! 48-node public testnet. It captures:
//! - node role topology,
//! - coarse cluster health states,
//! - launch-readiness gates,
//! - deterministic configuration commitments.
//!
//! The API is intentionally deterministic and side-effect-free:
//! - [`default_testnet_cluster_config`] returns a conservative baseline policy.
//! - [`validate_testnet_cluster_config`] emits complete field-scoped errors.
//! - [`compute_testnet_cluster_stats`] aggregates runtime node observations.
//! - [`compute_testnet_cluster_commitment`] returns a stable SHA-256 hex digest.
//!
//! Determinism notes:
//! - commitment hashing sorts metadata and gate labels,
//! - health/state computations avoid reliance on map iteration order,
//! - empty node inputs produce safe default stats.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Operational role for a node in the public testnet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeRole {
    /// Consensus participant producing and attesting to blocks.
    Validator,
    /// Fully synced execution+consensus follower serving RPC/state.
    FullNode,
    /// Discovery endpoint used for initial peer bootstrapping.
    BootNode,
    /// Historical retention node preserving full chain history.
    Archive,
    /// Reduced-resource node following light protocol paths.
    LightClient,
    /// Interop or cross-domain connectivity endpoint.
    Bridge,
}

/// Coarse-grained health state for the cluster.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClusterHealth {
    /// Stable, well-connected, and launch-capable.
    Healthy,
    /// Running but below target reliability or convergence.
    Degraded,
    /// Network appears split or materially under-connected.
    Partitioned,
    /// Improving after instability but not fully healthy yet.
    Recovering,
    /// Startup/convergence phase prior to full launch readiness.
    Launching,
    /// Effectively unavailable or entirely unhealthy.
    Stopped,
}

/// Declared network topology style.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TopologyKind {
    /// Every node aims to connect to all other nodes.
    FullMesh,
    /// Nodes connect in a ring with neighboring peers.
    Ring,
    /// Hub-and-spoke style with central relay points.
    Star,
    /// Peer sets are sampled pseudo-randomly.
    Random,
    /// Peering is region-aware with locality bias.
    Geographic,
    /// Tiered peering across layered infrastructure.
    Hierarchical,
}

/// Launch gate used to decide if public rollout can proceed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReadinessGate {
    /// Cluster state synchronization has converged.
    SyncComplete,
    /// Validator count and healthy quorum threshold are met.
    ValidatorQuorum,
    /// Genesis parameters are locked and immutable.
    GenesisLocked,
    /// Scheduled fork/upgrade activation data is configured.
    ForkScheduleSet,
    /// Monitoring and alerting surface is operational.
    MonitoringUp,
    /// Public faucet service is available.
    FaucetLive,
}

/// Runtime status for one testnet node.
///
/// `sync_pct` is interpreted in percentage points and normalized to `[0, 100]`
/// during aggregate calculations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestnetNode {
    /// Stable node identifier.
    pub id: String,
    /// Assigned operational role.
    pub role: NodeRole,
    /// Region label (for example `us-east` or `eu-west`).
    pub region: String,
    /// Current active peer count.
    pub peer_count: usize,
    /// Sync completion percentage.
    pub sync_pct: f64,
    /// Binary health signal from orchestration.
    pub is_healthy: bool,
    /// Free-form extension metadata.
    pub metadata: HashMap<String, String>,
}

/// Cluster-level deployment and launch policy.
///
/// This policy mixes size/quorum constraints with readiness gate definitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestnetClusterConfig {
    /// Expected cluster node count.
    pub target_node_count: usize,
    /// Minimum validator count for launch.
    pub min_validators: usize,
    /// Deployment topology profile.
    pub topology: TopologyKind,
    /// Maximum tolerated partition fraction in `[0, 1]`.
    pub max_partition_tolerance: f64,
    /// Health poll interval in seconds.
    pub health_check_interval_s: u64,
    /// Required launch gates.
    pub launch_gates: Vec<ReadinessGate>,
    /// Free-form governance and environment metadata.
    pub metadata: HashMap<String, String>,
}

/// Field-scoped validation error emitted by config checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestnetClusterValidationError {
    /// Field name associated with the problem.
    pub field: String,
    /// Human-readable explanation.
    pub reason: String,
}

/// Aggregated cluster metrics produced from runtime node data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestnetClusterStats {
    /// Number of nodes in the evaluated sample.
    pub total_nodes: usize,
    /// Count of validator-role nodes.
    pub validators: usize,
    /// Count of full-node-role nodes.
    pub full_nodes: usize,
    /// Derived coarse health status.
    pub health: ClusterHealth,
    /// Mean sync percentage.
    pub avg_sync_pct: f64,
    /// Normalized directed-link density in `[0, 1]`.
    pub peer_density: f64,
    /// Number of configured launch gates currently passing.
    pub gates_passed: usize,
}

/// Returns default policy for ETH2077 public testnet deployment.
///
/// Defaults are conservative and launch-oriented:
/// - fixed target of 48 nodes,
/// - 32 minimum validators,
/// - geographic topology,
/// - six launch gates enabled by default,
/// - metadata pre-seeded with readiness toggles.
pub fn default_testnet_cluster_config() -> TestnetClusterConfig {
    let mut metadata = HashMap::new();
    metadata.insert("network".to_string(), "ETH2077-testnet".to_string());
    metadata.insert("stage".to_string(), "public".to_string());
    metadata.insert("genesis_locked".to_string(), "true".to_string());
    metadata.insert("fork_schedule_set".to_string(), "true".to_string());
    metadata.insert("monitoring_up".to_string(), "true".to_string());
    metadata.insert("faucet_live".to_string(), "true".to_string());

    TestnetClusterConfig {
        target_node_count: 48,
        min_validators: 32,
        topology: TopologyKind::Geographic,
        max_partition_tolerance: 0.25,
        health_check_interval_s: 30,
        launch_gates: vec![
            ReadinessGate::SyncComplete,
            ReadinessGate::ValidatorQuorum,
            ReadinessGate::GenesisLocked,
            ReadinessGate::ForkScheduleSet,
            ReadinessGate::MonitoringUp,
            ReadinessGate::FaucetLive,
        ],
        metadata,
    }
}

/// Validates deployment policy and returns all discovered issues.
///
/// Validation rules:
/// - `target_node_count` must be exactly `48` for this public cluster module.
/// - `min_validators` must be within `[16, target_node_count]`.
/// - `max_partition_tolerance` must be finite and within `[0.0, 0.5]`.
/// - `health_check_interval_s` must be in `[5, 300]`.
/// - `launch_gates` must have at least three entries and no duplicates.
/// - metadata keys/values must be non-empty after trimming.
pub fn validate_testnet_cluster_config(
    config: &TestnetClusterConfig,
) -> Result<(), Vec<TestnetClusterValidationError>> {
    let mut errors = Vec::new();

    if config.target_node_count != 48 {
        push_validation_error(
            &mut errors,
            "target_node_count",
            "must be exactly 48 for the public testnet cluster",
        );
    }

    if config.min_validators < 16 {
        push_validation_error(
            &mut errors,
            "min_validators",
            "must be at least 16 for quorum resilience",
        );
    }
    if config.min_validators > config.target_node_count {
        push_validation_error(
            &mut errors,
            "min_validators",
            "must be less than or equal to target_node_count",
        );
    }

    if !config.max_partition_tolerance.is_finite()
        || config.max_partition_tolerance < 0.0
        || config.max_partition_tolerance > 0.5
    {
        push_validation_error(
            &mut errors,
            "max_partition_tolerance",
            "must be finite and within [0.0, 0.5]",
        );
    }

    if config.health_check_interval_s < 5 || config.health_check_interval_s > 300 {
        push_validation_error(
            &mut errors,
            "health_check_interval_s",
            "must be within [5, 300] seconds",
        );
    }

    if config.launch_gates.len() < 3 {
        push_validation_error(
            &mut errors,
            "launch_gates",
            "must include at least 3 readiness gates",
        );
    }

    let mut seen: HashMap<String, usize> = HashMap::new();
    for gate in &config.launch_gates {
        let label = readiness_gate_label(gate).to_string();
        let count = seen.entry(label.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            push_validation_error(
                &mut errors,
                "launch_gates",
                &format!("contains duplicate gate `{label}`"),
            );
        }
    }

    for (key, value) in &config.metadata {
        if key.trim().is_empty() {
            push_validation_error(
                &mut errors,
                "metadata",
                "metadata keys must not be empty",
            );
        }
        if value.trim().is_empty() {
            push_validation_error(
                &mut errors,
                "metadata",
                &format!("metadata value for key `{key}` must not be empty"),
            );
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Computes aggregate stats from runtime node observations.
///
/// Outputs include role counts, average sync, peer density, gate pass count,
/// and a derived [`ClusterHealth`] state.
pub fn compute_testnet_cluster_stats(
    nodes: &[TestnetNode],
    config: &TestnetClusterConfig,
) -> TestnetClusterStats {
    let total_nodes = nodes.len();
    let validators = nodes
        .iter()
        .filter(|node| node.role == NodeRole::Validator)
        .count();
    let full_nodes = nodes
        .iter()
        .filter(|node| node.role == NodeRole::FullNode)
        .count();
    let healthy_nodes = nodes.iter().filter(|node| node.is_healthy).count();
    let avg_sync_pct = compute_average_sync_pct(nodes);
    let peer_density = compute_peer_density(nodes);

    let gates_passed = config
        .launch_gates
        .iter()
        .filter(|gate| is_gate_passed(gate, nodes, config, avg_sync_pct, validators))
        .count();

    let health = classify_cluster_health(
        nodes,
        config,
        healthy_nodes,
        avg_sync_pct,
        peer_density,
        gates_passed,
    );

    TestnetClusterStats {
        total_nodes,
        validators,
        full_nodes,
        health,
        avg_sync_pct,
        peer_density,
        gates_passed,
    }
}

/// Computes deterministic SHA-256 hex commitment of config.
///
/// Included material:
/// - scalar fields,
/// - topology label,
/// - sorted readiness gate labels,
/// - sorted metadata pairs.
pub fn compute_testnet_cluster_commitment(config: &TestnetClusterConfig) -> String {
    let mut hasher = Sha256::new();

    hasher.update(format!(
        "target_node_count={}|min_validators={}|topology={}|max_partition_tolerance={:.6}|health_check_interval_s={}|",
        config.target_node_count,
        config.min_validators,
        topology_label(&config.topology),
        config.max_partition_tolerance,
        config.health_check_interval_s,
    ));

    let mut gate_labels: Vec<&'static str> =
        config.launch_gates.iter().map(readiness_gate_label).collect();
    gate_labels.sort_unstable();
    for label in gate_labels {
        hasher.update(label.as_bytes());
        hasher.update(b";");
    }

    let mut metadata_pairs: Vec<(&String, &String)> = config.metadata.iter().collect();
    metadata_pairs.sort_by(|(ka, va), (kb, vb)| ka.cmp(kb).then(va.cmp(vb)));
    for (key, value) in metadata_pairs {
        hasher.update(key.as_bytes());
        hasher.update(b"=");
        hasher.update(value.as_bytes());
        hasher.update(b";");
    }

    let digest = hasher.finalize();
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push_str(&format!("{:02x}", byte));
    }
    output
}

/// Appends one structured validation error.
fn push_validation_error(
    errors: &mut Vec<TestnetClusterValidationError>,
    field: &str,
    reason: &str,
) {
    errors.push(TestnetClusterValidationError {
        field: field.to_string(),
        reason: reason.to_string(),
    });
}

/// Computes mean sync percentage after clamping each node to `[0, 100]`.
fn compute_average_sync_pct(nodes: &[TestnetNode]) -> f64 {
    if nodes.is_empty() {
        return 0.0;
    }

    let total = nodes
        .iter()
        .map(|node| node.sync_pct.clamp(0.0, 100.0))
        .sum::<f64>();

    total / nodes.len() as f64
}

/// Computes normalized directed-link density in `[0, 1]`.
///
/// Formula: `sum(min(peer_count, n - 1)) / (n * (n - 1))`.
fn compute_peer_density(nodes: &[TestnetNode]) -> f64 {
    let n = nodes.len();
    if n <= 1 {
        return 0.0;
    }

    let max_peers_per_node = n - 1;
    let observed_links: usize = nodes
        .iter()
        .map(|node| node.peer_count.min(max_peers_per_node))
        .sum();

    observed_links as f64 / (n * max_peers_per_node) as f64
}

/// Evaluates whether a single readiness gate currently passes.
fn is_gate_passed(
    gate: &ReadinessGate,
    nodes: &[TestnetNode],
    config: &TestnetClusterConfig,
    avg_sync_pct: f64,
    validators: usize,
) -> bool {
    match gate {
        ReadinessGate::SyncComplete => {
            !nodes.is_empty()
                && avg_sync_pct >= 99.0
                && nodes.iter().all(|node| node.sync_pct.clamp(0.0, 100.0) >= 97.0)
        }
        ReadinessGate::ValidatorQuorum => {
            let healthy_validators = nodes
                .iter()
                .filter(|node| node.role == NodeRole::Validator && node.is_healthy)
                .count();
            let minimum_healthy = ((config.min_validators * 2) + 2) / 3;
            validators >= config.min_validators && healthy_validators >= minimum_healthy
        }
        ReadinessGate::GenesisLocked => {
            metadata_truthy(&config.metadata, "genesis_locked")
                || metadata_truthy(&config.metadata, "genesis_lock")
        }
        ReadinessGate::ForkScheduleSet => {
            metadata_truthy(&config.metadata, "fork_schedule_set")
                || config
                    .metadata
                    .get("fork_schedule")
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
        }
        ReadinessGate::MonitoringUp => {
            metadata_truthy(&config.metadata, "monitoring_up")
                || nodes.iter().any(|node| {
                    node.metadata
                        .get("monitoring")
                        .map(|value| is_affirmative(value))
                        .unwrap_or(false)
                })
        }
        ReadinessGate::FaucetLive => {
            metadata_truthy(&config.metadata, "faucet_live")
                || config
                    .metadata
                    .get("faucet_status")
                    .map(|value| is_live(value))
                    .unwrap_or(false)
        }
    }
}

/// Classifies cluster health from sync, connectivity, and gate posture.
fn classify_cluster_health(
    nodes: &[TestnetNode],
    config: &TestnetClusterConfig,
    healthy_nodes: usize,
    avg_sync_pct: f64,
    peer_density: f64,
    gates_passed: usize,
) -> ClusterHealth {
    let total_nodes = nodes.len();
    if total_nodes == 0 || healthy_nodes == 0 {
        return ClusterHealth::Stopped;
    }

    let low_peer_ratio = low_peer_fraction(nodes, &config.topology);
    if low_peer_ratio > config.max_partition_tolerance || peer_density < 0.20 {
        return ClusterHealth::Partitioned;
    }

    let healthy_ratio = healthy_nodes as f64 / total_nodes as f64;
    if gates_passed < config.launch_gates.len() && avg_sync_pct < 99.0 {
        return ClusterHealth::Launching;
    }

    if healthy_ratio >= 0.95 && avg_sync_pct >= 99.0 {
        ClusterHealth::Healthy
    } else if healthy_ratio >= 0.75 && avg_sync_pct >= 92.0 {
        ClusterHealth::Recovering
    } else {
        ClusterHealth::Degraded
    }
}

/// Computes fraction of nodes below topology-informed minimum peer target.
fn low_peer_fraction(nodes: &[TestnetNode], topology: &TopologyKind) -> f64 {
    if nodes.is_empty() {
        return 0.0;
    }

    let min_expected = min_expected_peers(topology, nodes.len());
    if min_expected == 0 {
        return 0.0;
    }

    let low_peer_nodes = nodes
        .iter()
        .filter(|node| node.peer_count < min_expected)
        .count();

    low_peer_nodes as f64 / nodes.len() as f64
}

/// Returns per-node minimum expected peer count for each topology profile.
fn min_expected_peers(topology: &TopologyKind, total_nodes: usize) -> usize {
    if total_nodes <= 1 {
        return 0;
    }

    let max_peer_count = total_nodes - 1;
    match topology {
        TopologyKind::FullMesh => max_peer_count,
        TopologyKind::Ring => 2.min(max_peer_count),
        TopologyKind::Star => 1,
        TopologyKind::Random => ((total_nodes as f64 * 0.20).ceil() as usize)
            .max(3)
            .min(max_peer_count),
        TopologyKind::Geographic => ((total_nodes as f64 * 0.15).ceil() as usize)
            .max(3)
            .min(max_peer_count),
        TopologyKind::Hierarchical => ((total_nodes as f64 * 0.10).ceil() as usize)
            .max(2)
            .min(max_peer_count),
    }
}

/// Returns whether metadata contains an affirmative value for `key`.
fn metadata_truthy(metadata: &HashMap<String, String>, key: &str) -> bool {
    metadata
        .get(key)
        .map(|value| is_affirmative(value))
        .unwrap_or(false)
}

/// Parses loosely affirmative state markers used in metadata.
fn is_affirmative(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "1" | "true" | "yes" | "y" | "up" | "ready" | "online" | "locked" | "live"
    )
}

/// Parses values commonly used to encode "live service" status.
fn is_live(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    matches!(normalized.as_str(), "live" | "up" | "ready" | "online" | "true")
}

/// Stable topology label for config commitment hashing.
fn topology_label(topology: &TopologyKind) -> &'static str {
    match topology {
        TopologyKind::FullMesh => "FullMesh",
        TopologyKind::Ring => "Ring",
        TopologyKind::Star => "Star",
        TopologyKind::Random => "Random",
        TopologyKind::Geographic => "Geographic",
        TopologyKind::Hierarchical => "Hierarchical",
    }
}

/// Stable readiness gate label for config commitment hashing.
fn readiness_gate_label(gate: &ReadinessGate) -> &'static str {
    match gate {
        ReadinessGate::SyncComplete => "SyncComplete",
        ReadinessGate::ValidatorQuorum => "ValidatorQuorum",
        ReadinessGate::GenesisLocked => "GenesisLocked",
        ReadinessGate::ForkScheduleSet => "ForkScheduleSet",
        ReadinessGate::MonitoringUp => "MonitoringUp",
        ReadinessGate::FaucetLive => "FaucetLive",
    }
}
