use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::HashSet;

const MIN_SECURITY_BITS: usize = 128;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PqAlgorithm {
    MLKEM768,
    MLDSA65,
    SPHINCSSHA2_128f,
    XMSS_SHA2_20,
    FalconPadded512,
    SLHDSAShake128f,
    HybridClassicalPQ,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MigrationPhase {
    Research,
    Prototype,
    TestnetDeploy,
    MainnetOptIn,
    MainnetDefault,
    ClassicDeprecated,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CryptoComponent {
    ValidatorSignatures,
    TransactionSignatures,
    AttestationAggregation,
    BeaconBlockSigning,
    DepositContract,
    WithdrawalCredentials,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PqMigrationEntry {
    pub component: CryptoComponent,
    pub current_algorithm: String,
    pub target_algorithm: PqAlgorithm,
    pub phase: MigrationPhase,
    pub signature_size_bytes: usize,
    pub verification_time_us: f64,
    pub security_bits: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PostQuantumConfig {
    pub entries: Vec<PqMigrationEntry>,
    pub target_phase: MigrationPhase,
    pub max_signature_overhead_pct: f64,
    pub max_verification_overhead_pct: f64,
    pub require_hybrid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PqValidationError {
    EmptyEntries,
    DuplicateComponent,
    OverheadExceedsLimit {
        component: String,
        overhead_pct: f64,
    },
    InsufficientSecurityBits {
        component: String,
        bits: usize,
        min: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PostQuantumStats {
    pub components_assessed: usize,
    pub on_track: usize,
    pub behind_schedule: usize,
    pub avg_signature_overhead_pct: f64,
    pub avg_verification_overhead_pct: f64,
    pub weakest_component: String,
    pub overall_phase: MigrationPhase,
    pub migration_readiness: f64,
}

pub fn default_post_quantum_config() -> PostQuantumConfig {
    PostQuantumConfig {
        entries: vec![
            PqMigrationEntry {
                component: CryptoComponent::ValidatorSignatures,
                current_algorithm: "BLS12-381".to_string(),
                target_algorithm: PqAlgorithm::HybridClassicalPQ,
                phase: MigrationPhase::TestnetDeploy,
                signature_size_bytes: 800,
                verification_time_us: 145.0,
                security_bits: 192,
            },
            PqMigrationEntry {
                component: CryptoComponent::TransactionSignatures,
                current_algorithm: "ECDSA-secp256k1".to_string(),
                target_algorithm: PqAlgorithm::FalconPadded512,
                phase: MigrationPhase::Prototype,
                signature_size_bytes: 666,
                verification_time_us: 95.0,
                security_bits: 128,
            },
            PqMigrationEntry {
                component: CryptoComponent::AttestationAggregation,
                current_algorithm: "BLS12-381".to_string(),
                target_algorithm: PqAlgorithm::MLDSA65,
                phase: MigrationPhase::Prototype,
                signature_size_bytes: 3309,
                verification_time_us: 240.0,
                security_bits: 192,
            },
            PqMigrationEntry {
                component: CryptoComponent::BeaconBlockSigning,
                current_algorithm: "BLS12-381".to_string(),
                target_algorithm: PqAlgorithm::HybridClassicalPQ,
                phase: MigrationPhase::TestnetDeploy,
                signature_size_bytes: 800,
                verification_time_us: 130.0,
                security_bits: 192,
            },
            PqMigrationEntry {
                component: CryptoComponent::DepositContract,
                current_algorithm: "ECDSA-secp256k1".to_string(),
                target_algorithm: PqAlgorithm::SLHDSAShake128f,
                phase: MigrationPhase::Research,
                signature_size_bytes: 7856,
                verification_time_us: 390.0,
                security_bits: 128,
            },
            PqMigrationEntry {
                component: CryptoComponent::WithdrawalCredentials,
                current_algorithm: "BLS12-381".to_string(),
                target_algorithm: PqAlgorithm::HybridClassicalPQ,
                phase: MigrationPhase::Prototype,
                signature_size_bytes: 800,
                verification_time_us: 135.0,
                security_bits: 192,
            },
        ],
        target_phase: MigrationPhase::TestnetDeploy,
        max_signature_overhead_pct: 20_000.0,
        max_verification_overhead_pct: 1_000.0,
        require_hybrid: true,
    }
}

pub fn validate_pq_config(config: &PostQuantumConfig) -> Result<(), Vec<PqValidationError>> {
    let mut errors = Vec::new();

    if config.entries.is_empty() {
        errors.push(PqValidationError::EmptyEntries);
        return Err(errors);
    }

    let mut seen = HashSet::new();
    for entry in &config.entries {
        if !seen.insert(entry.component) {
            errors.push(PqValidationError::DuplicateComponent);
        }

        let base_signature_size = component_baseline_signature_size(entry.component);
        let signature_overhead =
            estimate_signature_overhead(base_signature_size, entry.signature_size_bytes);

        let base_verification_time = component_baseline_verification_us(entry.component);
        let verification_overhead =
            estimate_overhead_pct(base_verification_time, entry.verification_time_us);
        let effective_overhead = signature_overhead.max(verification_overhead);

        if signature_overhead > config.max_signature_overhead_pct
            || verification_overhead > config.max_verification_overhead_pct
        {
            errors.push(PqValidationError::OverheadExceedsLimit {
                component: component_name(entry.component).to_string(),
                overhead_pct: effective_overhead,
            });
        }

        if entry.security_bits < MIN_SECURITY_BITS {
            errors.push(PqValidationError::InsufficientSecurityBits {
                component: component_name(entry.component).to_string(),
                bits: entry.security_bits,
                min: MIN_SECURITY_BITS,
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_pq_stats(config: &PostQuantumConfig) -> PostQuantumStats {
    let components_assessed = config.entries.len();
    if components_assessed == 0 {
        return PostQuantumStats {
            components_assessed: 0,
            on_track: 0,
            behind_schedule: 0,
            avg_signature_overhead_pct: 0.0,
            avg_verification_overhead_pct: 0.0,
            weakest_component: String::new(),
            overall_phase: MigrationPhase::Research,
            migration_readiness: 0.0,
        };
    }

    let target_idx = phase_index(config.target_phase);
    let mut on_track = 0usize;
    let mut total_signature_overhead = 0.0;
    let mut total_verification_overhead = 0.0;
    let mut weakest: Option<(&PqMigrationEntry, usize)> = None;
    let mut overall_phase = MigrationPhase::ClassicDeprecated;
    let mut phase_progress_sum = 0.0;

    for entry in &config.entries {
        let entry_phase_idx = phase_index(entry.phase);
        if entry_phase_idx >= target_idx {
            on_track += 1;
        }
        if entry_phase_idx < phase_index(overall_phase) {
            overall_phase = entry.phase;
        }

        total_signature_overhead += estimate_signature_overhead(
            component_baseline_signature_size(entry.component),
            entry.signature_size_bytes,
        );
        total_verification_overhead += estimate_overhead_pct(
            component_baseline_verification_us(entry.component),
            entry.verification_time_us,
        );
        phase_progress_sum +=
            entry_phase_idx as f64 / phase_index(MigrationPhase::ClassicDeprecated) as f64;

        match weakest {
            None => weakest = Some((entry, entry.security_bits)),
            Some((_, bits)) if entry.security_bits < bits => {
                weakest = Some((entry, entry.security_bits))
            }
            _ => {}
        }
    }

    let behind_schedule = components_assessed.saturating_sub(on_track);
    let avg_signature_overhead_pct = total_signature_overhead / components_assessed as f64;
    let avg_verification_overhead_pct = total_verification_overhead / components_assessed as f64;
    let weakest_component = weakest
        .map(|(entry, _)| component_name(entry.component).to_string())
        .unwrap_or_default();
    let weakest_bits = weakest.map(|(_, bits)| bits).unwrap_or(0);

    let phase_progress = phase_progress_sum / components_assessed as f64;
    let security_ratio = (weakest_bits as f64 / 256.0).clamp(0.0, 1.0);
    let signature_headroom =
        1.0 - (avg_signature_overhead_pct / config.max_signature_overhead_pct.max(1.0));
    let verification_headroom =
        1.0 - (avg_verification_overhead_pct / config.max_verification_overhead_pct.max(1.0));
    let headroom = (0.5 * signature_headroom + 0.5 * verification_headroom).clamp(0.0, 1.0);
    let migration_readiness =
        (0.5 * phase_progress + 0.3 * security_ratio + 0.2 * headroom).clamp(0.0, 1.0);

    PostQuantumStats {
        components_assessed,
        on_track,
        behind_schedule,
        avg_signature_overhead_pct,
        avg_verification_overhead_pct,
        weakest_component,
        overall_phase,
        migration_readiness,
    }
}

pub fn compare_algorithms(component: CryptoComponent) -> Vec<(String, usize, f64)> {
    let (classical, classical_sig, classical_verify) = component_baseline_profile(component);

    vec![
        (classical.to_string(), classical_sig, classical_verify),
        (
            "HybridClassicalPQ".to_string(),
            classical_sig + 704,
            classical_verify * 1.6,
        ),
        ("FalconPadded512".to_string(), 666, classical_verify * 1.3),
        ("MLDSA65".to_string(), 3309, classical_verify * 2.3),
        (
            "SPHINCSSHA2_128f".to_string(),
            17088,
            classical_verify * 11.0,
        ),
        ("SLHDSAShake128f".to_string(), 7856, classical_verify * 7.1),
        ("XMSS_SHA2_20".to_string(), 2500, classical_verify * 3.6),
        ("MLKEM768".to_string(), 1184, classical_verify * 2.0),
    ]
}

pub fn estimate_signature_overhead(current_size: usize, pq_size: usize) -> f64 {
    if current_size == 0 {
        return 0.0;
    }

    ((pq_size as f64 - current_size as f64) / current_size as f64) * 100.0
}

pub fn compute_pq_commitment(config: &PostQuantumConfig) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"eth2077-post-quantum-v1");
    hasher.update([phase_index(config.target_phase) as u8]);
    hasher.update(config.max_signature_overhead_pct.to_be_bytes());
    hasher.update(config.max_verification_overhead_pct.to_be_bytes());
    hasher.update([config.require_hybrid as u8]);

    let mut entries = config.entries.clone();
    entries.sort_by(|a, b| {
        component_index(a.component)
            .cmp(&component_index(b.component))
            .then_with(|| {
                algorithm_index(a.target_algorithm).cmp(&algorithm_index(b.target_algorithm))
            })
            .then_with(|| phase_index(a.phase).cmp(&phase_index(b.phase)))
            .then_with(|| a.current_algorithm.cmp(&b.current_algorithm))
            .then_with(|| a.signature_size_bytes.cmp(&b.signature_size_bytes))
            .then_with(|| {
                a.verification_time_us
                    .partial_cmp(&b.verification_time_us)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| a.security_bits.cmp(&b.security_bits))
    });

    hasher.update((entries.len() as u64).to_be_bytes());
    for entry in entries {
        hasher.update([component_index(entry.component) as u8]);
        hasher.update([algorithm_index(entry.target_algorithm) as u8]);
        hasher.update([phase_index(entry.phase) as u8]);
        hasher.update((entry.signature_size_bytes as u64).to_be_bytes());
        hasher.update(entry.verification_time_us.to_be_bytes());
        hasher.update((entry.security_bits as u64).to_be_bytes());
        hasher.update((entry.current_algorithm.len() as u64).to_be_bytes());
        hasher.update(entry.current_algorithm.as_bytes());
    }

    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn estimate_overhead_pct(current: f64, candidate: f64) -> f64 {
    if current <= 0.0 {
        return 0.0;
    }
    ((candidate - current) / current) * 100.0
}

fn component_baseline_profile(component: CryptoComponent) -> (&'static str, usize, f64) {
    match component {
        CryptoComponent::ValidatorSignatures => ("BLS12-381", 96, 80.0),
        CryptoComponent::TransactionSignatures => ("ECDSA-secp256k1", 65, 55.0),
        CryptoComponent::AttestationAggregation => ("BLS12-381", 96, 90.0),
        CryptoComponent::BeaconBlockSigning => ("BLS12-381", 96, 75.0),
        CryptoComponent::DepositContract => ("ECDSA-secp256k1", 65, 50.0),
        CryptoComponent::WithdrawalCredentials => ("BLS12-381", 96, 70.0),
    }
}

fn component_baseline_signature_size(component: CryptoComponent) -> usize {
    component_baseline_profile(component).1
}

fn component_baseline_verification_us(component: CryptoComponent) -> f64 {
    component_baseline_profile(component).2
}

fn phase_index(phase: MigrationPhase) -> usize {
    match phase {
        MigrationPhase::Research => 0,
        MigrationPhase::Prototype => 1,
        MigrationPhase::TestnetDeploy => 2,
        MigrationPhase::MainnetOptIn => 3,
        MigrationPhase::MainnetDefault => 4,
        MigrationPhase::ClassicDeprecated => 5,
    }
}

fn component_index(component: CryptoComponent) -> usize {
    match component {
        CryptoComponent::ValidatorSignatures => 0,
        CryptoComponent::TransactionSignatures => 1,
        CryptoComponent::AttestationAggregation => 2,
        CryptoComponent::BeaconBlockSigning => 3,
        CryptoComponent::DepositContract => 4,
        CryptoComponent::WithdrawalCredentials => 5,
    }
}

fn algorithm_index(algorithm: PqAlgorithm) -> usize {
    match algorithm {
        PqAlgorithm::MLKEM768 => 0,
        PqAlgorithm::MLDSA65 => 1,
        PqAlgorithm::SPHINCSSHA2_128f => 2,
        PqAlgorithm::XMSS_SHA2_20 => 3,
        PqAlgorithm::FalconPadded512 => 4,
        PqAlgorithm::SLHDSAShake128f => 5,
        PqAlgorithm::HybridClassicalPQ => 6,
    }
}

fn component_name(component: CryptoComponent) -> &'static str {
    match component {
        CryptoComponent::ValidatorSignatures => "ValidatorSignatures",
        CryptoComponent::TransactionSignatures => "TransactionSignatures",
        CryptoComponent::AttestationAggregation => "AttestationAggregation",
        CryptoComponent::BeaconBlockSigning => "BeaconBlockSigning",
        CryptoComponent::DepositContract => "DepositContract",
        CryptoComponent::WithdrawalCredentials => "WithdrawalCredentials",
    }
}
