use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const REWARD_SUM_TARGET: f64 = 1.0;
const REWARD_SUM_EPSILON: f64 = 1e-6;
const DEFAULT_MAX_DELAY_SLOTS: u64 = 8;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SeparationModel {
    // No separation (status quo)
    CurrentUnified,
    // Attesters prefer but don't enforce inclusion
    SoftSeparation,
    // Strict role split
    HardSeparation,
    // Rotating committee of includers
    CommitteeIncluder,
    // Includer role auctioned (like PBS)
    AuctionedIncluder,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CensorshipResistance {
    None,
    // Fork-choice enforced inclusion lists
    FocilStyle,
    // Encrypted mempool
    ThresholdEncryption,
    // Protocol-level forced inclusion after N slots
    ForcedInclusion,
    // Multiple proposers per slot
    MultiProposer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AISConfig {
    pub separation_model: SeparationModel,
    pub censorship_resistance: CensorshipResistance,
    pub attester_count: usize,
    pub includer_count: usize,
    pub inclusion_delay_slots: u64,
    pub max_inclusion_list_size: usize,
    // fraction of total reward
    pub attester_reward_share: f64,
    // fraction of total reward
    pub includer_reward_share: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AISValidationError {
    ZeroAttesters,
    ZeroIncluders,
    RewardShareMismatch { total: String },
    InclusionListTooLarge { size: usize, max: usize },
    IncompatibleModel { model: String, resistance: String },
    DelayTooHigh { slots: u64, max_slots: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AISImpactStats {
    pub censorship_resistance_score: f64,
    pub mev_extraction_difficulty: f64,
    pub attestation_overhead_ratio: f64,
    pub inclusion_latency_slots: f64,
    pub centralization_risk: f64,
    pub model_comparison: Vec<(String, f64)>,
}

pub fn default_ais_config() -> AISConfig {
    AISConfig {
        separation_model: SeparationModel::HardSeparation,
        censorship_resistance: CensorshipResistance::FocilStyle,
        attester_count: 128,
        includer_count: 16,
        inclusion_delay_slots: 1,
        max_inclusion_list_size: 256,
        attester_reward_share: 0.7,
        includer_reward_share: 0.3,
    }
}

pub fn validate_ais_config(config: &AISConfig) -> Result<(), Vec<AISValidationError>> {
    let mut errors = Vec::new();

    if config.attester_count == 0 {
        errors.push(AISValidationError::ZeroAttesters);
    }

    if config.includer_count == 0 {
        errors.push(AISValidationError::ZeroIncluders);
    }

    let reward_total = config.attester_reward_share + config.includer_reward_share;
    let valid_reward_bounds = config.attester_reward_share.is_finite()
        && config.includer_reward_share.is_finite()
        && config.attester_reward_share >= 0.0
        && config.includer_reward_share >= 0.0
        && config.attester_reward_share <= 1.0
        && config.includer_reward_share <= 1.0;

    if !valid_reward_bounds || (reward_total - REWARD_SUM_TARGET).abs() > REWARD_SUM_EPSILON {
        errors.push(AISValidationError::RewardShareMismatch {
            total: format!("{reward_total:.6}"),
        });
    }

    let max_inclusion_limit = max_inclusion_list_limit(config.censorship_resistance);
    if config.max_inclusion_list_size > max_inclusion_limit {
        errors.push(AISValidationError::InclusionListTooLarge {
            size: config.max_inclusion_list_size,
            max: max_inclusion_limit,
        });
    }

    let max_delay = max_delay_for_config(config);
    if config.inclusion_delay_slots > max_delay {
        errors.push(AISValidationError::DelayTooHigh {
            slots: config.inclusion_delay_slots,
            max_slots: max_delay,
        });
    }

    if !is_model_resistance_compatible(config.separation_model, config.censorship_resistance) {
        errors.push(AISValidationError::IncompatibleModel {
            model: format!("{:?}", config.separation_model),
            resistance: format!("{:?}", config.censorship_resistance),
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_ais_stats(config: &AISConfig) -> AISImpactStats {
    let censorship_resistance_score = estimate_censorship_resistance(config);
    let mev_extraction_difficulty = estimate_mev_difficulty(config);

    let overhead_baseline = match config.separation_model {
        SeparationModel::CurrentUnified => 0.98,
        SeparationModel::SoftSeparation => 1.04,
        SeparationModel::HardSeparation => 1.12,
        SeparationModel::CommitteeIncluder => 1.18,
        SeparationModel::AuctionedIncluder => 1.10,
    };

    let list_complexity = (config.max_inclusion_list_size as f64 / 1024.0).min(1.0) * 0.18;
    let delay_complexity = (config.inclusion_delay_slots as f64) * 0.02;
    let attestation_overhead_ratio =
        clamp_positive(overhead_baseline + list_complexity + delay_complexity);

    let resistance_latency = match config.censorship_resistance {
        CensorshipResistance::None => 0.0,
        CensorshipResistance::FocilStyle => 0.2,
        CensorshipResistance::ThresholdEncryption => 0.6,
        CensorshipResistance::ForcedInclusion => 0.4,
        CensorshipResistance::MultiProposer => -0.2,
    };

    let model_latency = match config.separation_model {
        SeparationModel::CurrentUnified => -0.1,
        SeparationModel::SoftSeparation => 0.0,
        SeparationModel::HardSeparation => 0.2,
        SeparationModel::CommitteeIncluder => 0.3,
        SeparationModel::AuctionedIncluder => 0.4,
    };

    let inclusion_latency_slots =
        clamp_positive(config.inclusion_delay_slots as f64 + resistance_latency + model_latency);

    let base_centralization_risk = match config.separation_model {
        SeparationModel::CurrentUnified => 0.55,
        SeparationModel::SoftSeparation => 0.50,
        SeparationModel::HardSeparation => 0.45,
        SeparationModel::CommitteeIncluder => 0.40,
        SeparationModel::AuctionedIncluder => 0.65,
    };

    let includer_concentration = (32.0 / config.includer_count.max(1) as f64).min(1.5);
    let attester_concentration = (128.0 / config.attester_count.max(1) as f64).min(1.5);

    let resistance_adjustment = match config.censorship_resistance {
        CensorshipResistance::None => 0.08,
        CensorshipResistance::FocilStyle => -0.08,
        CensorshipResistance::ThresholdEncryption => -0.04,
        CensorshipResistance::ForcedInclusion => -0.06,
        CensorshipResistance::MultiProposer => -0.10,
    };

    let reward_skew_penalty = (config.includer_reward_share - 0.30).max(0.0) * 0.50
        + (0.70 - config.attester_reward_share).max(0.0) * 0.30;

    let centralization_risk = clamp01(
        base_centralization_risk
            + 0.25 * includer_concentration
            + 0.10 * attester_concentration
            + resistance_adjustment
            + reward_skew_penalty,
    );

    let model_comparison = compare_separation_models(config);

    AISImpactStats {
        censorship_resistance_score,
        mev_extraction_difficulty,
        attestation_overhead_ratio,
        inclusion_latency_slots,
        centralization_risk,
        model_comparison,
    }
}

pub fn estimate_censorship_resistance(config: &AISConfig) -> f64 {
    let model_scores: HashMap<SeparationModel, f64> = HashMap::from([
        (SeparationModel::CurrentUnified, 0.30),
        (SeparationModel::SoftSeparation, 0.45),
        (SeparationModel::HardSeparation, 0.65),
        (SeparationModel::CommitteeIncluder, 0.70),
        (SeparationModel::AuctionedIncluder, 0.60),
    ]);

    let resistance_scores: HashMap<CensorshipResistance, f64> = HashMap::from([
        (CensorshipResistance::None, 0.00),
        (CensorshipResistance::FocilStyle, 0.20),
        (CensorshipResistance::ThresholdEncryption, 0.25),
        (CensorshipResistance::ForcedInclusion, 0.22),
        (CensorshipResistance::MultiProposer, 0.18),
    ]);

    let model_base = model_scores
        .get(&config.separation_model)
        .copied()
        .unwrap_or(0.0);
    let resistance_bonus = resistance_scores
        .get(&config.censorship_resistance)
        .copied()
        .unwrap_or(0.0);

    let delay_penalty = (config.inclusion_delay_slots.saturating_sub(1) as f64 * 0.02).min(0.20);
    let decentralization_bonus =
        ((config.includer_count as f64 / config.attester_count.max(1) as f64) * 0.20).min(0.10);

    clamp01(model_base + resistance_bonus + decentralization_bonus - delay_penalty)
}

pub fn estimate_mev_difficulty(config: &AISConfig) -> f64 {
    let model_scores: HashMap<SeparationModel, f64> = HashMap::from([
        (SeparationModel::CurrentUnified, 0.20),
        (SeparationModel::SoftSeparation, 0.35),
        (SeparationModel::HardSeparation, 0.60),
        (SeparationModel::CommitteeIncluder, 0.70),
        (SeparationModel::AuctionedIncluder, 0.50),
    ]);

    let resistance_scores: HashMap<CensorshipResistance, f64> = HashMap::from([
        (CensorshipResistance::None, 0.00),
        (CensorshipResistance::FocilStyle, 0.10),
        (CensorshipResistance::ThresholdEncryption, 0.25),
        (CensorshipResistance::ForcedInclusion, 0.15),
        (CensorshipResistance::MultiProposer, 0.12),
    ]);

    let model_base = model_scores
        .get(&config.separation_model)
        .copied()
        .unwrap_or(0.0);
    let resistance_bonus = resistance_scores
        .get(&config.censorship_resistance)
        .copied()
        .unwrap_or(0.0);

    let includer_dispersion = ((config.includer_count.max(1) as f64).log2() / 8.0).min(0.20);
    let latency_bonus = (config.inclusion_delay_slots as f64 * 0.03).min(0.15);
    let reward_balance = ((config.attester_reward_share - 0.50) * 0.20).max(-0.10);

    clamp01(model_base + resistance_bonus + includer_dispersion + latency_bonus + reward_balance)
}

pub fn compare_separation_models(config: &AISConfig) -> Vec<(String, f64)> {
    let mut model_scores = Vec::new();

    let models = [
        SeparationModel::CurrentUnified,
        SeparationModel::SoftSeparation,
        SeparationModel::HardSeparation,
        SeparationModel::CommitteeIncluder,
        SeparationModel::AuctionedIncluder,
    ];

    for model in models {
        let variant_config = AISConfig {
            separation_model: model,
            ..config.clone()
        };

        let censorship = estimate_censorship_resistance(&variant_config);
        let mev = estimate_mev_difficulty(&variant_config);
        let centralization = estimate_centralization_risk_for_compare(&variant_config);
        let latency = estimate_latency_for_compare(&variant_config);

        let effectiveness = clamp01(
            censorship * 0.40
                + mev * 0.35
                + (1.0 - centralization) * 0.15
                + (1.0 - latency / 6.0).clamp(0.0, 1.0) * 0.10,
        );

        model_scores.push((format!("{:?}", model), effectiveness));
    }

    model_scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    model_scores
}

pub fn compute_ais_commitment(attesters: &[[u8; 32]], includers: &[[u8; 32]]) -> [u8; 32] {
    let mut sorted_attesters = attesters.to_vec();
    let mut sorted_includers = includers.to_vec();

    sorted_attesters.sort_unstable();
    sorted_includers.sort_unstable();

    let mut hasher = Sha256::new();
    hasher.update(b"AIS-V1");
    hasher.update((sorted_attesters.len() as u64).to_le_bytes());
    for key in sorted_attesters {
        hasher.update([0xA1]);
        hasher.update(key);
    }

    hasher.update((sorted_includers.len() as u64).to_le_bytes());
    for key in sorted_includers {
        hasher.update([0xB2]);
        hasher.update(key);
    }

    let digest = hasher.finalize();
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn max_inclusion_list_limit(resistance: CensorshipResistance) -> usize {
    match resistance {
        CensorshipResistance::None => 1024,
        CensorshipResistance::FocilStyle => 4096,
        CensorshipResistance::ThresholdEncryption => 2048,
        CensorshipResistance::ForcedInclusion => 3072,
        CensorshipResistance::MultiProposer => 4096,
    }
}

fn max_delay_for_config(config: &AISConfig) -> u64 {
    match config.censorship_resistance {
        CensorshipResistance::ThresholdEncryption => 10,
        CensorshipResistance::ForcedInclusion => 12,
        _ => DEFAULT_MAX_DELAY_SLOTS,
    }
}

fn is_model_resistance_compatible(
    model: SeparationModel,
    resistance: CensorshipResistance,
) -> bool {
    !matches!(
        (model, resistance),
        (
            SeparationModel::CurrentUnified,
            CensorshipResistance::FocilStyle
        ) | (
            SeparationModel::CurrentUnified,
            CensorshipResistance::ThresholdEncryption,
        ) | (
            SeparationModel::CurrentUnified,
            CensorshipResistance::MultiProposer,
        ) | (
            SeparationModel::SoftSeparation,
            CensorshipResistance::ThresholdEncryption,
        ) | (
            SeparationModel::AuctionedIncluder,
            CensorshipResistance::MultiProposer,
        )
    )
}

fn estimate_centralization_risk_for_compare(config: &AISConfig) -> f64 {
    let model_base = match config.separation_model {
        SeparationModel::CurrentUnified => 0.55,
        SeparationModel::SoftSeparation => 0.50,
        SeparationModel::HardSeparation => 0.45,
        SeparationModel::CommitteeIncluder => 0.40,
        SeparationModel::AuctionedIncluder => 0.65,
    };

    let includer_concentration = (32.0 / config.includer_count.max(1) as f64).min(1.5);
    let attester_concentration = (128.0 / config.attester_count.max(1) as f64).min(1.5);

    clamp01(model_base + 0.20 * includer_concentration + 0.10 * attester_concentration)
}

fn estimate_latency_for_compare(config: &AISConfig) -> f64 {
    let resistance_latency = match config.censorship_resistance {
        CensorshipResistance::None => 0.0,
        CensorshipResistance::FocilStyle => 0.2,
        CensorshipResistance::ThresholdEncryption => 0.6,
        CensorshipResistance::ForcedInclusion => 0.4,
        CensorshipResistance::MultiProposer => -0.2,
    };

    let model_latency = match config.separation_model {
        SeparationModel::CurrentUnified => -0.1,
        SeparationModel::SoftSeparation => 0.0,
        SeparationModel::HardSeparation => 0.2,
        SeparationModel::CommitteeIncluder => 0.3,
        SeparationModel::AuctionedIncluder => 0.4,
    };

    clamp_positive(config.inclusion_delay_slots as f64 + resistance_latency + model_latency)
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn clamp_positive(value: f64) -> f64 {
    if value < 0.0 {
        0.0
    } else {
        value
    }
}
