//! Chain specification freeze types and helpers for ETH2077.
//!
//! This module models chain-spec snapshots used to freeze and release ETH2077
//! testnet deployments. It captures network identity, genesis parameters, fork
//! schedule/lifecycle, consensus profile, and release status/version metadata.
//!
//! Public helpers:
//! - [`default_chain_spec_config`]
//! - [`validate_chain_spec_config`]
//! - [`compute_chain_spec_stats`]
//! - [`compute_chain_spec_commitment`]
//!
//! Commitment generation is deterministic: metadata maps are serialized in
//! sorted-key order, while fork array order is preserved because schedule order
//! is part of governance intent.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Deployment class for a chain specification snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NetworkKind {
    /// Canonical production network.
    Mainnet,
    /// Public pre-production network for release validation.
    Testnet,
    /// Engineering network used for rapid iteration.
    Devnet,
    /// Environment mirroring production traffic shape and topology.
    Shadow,
    /// Local operator network for deterministic debugging.
    Local,
    /// User-defined network class with metadata-defined semantics.
    Custom,
}

/// Lifecycle phase of a fork entry in a schedule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ForkPhase {
    /// Approved and assigned a future activation epoch.
    Scheduled,
    /// Activated and part of the currently effective protocol.
    Activated,
    /// Historical fork that remains referenced but no longer preferred.
    Deprecated,
    /// Withdrawn fork that should not activate.
    Cancelled,
    /// Proposed fork awaiting scheduling approval.
    Pending,
    /// Urgent fork delivered for security or network safety reasons.
    Emergency,
}

/// Consensus family expected for this chain specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConsensusType {
    /// Stake-weighted consensus with validator participation.
    ProofOfStake,
    /// ETH2077 Citadel out-of-band consensus extension.
    CitadelOob,
    /// Mixed or phased consensus composition.
    Hybrid,
    /// Authority-based consensus among known validators.
    ProofOfAuthority,
    /// Delegated consensus where producers are elected.
    Delegated,
    /// Research or experimental consensus mode.
    Experimental,
}

/// Lifecycle status of a full chain-spec snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpecStatus {
    /// Mutable working draft.
    Draft,
    /// Governance-frozen candidate.
    Frozen,
    /// Published release expected to be consumed by clients.
    Released,
    /// Replaced by a newer release.
    Superseded,
    /// Explicitly invalidated.
    Revoked,
    /// Historical record retained for reference.
    Archived,
}

/// Single fork entry in a chain specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForkEntry {
    /// Stable fork label used in docs, tooling, and governance.
    pub name: String,
    /// Fork lifecycle phase.
    pub phase: ForkPhase,
    /// Activation epoch (or planned activation epoch).
    pub activation_epoch: u64,
    /// Protocol proposal identifiers bundled into this fork.
    pub eips: Vec<String>,
    /// Indicates whether this fork changes consensus semantics.
    pub consensus_change: bool,
    /// Extensible per-fork metadata.
    pub metadata: HashMap<String, String>,
}

/// Full chain specification configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChainSpecConfig {
    /// Transaction-domain chain identifier.
    pub chain_id: u64,
    /// Target network class.
    pub network: NetworkKind,
    /// Baseline consensus family.
    pub consensus: ConsensusType,
    /// Snapshot lifecycle status.
    pub status: SpecStatus,
    /// Genesis unix timestamp in seconds.
    pub genesis_time: u64,
    /// Number of slots in one epoch.
    pub slots_per_epoch: u64,
    /// Slot duration in seconds.
    pub seconds_per_slot: u64,
    /// Ordered fork schedule.
    pub forks: Vec<ForkEntry>,
    /// Extensible top-level metadata.
    pub metadata: HashMap<String, String>,
}

/// Validation error reported by [`validate_chain_spec_config`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChainSpecValidationError {
    /// Failing field identifier.
    pub field: String,
    /// Human-readable reason.
    pub reason: String,
}

/// Rollup counters derived from a chain specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChainSpecStats {
    /// Total fork entries in the schedule.
    pub total_forks: usize,
    /// Forks currently marked `Activated`.
    pub active_forks: usize,
    /// Forks still pre-activation (`Pending` or `Scheduled`).
    pub pending_forks: usize,
    /// Unique EIP identifiers across all forks.
    pub eip_count: usize,
    /// Whether any fork reports consensus transitions.
    pub has_consensus_changes: bool,
    /// Effective spec version label.
    pub spec_version: String,
}

