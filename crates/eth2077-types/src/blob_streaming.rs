use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const CHUNK_SIZE_BYTES: usize = 4_096;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BlobPropagationMode {
    // Current: fetch blobs after block header
    PostBlock,
    // EIP-4844 style sidecar attachment
    Sidecar,
    // Stream blob chunks interleaved with block data
    InterleavedStream,
    // Validators push blob data proactively
    PushBased,
    // Full DAS with random sampling
    DASampled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DASStrategy {
    FullDownload,
    RowColumn2D,
    RandomSampling,
    PeerDAS,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlobStreamConfig {
    pub blobs_per_block: usize,
    pub blob_size_bytes: usize,
    pub propagation_mode: BlobPropagationMode,
    pub das_strategy: DASStrategy,
    pub target_da_bandwidth_mbps: f64,
    pub max_blob_latency_ms: u64,
    // e.g. 0.5 means 2x redundancy
    pub erasure_coding_rate: f64,
    // for DAS sampling
    pub sample_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlobStreamValidationError {
    ZeroBlobs,
    ZeroBlobSize,
    BandwidthTooLow {
        required_mbps: String,
        available_mbps: String,
    },
    InvalidErasureRate,
    SampleCountExceedsTotal {
        samples: usize,
        total_chunks: usize,
    },
    LatencyBudgetExceeded {
        estimated_ms: String,
        max_ms: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlobStreamStats {
    pub total_da_bytes_per_block: usize,
    pub effective_da_throughput_mbps: f64,
    pub bandwidth_utilization: f64,
    pub estimated_propagation_ms: f64,
    pub das_security_bits: f64,
    // mode name -> throughput
    pub mode_comparison: Vec<(String, f64)>,
}

pub fn default_blob_stream_config() -> BlobStreamConfig {
    BlobStreamConfig {
        blobs_per_block: 6,
        blob_size_bytes: 128 * 1024,
        propagation_mode: BlobPropagationMode::Sidecar,
        das_strategy: DASStrategy::PeerDAS,
        target_da_bandwidth_mbps: 10.0,
        max_blob_latency_ms: 2_000,
        erasure_coding_rate: 0.5,
        sample_count: 75,
    }
}

pub fn validate_blob_stream_config(
    config: &BlobStreamConfig,
) -> Result<(), Vec<BlobStreamValidationError>> {
    let mut errors = Vec::new();

    if config.blobs_per_block == 0 {
        errors.push(BlobStreamValidationError::ZeroBlobs);
    }

    if config.blob_size_bytes == 0 {
        errors.push(BlobStreamValidationError::ZeroBlobSize);
    }

    if !config.erasure_coding_rate.is_finite()
        || config.erasure_coding_rate <= 0.0
        || config.erasure_coding_rate > 1.0
    {
        errors.push(BlobStreamValidationError::InvalidErasureRate);
    }

    let total_chunks = total_erasure_chunks(config);
    if config.sample_count > total_chunks {
        errors.push(BlobStreamValidationError::SampleCountExceedsTotal {
            samples: config.sample_count,
            total_chunks,
        });
    }

    let required_mbps = required_bandwidth_mbps(config);
    if !config.target_da_bandwidth_mbps.is_finite()
        || config.target_da_bandwidth_mbps < required_mbps
    {
        errors.push(BlobStreamValidationError::BandwidthTooLow {
            required_mbps: format!("{required_mbps:.3}"),
            available_mbps: format!("{:.3}", config.target_da_bandwidth_mbps),
        });
    }

    let estimated_ms = estimate_propagation_latency(config);
    if estimated_ms > config.max_blob_latency_ms as f64 {
        errors.push(BlobStreamValidationError::LatencyBudgetExceeded {
            estimated_ms: format!("{estimated_ms:.2}"),
            max_ms: format!("{:.2}", config.max_blob_latency_ms as f64),
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_blob_stream_stats(config: &BlobStreamConfig) -> BlobStreamStats {
    let total_da_bytes_per_block = total_da_bytes_per_block(config);
    let estimated_propagation_ms = estimate_propagation_latency(config).max(1.0);
    let effective_da_throughput_mbps =
        total_da_bytes_per_block as f64 * 8.0 / (estimated_propagation_ms / 1_000.0) / 1_000_000.0;
    let bandwidth_utilization = if config.target_da_bandwidth_mbps > 0.0 {
        effective_da_throughput_mbps / config.target_da_bandwidth_mbps
    } else {
        0.0
    };

    BlobStreamStats {
        total_da_bytes_per_block,
        effective_da_throughput_mbps,
        bandwidth_utilization,
        estimated_propagation_ms,
        das_security_bits: compute_das_security(config),
        mode_comparison: compare_propagation_modes(config),
    }
}

pub fn estimate_propagation_latency(config: &BlobStreamConfig) -> f64 {
    let encoded_bytes = encoded_da_bytes(config);
    let strategy_fraction = strategy_download_fraction(config);
    let mode_factor = mode_latency_factor(config.propagation_mode);
    let target_mbps = config.target_da_bandwidth_mbps.max(0.001);

    let transfer_megabits = encoded_bytes * strategy_fraction * 8.0 / 1_000_000.0;
    let transfer_ms = (transfer_megabits / target_mbps) * 1_000.0;
    let blob_scheduling_ms =
        per_blob_overhead_ms(config.propagation_mode) * config.blobs_per_block as f64;
    let strategy_base_ms = strategy_base_overhead_ms(config.das_strategy);
    let chunking_ms = (total_erasure_chunks(config) as f64).sqrt() * 1.5;

    transfer_ms * mode_factor + blob_scheduling_ms + strategy_base_ms + chunking_ms
}

pub fn compute_das_security(config: &BlobStreamConfig) -> f64 {
    if config.sample_count == 0 {
        return 0.0;
    }

    let rate = sanitize_erasure_rate(config.erasure_coding_rate);
    let hidden_fraction = (1.0 - rate).clamp(0.01, 0.99);
    let strategy_weight = strategy_security_weight(config.das_strategy);
    let effective_samples = config.sample_count as f64 * strategy_weight;

    let miss_probability = (1.0 - hidden_fraction)
        .powf(effective_samples)
        .clamp(1e-18, 1.0);

    (1.0 / miss_probability).log2()
}

pub fn compare_propagation_modes(config: &BlobStreamConfig) -> Vec<(String, f64)> {
    let mut throughput_by_mode: HashMap<BlobPropagationMode, f64> = HashMap::new();

    for mode in all_modes() {
        let mut mode_config = config.clone();
        mode_config.propagation_mode = mode;
        let latency_ms = estimate_propagation_latency(&mode_config).max(1.0);
        let throughput_mbps = total_da_bytes_per_block(&mode_config) as f64 * 8.0
            / (latency_ms / 1_000.0)
            / 1_000_000.0;
        throughput_by_mode.insert(mode, throughput_mbps);
    }

    all_modes()
        .into_iter()
        .map(|mode| {
            (
                format!("{mode:?}"),
                throughput_by_mode.get(&mode).copied().unwrap_or(0.0),
            )
        })
        .collect()
}

pub fn compute_blob_commitment(blobs: &[Vec<u8>]) -> [u8; 32] {
    let mut blob_hashes: Vec<[u8; 32]> = blobs
        .iter()
        .map(|blob| {
            let digest = Sha256::digest(blob);
            let mut hash = [0_u8; 32];
            hash.copy_from_slice(&digest);
            hash
        })
        .collect();
    blob_hashes.sort_unstable();

    let mut hasher = Sha256::new();
    hasher.update((blob_hashes.len() as u64).to_be_bytes());
    for hash in blob_hashes {
        hasher.update(hash);
    }

    let digest = hasher.finalize();
    let mut commitment = [0_u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn all_modes() -> [BlobPropagationMode; 5] {
    [
        BlobPropagationMode::PostBlock,
        BlobPropagationMode::Sidecar,
        BlobPropagationMode::InterleavedStream,
        BlobPropagationMode::PushBased,
        BlobPropagationMode::DASampled,
    ]
}

fn total_da_bytes_per_block(config: &BlobStreamConfig) -> usize {
    config
        .blobs_per_block
        .saturating_mul(config.blob_size_bytes)
}

fn encoded_da_bytes(config: &BlobStreamConfig) -> f64 {
    total_da_bytes_per_block(config) as f64 / sanitize_erasure_rate(config.erasure_coding_rate)
}

fn total_erasure_chunks(config: &BlobStreamConfig) -> usize {
    let raw_bytes = total_da_bytes_per_block(config);
    if raw_bytes == 0 {
        return 0;
    }

    let encoded_bytes = encoded_da_bytes(config).ceil() as usize;
    encoded_bytes.div_ceil(CHUNK_SIZE_BYTES).max(1)
}

fn sanitize_erasure_rate(rate: f64) -> f64 {
    if rate.is_finite() && rate > 0.0 {
        rate.min(1.0)
    } else {
        1.0
    }
}

fn strategy_download_fraction(config: &BlobStreamConfig) -> f64 {
    let total_chunks = total_erasure_chunks(config).max(1) as f64;

    match config.das_strategy {
        DASStrategy::FullDownload => 1.0,
        DASStrategy::RowColumn2D => 0.6,
        DASStrategy::RandomSampling => (config.sample_count as f64 / total_chunks).clamp(0.02, 1.0),
        DASStrategy::PeerDAS => {
            ((config.sample_count as f64 * 1.25) / total_chunks).clamp(0.05, 0.85)
        }
    }
}

fn mode_latency_factor(mode: BlobPropagationMode) -> f64 {
    let factors = HashMap::from([
        (BlobPropagationMode::PostBlock, 1.35),
        (BlobPropagationMode::Sidecar, 1.10),
        (BlobPropagationMode::InterleavedStream, 0.90),
        (BlobPropagationMode::PushBased, 0.78),
        (BlobPropagationMode::DASampled, 0.62),
    ]);

    factors.get(&mode).copied().unwrap_or(1.0)
}

fn per_blob_overhead_ms(mode: BlobPropagationMode) -> f64 {
    let overhead = HashMap::from([
        (BlobPropagationMode::PostBlock, 22.0),
        (BlobPropagationMode::Sidecar, 14.0),
        (BlobPropagationMode::InterleavedStream, 9.0),
        (BlobPropagationMode::PushBased, 7.0),
        (BlobPropagationMode::DASampled, 8.0),
    ]);

    overhead.get(&mode).copied().unwrap_or(10.0)
}

fn strategy_base_overhead_ms(strategy: DASStrategy) -> f64 {
    match strategy {
        DASStrategy::FullDownload => 120.0,
        DASStrategy::RowColumn2D => 92.0,
        DASStrategy::RandomSampling => 70.0,
        DASStrategy::PeerDAS => 95.0,
    }
}

fn strategy_security_weight(strategy: DASStrategy) -> f64 {
    match strategy {
        DASStrategy::FullDownload => 2.0,
        DASStrategy::RowColumn2D => 1.35,
        DASStrategy::RandomSampling => 1.0,
        DASStrategy::PeerDAS => 1.2,
    }
}

fn required_bandwidth_mbps(config: &BlobStreamConfig) -> f64 {
    let budget_s = config.max_blob_latency_ms.max(1) as f64 / 1_000.0;
    let bytes_needed = encoded_da_bytes(config) * strategy_download_fraction(config);
    let mode_factor = mode_latency_factor(config.propagation_mode);

    bytes_needed * 8.0 * mode_factor / (budget_s * 1_000_000.0)
}
