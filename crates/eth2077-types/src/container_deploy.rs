//! Containerized node deployment automation types for ETH2077.
//!
//! This module models orchestration intent and runtime snapshots for a
//! 48-container ETH2077 testnet deployment. It includes:
//! - deployment backends and rollout strategies,
//! - probe and image policy modeling,
//! - validation for safety and policy consistency,
//! - aggregate fleet statistics,
//! - deterministic SHA-256 commitments for change control.
//!
//! The types are designed to serialize cleanly with `serde` and to be consumed
//! by CI/CD pipelines, infra controllers, and simulation harnesses.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Supported orchestration backend for containerized ETH2077 nodes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Orchestrator {
    /// Kubernetes-style deployment controllers.
    Kubernetes,
    /// Docker Swarm manager/worker orchestration.
    DockerSwarm,
    /// HashiCorp Nomad scheduler.
    Nomad,
    /// Project-specific scheduler and control plane.
    Custom,
    /// Host-local container runtime orchestration.
    BareMetal,
    /// Serverless container platform orchestration.
    CloudRun,
}

/// Rollout strategy used for node upgrades and image refreshes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeployStrategy {
    /// Replace instances gradually while keeping service live.
    RollingUpdate,
    /// Run old/new stacks in parallel then switch over.
    BlueGreen,
    /// Shift a small fraction first before wider rollout.
    Canary,
    /// Stop old replicas before starting new replicas.
    Recreate,
    /// Multi-stage rollout with explicit promotion points.
    Staged,
    /// Operator-driven rollout without automatic progression.
    Manual,
}

/// Probe mode used to classify container readiness/liveness.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthProbe {
    /// HTTP-based health endpoint evaluation.
    HttpGet,
    /// TCP connect/open-socket evaluation.
    TcpSocket,
    /// gRPC health API evaluation.
    GrpcHealth,
    /// In-container command exit code evaluation.
    ExecCommand,
    /// Minimum peer connectivity evaluation.
    PeerCount,
    /// Chain synchronization progress evaluation.
    SyncStatus,
}

/// Policy describing how container images are pulled and trusted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ImagePolicy {
    /// Always pull image before launch/restart.
    AlwaysPull,
    /// Pull only if local cache does not have the image.
    IfNotPresent,
    /// Require a pinned tag convention.
    Pinned,
    /// Require digest-qualified image references.
    Digest,
    /// Require signature verification.
    Signed,
    /// Require supply-chain attestations.
    Attested,
}

/// Runtime snapshot for one containerized ETH2077 node.
///
/// The `metadata` map is intentionally open-ended. Expected keys include:
/// - `healthy` as explicit health override (`true`/`false`),
/// - `cpu_usage_pct`, `memory_usage_pct` for direct usage reports,
/// - probe-specific fields such as `http_status`, `grpc_status`,
///   `exec_exit_code`, `current_peers`, `min_peers`, `sync_pct`, and
///   `sync_state`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerNode {
    /// Stable node identifier.
    pub id: String,
    /// Container image reference.
    pub image: String,
    /// Orchestration backend this node is running under.
    pub orchestrator: Orchestrator,
    /// Deployment strategy used for this node rollout.
    pub strategy: DeployStrategy,
    /// Probe mode used to classify this node health.
    pub health_probe: HealthProbe,
    /// CPU limit in millicores.
    pub cpu_limit_milli: u64,
    /// Memory limit in megabytes.
    pub memory_limit_mb: u64,
    /// Free-form runtime metadata.
    pub metadata: HashMap<String, String>,
}

/// Desired state for a containerized ETH2077 node deployment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerDeployConfig {
    /// Desired number of replicas. ETH2077 policy expects 48.
    pub target_replicas: usize,
    /// Deployment orchestration backend.
    pub orchestrator: Orchestrator,
    /// Rollout strategy.
    pub strategy: DeployStrategy,
    /// Image pull/trust policy.
    pub image_policy: ImagePolicy,
    /// Health polling interval in seconds.
    pub health_check_interval_s: u64,
    /// Maximum extra capacity during rollout in percent.
    pub max_surge_pct: f64,
    /// Additional policy metadata.
    pub metadata: HashMap<String, String>,
}