/// Returns a conservative ETH2077 testnet chain-spec baseline.
/// The default includes non-zero timing fields, a three-entry fork timeline,
/// and top-level metadata containing `spec_version`.
pub fn default_chain_spec_config() -> ChainSpecConfig {
    let mut metadata = HashMap::new();
    metadata.insert("spec_version".to_string(), "2026.1-testnet".to_string());
    metadata.insert("owner".to_string(), "eth2077-core".to_string());
    metadata.insert("profile".to_string(), "freeze-candidate".to_string());

    let mut bootstrap_metadata = HashMap::new();
    bootstrap_metadata.insert("ticket".to_string(), "ETH2077-BOOT".to_string());
    bootstrap_metadata.insert("rollout".to_string(), "genesis".to_string());

    let mut citadel_metadata = HashMap::new();
    citadel_metadata.insert("ticket".to_string(), "ETH2077-CITADEL-01".to_string());
    citadel_metadata.insert("rollout".to_string(), "canary".to_string());

    let mut throughput_metadata = HashMap::new();
    throughput_metadata.insert("ticket".to_string(), "ETH2077-THR-02".to_string());
    throughput_metadata.insert("rollout".to_string(), "stage-1".to_string());

    ChainSpecConfig {
        chain_id: 20_770_001,
        network: NetworkKind::Testnet,
        consensus: ConsensusType::ProofOfStake,
        status: SpecStatus::Draft,
        genesis_time: 1_766_000_000,
        slots_per_epoch: 32,
        seconds_per_slot: 12,
        forks: vec![
            ForkEntry {
                name: "genesis-bootstrap".to_string(),
                phase: ForkPhase::Activated,
                activation_epoch: 0,
                eips: vec!["EIP-1559".to_string(), "EIP-4895".to_string()],
                consensus_change: false,
                metadata: bootstrap_metadata,
            },
            ForkEntry {
                name: "citadel-oob-alpha".to_string(),
                phase: ForkPhase::Scheduled,
                activation_epoch: 12_000,
                eips: vec!["EIP-7002".to_string(), "EIP-7251".to_string()],
                consensus_change: true,
                metadata: citadel_metadata,
            },
            ForkEntry {
                name: "throughput-balance-v1".to_string(),
                phase: ForkPhase::Pending,
                activation_epoch: 24_000,
                eips: vec!["EIP-7623".to_string()],
                consensus_change: false,
                metadata: throughput_metadata,
            },
        ],
        metadata,
    }
}

