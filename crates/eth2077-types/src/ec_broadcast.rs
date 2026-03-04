use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MIN_REDUNDANCY_FACTOR: f64 = 1.0;
const MAX_REDUNDANCY_FACTOR: f64 = 4.0;
const MAX_BROADCAST_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ErasureCodingScheme {
    ReedSolomon,
    LDPCCode,
    RaptorCode,
    FountainCode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BroadcastTopology {
    FullMesh,
    GossipSub,
    TreeBroadcast,
    HybridPushPull,
    ErasureGossip,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EcBroadcastConfig {
    pub coding_scheme: ErasureCodingScheme,
    pub topology: BroadcastTopology,
    pub shard_count: usize,
    pub redundancy_factor: f64,
    pub max_payload_bytes: usize,
    pub fanout: usize,
    pub target_latency_ms: f64,
    pub node_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EcBroadcastValidationError {
    ZeroShards,
    RedundancyTooLow { value: f64 },
    RedundancyTooHigh { value: f64 },
    PayloadTooLarge { size: usize, max: usize },
    FanoutExceedsNodes { fanout: usize, nodes: usize },
    InsufficientNodes,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EcBroadcastStats {
    pub bandwidth_per_node_bytes: usize,
    pub total_network_bytes: usize,
    pub propagation_rounds: usize,
    pub estimated_latency_ms: f64,
    pub redundancy_overhead: f64,
    pub recovery_threshold: usize,
    pub coding_efficiency: f64,
}

pub fn default_ec_broadcast_config() -> EcBroadcastConfig {
    EcBroadcastConfig {
        coding_scheme: ErasureCodingScheme::ReedSolomon,
        topology: BroadcastTopology::ErasureGossip,
        shard_count: 64,
        redundancy_factor: 1.5,
        max_payload_bytes: 1_048_576,
        fanout: 6,
        target_latency_ms: 240.0,
        node_count: 128,
    }
}

pub fn validate_ec_broadcast_config(
    config: &EcBroadcastConfig,
) -> Result<(), Vec<EcBroadcastValidationError>> {
    let mut errors = Vec::new();

    if config.shard_count == 0 {
        errors.push(EcBroadcastValidationError::ZeroShards);
    }

    if !config.redundancy_factor.is_finite() || config.redundancy_factor < MIN_REDUNDANCY_FACTOR {
        errors.push(EcBroadcastValidationError::RedundancyTooLow {
            value: config.redundancy_factor,
        });
    }

    if config.redundancy_factor.is_finite() && config.redundancy_factor > MAX_REDUNDANCY_FACTOR {
        errors.push(EcBroadcastValidationError::RedundancyTooHigh {
            value: config.redundancy_factor,
        });
    }

    if config.max_payload_bytes > MAX_BROADCAST_PAYLOAD_BYTES {
        errors.push(EcBroadcastValidationError::PayloadTooLarge {
            size: config.max_payload_bytes,
            max: MAX_BROADCAST_PAYLOAD_BYTES,
        });
    }

    if config.node_count < 2 || config.node_count < config.shard_count {
        errors.push(EcBroadcastValidationError::InsufficientNodes);
    }

    if config.node_count > 0 && config.fanout >= config.node_count {
        errors.push(EcBroadcastValidationError::FanoutExceedsNodes {
            fanout: config.fanout,
            nodes: config.node_count,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_ec_broadcast_stats(config: &EcBroadcastConfig) -> EcBroadcastStats {
    let safe_shards = config.shard_count.max(1);
    let safe_nodes = config.node_count.max(1);
    let safe_fanout = config.fanout.max(1);
    let safe_redundancy = sanitize_redundancy(config.redundancy_factor);

    // Per-node payload share: payload * redundancy / shards.
    let bandwidth_per_node_bytes =
        ((config.max_payload_bytes as f64 * safe_redundancy) / safe_shards as f64).ceil() as usize;
    let dissemination_factor =
        topology_transmission_factor(config.topology, safe_fanout, safe_nodes).max(1.0);
    let total_network_bytes =
        (bandwidth_per_node_bytes as f64 * safe_nodes as f64 * dissemination_factor).ceil()
            as usize;

    let base_rounds = base_propagation_rounds(safe_nodes, safe_fanout);
    let propagation_rounds =
        ((base_rounds as f64) * topology_round_factor(config.topology)).ceil() as usize;
    let propagation_rounds = propagation_rounds.max(1);

    let coding_overhead = estimate_coding_overhead(config.coding_scheme, safe_redundancy);
    let per_round_budget_ms = (config.target_latency_ms / base_rounds as f64).max(1.0);
    let estimated_latency_ms = propagation_rounds as f64
        * per_round_budget_ms
        * topology_latency_factor(config.topology)
        * coding_overhead;

    EcBroadcastStats {
        bandwidth_per_node_bytes,
        total_network_bytes,
        propagation_rounds,
        estimated_latency_ms,
        redundancy_overhead: safe_redundancy - 1.0,
        recovery_threshold: recovery_threshold(safe_shards, safe_redundancy),
        coding_efficiency: (1.0 / coding_overhead).clamp(0.0, 1.0),
    }
}

pub fn compare_topologies(config: &EcBroadcastConfig) -> Vec<(String, EcBroadcastStats)> {
    all_topologies()
        .into_iter()
        .map(|topology| {
            let mut variant = config.clone();
            variant.topology = topology;
            (
                format!("{topology:?}"),
                compute_ec_broadcast_stats(&variant),
            )
        })
        .collect()
}

pub fn estimate_coding_overhead(scheme: ErasureCodingScheme, redundancy: f64) -> f64 {
    let safe_redundancy = sanitize_redundancy(redundancy);
    let additional_redundancy = (safe_redundancy - 1.0).max(0.0);

    match scheme {
        ErasureCodingScheme::ReedSolomon => 1.30 + additional_redundancy * 0.42,
        ErasureCodingScheme::LDPCCode => 1.08 + additional_redundancy * 0.28,
        ErasureCodingScheme::RaptorCode => 0.97 + additional_redundancy * 0.20,
        ErasureCodingScheme::FountainCode => 1.00 + additional_redundancy * 0.24,
    }
}

pub fn compute_broadcast_commitment(
    config: &EcBroadcastConfig,
    shard_hashes: &[[u8; 32]],
) -> [u8; 32] {
    let mut sorted_hashes = shard_hashes.to_vec();
    sorted_hashes.sort_unstable();

    let mut hasher = Sha256::new();
    hasher.update(b"eth2077-ec-broadcast-v1");
    hasher.update([coding_scheme_discriminant(config.coding_scheme)]);
    hasher.update([topology_discriminant(config.topology)]);
    hasher.update((config.shard_count as u64).to_be_bytes());
    hasher.update(config.redundancy_factor.to_be_bytes());
    hasher.update((config.max_payload_bytes as u64).to_be_bytes());
    hasher.update((config.fanout as u64).to_be_bytes());
    hasher.update(config.target_latency_ms.to_be_bytes());
    hasher.update((config.node_count as u64).to_be_bytes());
    hasher.update((sorted_hashes.len() as u64).to_be_bytes());

    for hash in sorted_hashes {
        hasher.update(hash);
    }

    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn all_topologies() -> [BroadcastTopology; 5] {
    [
        BroadcastTopology::FullMesh,
        BroadcastTopology::GossipSub,
        BroadcastTopology::TreeBroadcast,
        BroadcastTopology::HybridPushPull,
        BroadcastTopology::ErasureGossip,
    ]
}

fn sanitize_redundancy(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(MIN_REDUNDANCY_FACTOR, MAX_REDUNDANCY_FACTOR)
    } else {
        MIN_REDUNDANCY_FACTOR
    }
}

fn base_propagation_rounds(node_count: usize, fanout: usize) -> usize {
    if node_count <= 1 {
        return 1;
    }

    if fanout <= 1 {
        return node_count - 1;
    }

    ((node_count as f64).log(fanout as f64)).ceil().max(1.0) as usize
}

fn topology_round_factor(topology: BroadcastTopology) -> f64 {
    match topology {
        BroadcastTopology::FullMesh => 1.0,
        BroadcastTopology::GossipSub => 1.20,
        BroadcastTopology::TreeBroadcast => 0.85,
        BroadcastTopology::HybridPushPull => 0.95,
        BroadcastTopology::ErasureGossip => 0.90,
    }
}

fn topology_latency_factor(topology: BroadcastTopology) -> f64 {
    match topology {
        BroadcastTopology::FullMesh => 1.22,
        BroadcastTopology::GossipSub => 1.00,
        BroadcastTopology::TreeBroadcast => 0.88,
        BroadcastTopology::HybridPushPull => 0.82,
        BroadcastTopology::ErasureGossip => 0.75,
    }
}

fn topology_transmission_factor(
    topology: BroadcastTopology,
    fanout: usize,
    node_count: usize,
) -> f64 {
    let peer_limit = node_count.saturating_sub(1).max(1);
    let effective_fanout = fanout.min(peer_limit).max(1) as f64;

    match topology {
        BroadcastTopology::FullMesh => peer_limit as f64,
        BroadcastTopology::GossipSub => effective_fanout * 1.15,
        BroadcastTopology::TreeBroadcast => 1.0,
        BroadcastTopology::HybridPushPull => effective_fanout * 0.75,
        BroadcastTopology::ErasureGossip => effective_fanout * 0.60,
    }
}

fn recovery_threshold(shard_count: usize, redundancy_factor: f64) -> usize {
    ((shard_count as f64) / redundancy_factor).ceil().max(1.0) as usize
}

fn coding_scheme_discriminant(scheme: ErasureCodingScheme) -> u8 {
    match scheme {
        ErasureCodingScheme::ReedSolomon => 0,
        ErasureCodingScheme::LDPCCode => 1,
        ErasureCodingScheme::RaptorCode => 2,
        ErasureCodingScheme::FountainCode => 3,
    }
}

fn topology_discriminant(topology: BroadcastTopology) -> u8 {
    match topology {
        BroadcastTopology::FullMesh => 0,
        BroadcastTopology::GossipSub => 1,
        BroadcastTopology::TreeBroadcast => 2,
        BroadcastTopology::HybridPushPull => 3,
        BroadcastTopology::ErasureGossip => 4,
    }
}
