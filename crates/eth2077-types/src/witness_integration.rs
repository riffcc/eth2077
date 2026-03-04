use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WitnessType {
    StateDiff,
    StorageProof,
    AccountProof,
    TransactionWitness,
    BlockWitness,
    CrossShardWitness,
    CompactWitness,
    FullWitness,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DiffStrategy {
    FullDiff,
    IncrementalDiff,
    CompressedDiff,
    MerklizedDiff,
    BatchedDiff,
    StreamingDiff,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncMode {
    FullSync,
    SnapSync,
    WitnessSync,
    BeamSync,
    DiffBasedSync,
    HybridSync,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PropagationMethod {
    GossipBroadcast,
    DirectPush,
    PullOnDemand,
    AnticipatorySend,
    PrioritizedRelay,
    MixedStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WitnessEntry {
    pub witness_type: WitnessType,
    pub size_bytes: usize,
    pub generation_time_ms: f64,
    pub verification_time_ms: f64,
    pub compression_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WitnessIntegrationConfig {
    pub sync_mode: SyncMode,
    pub diff_strategy: DiffStrategy,
    pub propagation_method: PropagationMethod,
    pub witness_types: Vec<WitnessType>,
    pub max_witness_size_bytes: usize,
    pub max_generation_time_ms: f64,
    pub target_compression_ratio: f64,
    pub max_propagation_delay_ms: f64,
    pub parallel_verification: bool,
    pub witness_cache_size_mb: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WitnessValidationError {
    EmptyWitnessTypes,
    WitnessSizeTooLarge { size: usize, max: usize },
    GenerationTimeTooSlow { time: f64, max: f64 },
    CompressionRatioInvalid { value: f64 },
    PropagationDelayNonPositive { value: f64 },
    CacheSizeZero,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WitnessIntegrationStats {
    pub avg_witness_size_bytes: f64,
    pub avg_generation_time_ms: f64,
    pub avg_verification_time_ms: f64,
    pub effective_compression: f64,
    pub sync_throughput_mbps: f64,
    pub propagation_efficiency: f64,
    pub bottleneck: String,
    pub recommendations: Vec<String>,
}

pub fn default_witness_integration_config() -> WitnessIntegrationConfig {
    WitnessIntegrationConfig {
        sync_mode: SyncMode::WitnessSync,
        diff_strategy: DiffStrategy::IncrementalDiff,
        propagation_method: PropagationMethod::MixedStrategy,
        witness_types: vec![
            WitnessType::StateDiff,
            WitnessType::StorageProof,
            WitnessType::AccountProof,
            WitnessType::BlockWitness,
        ],
        max_witness_size_bytes: 512 * 1024,
        max_generation_time_ms: 180.0,
        target_compression_ratio: 0.55,
        max_propagation_delay_ms: 250.0,
        parallel_verification: true,
        witness_cache_size_mb: 256,
    }
}

pub fn validate_witness_config(
    config: &WitnessIntegrationConfig,
) -> Result<(), Vec<WitnessValidationError>> {
    let mut errors = Vec::new();
    const MAX_ALLOWED_WITNESS_BYTES: usize = 64 * 1024 * 1024;
    const MAX_ALLOWED_GENERATION_MS: f64 = 10_000.0;

    if config.witness_types.is_empty() {
        errors.push(WitnessValidationError::EmptyWitnessTypes);
    }

    if config.max_witness_size_bytes > MAX_ALLOWED_WITNESS_BYTES {
        errors.push(WitnessValidationError::WitnessSizeTooLarge {
            size: config.max_witness_size_bytes,
            max: MAX_ALLOWED_WITNESS_BYTES,
        });
    }

    if !config.max_generation_time_ms.is_finite()
        || config.max_generation_time_ms <= 0.0
        || config.max_generation_time_ms > MAX_ALLOWED_GENERATION_MS
    {
        errors.push(WitnessValidationError::GenerationTimeTooSlow {
            time: config.max_generation_time_ms,
            max: MAX_ALLOWED_GENERATION_MS,
        });
    }

    if !config.target_compression_ratio.is_finite()
        || config.target_compression_ratio <= 0.0
        || config.target_compression_ratio > 1.0
    {
        errors.push(WitnessValidationError::CompressionRatioInvalid {
            value: config.target_compression_ratio,
        });
    }

    if !config.max_propagation_delay_ms.is_finite() || config.max_propagation_delay_ms <= 0.0 {
        errors.push(WitnessValidationError::PropagationDelayNonPositive {
            value: config.max_propagation_delay_ms,
        });
    }

    if config.witness_cache_size_mb == 0 {
        errors.push(WitnessValidationError::CacheSizeZero);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_witness_stats(config: &WitnessIntegrationConfig) -> WitnessIntegrationStats {
    let compression_target = if config.target_compression_ratio.is_finite() {
        config.target_compression_ratio.clamp(0.05, 1.0)
    } else {
        1.0
    };
    let max_size = config.max_witness_size_bytes.max(1) as f64;
    let max_generation = if config.max_generation_time_ms.is_finite() {
        config.max_generation_time_ms.max(1.0)
    } else {
        1_000.0
    };

    let entries: Vec<WitnessEntry> = config
        .witness_types
        .iter()
        .map(|witness_type| {
            let base_size =
                base_witness_size_bytes(witness_type) * diff_size_factor(&config.diff_strategy);
            let compressed_size = (base_size * compression_target).min(max_size).max(1.0);
            let ratio = (compressed_size / base_size.max(1.0)).clamp(0.01, 1.0);

            let generation_time = (base_generation_time_ms(witness_type)
                * diff_generation_factor(&config.diff_strategy)
                * sync_generation_factor(&config.sync_mode))
            .min(max_generation)
            .max(0.1);

            let verification_time = (base_verification_time_ms(witness_type)
                * sync_verification_factor(&config.sync_mode)
                * if config.parallel_verification {
                    0.72
                } else {
                    1.0
                }
                * (1.0 + compressed_size / 250_000.0))
                .max(0.1);

            WitnessEntry {
                witness_type: witness_type.clone(),
                size_bytes: compressed_size.round() as usize,
                generation_time_ms: generation_time,
                verification_time_ms: verification_time,
                compression_ratio: ratio,
            }
        })
        .collect();

    let witness_count = entries.len().max(1) as f64;
    let total_size_bytes = entries
        .iter()
        .map(|entry| entry.size_bytes as f64)
        .sum::<f64>();
    let total_generation_ms = entries
        .iter()
        .map(|entry| entry.generation_time_ms)
        .sum::<f64>();
    let total_verification_ms = entries
        .iter()
        .map(|entry| entry.verification_time_ms)
        .sum::<f64>();
    let avg_ratio = entries
        .iter()
        .map(|entry| entry.compression_ratio)
        .sum::<f64>()
        / witness_count;
    let avg_witness_size_bytes = total_size_bytes / witness_count;
    let avg_generation_time_ms = total_generation_ms / witness_count;
    let avg_verification_time_ms = total_verification_ms / witness_count;

    let raw_total_size = config
        .witness_types
        .iter()
        .map(|witness_type| base_witness_size_bytes(witness_type))
        .sum::<f64>()
        .max(1.0);
    let effective_compression = (raw_total_size / total_size_bytes.max(1.0)).max(1.0);

    let propagation_delay_ms = config.max_propagation_delay_ms.max(1.0)
        * propagation_delay_factor(&config.propagation_method);
    let method_efficiency = propagation_method_efficiency(&config.propagation_method);
    let propagation_efficiency =
        (method_efficiency * (120.0 / (120.0 + propagation_delay_ms))).clamp(0.01, 1.0);

    let sync_pipeline = sync_pipeline_factor(&config.sync_mode);
    let total_time_ms = ((total_generation_ms + total_verification_ms) * sync_pipeline
        + propagation_delay_ms)
        .max(1.0);
    let mut sync_throughput_mbps =
        ((total_size_bytes * 8.0) / (total_time_ms / 1_000.0)) / 1_000_000.0;
    sync_throughput_mbps *= sync_throughput_factor(&config.sync_mode);
    sync_throughput_mbps *= propagation_efficiency;
    if config.parallel_verification {
        sync_throughput_mbps *= 1.12;
    }
    sync_throughput_mbps = sync_throughput_mbps.max(0.000_1);

    let bottleneck = if propagation_delay_ms > total_generation_ms.max(total_verification_ms) {
        "Propagation".to_string()
    } else if total_generation_ms > total_verification_ms {
        "WitnessGeneration".to_string()
    } else {
        "WitnessVerification".to_string()
    };

    let mut recommendations = Vec::new();
    if !config.parallel_verification {
        recommendations.push(
            "Enable parallel verification to improve witness validation throughput.".to_string(),
        );
    }
    if avg_ratio > 0.70 {
        recommendations.push(
            "Apply a stronger diff/compression policy to lower witness payload sizes.".to_string(),
        );
    }
    if config.max_propagation_delay_ms > 300.0 {
        recommendations.push(
            "Reduce propagation delay budget to avoid network-bound sync bottlenecks.".to_string(),
        );
    }
    if config.witness_cache_size_mb < 128 && config.witness_types.len() > 3 {
        recommendations
            .push("Increase witness cache capacity to improve hot-proof reuse.".to_string());
    }
    if recommendations.is_empty() {
        recommendations
            .push("Configuration is balanced for witness-driven synchronization.".to_string());
    }

    WitnessIntegrationStats {
        avg_witness_size_bytes,
        avg_generation_time_ms,
        avg_verification_time_ms,
        effective_compression,
        sync_throughput_mbps,
        propagation_efficiency,
        bottleneck,
        recommendations,
    }
}

pub fn compare_sync_modes(
    config: &WitnessIntegrationConfig,
) -> Vec<(String, WitnessIntegrationStats)> {
    all_sync_modes()
        .into_iter()
        .map(|sync_mode| {
            let mut variant = config.clone();
            variant.sync_mode = sync_mode;
            (
                format!("{:?}", variant.sync_mode),
                compute_witness_stats(&variant),
            )
        })
        .collect()
}

pub fn compute_witness_commitment(config: &WitnessIntegrationConfig) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ETH2077::WITNESS_INTEGRATION::V1");
    hasher.update([sync_mode_discriminant(&config.sync_mode)]);
    hasher.update([diff_strategy_discriminant(&config.diff_strategy)]);
    hasher.update([propagation_discriminant(&config.propagation_method)]);
    hasher.update((config.witness_types.len() as u64).to_be_bytes());
    for witness_type in &config.witness_types {
        hasher.update([witness_type_discriminant(witness_type)]);
    }
    hasher.update((config.max_witness_size_bytes as u64).to_be_bytes());
    hasher.update(config.max_generation_time_ms.to_bits().to_be_bytes());
    hasher.update(config.target_compression_ratio.to_bits().to_be_bytes());
    hasher.update(config.max_propagation_delay_ms.to_bits().to_be_bytes());
    hasher.update([u8::from(config.parallel_verification)]);
    hasher.update((config.witness_cache_size_mb as u64).to_be_bytes());

    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn all_sync_modes() -> [SyncMode; 6] {
    [
        SyncMode::FullSync,
        SyncMode::SnapSync,
        SyncMode::WitnessSync,
        SyncMode::BeamSync,
        SyncMode::DiffBasedSync,
        SyncMode::HybridSync,
    ]
}

fn base_witness_size_bytes(witness_type: &WitnessType) -> f64 {
    match witness_type {
        WitnessType::StateDiff => 72_000.0,
        WitnessType::StorageProof => 128_000.0,
        WitnessType::AccountProof => 96_000.0,
        WitnessType::TransactionWitness => 64_000.0,
        WitnessType::BlockWitness => 160_000.0,
        WitnessType::CrossShardWitness => 196_000.0,
        WitnessType::CompactWitness => 40_000.0,
        WitnessType::FullWitness => 256_000.0,
    }
}

fn base_generation_time_ms(witness_type: &WitnessType) -> f64 {
    match witness_type {
        WitnessType::StateDiff => 22.0,
        WitnessType::StorageProof => 35.0,
        WitnessType::AccountProof => 28.0,
        WitnessType::TransactionWitness => 18.0,
        WitnessType::BlockWitness => 44.0,
        WitnessType::CrossShardWitness => 58.0,
        WitnessType::CompactWitness => 14.0,
        WitnessType::FullWitness => 72.0,
    }
}

fn base_verification_time_ms(witness_type: &WitnessType) -> f64 {
    match witness_type {
        WitnessType::StateDiff => 9.0,
        WitnessType::StorageProof => 16.0,
        WitnessType::AccountProof => 12.0,
        WitnessType::TransactionWitness => 8.0,
        WitnessType::BlockWitness => 20.0,
        WitnessType::CrossShardWitness => 26.0,
        WitnessType::CompactWitness => 6.0,
        WitnessType::FullWitness => 34.0,
    }
}

fn diff_size_factor(strategy: &DiffStrategy) -> f64 {
    match strategy {
        DiffStrategy::FullDiff => 1.00,
        DiffStrategy::IncrementalDiff => 0.78,
        DiffStrategy::CompressedDiff => 0.66,
        DiffStrategy::MerklizedDiff => 0.72,
        DiffStrategy::BatchedDiff => 0.86,
        DiffStrategy::StreamingDiff => 0.70,
    }
}

fn diff_generation_factor(strategy: &DiffStrategy) -> f64 {
    match strategy {
        DiffStrategy::FullDiff => 1.00,
        DiffStrategy::IncrementalDiff => 0.92,
        DiffStrategy::CompressedDiff => 1.08,
        DiffStrategy::MerklizedDiff => 1.04,
        DiffStrategy::BatchedDiff => 0.88,
        DiffStrategy::StreamingDiff => 0.84,
    }
}

fn sync_generation_factor(mode: &SyncMode) -> f64 {
    match mode {
        SyncMode::FullSync => 1.10,
        SyncMode::SnapSync => 0.92,
        SyncMode::WitnessSync => 0.84,
        SyncMode::BeamSync => 0.90,
        SyncMode::DiffBasedSync => 0.86,
        SyncMode::HybridSync => 0.88,
    }
}

fn sync_verification_factor(mode: &SyncMode) -> f64 {
    match mode {
        SyncMode::FullSync => 1.08,
        SyncMode::SnapSync => 0.96,
        SyncMode::WitnessSync => 0.86,
        SyncMode::BeamSync => 0.94,
        SyncMode::DiffBasedSync => 0.88,
        SyncMode::HybridSync => 0.90,
    }
}

fn sync_pipeline_factor(mode: &SyncMode) -> f64 {
    match mode {
        SyncMode::FullSync => 1.12,
        SyncMode::SnapSync => 0.94,
        SyncMode::WitnessSync => 0.84,
        SyncMode::BeamSync => 0.92,
        SyncMode::DiffBasedSync => 0.88,
        SyncMode::HybridSync => 0.86,
    }
}

fn sync_throughput_factor(mode: &SyncMode) -> f64 {
    match mode {
        SyncMode::FullSync => 0.78,
        SyncMode::SnapSync => 1.08,
        SyncMode::WitnessSync => 1.22,
        SyncMode::BeamSync => 1.05,
        SyncMode::DiffBasedSync => 1.16,
        SyncMode::HybridSync => 1.18,
    }
}

fn propagation_delay_factor(method: &PropagationMethod) -> f64 {
    match method {
        PropagationMethod::GossipBroadcast => 1.00,
        PropagationMethod::DirectPush => 0.78,
        PropagationMethod::PullOnDemand => 1.15,
        PropagationMethod::AnticipatorySend => 0.72,
        PropagationMethod::PrioritizedRelay => 0.82,
        PropagationMethod::MixedStrategy => 0.80,
    }
}

fn propagation_method_efficiency(method: &PropagationMethod) -> f64 {
    match method {
        PropagationMethod::GossipBroadcast => 0.78,
        PropagationMethod::DirectPush => 0.92,
        PropagationMethod::PullOnDemand => 0.68,
        PropagationMethod::AnticipatorySend => 0.95,
        PropagationMethod::PrioritizedRelay => 0.89,
        PropagationMethod::MixedStrategy => 0.90,
    }
}

fn witness_type_discriminant(value: &WitnessType) -> u8 {
    match value {
        WitnessType::StateDiff => 0,
        WitnessType::StorageProof => 1,
        WitnessType::AccountProof => 2,
        WitnessType::TransactionWitness => 3,
        WitnessType::BlockWitness => 4,
        WitnessType::CrossShardWitness => 5,
        WitnessType::CompactWitness => 6,
        WitnessType::FullWitness => 7,
    }
}

fn diff_strategy_discriminant(value: &DiffStrategy) -> u8 {
    match value {
        DiffStrategy::FullDiff => 0,
        DiffStrategy::IncrementalDiff => 1,
        DiffStrategy::CompressedDiff => 2,
        DiffStrategy::MerklizedDiff => 3,
        DiffStrategy::BatchedDiff => 4,
        DiffStrategy::StreamingDiff => 5,
    }
}

fn sync_mode_discriminant(value: &SyncMode) -> u8 {
    match value {
        SyncMode::FullSync => 0,
        SyncMode::SnapSync => 1,
        SyncMode::WitnessSync => 2,
        SyncMode::BeamSync => 3,
        SyncMode::DiffBasedSync => 4,
        SyncMode::HybridSync => 5,
    }
}

fn propagation_discriminant(value: &PropagationMethod) -> u8 {
    match value {
        PropagationMethod::GossipBroadcast => 0,
        PropagationMethod::DirectPush => 1,
        PropagationMethod::PullOnDemand => 2,
        PropagationMethod::AnticipatorySend => 3,
        PropagationMethod::PrioritizedRelay => 4,
        PropagationMethod::MixedStrategy => 5,
    }
}
