use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SLOT_TIME_MS: f64 = 12_000.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FinalityMode {
    CasperFFG,
    SingleSlotFinality,
    OneRoundFinality,
    OptimisticFinality,
    PipelinedFinality,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SafetyProperty {
    AccountableSafety,
    ByzantineFaultTolerance,
    OptimisticSafety,
    EconomicSafety,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OneRoundFinalityConfig {
    pub mode: FinalityMode,
    pub validator_count: usize,
    pub quorum_threshold: f64,
    pub message_complexity_bound: usize,
    pub expected_finality_slots: f64,
    pub safety_property: SafetyProperty,
    pub max_tolerable_byzantine_fraction: f64,
    pub signature_aggregation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FinalityValidationError {
    InsufficientValidators,
    QuorumTooLow { value: f64 },
    QuorumTooHigh { value: f64 },
    ByzantineFractionInvalid { value: f64 },
    FinalitySlotsTooHigh { value: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FinalityImpactStats {
    pub finality_time_ms: f64,
    pub message_count: usize,
    pub bandwidth_per_validator_bytes: usize,
    pub safety_margin: f64,
    pub liveness_probability: f64,
    pub validator_overhead_factor: f64,
}

pub fn default_one_round_finality_config() -> OneRoundFinalityConfig {
    OneRoundFinalityConfig {
        mode: FinalityMode::OneRoundFinality,
        validator_count: 8_192,
        quorum_threshold: 0.67,
        message_complexity_bound: 200_000,
        expected_finality_slots: 1.0,
        safety_property: SafetyProperty::AccountableSafety,
        max_tolerable_byzantine_fraction: 0.33,
        signature_aggregation: true,
    }
}

pub fn validate_finality_config(
    config: &OneRoundFinalityConfig,
) -> Result<(), Vec<FinalityValidationError>> {
    let mut errors = Vec::new();

    if config.validator_count < min_validators_for_mode(config.mode) {
        errors.push(FinalityValidationError::InsufficientValidators);
    }

    if !config.quorum_threshold.is_finite() || config.quorum_threshold < 0.5 {
        errors.push(FinalityValidationError::QuorumTooLow {
            value: config.quorum_threshold,
        });
    }

    if config.quorum_threshold > 1.0 {
        errors.push(FinalityValidationError::QuorumTooHigh {
            value: config.quorum_threshold,
        });
    }

    if !config.max_tolerable_byzantine_fraction.is_finite()
        || !(0.0..0.5).contains(&config.max_tolerable_byzantine_fraction)
    {
        errors.push(FinalityValidationError::ByzantineFractionInvalid {
            value: config.max_tolerable_byzantine_fraction,
        });
    }

    if !config.expected_finality_slots.is_finite()
        || config.expected_finality_slots > max_expected_slots_for_mode(config.mode)
    {
        errors.push(FinalityValidationError::FinalitySlotsTooHigh {
            value: config.expected_finality_slots,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_finality_stats(config: &OneRoundFinalityConfig) -> FinalityImpactStats {
    let validator_count = config.validator_count.max(1);
    let quorum_size =
        ((validator_count as f64) * config.quorum_threshold.clamp(0.0, 1.0)).ceil() as usize;

    let base_messages = match config.mode {
        FinalityMode::CasperFFG => quorum_size.saturating_mul(2),
        FinalityMode::SingleSlotFinality => validator_count.saturating_add(quorum_size),
        FinalityMode::OneRoundFinality => quorum_size
            .saturating_mul(2)
            .saturating_add(validator_count.saturating_mul(validator_count.saturating_sub(1)) / 16),
        FinalityMode::OptimisticFinality => validator_count.saturating_add(quorum_size / 2),
        FinalityMode::PipelinedFinality => validator_count
            .saturating_mul(2)
            .saturating_add(quorum_size),
    };

    let message_multiplier = if config.signature_aggregation {
        0.35
    } else {
        1.0
    };
    let adjusted_messages = (base_messages as f64 * message_multiplier).round() as usize;
    let message_count = adjusted_messages.min(config.message_complexity_bound.max(1));

    let per_validator_messages =
        (message_count as f64 / validator_count as f64).max(if config.signature_aggregation {
            1.0
        } else {
            1.5
        });
    let bytes_per_message = if config.signature_aggregation {
        112.0
    } else {
        176.0
    };
    let bandwidth_per_validator_bytes =
        (per_validator_messages * bytes_per_message * 1.25).round() as usize;

    let safety_adjustment = match config.safety_property {
        SafetyProperty::AccountableSafety => 0.05,
        SafetyProperty::ByzantineFaultTolerance => 0.03,
        SafetyProperty::OptimisticSafety => -0.08,
        SafetyProperty::EconomicSafety => -0.03,
    };
    let safety_margin = (estimate_safety_margin(
        config.quorum_threshold,
        config.max_tolerable_byzantine_fraction,
    ) + safety_adjustment)
        .clamp(-1.0, 1.0);

    let liveness_mode_adjustment = match config.mode {
        FinalityMode::CasperFFG => 0.03,
        FinalityMode::SingleSlotFinality => -0.04,
        FinalityMode::OneRoundFinality => -0.08,
        FinalityMode::OptimisticFinality => 0.05,
        FinalityMode::PipelinedFinality => -0.02,
    };
    let liveness_probability =
        (estimate_liveness(validator_count, config.max_tolerable_byzantine_fraction)
            + liveness_mode_adjustment)
            .clamp(0.0, 1.0);

    let mode_time_floor = baseline_slots_for_mode(config.mode);
    let finality_time_ms =
        config.expected_finality_slots.max(mode_time_floor).max(0.0) * SLOT_TIME_MS;

    let mode_overhead_multiplier = match config.mode {
        FinalityMode::CasperFFG => 0.95,
        FinalityMode::SingleSlotFinality => 1.20,
        FinalityMode::OneRoundFinality => 1.60,
        FinalityMode::OptimisticFinality => 1.05,
        FinalityMode::PipelinedFinality => 1.35,
    };
    let crypto_cost_multiplier = if config.signature_aggregation {
        0.75
    } else {
        1.10
    };
    let validator_overhead_factor = ((message_count as f64 / validator_count as f64) / 2.0)
        * mode_overhead_multiplier
        * crypto_cost_multiplier;

    FinalityImpactStats {
        finality_time_ms,
        message_count,
        bandwidth_per_validator_bytes,
        safety_margin,
        liveness_probability,
        validator_overhead_factor,
    }
}

pub fn compare_finality_modes(
    config: &OneRoundFinalityConfig,
) -> Vec<(String, FinalityImpactStats)> {
    let modes = [
        FinalityMode::CasperFFG,
        FinalityMode::SingleSlotFinality,
        FinalityMode::OneRoundFinality,
        FinalityMode::OptimisticFinality,
        FinalityMode::PipelinedFinality,
    ];

    let mut comparison = Vec::with_capacity(modes.len());
    for mode in modes {
        let mut variant = config.clone();
        variant.mode = mode;
        if variant.expected_finality_slots < baseline_slots_for_mode(mode) {
            variant.expected_finality_slots = baseline_slots_for_mode(mode);
        }
        comparison.push((
            mode_name(mode).to_string(),
            compute_finality_stats(&variant),
        ));
    }

    comparison
}

pub fn estimate_safety_margin(quorum: f64, byzantine_fraction: f64) -> f64 {
    if !quorum.is_finite() || !byzantine_fraction.is_finite() {
        return 0.0;
    }

    (quorum.clamp(0.0, 1.0) - (2.0 * byzantine_fraction.clamp(0.0, 1.0))).clamp(-1.0, 1.0)
}

pub fn estimate_liveness(validator_count: usize, byzantine_fraction: f64) -> f64 {
    if validator_count == 0 || !byzantine_fraction.is_finite() {
        return 0.0;
    }

    let active_fraction = (1.0 - byzantine_fraction).clamp(0.0, 1.0);
    let size_factor = 1.0 - 1.0 / (1.0 + (validator_count as f64).sqrt());
    let baseline = 0.45 + 0.55 * size_factor;
    (baseline * active_fraction).clamp(0.0, 1.0)
}

pub fn compute_finality_commitment(
    config: &OneRoundFinalityConfig,
    validator_pubkeys: &[[u8; 32]],
) -> [u8; 32] {
    let mut sorted = validator_pubkeys.to_vec();
    sorted.sort_unstable();

    let mut hasher = Sha256::new();
    hasher.update(b"ORF-V1");
    hasher.update([mode_to_byte(config.mode)]);
    hasher.update([safety_to_byte(config.safety_property)]);
    hasher.update((config.validator_count as u64).to_le_bytes());
    hasher.update(config.quorum_threshold.to_le_bytes());
    hasher.update((config.message_complexity_bound as u64).to_le_bytes());
    hasher.update(config.expected_finality_slots.to_le_bytes());
    hasher.update(config.max_tolerable_byzantine_fraction.to_le_bytes());
    hasher.update([if config.signature_aggregation { 1 } else { 0 }]);
    hasher.update((sorted.len() as u64).to_le_bytes());
    for key in sorted {
        hasher.update(key);
    }

    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn mode_name(mode: FinalityMode) -> &'static str {
    match mode {
        FinalityMode::CasperFFG => "CasperFFG",
        FinalityMode::SingleSlotFinality => "SingleSlotFinality",
        FinalityMode::OneRoundFinality => "OneRoundFinality",
        FinalityMode::OptimisticFinality => "OptimisticFinality",
        FinalityMode::PipelinedFinality => "PipelinedFinality",
    }
}

fn mode_to_byte(mode: FinalityMode) -> u8 {
    match mode {
        FinalityMode::CasperFFG => 0,
        FinalityMode::SingleSlotFinality => 1,
        FinalityMode::OneRoundFinality => 2,
        FinalityMode::OptimisticFinality => 3,
        FinalityMode::PipelinedFinality => 4,
    }
}

fn safety_to_byte(safety: SafetyProperty) -> u8 {
    match safety {
        SafetyProperty::AccountableSafety => 0,
        SafetyProperty::ByzantineFaultTolerance => 1,
        SafetyProperty::OptimisticSafety => 2,
        SafetyProperty::EconomicSafety => 3,
    }
}

fn min_validators_for_mode(mode: FinalityMode) -> usize {
    match mode {
        FinalityMode::CasperFFG => 64,
        FinalityMode::SingleSlotFinality => 512,
        FinalityMode::OneRoundFinality => 2_048,
        FinalityMode::OptimisticFinality => 128,
        FinalityMode::PipelinedFinality => 256,
    }
}

fn max_expected_slots_for_mode(mode: FinalityMode) -> f64 {
    match mode {
        FinalityMode::CasperFFG => 64.0,
        FinalityMode::SingleSlotFinality => 2.0,
        FinalityMode::OneRoundFinality => 1.5,
        FinalityMode::OptimisticFinality => 4.0,
        FinalityMode::PipelinedFinality => 3.0,
    }
}

fn baseline_slots_for_mode(mode: FinalityMode) -> f64 {
    match mode {
        FinalityMode::CasperFFG => 60.0,
        FinalityMode::SingleSlotFinality => 1.0,
        FinalityMode::OneRoundFinality => 1.0,
        FinalityMode::OptimisticFinality => 2.0,
        FinalityMode::PipelinedFinality => 1.5,
    }
}
