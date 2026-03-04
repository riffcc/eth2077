use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum L2ScalingStrategy {
    RollupStacking,
    ValidiumHybrid,
    DataSharding,
    ParallelRollups,
    BaseLayerDA,
    InterleavedBatching,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DataLaneType {
    CallData,
    BlobData,
    DASampled,
    OffChainDA,
    HybridDA,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CompressionScheme {
    None,
    Zstd,
    Brotli,
    CustomDelta,
    StateProof,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeragasConfig {
    pub strategy: L2ScalingStrategy,
    pub data_lane: DataLaneType,
    pub compression: CompressionScheme,
    pub target_throughput_gbps: f64,
    pub current_throughput_mbps: f64,
    pub rollup_count: usize,
    pub batch_size_kb: usize,
    pub compression_ratio: f64,
    pub l1_blob_capacity_mbps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TeragasValidationError {
    TargetBelowCurrent,
    ZeroRollups,
    CompressionRatioInvalid { value: f64 },
    BatchSizeTooLarge { size_kb: usize, max_kb: usize },
    InsufficientL1Capacity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeragasStats {
    pub projected_throughput_gbps: f64,
    pub scaling_factor: f64,
    pub bottleneck: String,
    pub effective_compression: f64,
    pub rollup_efficiency: f64,
    pub meets_target: bool,
    pub gap_pct: f64,
    pub equivalent_tps: f64,
}

pub fn default_teragas_config() -> TeragasConfig {
    TeragasConfig {
        strategy: L2ScalingStrategy::ParallelRollups,
        data_lane: DataLaneType::HybridDA,
        compression: CompressionScheme::CustomDelta,
        target_throughput_gbps: 1.0,
        current_throughput_mbps: 220.0,
        rollup_count: 16,
        batch_size_kb: 768,
        compression_ratio: 4.0,
        l1_blob_capacity_mbps: 600.0,
    }
}

pub fn validate_teragas_config(config: &TeragasConfig) -> Result<(), Vec<TeragasValidationError>> {
    let mut errors = Vec::new();

    let current_gbps = config.current_throughput_mbps / 1_000.0;
    if config.target_throughput_gbps < current_gbps {
        errors.push(TeragasValidationError::TargetBelowCurrent);
    }

    if config.rollup_count == 0 {
        errors.push(TeragasValidationError::ZeroRollups);
    }

    if !config.compression_ratio.is_finite() || config.compression_ratio < 1.0 {
        errors.push(TeragasValidationError::CompressionRatioInvalid {
            value: config.compression_ratio,
        });
    }

    const MAX_BATCH_SIZE_KB: usize = 16_384;
    if config.batch_size_kb > MAX_BATCH_SIZE_KB {
        errors.push(TeragasValidationError::BatchSizeTooLarge {
            size_kb: config.batch_size_kb,
            max_kb: MAX_BATCH_SIZE_KB,
        });
    }

    let required_l1_capacity = required_l1_for_target(config);
    if config.l1_blob_capacity_mbps < required_l1_capacity {
        errors.push(TeragasValidationError::InsufficientL1Capacity);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_teragas_stats(config: &TeragasConfig) -> TeragasStats {
    let strategy_gain = strategy_multiplier(config.strategy);
    let data_lane_gain = data_lane_multiplier(config.data_lane);
    let compression_gain = compression_multiplier(config.compression, config.compression_ratio);
    let rollup_efficiency = estimate_rollup_efficiency(config.rollup_count, config.batch_size_kb);

    let rollup_parallel_gain = if config.rollup_count == 0 {
        0.0
    } else {
        1.0 + (((config.rollup_count as f64).sqrt() - 1.0) * 0.42)
    };

    let batch_factor = batch_size_factor(config.batch_size_kb);

    let raw_projected_mbps = config.current_throughput_mbps
        * strategy_gain
        * data_lane_gain
        * compression_gain
        * rollup_efficiency
        * rollup_parallel_gain
        * batch_factor;

    let effective_compression = effective_compression_ratio(
        config.compression,
        config.compression_ratio,
        rollup_efficiency,
    );

    let l1_dependency = l1_dependency_factor(config.data_lane);
    let l1_demand_mbps = if effective_compression <= 0.0 {
        raw_projected_mbps * l1_dependency
    } else {
        raw_projected_mbps * l1_dependency / effective_compression
    };

    let l1_headroom = if l1_demand_mbps <= 0.0 {
        1.0
    } else {
        (config.l1_blob_capacity_mbps / l1_demand_mbps).clamp(0.15, 1.20)
    };

    let projected_mbps = raw_projected_mbps * l1_headroom;
    let projected_throughput_gbps = projected_mbps / 1_000.0;

    let scaling_factor = if config.current_throughput_mbps <= 0.0 {
        0.0
    } else {
        projected_mbps / config.current_throughput_mbps
    };

    let meets_target = projected_throughput_gbps >= config.target_throughput_gbps;
    let gap_pct = if config.target_throughput_gbps <= 0.0 || meets_target {
        0.0
    } else {
        ((config.target_throughput_gbps - projected_throughput_gbps)
            / config.target_throughput_gbps)
            * 100.0
    };

    let equivalent_tps = projected_mbps * 55.0;

    let bottleneck = classify_bottleneck(
        l1_headroom,
        rollup_efficiency,
        batch_factor,
        compression_gain,
    );

    TeragasStats {
        projected_throughput_gbps,
        scaling_factor,
        bottleneck,
        effective_compression,
        rollup_efficiency,
        meets_target,
        gap_pct,
        equivalent_tps,
    }
}

pub fn compare_l2_strategies(config: &TeragasConfig) -> Vec<(String, TeragasStats)> {
    all_strategies()
        .into_iter()
        .map(|strategy| {
            let mut variant = config.clone();
            variant.strategy = strategy;
            (format!("{strategy:?}"), compute_teragas_stats(&variant))
        })
        .collect()
}

pub fn estimate_rollup_efficiency(rollup_count: usize, batch_size_kb: usize) -> f64 {
    if rollup_count == 0 {
        return 0.0;
    }

    let coordination_penalty = ((rollup_count as f64).ln_1p() / 6.0).clamp(0.0, 0.60);
    let batching_gain = ((batch_size_kb.max(1) as f64).ln_1p() / 16.0).clamp(0.0, 0.18);

    (0.94 - coordination_penalty + batching_gain).clamp(0.10, 0.98)
}

pub fn compute_teragas_commitment(config: &TeragasConfig) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([strategy_discriminant(config.strategy)]);
    hasher.update([data_lane_discriminant(config.data_lane)]);
    hasher.update([compression_discriminant(config.compression)]);
    hasher.update(config.target_throughput_gbps.to_bits().to_be_bytes());
    hasher.update(config.current_throughput_mbps.to_bits().to_be_bytes());
    hasher.update((config.rollup_count as u64).to_be_bytes());
    hasher.update((config.batch_size_kb as u64).to_be_bytes());
    hasher.update(config.compression_ratio.to_bits().to_be_bytes());
    hasher.update(config.l1_blob_capacity_mbps.to_bits().to_be_bytes());

    let digest = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

fn all_strategies() -> [L2ScalingStrategy; 6] {
    [
        L2ScalingStrategy::RollupStacking,
        L2ScalingStrategy::ValidiumHybrid,
        L2ScalingStrategy::DataSharding,
        L2ScalingStrategy::ParallelRollups,
        L2ScalingStrategy::BaseLayerDA,
        L2ScalingStrategy::InterleavedBatching,
    ]
}

fn strategy_multiplier(strategy: L2ScalingStrategy) -> f64 {
    match strategy {
        L2ScalingStrategy::RollupStacking => 1.55,
        L2ScalingStrategy::ValidiumHybrid => 1.75,
        L2ScalingStrategy::DataSharding => 1.95,
        L2ScalingStrategy::ParallelRollups => 2.15,
        L2ScalingStrategy::BaseLayerDA => 1.35,
        L2ScalingStrategy::InterleavedBatching => 1.68,
    }
}

fn data_lane_multiplier(lane: DataLaneType) -> f64 {
    match lane {
        DataLaneType::CallData => 0.82,
        DataLaneType::BlobData => 1.28,
        DataLaneType::DASampled => 1.18,
        DataLaneType::OffChainDA => 1.52,
        DataLaneType::HybridDA => 1.34,
    }
}

fn l1_dependency_factor(lane: DataLaneType) -> f64 {
    match lane {
        DataLaneType::CallData => 1.15,
        DataLaneType::BlobData => 1.00,
        DataLaneType::DASampled => 0.65,
        DataLaneType::OffChainDA => 0.15,
        DataLaneType::HybridDA => 0.75,
    }
}

fn compression_multiplier(scheme: CompressionScheme, ratio: f64) -> f64 {
    let sane_ratio = ratio.clamp(1.0, 64.0);
    let delta = sane_ratio - 1.0;

    match scheme {
        CompressionScheme::None => 1.0,
        CompressionScheme::Zstd => (1.0 + delta * 0.35).clamp(1.0, 8.0),
        CompressionScheme::Brotli => (1.0 + delta * 0.40).clamp(1.0, 8.5),
        CompressionScheme::CustomDelta => (1.0 + delta * 0.52).clamp(1.0, 10.0),
        CompressionScheme::StateProof => (1.0 + delta * 0.60).clamp(1.0, 11.0),
    }
}

fn effective_compression_ratio(
    scheme: CompressionScheme,
    ratio: f64,
    rollup_efficiency: f64,
) -> f64 {
    let scheme_factor = match scheme {
        CompressionScheme::None => 1.0,
        CompressionScheme::Zstd => 0.92,
        CompressionScheme::Brotli => 0.95,
        CompressionScheme::CustomDelta => 1.05,
        CompressionScheme::StateProof => 1.10,
    };

    let efficiency_factor = 0.85 + (rollup_efficiency * 0.25);
    (ratio.max(1.0) * scheme_factor * efficiency_factor).clamp(1.0, 64.0)
}

fn batch_size_factor(batch_size_kb: usize) -> f64 {
    (0.78 + (batch_size_kb.max(1) as f64).ln_1p() / 14.0).clamp(0.75, 1.30)
}

fn required_l1_for_target(config: &TeragasConfig) -> f64 {
    let target_mbps = config.target_throughput_gbps.max(0.0) * 1_000.0;
    let dependency = l1_dependency_factor(config.data_lane);
    let ratio = config.compression_ratio.max(1.0);
    target_mbps * dependency / ratio
}

fn classify_bottleneck(
    l1_headroom: f64,
    rollup_efficiency: f64,
    batch_factor: f64,
    compression_gain: f64,
) -> String {
    if l1_headroom < 0.95 {
        "L1DataCapacity".to_string()
    } else if rollup_efficiency < 0.50 {
        "RollupCoordination".to_string()
    } else if batch_factor < 0.95 {
        "BatchingOverhead".to_string()
    } else if compression_gain < 1.15 {
        "CompressionLimits".to_string()
    } else {
        "ExecutionHeadroom".to_string()
    }
}

fn strategy_discriminant(strategy: L2ScalingStrategy) -> u8 {
    match strategy {
        L2ScalingStrategy::RollupStacking => 0,
        L2ScalingStrategy::ValidiumHybrid => 1,
        L2ScalingStrategy::DataSharding => 2,
        L2ScalingStrategy::ParallelRollups => 3,
        L2ScalingStrategy::BaseLayerDA => 4,
        L2ScalingStrategy::InterleavedBatching => 5,
    }
}

fn data_lane_discriminant(lane: DataLaneType) -> u8 {
    match lane {
        DataLaneType::CallData => 0,
        DataLaneType::BlobData => 1,
        DataLaneType::DASampled => 2,
        DataLaneType::OffChainDA => 3,
        DataLaneType::HybridDA => 4,
    }
}

fn compression_discriminant(scheme: CompressionScheme) -> u8 {
    match scheme {
        CompressionScheme::None => 0,
        CompressionScheme::Zstd => 1,
        CompressionScheme::Brotli => 2,
        CompressionScheme::CustomDelta => 3,
        CompressionScheme::StateProof => 4,
    }
}