/// Validation error for a specific config field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerDeployValidationError {
    /// Field or section where validation failed.
    pub field: String,
    /// Human-readable failure reason.
    pub reason: String,
}

/// Aggregated deployment-level statistics derived from node snapshots.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerDeployStats {
    /// Total observed containers.
    pub total_containers: usize,
    /// Number of healthy containers.
    pub healthy: usize,
    /// Number of unhealthy containers.
    pub unhealthy: usize,
    /// Mean CPU usage percentage in `[0, 100]`.
    pub avg_cpu_usage_pct: f64,
    /// Mean memory usage percentage in `[0, 100]`.
    pub avg_memory_usage_pct: f64,
    /// Whether rollout appears complete for a 48-node target.
    pub rollout_complete: bool,
}

/// Returns the default container deployment config for ETH2077 testnet.
///
/// Defaults are conservative and automation-friendly:
/// - `target_replicas = 48`,
/// - `Kubernetes` + `RollingUpdate`,
/// - `Digest` image policy,
/// - 15-second health checks,
/// - 20% surge allowance.
pub fn default_container_deploy_config() -> ContainerDeployConfig {
    let mut metadata = HashMap::new();
    metadata.insert("network".to_string(), "ETH2077-testnet".to_string());
    metadata.insert("stage".to_string(), "containerized".to_string());
    metadata.insert("image_digest".to_string(), "sha256:pending".to_string());
    metadata.insert("rollout_guard".to_string(), "strict".to_string());
    metadata.insert("observability".to_string(), "enabled".to_string());

    ContainerDeployConfig {
        target_replicas: 48,
        orchestrator: Orchestrator::Kubernetes,
        strategy: DeployStrategy::RollingUpdate,
        image_policy: ImagePolicy::Digest,
        health_check_interval_s: 15,
        max_surge_pct: 20.0,
        metadata,
    }
}

