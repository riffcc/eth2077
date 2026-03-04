use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MIN_FIELD_SIZE_BITS: usize = 128;
const MIN_CLASSICAL_SECURITY_BITS: usize = 96;
const MIN_POST_QUANTUM_SECURITY_BITS: usize = 124;
const MAX_SUPPORTED_PROOF_SIZE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProvingSystem {
    WHIR,
    SuperSpartan,
    Groth16,
    Plonk,
    Halo2,
    STARKs,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SignatureScheme {
    XMSS,
    SPHINCS,
    Dilithium,
    BLS,
    ECDSA,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VerificationStrategy {
    DirectVerify,
    AggregatedProof,
    RecursiveComposition,
    BatchVerification,
    IncrementalVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormalVerificationConfig {
    pub proving_system: ProvingSystem,
    pub signature_scheme: SignatureScheme,
    pub strategy: VerificationStrategy,
    pub security_bits: usize,
    pub max_aggregation_depth: usize,
    pub field_size_bits: usize,
    pub post_quantum: bool,
    pub max_proof_size_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerificationValidationError {
    InsufficientSecurity { bits: usize, minimum: usize },
    AggregationDepthZero,
    FieldSizeTooSmall { bits: usize },
    ProofSizeTooLarge { size: usize, max: usize },
    IncompatibleStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerificationImpactStats {
    pub proof_size_bytes: usize,
    pub verification_time_ms: f64,
    pub aggregation_savings_factor: f64,
    pub post_quantum_ready: bool,
    pub security_level_bits: usize,
    pub recursion_overhead_factor: f64,
}

pub fn default_formal_verification_config() -> FormalVerificationConfig {
    FormalVerificationConfig {
        proving_system: ProvingSystem::WHIR,
        signature_scheme: SignatureScheme::XMSS,
        strategy: VerificationStrategy::RecursiveComposition,
        security_bits: 124,
        max_aggregation_depth: 16,
        field_size_bits: 255,
        post_quantum: true,
        max_proof_size_bytes: 256_000,
    }
}

pub fn validate_verification_config(
    config: &FormalVerificationConfig,
) -> Result<(), Vec<VerificationValidationError>> {
    let mut errors = Vec::new();

    let min_security = if config.post_quantum {
        MIN_POST_QUANTUM_SECURITY_BITS
    } else {
        MIN_CLASSICAL_SECURITY_BITS
    };
    if config.security_bits < min_security {
        errors.push(VerificationValidationError::InsufficientSecurity {
            bits: config.security_bits,
            minimum: min_security,
        });
    }

    if config.max_aggregation_depth == 0 {
        errors.push(VerificationValidationError::AggregationDepthZero);
    }

    if config.field_size_bits < MIN_FIELD_SIZE_BITS {
        errors.push(VerificationValidationError::FieldSizeTooSmall {
            bits: config.field_size_bits,
        });
    }

    if config.max_proof_size_bytes > MAX_SUPPORTED_PROOF_SIZE_BYTES {
        errors.push(VerificationValidationError::ProofSizeTooLarge {
            size: config.max_proof_size_bytes,
            max: MAX_SUPPORTED_PROOF_SIZE_BYTES,
        });
    }

    if has_incompatible_strategy(config) {
        errors.push(VerificationValidationError::IncompatibleStrategy);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_verification_stats(config: &FormalVerificationConfig) -> VerificationImpactStats {
    let base_proof_size = base_proof_size_bytes(config.proving_system) as f64;
    let base_verification_time = base_verification_time_ms(config.proving_system);

    let (scheme_size_multiplier, scheme_time_multiplier) =
        signature_multipliers(config.signature_scheme);
    let (strategy_size_multiplier, strategy_time_multiplier) =
        strategy_multipliers(config.strategy);

    let depth = config.max_aggregation_depth.max(1);
    let aggregation_savings_factor = match config.strategy {
        VerificationStrategy::DirectVerify => 1.0,
        _ => estimate_aggregation_savings(depth, base_proof_size as usize),
    };
    let recursion_overhead_factor =
        estimate_recursion_overhead(config.proving_system, config.strategy, depth);

    let proof_size_bytes = (base_proof_size
        * scheme_size_multiplier
        * strategy_size_multiplier
        * recursion_overhead_factor
        / aggregation_savings_factor)
        .round()
        .max(1.0) as usize;

    let verification_time_ms = (base_verification_time
        * scheme_time_multiplier
        * strategy_time_multiplier
        * recursion_overhead_factor
        / aggregation_savings_factor.sqrt())
    .max(0.01);

    let security_level_bits = if config.post_quantum {
        estimate_pq_security(config.signature_scheme, config.security_bits)
    } else {
        config.security_bits
    };
    let post_quantum_ready = config.post_quantum
        && signature_is_post_quantum(config.signature_scheme)
        && security_level_bits >= MIN_POST_QUANTUM_SECURITY_BITS;

    VerificationImpactStats {
        proof_size_bytes,
        verification_time_ms,
        aggregation_savings_factor,
        post_quantum_ready,
        security_level_bits,
        recursion_overhead_factor,
    }
}

pub fn compare_proving_systems(
    config: &FormalVerificationConfig,
) -> Vec<(String, VerificationImpactStats)> {
    all_proving_systems()
        .into_iter()
        .map(|proving_system| {
            let mut variant = config.clone();
            variant.proving_system = proving_system;
            (
                proving_system_name(proving_system).to_owned(),
                compute_verification_stats(&variant),
            )
        })
        .collect()
}

pub fn estimate_aggregation_savings(depth: usize, base_proof_size: usize) -> f64 {
    if depth <= 1 {
        return 1.0;
    }

    let depth_gain = (depth as f64).log2() + 1.0;
    let size_penalty = 1.0 + ((base_proof_size.max(1) as f64 / 65_536.0).log2().max(0.0) * 0.2);
    (1.0 + (depth_gain * 0.72 / size_penalty)).clamp(1.0, depth as f64)
}

pub fn estimate_pq_security(scheme: SignatureScheme, classical_bits: usize) -> usize {
    match scheme {
        SignatureScheme::XMSS => classical_bits,
        SignatureScheme::SPHINCS => classical_bits.saturating_mul(94) / 100,
        SignatureScheme::Dilithium => classical_bits.saturating_mul(90) / 100,
        SignatureScheme::BLS | SignatureScheme::ECDSA => classical_bits / 2,
    }
}

pub fn compute_verification_commitment(
    config: &FormalVerificationConfig,
    proof_hashes: &[[u8; 32]],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"eth2077-formal-verification-v1");
    hasher.update([proving_system_discriminant(config.proving_system)]);
    hasher.update([signature_discriminant(config.signature_scheme)]);
    hasher.update([strategy_discriminant(config.strategy)]);
    hasher.update((config.security_bits as u64).to_be_bytes());
    hasher.update((config.max_aggregation_depth as u64).to_be_bytes());
    hasher.update((config.field_size_bits as u64).to_be_bytes());
    hasher.update([u8::from(config.post_quantum)]);
    hasher.update((config.max_proof_size_bytes as u64).to_be_bytes());
    hasher.update((proof_hashes.len() as u64).to_be_bytes());

    for hash in proof_hashes {
        hasher.update(hash);
    }

    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn has_incompatible_strategy(config: &FormalVerificationConfig) -> bool {
    let aggregation_lane = matches!(
        config.strategy,
        VerificationStrategy::AggregatedProof
            | VerificationStrategy::RecursiveComposition
            | VerificationStrategy::IncrementalVerification
    );
    if aggregation_lane && config.max_aggregation_depth < 2 {
        return true;
    }

    if config.strategy == VerificationStrategy::RecursiveComposition
        && config.proving_system == ProvingSystem::Groth16
    {
        return true;
    }

    config.post_quantum && !signature_is_post_quantum(config.signature_scheme)
}

fn signature_is_post_quantum(scheme: SignatureScheme) -> bool {
    matches!(
        scheme,
        SignatureScheme::XMSS | SignatureScheme::SPHINCS | SignatureScheme::Dilithium
    )
}

fn all_proving_systems() -> [ProvingSystem; 6] {
    [
        ProvingSystem::WHIR,
        ProvingSystem::SuperSpartan,
        ProvingSystem::Groth16,
        ProvingSystem::Plonk,
        ProvingSystem::Halo2,
        ProvingSystem::STARKs,
    ]
}

fn proving_system_name(system: ProvingSystem) -> &'static str {
    match system {
        ProvingSystem::WHIR => "WHIR",
        ProvingSystem::SuperSpartan => "SuperSpartan",
        ProvingSystem::Groth16 => "Groth16",
        ProvingSystem::Plonk => "Plonk",
        ProvingSystem::Halo2 => "Halo2",
        ProvingSystem::STARKs => "STARKs",
    }
}

fn base_proof_size_bytes(system: ProvingSystem) -> usize {
    match system {
        ProvingSystem::WHIR => 68_000,
        ProvingSystem::SuperSpartan => 96_000,
        ProvingSystem::Groth16 => 1_024,
        ProvingSystem::Plonk => 28_000,
        ProvingSystem::Halo2 => 48_000,
        ProvingSystem::STARKs => 140_000,
    }
}

fn base_verification_time_ms(system: ProvingSystem) -> f64 {
    match system {
        ProvingSystem::WHIR => 34.0,
        ProvingSystem::SuperSpartan => 41.0,
        ProvingSystem::Groth16 => 8.0,
        ProvingSystem::Plonk => 20.0,
        ProvingSystem::Halo2 => 26.0,
        ProvingSystem::STARKs => 52.0,
    }
}

fn signature_multipliers(scheme: SignatureScheme) -> (f64, f64) {
    match scheme {
        SignatureScheme::XMSS => (1.0, 1.0),
        SignatureScheme::SPHINCS => (1.28, 1.2),
        SignatureScheme::Dilithium => (0.94, 0.92),
        SignatureScheme::BLS => (0.72, 0.75),
        SignatureScheme::ECDSA => (0.78, 0.8),
    }
}

fn strategy_multipliers(strategy: VerificationStrategy) -> (f64, f64) {
    match strategy {
        VerificationStrategy::DirectVerify => (1.0, 1.0),
        VerificationStrategy::AggregatedProof => (0.78, 0.72),
        VerificationStrategy::RecursiveComposition => (1.15, 0.6),
        VerificationStrategy::BatchVerification => (0.88, 0.74),
        VerificationStrategy::IncrementalVerification => (0.96, 0.84),
    }
}

fn estimate_recursion_overhead(
    proving_system: ProvingSystem,
    strategy: VerificationStrategy,
    depth: usize,
) -> f64 {
    let depth_term = (depth as f64).log2().max(1.0);
    let base = match proving_system {
        ProvingSystem::WHIR => 1.06,
        ProvingSystem::SuperSpartan => 1.08,
        ProvingSystem::Groth16 => 1.24,
        ProvingSystem::Plonk => 1.12,
        ProvingSystem::Halo2 => 1.1,
        ProvingSystem::STARKs => 1.16,
    };

    match strategy {
        VerificationStrategy::RecursiveComposition => base + depth_term * 0.08,
        VerificationStrategy::IncrementalVerification => {
            1.0 + (base - 1.0) * 0.6 + depth_term * 0.03
        }
        _ => 1.0,
    }
}

fn proving_system_discriminant(system: ProvingSystem) -> u8 {
    match system {
        ProvingSystem::WHIR => 0,
        ProvingSystem::SuperSpartan => 1,
        ProvingSystem::Groth16 => 2,
        ProvingSystem::Plonk => 3,
        ProvingSystem::Halo2 => 4,
        ProvingSystem::STARKs => 5,
    }
}

fn signature_discriminant(scheme: SignatureScheme) -> u8 {
    match scheme {
        SignatureScheme::XMSS => 0,
        SignatureScheme::SPHINCS => 1,
        SignatureScheme::Dilithium => 2,
        SignatureScheme::BLS => 3,
        SignatureScheme::ECDSA => 4,
    }
}

fn strategy_discriminant(strategy: VerificationStrategy) -> u8 {
    match strategy {
        VerificationStrategy::DirectVerify => 0,
        VerificationStrategy::AggregatedProof => 1,
        VerificationStrategy::RecursiveComposition => 2,
        VerificationStrategy::BatchVerification => 3,
        VerificationStrategy::IncrementalVerification => 4,
    }
}
