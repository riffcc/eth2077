use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ExecutionBackend {
    SequentialEVM,
    ParallelEVM,
    RISCV,
    HybridRISCVEVM,
    PipelinedExecution,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GasAccountingModel {
    CurrentEIP1559,
    ModifiedEIP1559,
    MultidimensionalGas,
    BlobGasSeparate,
    ComputeOnlyGas,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ScalingApproach {
    VerticalScaling,
    HorizontalSharding,
    StatelessExecution,
    ParallelTransactions,
    SpeculativeExecution,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GigagasConfig {
    pub backend: ExecutionBackend,
    pub gas_model: GasAccountingModel,
    pub scaling_approach: ScalingApproach,
    pub target_ggas_per_sec: f64,
    pub current_ggas_per_sec: f64,
    pub core_count: usize,
    pub state_size_gb: f64,
    pub io_bandwidth_gbps: f64,
    pub memory_gb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GigagasValidationError {
    TargetBelowCurrent,
    ZeroCores,
    InsufficientMemory {
        required_gb: f64,
        available_gb: f64,
    },
    InsufficientIO {
        required_gbps: f64,
        available_gbps: f64,
    },
    StateExceedsMemory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GigagasStats {
    pub projected_ggas_per_sec: f64,
    pub scaling_factor: f64,
    pub bottleneck: String,
    pub state_access_overhead_pct: f64,
    pub parallelism_efficiency: f64,
    pub meets_target: bool,
    pub gap_to_target_pct: f64,
    pub estimated_tps_equivalent: f64,
}

pub fn default_gigagas_config() -> GigagasConfig {
    GigagasConfig {
        backend: ExecutionBackend::HybridRISCVEVM,
        gas_model: GasAccountingModel::MultidimensionalGas,
        scaling_approach: ScalingApproach::ParallelTransactions,
        target_ggas_per_sec: 1.0,
        current_ggas_per_sec: 0.12,
        core_count: 128,
        state_size_gb: 768.0,
        io_bandwidth_gbps: 320.0,
        memory_gb: 1_536.0,
    }
}

pub fn validate_gigagas_config(config: &GigagasConfig) -> Result<(), Vec<GigagasValidationError>> {
    let mut errors = Vec::new();

    if config.target_ggas_per_sec < config.current_ggas_per_sec {
        errors.push(GigagasValidationError::TargetBelowCurrent);
    }

    if config.core_count == 0 {
        errors.push(GigagasValidationError::ZeroCores);
    }

    let required_memory_gb = required_memory_gb(config);
    if config.memory_gb < required_memory_gb {
        errors.push(GigagasValidationError::InsufficientMemory {
            required_gb: required_memory_gb,
            available_gb: config.memory_gb,
        });
    }

    let required_io_gbps = required_io_gbps(config);
    if config.io_bandwidth_gbps < required_io_gbps {
        errors.push(GigagasValidationError::InsufficientIO {
            required_gbps: required_io_gbps,
            available_gbps: config.io_bandwidth_gbps,
        });
    }

    if config.state_size_gb > config.memory_gb {
        errors.push(GigagasValidationError::StateExceedsMemory);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_gigagas_stats(config: &GigagasConfig) -> GigagasStats {
    let backend_factor = backend_multiplier(config.backend);
    let gas_factor = gas_model_multiplier(config.gas_model);
    let scaling_factor = scaling_multiplier(config.scaling_approach);
    let contention = contention_assumption(config.scaling_approach);
    let parallelism_efficiency = estimate_parallelism_efficiency(config.core_count, contention);

    let core_parallel_gain = if config.core_count == 0 {
        0.0
    } else {
        1.0 + ((config.core_count as f64).sqrt() - 1.0) * parallelism_efficiency
    };

    let required_memory = required_memory_gb(config);
    let required_io = required_io_gbps(config);
    let memory_headroom = if required_memory == 0.0 {
        1.0
    } else {
        (config.memory_gb / required_memory).clamp(0.20, 1.25)
    };
    let io_headroom = if required_io == 0.0 {
        1.0
    } else {
        (config.io_bandwidth_gbps / required_io).clamp(0.20, 1.20)
    };

    let state_pressure = if config.memory_gb <= 0.0 {
        2.0
    } else {
        (config.state_size_gb / config.memory_gb).clamp(0.0, 2.0)
    };
    let state_access_overhead_pct =
        (5.0 + 38.0 * state_pressure + 28.0 * contention).clamp(5.0, 92.0);

    let scaling_factor_total = backend_factor
        * gas_factor
        * scaling_factor
        * core_parallel_gain
        * (1.0 - state_access_overhead_pct / 100.0)
        * memory_headroom
        * io_headroom;

    let projected_ggas_per_sec = config.current_ggas_per_sec * scaling_factor_total;
    let meets_target = projected_ggas_per_sec >= config.target_ggas_per_sec;
    let gap_to_target_pct = if config.target_ggas_per_sec <= 0.0 || meets_target {
        0.0
    } else {
        ((config.target_ggas_per_sec - projected_ggas_per_sec) / config.target_ggas_per_sec) * 100.0
    };

    let bottleneck = classify_bottleneck(
        config,
        memory_headroom,
        io_headroom,
        parallelism_efficiency,
        state_access_overhead_pct,
    );
    let estimated_tps_equivalent = projected_ggas_per_sec * 20_000.0;

    GigagasStats {
        projected_ggas_per_sec,
        scaling_factor: scaling_factor_total,
        bottleneck,
        state_access_overhead_pct,
        parallelism_efficiency,
        meets_target,
        gap_to_target_pct,
        estimated_tps_equivalent,
    }
}

pub fn compare_scaling_approaches(config: &GigagasConfig) -> Vec<(String, GigagasStats)> {
    all_scaling_approaches()
        .into_iter()
        .map(|approach| {
            let mut variant = config.clone();
            variant.scaling_approach = approach;
            (format!("{approach:?}"), compute_gigagas_stats(&variant))
        })
        .collect()
}

pub fn estimate_parallelism_efficiency(core_count: usize, state_contention: f64) -> f64 {
    if core_count == 0 {
        return 0.0;
    }

    let contention = state_contention.clamp(0.0, 1.0);
    let topology_penalty = (core_count as f64).ln_1p() / 120.0;
    (0.95 - 0.60 * contention - topology_penalty).clamp(0.05, 0.98)
}

pub fn compute_gigagas_commitment(config: &GigagasConfig) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([execution_backend_discriminant(config.backend)]);
    hasher.update([gas_model_discriminant(config.gas_model)]);
    hasher.update([scaling_approach_discriminant(config.scaling_approach)]);
    hasher.update(config.target_ggas_per_sec.to_bits().to_be_bytes());
    hasher.update(config.current_ggas_per_sec.to_bits().to_be_bytes());
    hasher.update((config.core_count as u64).to_be_bytes());
    hasher.update(config.state_size_gb.to_bits().to_be_bytes());
    hasher.update(config.io_bandwidth_gbps.to_bits().to_be_bytes());
    hasher.update(config.memory_gb.to_bits().to_be_bytes());

    let digest = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

fn required_memory_gb(config: &GigagasConfig) -> f64 {
    let state_cache_multiplier = match config.scaling_approach {
        ScalingApproach::VerticalScaling => 1.10,
        ScalingApproach::HorizontalSharding => 0.85,
        ScalingApproach::StatelessExecution => 0.65,
        ScalingApproach::ParallelTransactions => 1.05,
        ScalingApproach::SpeculativeExecution => 1.20,
    };

    (config.state_size_gb * state_cache_multiplier) + 16.0 + (config.core_count as f64 * 0.40)
}

fn required_io_gbps(config: &GigagasConfig) -> f64 {
    let backend_io_factor = match config.backend {
        ExecutionBackend::SequentialEVM => 1.15,
        ExecutionBackend::ParallelEVM => 1.30,
        ExecutionBackend::RISCV => 1.25,
        ExecutionBackend::HybridRISCVEVM => 1.40,
        ExecutionBackend::PipelinedExecution => 1.45,
    };

    let gas_model_io_factor = match config.gas_model {
        GasAccountingModel::CurrentEIP1559 => 1.00,
        GasAccountingModel::ModifiedEIP1559 => 1.03,
        GasAccountingModel::MultidimensionalGas => 1.10,
        GasAccountingModel::BlobGasSeparate => 1.18,
        GasAccountingModel::ComputeOnlyGas => 0.92,
    };

    let target = config.target_ggas_per_sec.max(config.current_ggas_per_sec);
    (target * 120.0) * backend_io_factor * gas_model_io_factor
}

fn classify_bottleneck(
    config: &GigagasConfig,
    memory_headroom: f64,
    io_headroom: f64,
    parallelism_efficiency: f64,
    state_access_overhead_pct: f64,
) -> String {
    if config.core_count == 0 || parallelism_efficiency < 0.20 {
        return "ComputeParallelism".to_string();
    }
    if memory_headroom < 1.0 {
        return "MemoryCapacity".to_string();
    }
    if io_headroom < 1.0 {
        return "IOBandwidth".to_string();
    }
    if state_access_overhead_pct > 45.0 {
        return "StateAccess".to_string();
    }
    "ExecutionPipeline".to_string()
}

fn backend_multiplier(backend: ExecutionBackend) -> f64 {
    match backend {
        ExecutionBackend::SequentialEVM => 0.85,
        ExecutionBackend::ParallelEVM => 1.30,
        ExecutionBackend::RISCV => 1.45,
        ExecutionBackend::HybridRISCVEVM => 1.62,
        ExecutionBackend::PipelinedExecution => 1.80,
    }
}

fn gas_model_multiplier(model: GasAccountingModel) -> f64 {
    match model {
        GasAccountingModel::CurrentEIP1559 => 1.00,
        GasAccountingModel::ModifiedEIP1559 => 1.08,
        GasAccountingModel::MultidimensionalGas => 1.22,
        GasAccountingModel::BlobGasSeparate => 1.15,
        GasAccountingModel::ComputeOnlyGas => 1.28,
    }
}

fn scaling_multiplier(approach: ScalingApproach) -> f64 {
    match approach {
        ScalingApproach::VerticalScaling => 1.10,
        ScalingApproach::HorizontalSharding => 1.34,
        ScalingApproach::StatelessExecution => 1.28,
        ScalingApproach::ParallelTransactions => 1.52,
        ScalingApproach::SpeculativeExecution => 1.70,
    }
}

fn contention_assumption(approach: ScalingApproach) -> f64 {
    match approach {
        ScalingApproach::VerticalScaling => 0.55,
        ScalingApproach::HorizontalSharding => 0.30,
        ScalingApproach::StatelessExecution => 0.22,
        ScalingApproach::ParallelTransactions => 0.25,
        ScalingApproach::SpeculativeExecution => 0.40,
    }
}

fn all_scaling_approaches() -> [ScalingApproach; 5] {
    [
        ScalingApproach::VerticalScaling,
        ScalingApproach::HorizontalSharding,
        ScalingApproach::StatelessExecution,
        ScalingApproach::ParallelTransactions,
        ScalingApproach::SpeculativeExecution,
    ]
}

fn execution_backend_discriminant(backend: ExecutionBackend) -> u8 {
    match backend {
        ExecutionBackend::SequentialEVM => 0,
        ExecutionBackend::ParallelEVM => 1,
        ExecutionBackend::RISCV => 2,
        ExecutionBackend::HybridRISCVEVM => 3,
        ExecutionBackend::PipelinedExecution => 4,
    }
}

fn gas_model_discriminant(model: GasAccountingModel) -> u8 {
    match model {
        GasAccountingModel::CurrentEIP1559 => 0,
        GasAccountingModel::ModifiedEIP1559 => 1,
        GasAccountingModel::MultidimensionalGas => 2,
        GasAccountingModel::BlobGasSeparate => 3,
        GasAccountingModel::ComputeOnlyGas => 4,
    }
}

fn scaling_approach_discriminant(approach: ScalingApproach) -> u8 {
    match approach {
        ScalingApproach::VerticalScaling => 0,
        ScalingApproach::HorizontalSharding => 1,
        ScalingApproach::StatelessExecution => 2,
        ScalingApproach::ParallelTransactions => 3,
        ScalingApproach::SpeculativeExecution => 4,
    }
}