/// Validates deployment policy and returns all discovered failures.
///
/// Rules enforced:
/// - `target_replicas` must be exactly `48`.
/// - `health_check_interval_s` must be in `[5, 300]`.
/// - `max_surge_pct` must be finite and in `[0.0, 100.0]`.
/// - Strategy constraints:
///   - `Recreate` requires `max_surge_pct == 0.0`.
///   - `BlueGreen` requires `max_surge_pct >= 100.0`.
///   - `Canary` requires `max_surge_pct <= 25.0`.
///   - `Manual` requires `max_surge_pct <= 10.0`.
/// - Orchestrator constraints:
///   - `BareMetal` requires `max_surge_pct <= 10.0`.
///   - `CloudRun` rejects `Recreate`.
/// - Image policy metadata requirements:
///   - `Digest` requires `metadata.image_digest`.
///   - `Signed` requires `metadata.signature_profile`.
///   - `Attested` requires `metadata.attestation_policy`.
/// - Metadata keys and values must be non-empty after trim.
pub fn validate_container_deploy_config(
    config: &ContainerDeployConfig,
) -> Result<(), Vec<ContainerDeployValidationError>> {
    let mut errors = Vec::new();

    if config.target_replicas != 48 {
        push_validation_error(
            &mut errors,
            "target_replicas",
            "must be exactly 48 for ETH2077 testnet deployment",
        );
    }

    if config.health_check_interval_s < 5 || config.health_check_interval_s > 300 {
        push_validation_error(
            &mut errors,
            "health_check_interval_s",
            "must be within [5, 300] seconds",
        );
    }

    if !config.max_surge_pct.is_finite()
        || config.max_surge_pct < 0.0
        || config.max_surge_pct > 100.0
    {
        push_validation_error(
            &mut errors,
            "max_surge_pct",
            "must be finite and within [0.0, 100.0]",
        );
    }

    validate_strategy_constraints(config, &mut errors);
    validate_orchestrator_constraints(config, &mut errors);
    validate_policy_metadata_requirements(config, &mut errors);

    for (key, value) in &config.metadata {
        if key.trim().is_empty() {
            push_validation_error(&mut errors, "metadata", "contains an empty metadata key");
        }
        if value.trim().is_empty() {
            push_validation_error(
                &mut errors,
                "metadata",
                &format!("metadata value for key `{key}` must be non-empty"),
            );
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Computes aggregate stats from observed container nodes.
///
/// Behavior summary:
/// - Empty input returns zeroed metrics and `rollout_complete = false`.
/// - Health is computed from baseline resource checks plus probe interpretation.
/// - `healthy` metadata overrides probe output when present.
/// - Utilization metrics use metadata when provided, else deterministic
///   limit-based estimates.
/// - Rollout completes only when:
///   - at least 48 nodes are present,
///   - all nodes are healthy,
///   - all images are non-empty,
///   - all node IDs are unique.
pub fn compute_container_deploy_stats(nodes: &[ContainerNode]) -> ContainerDeployStats {
    let total_containers = nodes.len();
    if total_containers == 0 {
        return ContainerDeployStats {
            total_containers: 0,
            healthy: 0,
            unhealthy: 0,
            avg_cpu_usage_pct: 0.0,
            avg_memory_usage_pct: 0.0,
            rollout_complete: false,
        };
    }

    let mut healthy = 0usize;
    let mut cpu_sum = 0.0;
    let mut memory_sum = 0.0;
    let mut all_images_non_empty = true;
    let mut id_counts: HashMap<&str, usize> = HashMap::new();

    for node in nodes {
        if node_is_healthy(node) {
            healthy += 1;
        }

        cpu_sum += node_cpu_usage_pct(node);
        memory_sum += node_memory_usage_pct(node);

        if node.image.trim().is_empty() {
            all_images_non_empty = false;
        }

        let count = id_counts.entry(node.id.as_str()).or_insert(0);
        *count += 1;
    }

    let unhealthy = total_containers.saturating_sub(healthy);
    let avg_cpu_usage_pct = (cpu_sum / total_containers as f64).clamp(0.0, 100.0);
    let avg_memory_usage_pct = (memory_sum / total_containers as f64).clamp(0.0, 100.0);
    let unique_ids = id_counts.values().all(|count| *count == 1);

    let rollout_complete =
        total_containers >= 48 && healthy == total_containers && all_images_non_empty && unique_ids;

    ContainerDeployStats {
        total_containers,
        healthy,
        unhealthy,
        avg_cpu_usage_pct,
        avg_memory_usage_pct,
        rollout_complete,
    }
}

/// Computes a deterministic SHA-256 commitment for deployment config.
///
/// Commitment input includes:
/// - a domain separator (`ETH2077::CONTAINER_DEPLOY::V1`),
/// - scalar config fields,
/// - labeled enum variants,
/// - metadata key/value pairs sorted lexicographically.
///
/// Sorting metadata guarantees the same hash regardless of insertion order.
pub fn compute_container_deploy_commitment(config: &ContainerDeployConfig) -> String {
    let mut hasher = Sha256::new();

    hasher.update(b"ETH2077::CONTAINER_DEPLOY::V1");
    hasher.update(
        format!(
            "target_replicas={}|orchestrator={}|strategy={}|image_policy={}|health_check_interval_s={}|max_surge_pct={:.6}|",
            config.target_replicas,
            orchestrator_label(&config.orchestrator),
            strategy_label(&config.strategy),
            image_policy_label(&config.image_policy),
            config.health_check_interval_s,
            config.max_surge_pct,
        )
        .as_bytes(),
    );

    let mut metadata_pairs: Vec<(&String, &String)> = config.metadata.iter().collect();
    metadata_pairs.sort_by(|(ka, va), (kb, vb)| ka.cmp(kb).then(va.cmp(vb)));

    for (key, value) in metadata_pairs {
        hasher.update(key.as_bytes());
        hasher.update(b"=");
        hasher.update(value.as_bytes());
        hasher.update(b";");
    }

    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

fn validate_strategy_constraints(
    config: &ContainerDeployConfig,
    errors: &mut Vec<ContainerDeployValidationError>,
) {
    match config.strategy {
        DeployStrategy::Recreate => {
            if config.max_surge_pct != 0.0 {
                push_validation_error(
                    errors,
                    "max_surge_pct",
                    "must be 0.0 when strategy is Recreate",
                );
            }
        }
        DeployStrategy::BlueGreen => {
            if config.max_surge_pct < 100.0 {
                push_validation_error(
                    errors,
                    "max_surge_pct",
                    "must be at least 100.0 when strategy is BlueGreen",
                );
            }
        }
        DeployStrategy::Canary => {
            if config.max_surge_pct > 25.0 {
                push_validation_error(
                    errors,
                    "max_surge_pct",
                    "must be at most 25.0 when strategy is Canary",
                );
            }
        }
        DeployStrategy::Manual => {
            if config.max_surge_pct > 10.0 {
                push_validation_error(
                    errors,
                    "max_surge_pct",
                    "must be at most 10.0 when strategy is Manual",
                );
            }
        }
        DeployStrategy::RollingUpdate | DeployStrategy::Staged => {}
    }
}

fn validate_orchestrator_constraints(
    config: &ContainerDeployConfig,
    errors: &mut Vec<ContainerDeployValidationError>,
) {
    if config.orchestrator == Orchestrator::BareMetal && config.max_surge_pct > 10.0 {
        push_validation_error(
            errors,
            "orchestrator",
            "BareMetal deployment cannot exceed 10.0 max_surge_pct",
        );
    }

    if config.orchestrator == Orchestrator::CloudRun && config.strategy == DeployStrategy::Recreate
    {
        push_validation_error(
            errors,
            "strategy",
            "CloudRun does not support Recreate strategy in this model",
        );
    }
}

fn validate_policy_metadata_requirements(
    config: &ContainerDeployConfig,
    errors: &mut Vec<ContainerDeployValidationError>,
) {
    match config.image_policy {
        ImagePolicy::Digest => {
            if !metadata_has_text(&config.metadata, "image_digest") {
                push_validation_error(
                    errors,
                    "image_policy",
                    "Digest policy requires metadata.image_digest",
                );
            }
        }
        ImagePolicy::Signed => {
            if !metadata_has_text(&config.metadata, "signature_profile") {
                push_validation_error(
                    errors,
                    "image_policy",
                    "Signed policy requires metadata.signature_profile",
                );
            }
        }
        ImagePolicy::Attested => {
            if !metadata_has_text(&config.metadata, "attestation_policy") {
                push_validation_error(
                    errors,
                    "image_policy",
                    "Attested policy requires metadata.attestation_policy",
                );
            }
        }
        ImagePolicy::AlwaysPull | ImagePolicy::IfNotPresent | ImagePolicy::Pinned => {}
    }
}

fn node_is_healthy(node: &ContainerNode) -> bool {
    let baseline =
        node.cpu_limit_milli > 0 && node.memory_limit_mb > 0 && !node.image.trim().is_empty();
    if !baseline {
        return false;
    }

    if let Some(value) = metadata_bool(&node.metadata, "healthy") {
        return value;
    }

    match node.health_probe {
        HealthProbe::HttpGet => {
            if let Some(status) = metadata_u64(&node.metadata, "http_status") {
                return (200..=399).contains(&status);
            }
            metadata_bool(&node.metadata, "http_ok").unwrap_or(false)
        }
        HealthProbe::TcpSocket => metadata_bool(&node.metadata, "tcp_open")
            .or_else(|| metadata_bool(&node.metadata, "socket_ready"))
            .unwrap_or(false),
        HealthProbe::GrpcHealth => {
            if let Some(state) = metadata_text(&node.metadata, "grpc_status") {
                return state.eq_ignore_ascii_case("serving")
                    || state.eq_ignore_ascii_case("ready");
            }
            metadata_bool(&node.metadata, "grpc_ok").unwrap_or(false)
        }
        HealthProbe::ExecCommand => {
            if let Some(exit_code) = metadata_i64(&node.metadata, "exec_exit_code") {
                return exit_code == 0;
            }
            metadata_bool(&node.metadata, "exec_ok").unwrap_or(false)
        }
        HealthProbe::PeerCount => {
            let current = metadata_u64(&node.metadata, "current_peers").unwrap_or(0);
            let required = metadata_u64(&node.metadata, "min_peers").unwrap_or(8);
            current >= required
        }
        HealthProbe::SyncStatus => {
            if let Some(sync_pct) = metadata_f64(&node.metadata, "sync_pct") {
                if sync_pct.is_finite() {
                    return sync_pct >= 99.0;
                }
            }
            if let Some(state) = metadata_text(&node.metadata, "sync_state") {
                return state.eq_ignore_ascii_case("synced")
                    || state.eq_ignore_ascii_case("ready")
                    || state.eq_ignore_ascii_case("up_to_date");
            }
            false
        }
    }
}

fn node_cpu_usage_pct(node: &ContainerNode) -> f64 {
    if let Some(value) = metadata_f64(&node.metadata, "cpu_usage_pct") {
        if value.is_finite() {
            return value.clamp(0.0, 100.0);
        }
    }
    estimated_usage(node.cpu_limit_milli as f64, 4000.0)
}

fn node_memory_usage_pct(node: &ContainerNode) -> f64 {
    if let Some(value) = metadata_f64(&node.metadata, "memory_usage_pct") {
        if value.is_finite() {
            return value.clamp(0.0, 100.0);
        }
    }
    estimated_usage(node.memory_limit_mb as f64, 8192.0)
}

fn estimated_usage(limit: f64, reference: f64) -> f64 {
    if !limit.is_finite() || limit <= 0.0 || !reference.is_finite() || reference <= 0.0 {
        return 0.0;
    }
    ((limit / reference) * 100.0 * 0.82 + 8.0).clamp(0.0, 100.0)
}

fn push_validation_error(
    errors: &mut Vec<ContainerDeployValidationError>,
    field: &str,
    reason: &str,
) {
    errors.push(ContainerDeployValidationError {
        field: field.to_string(),
        reason: reason.to_string(),
    });
}

fn metadata_has_text(metadata: &HashMap<String, String>, key: &str) -> bool {
    metadata
        .get(key)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn metadata_text<'a>(metadata: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    metadata
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}

fn metadata_u64(metadata: &HashMap<String, String>, key: &str) -> Option<u64> {
    metadata
        .get(key)
        .and_then(|value| value.trim().parse::<u64>().ok())
}

fn metadata_i64(metadata: &HashMap<String, String>, key: &str) -> Option<i64> {
    metadata
        .get(key)
        .and_then(|value| value.trim().parse::<i64>().ok())
}

fn metadata_f64(metadata: &HashMap<String, String>, key: &str) -> Option<f64> {
    metadata
        .get(key)
        .and_then(|value| value.trim().parse::<f64>().ok())
}

fn metadata_bool(metadata: &HashMap<String, String>, key: &str) -> Option<bool> {
    metadata_text(metadata, key).and_then(parse_bool_like)
}

fn parse_bool_like(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" | "on" | "up" | "ready" | "healthy" | "ok" => Some(true),
        "0" | "false" | "no" | "n" | "off" | "down" | "unhealthy" | "failed" => Some(false),
        _ => None,
    }
}

fn orchestrator_label(orchestrator: &Orchestrator) -> &'static str {
    match orchestrator {
        Orchestrator::Kubernetes => "kubernetes",
        Orchestrator::DockerSwarm => "docker-swarm",
        Orchestrator::Nomad => "nomad",
        Orchestrator::Custom => "custom",
        Orchestrator::BareMetal => "bare-metal",
        Orchestrator::CloudRun => "cloud-run",
    }
}

fn strategy_label(strategy: &DeployStrategy) -> &'static str {
    match strategy {
        DeployStrategy::RollingUpdate => "rolling-update",
        DeployStrategy::BlueGreen => "blue-green",
        DeployStrategy::Canary => "canary",
        DeployStrategy::Recreate => "recreate",
        DeployStrategy::Staged => "staged",
        DeployStrategy::Manual => "manual",
    }
}

fn image_policy_label(policy: &ImagePolicy) -> &'static str {
    match policy {
        ImagePolicy::AlwaysPull => "always-pull",
        ImagePolicy::IfNotPresent => "if-not-present",
        ImagePolicy::Pinned => "pinned",
        ImagePolicy::Digest => "digest",
        ImagePolicy::Signed => "signed",
        ImagePolicy::Attested => "attested",
    }
}