/// Validates a chain specification and returns all discovered errors.
///
/// Validation rules:
/// - `chain_id`, `genesis_time`, `slots_per_epoch`, `seconds_per_slot` > 0.
/// - at least one fork entry must be present.
/// - `metadata.spec_version` must exist and be non-empty after trimming.
/// - metadata keys must be non-empty at both top-level and per-fork.
/// - fork names must be non-empty and unique (case-insensitive).
/// - fork activation epochs must be monotonically non-decreasing.
/// - `Scheduled` and `Pending` forks require `activation_epoch > 0`.
/// - EIP entries must be non-empty and unique within each fork.
/// - `NetworkKind::Custom` requires `metadata.network_name`.
///
/// The function accumulates all failures to support CI and review tooling.
pub fn validate_chain_spec_config(
    config: &ChainSpecConfig,
) -> Result<(), Vec<ChainSpecValidationError>> {
    let mut errors = Vec::new();

    if config.chain_id == 0 {
        push_error(&mut errors, "chain_id", "must be greater than zero");
    }
    if config.genesis_time == 0 {
        push_error(&mut errors, "genesis_time", "must be greater than zero");
    }
    if config.slots_per_epoch == 0 {
        push_error(&mut errors, "slots_per_epoch", "must be greater than zero");
    }
    if config.seconds_per_slot == 0 {
        push_error(&mut errors, "seconds_per_slot", "must be greater than zero");
    }
    if config.forks.is_empty() {
        push_error(&mut errors, "forks", "must include at least one fork entry");
    }

    let spec_version = config
        .metadata
        .get("spec_version")
        .map(|v| v.trim())
        .unwrap_or("");
    if spec_version.is_empty() {
        push_error(
            &mut errors,
            "metadata.spec_version",
            "must be present and non-empty",
        );
    }

    for key in config.metadata.keys() {
        if key.trim().is_empty() {
            push_error(&mut errors, "metadata", "metadata keys must not be empty");
            break;
        }
    }

    if config.network == NetworkKind::Custom {
        let custom_name = config
            .metadata
            .get("network_name")
            .map(|v| v.trim())
            .unwrap_or("");
        if custom_name.is_empty() {
            push_error(
                &mut errors,
                "metadata.network_name",
                "is required when network is Custom",
            );
        }
    }

    let mut seen_names: HashMap<String, usize> = HashMap::new();
    let mut previous_epoch: Option<u64> = None;

    for (index, fork) in config.forks.iter().enumerate() {
        let name = fork.name.trim();
        if name.is_empty() {
            push_error(&mut errors, "forks.name", "fork names must not be empty");
        }

        let normalized_name = name.to_lowercase();
        if let Some(prior_index) = seen_names.get(&normalized_name) {
            push_error(
                &mut errors,
                "forks.name",
                &format!(
                    "fork name '{}' is duplicated at indices {} and {}",
                    name, prior_index, index
                ),
            );
        } else {
            seen_names.insert(normalized_name, index);
        }

        if matches!(fork.phase, ForkPhase::Scheduled | ForkPhase::Pending)
            && fork.activation_epoch == 0
        {
            push_error(
                &mut errors,
                "forks.activation_epoch",
                &format!(
                    "fork '{}' in phase {:?} must have activation_epoch > 0",
                    name, fork.phase
                ),
            );
        }

        if let Some(prior) = previous_epoch {
            if fork.activation_epoch < prior {
                push_error(
                    &mut errors,
                    "forks.activation_epoch",
                    &format!(
                        "fork '{}' has non-monotonic activation epoch {} (previous {})",
                        name, fork.activation_epoch, prior
                    ),
                );
            }
        }
        previous_epoch = Some(fork.activation_epoch);

        let mut seen_eips: HashMap<String, usize> = HashMap::new();
        for eip in &fork.eips {
            let token = eip.trim();
            if token.is_empty() {
                push_error(
                    &mut errors,
                    "forks.eips",
                    &format!("fork '{}' contains an empty EIP entry", name),
                );
                continue;
            }

            let normalized = token.to_uppercase();
            if seen_eips.contains_key(&normalized) {
                push_error(
                    &mut errors,
                    "forks.eips",
                    &format!("fork '{}' contains duplicate EIP '{}'", name, token),
                );
            } else {
                seen_eips.insert(normalized, 1);
            }
        }

        for key in fork.metadata.keys() {
            if key.trim().is_empty() {
                push_error(
                    &mut errors,
                    "forks.metadata",
                    &format!("fork '{}' contains an empty metadata key", name),
                );
                break;
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Computes summary statistics for a chain specification.
/// `pending_forks` counts `Pending` plus `Scheduled`, and `eip_count` is unique
/// after trim/case normalization.
pub fn compute_chain_spec_stats(config: &ChainSpecConfig) -> ChainSpecStats {
    let total_forks = config.forks.len();
    let mut active_forks = 0usize;
    let mut pending_forks = 0usize;
    let mut has_consensus_changes = false;
    let mut eips: HashMap<String, usize> = HashMap::new();

    for fork in &config.forks {
        if fork.phase == ForkPhase::Activated {
            active_forks += 1;
        }
        if matches!(fork.phase, ForkPhase::Pending | ForkPhase::Scheduled) {
            pending_forks += 1;
        }
        if fork.consensus_change {
            has_consensus_changes = true;
        }

        for eip in &fork.eips {
            let normalized = eip.trim().to_uppercase();
            if !normalized.is_empty() {
                eips.insert(normalized, 1);
            }
        }
    }

    ChainSpecStats {
        total_forks,
        active_forks,
        pending_forks,
        eip_count: eips.len(),
        has_consensus_changes,
        spec_version: resolve_spec_version(config),
    }
}

/// Computes a deterministic SHA-256 commitment over a chain specification.
///
/// Canonicalization strategy:
/// - scalar fields serialized as `key=value` lines,
/// - enum values converted to fixed lowercase labels,
/// - metadata maps serialized by sorted keys,
/// - fork and EIP vectors serialized by explicit indices.
///
/// Returns a 64-character lowercase hex digest.
pub fn compute_chain_spec_commitment(config: &ChainSpecConfig) -> String {
    let mut canonical = String::new();

    canonical.push_str(&format!("chain_id={}\n", config.chain_id));
    canonical.push_str(&format!("network={}\n", network_label(&config.network)));
    canonical.push_str(&format!(
        "consensus={}\n",
        consensus_label(&config.consensus)
    ));
    canonical.push_str(&format!("status={}\n", status_label(&config.status)));
    canonical.push_str(&format!("genesis_time={}\n", config.genesis_time));
    canonical.push_str(&format!("slots_per_epoch={}\n", config.slots_per_epoch));
    canonical.push_str(&format!("seconds_per_slot={}\n", config.seconds_per_slot));
    canonical.push_str(&format!("spec_version={}\n", resolve_spec_version(config)));

    let mut config_metadata: Vec<(&String, &String)> = config.metadata.iter().collect();
    config_metadata.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
    for (key, value) in config_metadata {
        canonical.push_str(&format!("metadata.{}={}\n", key, value));
    }

    for (fork_index, fork) in config.forks.iter().enumerate() {
        canonical.push_str(&format!("fork[{fork_index}].name={}\n", fork.name));
        canonical.push_str(&format!(
            "fork[{fork_index}].phase={}\n",
            fork_phase_label(&fork.phase)
        ));
        canonical.push_str(&format!(
            "fork[{fork_index}].activation_epoch={}\n",
            fork.activation_epoch
        ));
        canonical.push_str(&format!(
            "fork[{fork_index}].consensus_change={}\n",
            fork.consensus_change
        ));

        for (eip_index, eip) in fork.eips.iter().enumerate() {
            canonical.push_str(&format!("fork[{fork_index}].eip[{eip_index}]={}\n", eip));
        }

        let mut fork_metadata: Vec<(&String, &String)> = fork.metadata.iter().collect();
        fork_metadata.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        for (key, value) in fork_metadata {
            canonical.push_str(&format!("fork[{fork_index}].metadata.{}={}\n", key, value));
        }
    }

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let digest = hasher.finalize();
    lower_hex(&digest)
}

fn push_error(errors: &mut Vec<ChainSpecValidationError>, field: &str, reason: &str) {
    errors.push(ChainSpecValidationError {
        field: field.to_string(),
        reason: reason.to_string(),
    });
}

fn resolve_spec_version(config: &ChainSpecConfig) -> String {
    if let Some(version) = config.metadata.get("spec_version") {
        let trimmed = version.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    format!(
        "{}-{}-{}",
        status_label(&config.status),
        config.chain_id,
        config.forks.len()
    )
}

fn network_label(value: &NetworkKind) -> &'static str {
    match value {
        NetworkKind::Mainnet => "mainnet",
        NetworkKind::Testnet => "testnet",
        NetworkKind::Devnet => "devnet",
        NetworkKind::Shadow => "shadow",
        NetworkKind::Local => "local",
        NetworkKind::Custom => "custom",
    }
}

fn fork_phase_label(value: &ForkPhase) -> &'static str {
    match value {
        ForkPhase::Scheduled => "scheduled",
        ForkPhase::Activated => "activated",
        ForkPhase::Deprecated => "deprecated",
        ForkPhase::Cancelled => "cancelled",
        ForkPhase::Pending => "pending",
        ForkPhase::Emergency => "emergency",
    }
}

fn consensus_label(value: &ConsensusType) -> &'static str {
    match value {
        ConsensusType::ProofOfStake => "proof_of_stake",
        ConsensusType::CitadelOob => "citadel_oob",
        ConsensusType::Hybrid => "hybrid",
        ConsensusType::ProofOfAuthority => "proof_of_authority",
        ConsensusType::Delegated => "delegated",
        ConsensusType::Experimental => "experimental",
    }
}

fn status_label(value: &SpecStatus) -> &'static str {
    match value {
        SpecStatus::Draft => "draft",
        SpecStatus::Frozen => "frozen",
        SpecStatus::Released => "released",
        SpecStatus::Superseded => "superseded",
        SpecStatus::Revoked => "revoked",
        SpecStatus::Archived => "archived",
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push_str(&format!("{:02x}", byte));
    }
    encoded
}
