use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_BLOB_SIZE_BYTES: usize = 2 * 1024 * 1024;
const REFERENCE_BLOCK_TIME_S: f64 = 12.0;
const AVG_TX_SIZE_BYTES: usize = 220;
const MIN_DA_BANDWIDTH_MBPS: f64 = 0.001;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DaScalingStrategy {
    IncreaseBlobCount,
    IncreaseBlobSize,
    ParallelVerification,
    PipelinedStreaming,
    CompressedBlobs,
    ShardedDA,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VerificationMode {
    FullKZG,
    SampledDAS,
    OptimisticWithFraudProof,
    BatchedKZG,
    IncrementalVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaThroughputConfig {
    pub strategy: DaScalingStrategy,
    pub verification_mode: VerificationMode,
    pub blobs_per_block: usize,
    pub blob_size_bytes: usize,
    pub verification_parallelism: usize,
    pub target_da_bandwidth_mbps: f64,
    pub max_verification_time_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DaThroughputValidationError {
    ZeroBlobs,
    BlobSizeTooLarge { size: usize, max: usize },
    ZeroParallelism,
    BandwidthTooLow { value: f64 },
    VerificationTimeTooHigh { value: f64, max: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaThroughputStats {
    pub effective_da_bandwidth_mbps: f64,
    pub blobs_per_second: f64,
    pub verification_time_ms: f64,
    pub tps_contribution: f64,
    pub bottleneck: String,
    pub scaling_factor: f64,
}

pub fn default_da_throughput_config() -> DaThroughputConfig {
    DaThroughputConfig {
        strategy: DaScalingStrategy::PipelinedStreaming,
        verification_mode: VerificationMode::BatchedKZG,
        blobs_per_block: 6,
        blob_size_bytes: 128 * 1024,
        verification_parallelism: 8,
        target_da_bandwidth_mbps: 25.0,
        max_verification_time_ms: 120.0,
    }
}

pub fn validate_da_throughput_config(
    config: &DaThroughputConfig,
) -> Result<(), Vec<DaThroughputValidationError>> {
    let mut errors = Vec::new();

    if config.blobs_per_block == 0 {
        errors.push(DaThroughputValidationError::ZeroBlobs);
    }

    if config.blob_size_bytes > MAX_BLOB_SIZE_BYTES {
        errors.push(DaThroughputValidationError::BlobSizeTooLarge {
            size: config.blob_size_bytes,
            max: MAX_BLOB_SIZE_BYTES,
        });
    }

    if config.verification_parallelism == 0 {
        errors.push(DaThroughputValidationError::ZeroParallelism);
    }

    let required_mbps = required_bandwidth_mbps(config);
    if !config.target_da_bandwidth_mbps.is_finite()
        || config.target_da_bandwidth_mbps < required_mbps
        || config.target_da_bandwidth_mbps < MIN_DA_BANDWIDTH_MBPS
    {
        errors.push(DaThroughputValidationError::BandwidthTooLow {
            value: config.target_da_bandwidth_mbps,
        });
    }

    let verification_time_ms = compute_verification_time(
        config.verification_mode,
        config.blobs_per_block,
        config.verification_parallelism,
    );
    if !config.max_verification_time_ms.is_finite()
        || verification_time_ms > config.max_verification_time_ms
    {
        errors.push(DaThroughputValidationError::VerificationTimeTooHigh {
            value: verification_time_ms,
            max: config.max_verification_time_ms,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_da_throughput_stats(config: &DaThroughputConfig) -> DaThroughputStats {
    let profile = strategy_profile(config.strategy);

    let adjusted_blob_count = ((config.blobs_per_block as f64) * profile.blob_count_multiplier)
        .round()
        .max(1.0) as usize;
    let adjusted_blob_size_bytes = ((config.blob_size_bytes as f64) * profile.blob_size_multiplier)
        .round()
        .max(1.0);
    let adjusted_parallelism = ((config.verification_parallelism as f64)
        * profile.parallelism_multiplier)
        .round()
        .max(1.0) as usize;

    let verification_time_ms = compute_verification_time(
        config.verification_mode,
        adjusted_blob_count,
        adjusted_parallelism,
    ) * profile.verification_multiplier;

    let payload_bytes =
        adjusted_blob_count as f64 * adjusted_blob_size_bytes * profile.compression_ratio;
    let network_capacity_mbps =
        (config.target_da_bandwidth_mbps * profile.network_multiplier).max(MIN_DA_BANDWIDTH_MBPS);
    let transfer_time_ms = payload_bytes * 8.0 / (network_capacity_mbps * 1_000_000.0) * 1_000.0;

    let overlap_ms = profile.overlap_factor * transfer_time_ms.min(verification_time_ms);
    let end_to_end_ms =
        (transfer_time_ms + verification_time_ms - overlap_ms + profile.fixed_overhead_ms).max(1.0);

    let blobs_per_second = adjusted_blob_count as f64 / (end_to_end_ms / 1_000.0);
    let effective_da_bandwidth_mbps = payload_bytes * 8.0 / (end_to_end_ms / 1_000.0) / 1_000_000.0;
    let tps_contribution = estimate_tps_from_da(effective_da_bandwidth_mbps, AVG_TX_SIZE_BYTES);

    let baseline_tps = estimate_tps_from_da(config.target_da_bandwidth_mbps, AVG_TX_SIZE_BYTES);
    let scaling_factor = if baseline_tps > 0.0 {
        tps_contribution / baseline_tps
    } else {
        0.0
    };

    let bottleneck = if transfer_time_ms > verification_time_ms * 1.2 {
        "Network bandwidth".to_string()
    } else if verification_time_ms > transfer_time_ms * 1.2 {
        "Verification pipeline".to_string()
    } else {
        "Balanced pipeline".to_string()
    };

    DaThroughputStats {
        effective_da_bandwidth_mbps,
        blobs_per_second,
        verification_time_ms,
        tps_contribution,
        bottleneck,
        scaling_factor,
    }
}

pub fn compare_scaling_strategies(config: &DaThroughputConfig) -> Vec<(String, DaThroughputStats)> {
    all_strategies()
        .into_iter()
        .map(|strategy| {
            let mut strategy_config = config.clone();
            strategy_config.strategy = strategy;
            (
                format!("{strategy:?}"),
                compute_da_throughput_stats(&strategy_config),
            )
        })
        .collect()
}

pub fn estimate_tps_from_da(da_bandwidth_mbps: f64, avg_tx_size_bytes: usize) -> f64 {
    if !da_bandwidth_mbps.is_finite() || da_bandwidth_mbps <= 0.0 || avg_tx_size_bytes == 0 {
        return 0.0;
    }

    let bytes_per_second = da_bandwidth_mbps * 1_000_000.0 / 8.0;
    bytes_per_second / avg_tx_size_bytes as f64
}

pub fn compute_verification_time(
    mode: VerificationMode,
    blob_count: usize,
    parallelism: usize,
) -> f64 {
    if blob_count == 0 {
        return 0.0;
    }

    let parallelism = parallelism.max(1) as f64;
    let parallel_efficiency = (0.72 + 0.28 / parallelism.sqrt()).clamp(0.35, 1.0);
    let base_overhead_ms = mode_base_overhead_ms(mode);
    let work_units = mode_work_units(mode, blob_count);
    let ms_per_work_unit = mode_ms_per_work_unit(mode);

    base_overhead_ms + (work_units * ms_per_work_unit) / (parallelism * parallel_efficiency)
}

pub fn compute_da_commitment(config: &DaThroughputConfig, blob_roots: &[[u8; 32]]) -> [u8; 32] {
    let mut sorted_roots: Vec<[u8; 32]> = blob_roots.to_vec();
    sorted_roots.sort_unstable();

    let mut hasher = Sha256::new();
    hasher.update(b"ETH2077::DA_THROUGHPUT::V1");
    hasher.update([scaling_strategy_tag(config.strategy)]);
    hasher.update([verification_mode_tag(config.verification_mode)]);
    hasher.update((config.blobs_per_block as u64).to_be_bytes());
    hasher.update((config.blob_size_bytes as u64).to_be_bytes());
    hasher.update((config.verification_parallelism as u64).to_be_bytes());
    hasher.update(config.target_da_bandwidth_mbps.to_bits().to_be_bytes());
    hasher.update(config.max_verification_time_ms.to_bits().to_be_bytes());
    hasher.update((sorted_roots.len() as u64).to_be_bytes());

    for root in sorted_roots {
        hasher.update(root);
    }

    let digest = hasher.finalize();
    let mut commitment = [0_u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn required_bandwidth_mbps(config: &DaThroughputConfig) -> f64 {
    let payload_bits_per_block = (config
        .blobs_per_block
        .saturating_mul(config.blob_size_bytes)) as f64
        * 8.0;
    payload_bits_per_block / (REFERENCE_BLOCK_TIME_S * 1_000_000.0)
}

fn all_strategies() -> [DaScalingStrategy; 6] {
    [
        DaScalingStrategy::IncreaseBlobCount,
        DaScalingStrategy::IncreaseBlobSize,
        DaScalingStrategy::ParallelVerification,
        DaScalingStrategy::PipelinedStreaming,
        DaScalingStrategy::CompressedBlobs,
        DaScalingStrategy::ShardedDA,
    ]
}

#[derive(Debug, Clone, Copy)]
struct StrategyProfile {
    blob_count_multiplier: f64,
    blob_size_multiplier: f64,
    parallelism_multiplier: f64,
    network_multiplier: f64,
    compression_ratio: f64,
    verification_multiplier: f64,
    overlap_factor: f64,
    fixed_overhead_ms: f64,
}

fn strategy_profile(strategy: DaScalingStrategy) -> StrategyProfile {
    match strategy {
        DaScalingStrategy::IncreaseBlobCount => StrategyProfile {
            blob_count_multiplier: 1.60,
            blob_size_multiplier: 1.00,
            parallelism_multiplier: 1.00,
            network_multiplier: 0.95,
            compression_ratio: 1.00,
            verification_multiplier: 1.15,
            overlap_factor: 0.10,
            fixed_overhead_ms: 16.0,
        },
        DaScalingStrategy::IncreaseBlobSize => StrategyProfile {
            blob_count_multiplier: 1.00,
            blob_size_multiplier: 1.75,
            parallelism_multiplier: 1.00,
            network_multiplier: 0.93,
            compression_ratio: 1.00,
            verification_multiplier: 1.20,
            overlap_factor: 0.08,
            fixed_overhead_ms: 18.0,
        },
        DaScalingStrategy::ParallelVerification => StrategyProfile {
            blob_count_multiplier: 1.00,
            blob_size_multiplier: 1.00,
            parallelism_multiplier: 2.60,
            network_multiplier: 1.05,
            compression_ratio: 1.00,
            verification_multiplier: 0.92,
            overlap_factor: 0.25,
            fixed_overhead_ms: 14.0,
        },
        DaScalingStrategy::PipelinedStreaming => StrategyProfile {
            blob_count_multiplier: 1.00,
            blob_size_multiplier: 1.00,
            parallelism_multiplier: 1.20,
            network_multiplier: 1.35,
            compression_ratio: 1.00,
            verification_multiplier: 0.88,
            overlap_factor: 0.62,
            fixed_overhead_ms: 11.0,
        },
        DaScalingStrategy::CompressedBlobs => StrategyProfile {
            blob_count_multiplier: 1.00,
            blob_size_multiplier: 1.00,
            parallelism_multiplier: 1.15,
            network_multiplier: 1.20,
            compression_ratio: 0.58,
            verification_multiplier: 0.82,
            overlap_factor: 0.30,
            fixed_overhead_ms: 12.0,
        },
        DaScalingStrategy::ShardedDA => StrategyProfile {
            blob_count_multiplier: 2.20,
            blob_size_multiplier: 1.20,
            parallelism_multiplier: 1.80,
            network_multiplier: 2.10,
            compression_ratio: 0.92,
            verification_multiplier: 0.70,
            overlap_factor: 0.55,
            fixed_overhead_ms: 20.0,
        },
    }
}

fn mode_base_overhead_ms(mode: VerificationMode) -> f64 {
    match mode {
        VerificationMode::FullKZG => 18.0,
        VerificationMode::SampledDAS => 9.0,
        VerificationMode::OptimisticWithFraudProof => 6.0,
        VerificationMode::BatchedKZG => 14.0,
        VerificationMode::IncrementalVerification => 11.0,
    }
}

fn mode_ms_per_work_unit(mode: VerificationMode) -> f64 {
    match mode {
        VerificationMode::FullKZG => 4.8,
        VerificationMode::SampledDAS => 3.2,
        VerificationMode::OptimisticWithFraudProof => 2.0,
        VerificationMode::BatchedKZG => 5.3,
        VerificationMode::IncrementalVerification => 3.8,
    }
}

fn mode_work_units(mode: VerificationMode, blob_count: usize) -> f64 {
    let n = blob_count as f64;
    match mode {
        VerificationMode::FullKZG => n,
        VerificationMode::SampledDAS => n * 0.45,
        VerificationMode::OptimisticWithFraudProof => n * 0.25,
        VerificationMode::BatchedKZG => n.powf(0.78) * 1.6,
        VerificationMode::IncrementalVerification => n * 0.65,
    }
}

fn scaling_strategy_tag(strategy: DaScalingStrategy) -> u8 {
    match strategy {
        DaScalingStrategy::IncreaseBlobCount => 0,
        DaScalingStrategy::IncreaseBlobSize => 1,
        DaScalingStrategy::ParallelVerification => 2,
        DaScalingStrategy::PipelinedStreaming => 3,
        DaScalingStrategy::CompressedBlobs => 4,
        DaScalingStrategy::ShardedDA => 5,
    }
}

fn verification_mode_tag(mode: VerificationMode) -> u8 {
    match mode {
        VerificationMode::FullKZG => 0,
        VerificationMode::SampledDAS => 1,
        VerificationMode::OptimisticWithFraudProof => 2,
        VerificationMode::BatchedKZG => 3,
        VerificationMode::IncrementalVerification => 4,
    }
}
