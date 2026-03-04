use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SlotDuration {
    Standard12s,
    Fast8s,
    Quick6s,
    Rapid4s,
    Ultra2s,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FinalityMode {
    // Current: 2 epochs (~12.8 min)
    EpochBased,
    // 3-slot finality protocol
    ThreeSlotFinality,
    // SSF
    SingleSlotFinality,
    // ETH2077's out-of-band Citadel consensus
    OobCitadel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlotConfig {
    pub slot_duration: SlotDuration,
    pub finality_mode: FinalityMode,
    // time budget for block propagation
    pub propagation_budget_ms: u64,
    // deadline for attestation aggregation
    pub attestation_deadline_ms: u64,
    pub max_validators_per_slot: usize,
    // expected p95 network latency
    pub network_latency_p95_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlotTradeoffAnalysis {
    pub slot_duration: SlotDuration,
    pub finality_mode: FinalityMode,
    pub inclusion_latency_avg_ms: f64,
    pub finality_latency_ms: f64,
    // probability of missed slots due to propagation
    pub missed_slot_probability: f64,
    // relative to 12s baseline
    pub throughput_multiplier: f64,
    // 0.0 to 1.0
    pub security_margin: f64,
    // multiplier on bandwidth requirements
    pub bandwidth_overhead_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum QuickSlotValidationError {
    PropagationExceedsSlot { propagation_ms: u64, slot_ms: u64 },
    AttestationExceedsSlot { attestation_ms: u64, slot_ms: u64 },
    InsufficientSecurityMargin { margin: String },
    TooManyValidators { count: usize, max: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuickSlotStats {
    pub configs_evaluated: usize,
    pub best_throughput_config: String,
    pub best_finality_config: String,
    pub avg_security_margin: f64,
    pub total_bandwidth_overhead: f64,
}

pub fn slot_duration_ms(slot: SlotDuration) -> u64 {
    match slot {
        SlotDuration::Standard12s => 12_000,
        SlotDuration::Fast8s => 8_000,
        SlotDuration::Quick6s => 6_000,
        SlotDuration::Rapid4s => 4_000,
        SlotDuration::Ultra2s => 2_000,
    }
}

pub fn default_slot_config() -> SlotConfig {
    SlotConfig {
        slot_duration: SlotDuration::Standard12s,
        finality_mode: FinalityMode::EpochBased,
        propagation_budget_ms: 4_000,
        attestation_deadline_ms: 4_000,
        max_validators_per_slot: 32,
        network_latency_p95_ms: 500,
    }
}

pub fn validate_slot_config(config: &SlotConfig) -> Result<(), Vec<QuickSlotValidationError>> {
    let slot_ms = slot_duration_ms(config.slot_duration);
    let mut errors = Vec::new();

    if config.propagation_budget_ms > slot_ms {
        errors.push(QuickSlotValidationError::PropagationExceedsSlot {
            propagation_ms: config.propagation_budget_ms,
            slot_ms,
        });
    }

    if config.attestation_deadline_ms > slot_ms {
        errors.push(QuickSlotValidationError::AttestationExceedsSlot {
            attestation_ms: config.attestation_deadline_ms,
            slot_ms,
        });
    }

    let security_margin = compute_security_margin(config);
    if security_margin <= 0.2 {
        errors.push(QuickSlotValidationError::InsufficientSecurityMargin {
            margin: format!("{:.3}", security_margin),
        });
    }

    let max_supported_validators = validators_limit(config.slot_duration);
    if config.max_validators_per_slot > max_supported_validators {
        errors.push(QuickSlotValidationError::TooManyValidators {
            count: config.max_validators_per_slot,
            max: max_supported_validators,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn analyze_tradeoffs(config: &SlotConfig) -> SlotTradeoffAnalysis {
    let slot_ms = slot_duration_ms(config.slot_duration) as f64;
    let throughput_multiplier = 12_000.0 / slot_ms;
    let security_margin = compute_security_margin(config).clamp(0.0, 1.0);

    let finality_latency_ms = match config.finality_mode {
        FinalityMode::EpochBased => 64.0 * slot_ms,
        FinalityMode::ThreeSlotFinality => 3.0 * slot_ms,
        FinalityMode::SingleSlotFinality => slot_ms,
        FinalityMode::OobCitadel => {
            let oob_gain = estimate_oob_benefit(config.slot_duration);
            (3.0 * slot_ms) / oob_gain
        }
    };

    let propagation_budget = config.propagation_budget_ms.max(1) as f64;
    let latency_ratio = config.network_latency_p95_ms as f64 / propagation_budget;
    let base_miss_probability = if latency_ratio <= 0.5 {
        0.01
    } else if latency_ratio <= 0.75 {
        0.03
    } else if latency_ratio <= 1.0 {
        0.08
    } else {
        (0.08 + (latency_ratio - 1.0) * 0.35).min(1.0)
    };
    let headroom_penalty = (1.0 - security_margin).max(0.0) * 0.06;
    let missed_slot_probability = (base_miss_probability + headroom_penalty).clamp(0.0, 1.0);

    let bandwidth_overhead_factor = throughput_multiplier
        * (1.0 + latency_ratio.min(2.0) * 0.1 + missed_slot_probability * 0.1);

    SlotTradeoffAnalysis {
        slot_duration: config.slot_duration,
        finality_mode: config.finality_mode,
        inclusion_latency_avg_ms: slot_ms / 2.0,
        finality_latency_ms,
        missed_slot_probability,
        throughput_multiplier,
        security_margin,
        bandwidth_overhead_factor,
    }
}

pub fn compare_configurations(configs: &[SlotConfig]) -> QuickSlotStats {
    if configs.is_empty() {
        return QuickSlotStats {
            configs_evaluated: 0,
            best_throughput_config: "N/A".to_string(),
            best_finality_config: "N/A".to_string(),
            avg_security_margin: 0.0,
            total_bandwidth_overhead: 0.0,
        };
    }

    let analyses: Vec<SlotTradeoffAnalysis> = configs.iter().map(analyze_tradeoffs).collect();

    let labels: HashMap<(SlotDuration, FinalityMode), String> = analyses
        .iter()
        .map(|analysis| {
            (
                (analysis.slot_duration, analysis.finality_mode),
                format!("{:?}+{:?}", analysis.slot_duration, analysis.finality_mode),
            )
        })
        .collect();

    let best_throughput = analyses.iter().max_by(|a, b| {
        a.throughput_multiplier
            .total_cmp(&b.throughput_multiplier)
            .then_with(|| {
                b.missed_slot_probability
                    .total_cmp(&a.missed_slot_probability)
            })
    });

    let best_finality = analyses.iter().min_by(|a, b| {
        a.finality_latency_ms
            .total_cmp(&b.finality_latency_ms)
            .then_with(|| b.security_margin.total_cmp(&a.security_margin))
    });

    let avg_security_margin = analyses
        .iter()
        .map(|analysis| analysis.security_margin)
        .sum::<f64>()
        / analyses.len() as f64;
    let total_bandwidth_overhead = analyses
        .iter()
        .map(|analysis| analysis.bandwidth_overhead_factor)
        .sum::<f64>();

    let throughput_label = best_throughput
        .and_then(|analysis| labels.get(&(analysis.slot_duration, analysis.finality_mode)))
        .cloned()
        .unwrap_or_else(|| "N/A".to_string());
    let finality_label = best_finality
        .and_then(|analysis| labels.get(&(analysis.slot_duration, analysis.finality_mode)))
        .cloned()
        .unwrap_or_else(|| "N/A".to_string());

    QuickSlotStats {
        configs_evaluated: configs.len(),
        best_throughput_config: throughput_label,
        best_finality_config: finality_label,
        avg_security_margin,
        total_bandwidth_overhead,
    }
}

pub fn estimate_oob_benefit(slot: SlotDuration) -> f64 {
    match slot {
        SlotDuration::Standard12s => 1.1,
        SlotDuration::Fast8s => 1.3,
        SlotDuration::Quick6s => 1.5,
        SlotDuration::Rapid4s => 1.8,
        SlotDuration::Ultra2s => 2.4,
    }
}

fn compute_security_margin(config: &SlotConfig) -> f64 {
    let slot_ms = slot_duration_ms(config.slot_duration) as f64;
    if slot_ms == 0.0 {
        return 0.0;
    }

    1.0 - (config.propagation_budget_ms + config.attestation_deadline_ms) as f64 / slot_ms
}

fn validators_limit(slot: SlotDuration) -> usize {
    match slot {
        SlotDuration::Standard12s => 128,
        SlotDuration::Fast8s => 96,
        SlotDuration::Quick6s => 72,
        SlotDuration::Rapid4s => 48,
        SlotDuration::Ultra2s => 32,
    }
}
