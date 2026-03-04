use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_PROOF_SIZE_BYTES: usize = 128 * 1024;
const MAX_PROVING_TIME_MS: f64 = 20_000.0;
const MIN_ANONYMITY_SET: usize = 64;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PrivacyProtocol {
    ZKShielded,
    RingSignature,
    StealthAddress,
    ConfidentialTransaction,
    MixerBased,
    FullHomomorphic,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PrivacyLevel {
    Transparent,
    SenderPrivate,
    ReceiverPrivate,
    AmountPrivate,
    FullyShielded,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ComplianceMode {
    NoCompliance,
    ViewKeyOptIn,
    RegulatoryBackdoor,
    ZKCompliance,
    SelectiveDisclosure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShieldedTransferConfig {
    pub protocol: PrivacyProtocol,
    pub privacy_level: PrivacyLevel,
    pub compliance_mode: ComplianceMode,
    pub proof_size_bytes: usize,
    pub proving_time_ms: f64,
    pub verification_time_ms: f64,
    pub max_anonymity_set: usize,
    pub supports_programmability: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ShieldedValidationError {
    ProofSizeTooLarge { size: usize, max: usize },
    ProvingTimeTooHigh { ms: f64, max: f64 },
    AnonymitySetTooSmall { size: usize, min: usize },
    IncompatibleComplianceMode { protocol: String, mode: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShieldedStats {
    pub effective_anonymity_set: usize,
    pub throughput_tps: f64,
    pub proof_overhead_pct: f64,
    pub privacy_score: f64,
    pub compliance_compatible: bool,
    pub bottleneck: String,
    pub gas_cost_estimate: u64,
}

pub fn default_shielded_config() -> ShieldedTransferConfig {
    ShieldedTransferConfig {
        protocol: PrivacyProtocol::ZKShielded,
        privacy_level: PrivacyLevel::FullyShielded,
        compliance_mode: ComplianceMode::ZKCompliance,
        proof_size_bytes: 12_288,
        proving_time_ms: 1_800.0,
        verification_time_ms: 35.0,
        max_anonymity_set: 131_072,
        supports_programmability: true,
    }
}

pub fn validate_shielded_config(
    config: &ShieldedTransferConfig,
) -> Result<(), Vec<ShieldedValidationError>> {
    let mut errors = Vec::new();

    if config.proof_size_bytes > MAX_PROOF_SIZE_BYTES {
        errors.push(ShieldedValidationError::ProofSizeTooLarge {
            size: config.proof_size_bytes,
            max: MAX_PROOF_SIZE_BYTES,
        });
    }

    if !config.proving_time_ms.is_finite() || config.proving_time_ms > MAX_PROVING_TIME_MS {
        errors.push(ShieldedValidationError::ProvingTimeTooHigh {
            ms: config.proving_time_ms,
            max: MAX_PROVING_TIME_MS,
        });
    }

    if config.max_anonymity_set < MIN_ANONYMITY_SET {
        errors.push(ShieldedValidationError::AnonymitySetTooSmall {
            size: config.max_anonymity_set,
            min: MIN_ANONYMITY_SET,
        });
    }

    if !is_compliance_mode_compatible(config.protocol, config.compliance_mode) {
        errors.push(ShieldedValidationError::IncompatibleComplianceMode {
            protocol: format!("{:?}", config.protocol),
            mode: format!("{:?}", config.compliance_mode),
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_shielded_stats(config: &ShieldedTransferConfig) -> ShieldedStats {
    let effective_anonymity_set = estimate_anonymity_set(config.protocol, config.max_anonymity_set);
    let proving_time_ms = config.proving_time_ms.max(1.0);
    let verification_time_ms = config.verification_time_ms.max(0.1);
    let protocol_parallelism = protocol_parallelism_factor(config.protocol);
    let pipeline_ms = proving_time_ms * 0.35
        + verification_time_ms * 3.5
        + (config.proof_size_bytes as f64 / 1_500.0);
    let throughput_tps = ((1_000.0 / pipeline_ms.max(1.0)) * protocol_parallelism).max(0.01);

    let proof_overhead_pct = ((config.proof_size_bytes as f64 / 220.0) * 100.0).max(0.0);
    let privacy_score = privacy_score(config, effective_anonymity_set);
    let compliance_compatible =
        is_compliance_mode_compatible(config.protocol, config.compliance_mode);
    let bottleneck = classify_bottleneck(config, effective_anonymity_set);
    let gas_cost_estimate = estimate_gas_cost(config);

    ShieldedStats {
        effective_anonymity_set,
        throughput_tps,
        proof_overhead_pct,
        privacy_score,
        compliance_compatible,
        bottleneck,
        gas_cost_estimate,
    }
}

pub fn compare_privacy_protocols(config: &ShieldedTransferConfig) -> Vec<(String, ShieldedStats)> {
    all_privacy_protocols()
        .into_iter()
        .map(|protocol| {
            let mut variant = config.clone();
            variant.protocol = protocol;
            (format!("{protocol:?}"), compute_shielded_stats(&variant))
        })
        .collect()
}

pub fn estimate_anonymity_set(protocol: PrivacyProtocol, max_set: usize) -> usize {
    if max_set == 0 {
        return 0;
    }

    let estimated = match protocol {
        PrivacyProtocol::ZKShielded => (max_set as f64 * 0.90).round() as usize,
        PrivacyProtocol::RingSignature => max_set.min(2_048),
        PrivacyProtocol::StealthAddress => (max_set as f64 * 0.18).round() as usize,
        PrivacyProtocol::ConfidentialTransaction => (max_set as f64 * 0.30).round() as usize,
        PrivacyProtocol::MixerBased => (max_set as f64 * 0.72).round() as usize,
        PrivacyProtocol::FullHomomorphic => (max_set as f64 * 0.96).round() as usize,
    };

    estimated.clamp(1, max_set)
}

pub fn compute_shielded_commitment(config: &ShieldedTransferConfig) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ETH2077-SHIELDED-V1");
    hasher.update([privacy_protocol_discriminant(config.protocol)]);
    hasher.update([privacy_level_discriminant(config.privacy_level)]);
    hasher.update([compliance_mode_discriminant(config.compliance_mode)]);
    hasher.update((config.proof_size_bytes as u64).to_be_bytes());
    hasher.update(config.proving_time_ms.to_bits().to_be_bytes());
    hasher.update(config.verification_time_ms.to_bits().to_be_bytes());
    hasher.update((config.max_anonymity_set as u64).to_be_bytes());
    hasher.update([if config.supports_programmability {
        1
    } else {
        0
    }]);

    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn all_privacy_protocols() -> [PrivacyProtocol; 6] {
    [
        PrivacyProtocol::ZKShielded,
        PrivacyProtocol::RingSignature,
        PrivacyProtocol::StealthAddress,
        PrivacyProtocol::ConfidentialTransaction,
        PrivacyProtocol::MixerBased,
        PrivacyProtocol::FullHomomorphic,
    ]
}

fn is_compliance_mode_compatible(protocol: PrivacyProtocol, mode: ComplianceMode) -> bool {
    match mode {
        ComplianceMode::NoCompliance => true,
        ComplianceMode::ViewKeyOptIn => !matches!(protocol, PrivacyProtocol::RingSignature),
        ComplianceMode::RegulatoryBackdoor => !matches!(
            protocol,
            PrivacyProtocol::MixerBased | PrivacyProtocol::FullHomomorphic
        ),
        ComplianceMode::ZKCompliance => matches!(
            protocol,
            PrivacyProtocol::ZKShielded
                | PrivacyProtocol::ConfidentialTransaction
                | PrivacyProtocol::FullHomomorphic
        ),
        ComplianceMode::SelectiveDisclosure => !matches!(protocol, PrivacyProtocol::MixerBased),
    }
}

fn protocol_parallelism_factor(protocol: PrivacyProtocol) -> f64 {
    match protocol {
        PrivacyProtocol::ZKShielded => 1.55,
        PrivacyProtocol::RingSignature => 1.40,
        PrivacyProtocol::StealthAddress => 2.10,
        PrivacyProtocol::ConfidentialTransaction => 1.65,
        PrivacyProtocol::MixerBased => 1.80,
        PrivacyProtocol::FullHomomorphic => 0.82,
    }
}

fn privacy_score(config: &ShieldedTransferConfig, effective_anonymity_set: usize) -> f64 {
    let base = match config.privacy_level {
        PrivacyLevel::Transparent => 20.0,
        PrivacyLevel::SenderPrivate => 45.0,
        PrivacyLevel::ReceiverPrivate => 50.0,
        PrivacyLevel::AmountPrivate => 70.0,
        PrivacyLevel::FullyShielded => 88.0,
    };

    let protocol_bonus = match config.protocol {
        PrivacyProtocol::ZKShielded => 7.0,
        PrivacyProtocol::RingSignature => 5.0,
        PrivacyProtocol::StealthAddress => 3.0,
        PrivacyProtocol::ConfidentialTransaction => 6.0,
        PrivacyProtocol::MixerBased => 4.0,
        PrivacyProtocol::FullHomomorphic => 9.0,
    };

    let anonymity_bonus =
        ((effective_anonymity_set.max(1) as f64).log2() / 16.0).clamp(0.0, 1.0) * 12.0;
    let programmability_bonus = if config.supports_programmability {
        2.0
    } else {
        0.0
    };
    let compliance_penalty = match config.compliance_mode {
        ComplianceMode::NoCompliance => 0.0,
        ComplianceMode::ViewKeyOptIn => 2.0,
        ComplianceMode::RegulatoryBackdoor => 12.0,
        ComplianceMode::ZKCompliance => 3.0,
        ComplianceMode::SelectiveDisclosure => 6.0,
    };

    (base + protocol_bonus + anonymity_bonus + programmability_bonus - compliance_penalty)
        .clamp(0.0, 100.0)
}

fn classify_bottleneck(config: &ShieldedTransferConfig, effective_anonymity_set: usize) -> String {
    let proof_pressure = config.proof_size_bytes as f64 / 48_000.0;
    let proving_pressure = config.proving_time_ms.max(0.0) / 4_000.0;
    let verification_pressure = config.verification_time_ms.max(0.0) / 110.0;
    let anonymity_pressure = if effective_anonymity_set == 0 {
        2.0
    } else {
        (MIN_ANONYMITY_SET as f64 / effective_anonymity_set as f64).clamp(0.0, 1.5)
    };

    let mut max_name = "ProvingTime";
    let mut max_value = proving_pressure;
    for (name, value) in [
        ("ProofSize", proof_pressure),
        ("VerificationTime", verification_pressure),
        ("AnonymitySet", anonymity_pressure),
    ] {
        if value > max_value {
            max_name = name;
            max_value = value;
        }
    }

    max_name.to_string()
}

fn estimate_gas_cost(config: &ShieldedTransferConfig) -> u64 {
    let base_cost = 21_000.0
        + config.proof_size_bytes as f64 * 5.5
        + config.verification_time_ms.max(0.0) * 260.0
        + config.proving_time_ms.max(0.0) * 9.0;

    let protocol_multiplier = match config.protocol {
        PrivacyProtocol::ZKShielded => 2.10,
        PrivacyProtocol::RingSignature => 1.70,
        PrivacyProtocol::StealthAddress => 1.25,
        PrivacyProtocol::ConfidentialTransaction => 1.85,
        PrivacyProtocol::MixerBased => 1.55,
        PrivacyProtocol::FullHomomorphic => 3.40,
    };

    let compliance_multiplier = match config.compliance_mode {
        ComplianceMode::NoCompliance => 1.0,
        ComplianceMode::ViewKeyOptIn => 1.04,
        ComplianceMode::RegulatoryBackdoor => 1.08,
        ComplianceMode::ZKCompliance => 1.13,
        ComplianceMode::SelectiveDisclosure => 1.06,
    };

    (base_cost * protocol_multiplier * compliance_multiplier)
        .clamp(0.0, u64::MAX as f64)
        .round() as u64
}

fn privacy_protocol_discriminant(protocol: PrivacyProtocol) -> u8 {
    match protocol {
        PrivacyProtocol::ZKShielded => 0,
        PrivacyProtocol::RingSignature => 1,
        PrivacyProtocol::StealthAddress => 2,
        PrivacyProtocol::ConfidentialTransaction => 3,
        PrivacyProtocol::MixerBased => 4,
        PrivacyProtocol::FullHomomorphic => 5,
    }
}

fn privacy_level_discriminant(level: PrivacyLevel) -> u8 {
    match level {
        PrivacyLevel::Transparent => 0,
        PrivacyLevel::SenderPrivate => 1,
        PrivacyLevel::ReceiverPrivate => 2,
        PrivacyLevel::AmountPrivate => 3,
        PrivacyLevel::FullyShielded => 4,
    }
}

fn compliance_mode_discriminant(mode: ComplianceMode) -> u8 {
    match mode {
        ComplianceMode::NoCompliance => 0,
        ComplianceMode::ViewKeyOptIn => 1,
        ComplianceMode::RegulatoryBackdoor => 2,
        ComplianceMode::ZKCompliance => 3,
        ComplianceMode::SelectiveDisclosure => 4,
    }
}
