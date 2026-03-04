use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

const MIN_MODULUS_BITS: usize = 10;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum NttScheme {
    Falcon512,
    Falcon1024,
    Dilithium2,
    Dilithium3,
    Dilithium5,
    Kyber512,
    Kyber768,
    Kyber1024,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PrecompileStrategy {
    NoPrecompile,        // Pure EVM implementation
    GenericNtt,          // Single NTT precompile, scheme-agnostic
    SchemeSpecific,      // Separate precompile per scheme
    BatchedNtt,          // Batched NTT operations for amortization
    HardwareAccelerated, // With hardware NTT support assumption
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NttConfig {
    pub target_scheme: NttScheme,
    pub precompile_strategy: PrecompileStrategy,
    pub ntt_degree: usize,          // polynomial degree (512, 1024)
    pub modulus_bits: usize,        // prime modulus bit width
    pub batch_size: usize,          // for batched mode
    pub target_gas_per_verify: u64, // gas budget for one sig verify
    pub current_evm_gas: u64,       // baseline EVM gas without precompile
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NttValidationError {
    InvalidDegree { degree: usize }, // must be power of 2
    ModulusTooSmall { bits: usize, min: usize },
    GasBudgetZero,
    BatchSizeZero,
    IncompatibleSchemeAndDegree { scheme: String, degree: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NttImpactStats {
    pub gas_reduction_factor: f64,
    pub estimated_precompile_gas: u64,
    pub ntt_operations_per_verify: usize,
    pub post_quantum_readiness_score: f64,
    pub strategy_comparison: Vec<(String, f64)>, // strategy name -> gas cost
    pub scheme_comparison: Vec<(String, u64)>,   // scheme name -> verify gas
}

pub fn default_ntt_config() -> NttConfig {
    NttConfig {
        target_scheme: NttScheme::Falcon512,
        precompile_strategy: PrecompileStrategy::GenericNtt,
        ntt_degree: 512,
        modulus_bits: 12,
        batch_size: 1,
        target_gas_per_verify: 50_000,
        current_evm_gas: 600_000,
    }
}

pub fn validate_ntt_config(config: &NttConfig) -> Result<(), Vec<NttValidationError>> {
    let mut errors = Vec::new();

    if !config.ntt_degree.is_power_of_two() {
        errors.push(NttValidationError::InvalidDegree {
            degree: config.ntt_degree,
        });
    }

    if config.modulus_bits < MIN_MODULUS_BITS {
        errors.push(NttValidationError::ModulusTooSmall {
            bits: config.modulus_bits,
            min: MIN_MODULUS_BITS,
        });
    }

    if config.target_gas_per_verify == 0 {
        errors.push(NttValidationError::GasBudgetZero);
    }

    if config.batch_size == 0 {
        errors.push(NttValidationError::BatchSizeZero);
    }

    let expected_degree = scheme_ntt_degree(config.target_scheme);
    if config.ntt_degree != expected_degree {
        errors.push(NttValidationError::IncompatibleSchemeAndDegree {
            scheme: scheme_name(config.target_scheme).to_owned(),
            degree: config.ntt_degree,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_ntt_stats(config: &NttConfig) -> NttImpactStats {
    let estimated_precompile_gas = estimate_precompile_gas(config);
    let reduction_denominator = estimated_precompile_gas.max(1) as f64;
    let gas_reduction_factor = config.current_evm_gas as f64 / reduction_denominator;
    let ntt_operations_per_verify = scheme_ntt_operations(config.target_scheme);
    let supported_schemes =
        strategy_supported_schemes(config.precompile_strategy, config.target_scheme);
    let post_quantum_readiness_score = compute_pq_readiness(&supported_schemes);
    let strategy_comparison = compare_strategies(config);

    let scheme_comparison = all_schemes()
        .into_iter()
        .map(|scheme| {
            let mut scheme_config = config.clone();
            scheme_config.target_scheme = scheme;
            scheme_config.ntt_degree = scheme_ntt_degree(scheme);
            (
                scheme_name(scheme).to_owned(),
                estimate_precompile_gas(&scheme_config),
            )
        })
        .collect();

    NttImpactStats {
        gas_reduction_factor,
        estimated_precompile_gas,
        ntt_operations_per_verify,
        post_quantum_readiness_score,
        strategy_comparison,
        scheme_comparison,
    }
}

pub fn estimate_precompile_gas(config: &NttConfig) -> u64 {
    estimate_precompile_gas_float(config).round().max(1.0) as u64
}

pub fn scheme_ntt_degree(scheme: NttScheme) -> usize {
    match scheme {
        NttScheme::Falcon512 => 512,
        NttScheme::Falcon1024 => 1024,
        NttScheme::Dilithium2
        | NttScheme::Dilithium3
        | NttScheme::Dilithium5
        | NttScheme::Kyber512
        | NttScheme::Kyber768
        | NttScheme::Kyber1024 => 256,
    }
}

pub fn compare_strategies(config: &NttConfig) -> Vec<(String, f64)> {
    all_strategies()
        .into_iter()
        .map(|strategy| {
            let mut variant = config.clone();
            variant.precompile_strategy = strategy;
            (
                strategy_name(strategy).to_owned(),
                estimate_precompile_gas_float(&variant),
            )
        })
        .collect()
}

pub fn compute_pq_readiness(schemes_supported: &[NttScheme]) -> f64 {
    if schemes_supported.is_empty() {
        return 0.0;
    }

    let unique: HashSet<NttScheme> = schemes_supported.iter().copied().collect();
    let coverage_ratio = unique.len() as f64 / all_schemes().len() as f64;

    let mut family_capacity = HashMap::new();
    family_capacity.insert("falcon", 2usize);
    family_capacity.insert("dilithium", 3usize);
    family_capacity.insert("kyber", 3usize);

    let mut family_counts: HashMap<&str, usize> = HashMap::new();
    for scheme in unique {
        *family_counts.entry(scheme_family(scheme)).or_insert(0) += 1;
    }

    let family_coverage_ratio = family_counts.len() as f64 / family_capacity.len() as f64;
    let family_depth_ratio = family_capacity
        .iter()
        .map(|(family, cap)| {
            let have = *family_counts.get(*family).unwrap_or(&0) as f64;
            (have / *cap as f64).min(1.0)
        })
        .sum::<f64>()
        / family_capacity.len() as f64;

    (0.5 * coverage_ratio + 0.3 * family_coverage_ratio + 0.2 * family_depth_ratio).clamp(0.0, 1.0)
}

pub fn compute_ntt_commitment(config: &NttConfig, params: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"eth2077-ntt-v1");
    hasher.update([scheme_discriminant(config.target_scheme)]);
    hasher.update([strategy_discriminant(config.precompile_strategy)]);
    hasher.update((config.ntt_degree as u64).to_be_bytes());
    hasher.update((config.modulus_bits as u64).to_be_bytes());
    hasher.update((config.batch_size as u64).to_be_bytes());
    hasher.update(config.target_gas_per_verify.to_be_bytes());
    hasher.update(config.current_evm_gas.to_be_bytes());
    hasher.update((params.len() as u64).to_be_bytes());
    hasher.update(params);

    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn estimate_precompile_gas_float(config: &NttConfig) -> f64 {
    let degree = config.ntt_degree.max(1);
    let complexity = degree as f64 * (degree as f64).log2();
    let modulus_multiplier =
        1.0 + ((config.modulus_bits as f64 - MIN_MODULUS_BITS as f64).max(0.0) * 0.04);
    let ntt_base = 350.0 + complexity * modulus_multiplier * 0.95;
    let operation_count = scheme_ntt_operations(config.target_scheme) as f64;
    let strategy_multiplier = strategy_multipliers()
        .get(&config.precompile_strategy)
        .copied()
        .unwrap_or(1.0);

    let batch_size = config.batch_size.max(1) as f64;
    let batch_discount = match config.precompile_strategy {
        PrecompileStrategy::NoPrecompile => 1.0,
        PrecompileStrategy::BatchedNtt => 1.0 / (1.0 + (batch_size - 1.0).sqrt() * 0.22),
        PrecompileStrategy::HardwareAccelerated => 1.0 / (1.0 + (batch_size - 1.0).sqrt() * 0.18),
        _ => 1.0 / (1.0 + (batch_size - 1.0).sqrt() * 0.12),
    };

    (ntt_base * operation_count * strategy_multiplier * batch_discount).max(1.0)
}

fn all_schemes() -> [NttScheme; 8] {
    [
        NttScheme::Falcon512,
        NttScheme::Falcon1024,
        NttScheme::Dilithium2,
        NttScheme::Dilithium3,
        NttScheme::Dilithium5,
        NttScheme::Kyber512,
        NttScheme::Kyber768,
        NttScheme::Kyber1024,
    ]
}

fn all_strategies() -> [PrecompileStrategy; 5] {
    [
        PrecompileStrategy::NoPrecompile,
        PrecompileStrategy::GenericNtt,
        PrecompileStrategy::SchemeSpecific,
        PrecompileStrategy::BatchedNtt,
        PrecompileStrategy::HardwareAccelerated,
    ]
}

fn strategy_multipliers() -> HashMap<PrecompileStrategy, f64> {
    let mut multipliers = HashMap::new();
    multipliers.insert(PrecompileStrategy::NoPrecompile, 11.8);
    multipliers.insert(PrecompileStrategy::GenericNtt, 1.0);
    multipliers.insert(PrecompileStrategy::SchemeSpecific, 0.84);
    multipliers.insert(PrecompileStrategy::BatchedNtt, 0.72);
    multipliers.insert(PrecompileStrategy::HardwareAccelerated, 0.45);
    multipliers
}

fn scheme_ntt_operations(scheme: NttScheme) -> usize {
    let mut op_counts = HashMap::new();
    op_counts.insert(NttScheme::Falcon512, 10usize);
    op_counts.insert(NttScheme::Falcon1024, 12usize);
    op_counts.insert(NttScheme::Dilithium2, 14usize);
    op_counts.insert(NttScheme::Dilithium3, 16usize);
    op_counts.insert(NttScheme::Dilithium5, 18usize);
    op_counts.insert(NttScheme::Kyber512, 12usize);
    op_counts.insert(NttScheme::Kyber768, 14usize);
    op_counts.insert(NttScheme::Kyber1024, 16usize);
    op_counts.get(&scheme).copied().unwrap_or(12)
}

fn strategy_supported_schemes(strategy: PrecompileStrategy, target: NttScheme) -> Vec<NttScheme> {
    match strategy {
        PrecompileStrategy::NoPrecompile => vec![target],
        _ => all_schemes().to_vec(),
    }
}

fn scheme_name(scheme: NttScheme) -> &'static str {
    match scheme {
        NttScheme::Falcon512 => "Falcon512",
        NttScheme::Falcon1024 => "Falcon1024",
        NttScheme::Dilithium2 => "Dilithium2",
        NttScheme::Dilithium3 => "Dilithium3",
        NttScheme::Dilithium5 => "Dilithium5",
        NttScheme::Kyber512 => "Kyber512",
        NttScheme::Kyber768 => "Kyber768",
        NttScheme::Kyber1024 => "Kyber1024",
    }
}

fn strategy_name(strategy: PrecompileStrategy) -> &'static str {
    match strategy {
        PrecompileStrategy::NoPrecompile => "NoPrecompile",
        PrecompileStrategy::GenericNtt => "GenericNtt",
        PrecompileStrategy::SchemeSpecific => "SchemeSpecific",
        PrecompileStrategy::BatchedNtt => "BatchedNtt",
        PrecompileStrategy::HardwareAccelerated => "HardwareAccelerated",
    }
}

fn scheme_family(scheme: NttScheme) -> &'static str {
    match scheme {
        NttScheme::Falcon512 | NttScheme::Falcon1024 => "falcon",
        NttScheme::Dilithium2 | NttScheme::Dilithium3 | NttScheme::Dilithium5 => "dilithium",
        NttScheme::Kyber512 | NttScheme::Kyber768 | NttScheme::Kyber1024 => "kyber",
    }
}

fn scheme_discriminant(scheme: NttScheme) -> u8 {
    match scheme {
        NttScheme::Falcon512 => 0,
        NttScheme::Falcon1024 => 1,
        NttScheme::Dilithium2 => 2,
        NttScheme::Dilithium3 => 3,
        NttScheme::Dilithium5 => 4,
        NttScheme::Kyber512 => 5,
        NttScheme::Kyber768 => 6,
        NttScheme::Kyber1024 => 7,
    }
}

fn strategy_discriminant(strategy: PrecompileStrategy) -> u8 {
    match strategy {
        PrecompileStrategy::NoPrecompile => 0,
        PrecompileStrategy::GenericNtt => 1,
        PrecompileStrategy::SchemeSpecific => 2,
        PrecompileStrategy::BatchedNtt => 3,
        PrecompileStrategy::HardwareAccelerated => 4,
    }
}
