use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FinalityTarget {
    SingleSlot,
    FewSlots,
    SubMinute,
    CurrentBaseline,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum InclusionMode {
    Immediate,
    NextSlot,
    BestEffort,
    Guaranteed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LatencyComponent {
    PropagationDelay,
    ExecutionTime,
    ConsensusRounds,
    AttestationGathering,
    FinalityVoting,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LatencyBudget {
    pub component: LatencyComponent,
    pub budget_ms: f64,
    pub measured_ms: Option<f64>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FastL1Config {
    pub finality_target: FinalityTarget,
    pub inclusion_mode: InclusionMode,
    pub slot_time_ms: f64,
    pub target_inclusion_ms: f64,
    pub target_finality_ms: f64,
    pub latency_budgets: Vec<LatencyBudget>,
    pub validator_count: usize,
    pub network_diameter_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FastL1ValidationError {
    SlotTimeTooLow { value: f64 },
    InclusionExceedsFinality,
    BudgetExceedsSlot { total_ms: f64, slot_ms: f64 },
    ValidatorCountZero,
    NetworkDiameterInvalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FastL1Stats {
    pub achievable_inclusion_ms: f64,
    pub achievable_finality_ms: f64,
    pub slot_utilization: f64,
    pub bottleneck_component: String,
    pub feasibility_score: f64,
    pub meets_target: bool,
    pub headroom_ms: f64,
}

pub fn default_fast_l1_config() -> FastL1Config {
    FastL1Config {
        finality_target: FinalityTarget::FewSlots,
        inclusion_mode: InclusionMode::Immediate,
        slot_time_ms: 4_000.0,
        target_inclusion_ms: 1_500.0,
        target_finality_ms: 10_000.0,
        latency_budgets: vec![
            LatencyBudget {
                component: LatencyComponent::PropagationDelay,
                budget_ms: 220.0,
                measured_ms: Some(210.0),
                confidence: 0.92,
            },
            LatencyBudget {
                component: LatencyComponent::ExecutionTime,
                budget_ms: 320.0,
                measured_ms: Some(305.0),
                confidence: 0.90,
            },
            LatencyBudget {
                component: LatencyComponent::ConsensusRounds,
                budget_ms: 180.0,
                measured_ms: Some(170.0),
                confidence: 0.88,
            },
            LatencyBudget {
                component: LatencyComponent::AttestationGathering,
                budget_ms: 450.0,
                measured_ms: Some(430.0),
                confidence: 0.87,
            },
            LatencyBudget {
                component: LatencyComponent::FinalityVoting,
                budget_ms: 700.0,
                measured_ms: Some(690.0),
                confidence: 0.85,
            },
        ],
        validator_count: 2_048,
        network_diameter_ms: 120.0,
    }
}

pub fn validate_fast_l1_config(config: &FastL1Config) -> Result<(), Vec<FastL1ValidationError>> {
    let mut errors = Vec::new();
    let min_slot_ms = 500.0;

    if !config.slot_time_ms.is_finite() || config.slot_time_ms < min_slot_ms {
        errors.push(FastL1ValidationError::SlotTimeTooLow {
            value: config.slot_time_ms,
        });
    }

    if !config.target_inclusion_ms.is_finite()
        || !config.target_finality_ms.is_finite()
        || config.target_inclusion_ms > config.target_finality_ms
    {
        errors.push(FastL1ValidationError::InclusionExceedsFinality);
    }

    let total_budget_ms = config
        .latency_budgets
        .iter()
        .map(|budget| budget.budget_ms.max(0.0))
        .sum::<f64>();
    if total_budget_ms > config.slot_time_ms {
        errors.push(FastL1ValidationError::BudgetExceedsSlot {
            total_ms: total_budget_ms,
            slot_ms: config.slot_time_ms,
        });
    }

    if config.validator_count == 0 {
        errors.push(FastL1ValidationError::ValidatorCountZero);
    }

    if !config.network_diameter_ms.is_finite() || config.network_diameter_ms <= 0.0 {
        errors.push(FastL1ValidationError::NetworkDiameterInvalid);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_fast_l1_stats(config: &FastL1Config) -> FastL1Stats {
    let propagation_estimate =
        estimate_propagation_delay(config.validator_count, config.network_diameter_ms);
    let budgets = component_budget_map(config);
    let propagation = budgets.0.max(propagation_estimate);
    let execution = budgets.1;
    let consensus = budgets.2;
    let attestation = budgets.3;
    let finality_voting = budgets.4;

    let pipeline_inclusion = propagation + execution + consensus;
    let achievable_inclusion_ms = match config.inclusion_mode {
        InclusionMode::Immediate => pipeline_inclusion,
        InclusionMode::NextSlot => config.slot_time_ms + (pipeline_inclusion * 0.20),
        InclusionMode::BestEffort => (pipeline_inclusion * 1.15).min(config.slot_time_ms * 1.1),
        InclusionMode::Guaranteed => {
            (config.slot_time_ms + (pipeline_inclusion * 0.30)).max(pipeline_inclusion * 1.2)
        }
    };

    let target_slots = slots_for_finality_target(config.finality_target, config.slot_time_ms);
    let finality_pipeline_overhead =
        (consensus + attestation + finality_voting) + (propagation * 0.5);
    let achievable_finality_ms = (target_slots * config.slot_time_ms) + finality_pipeline_overhead;

    let measured_total = total_measured_latency(config);
    let slot_utilization = if config.slot_time_ms > 0.0 {
        measured_total / config.slot_time_ms
    } else {
        1.0
    };
    let bottleneck_component = bottleneck_component_name(config);
    let confidence = avg_confidence(config);
    let inclusion_ratio = if achievable_inclusion_ms > 0.0 {
        config.target_inclusion_ms / achievable_inclusion_ms
    } else {
        0.0
    };
    let finality_ratio = if achievable_finality_ms > 0.0 {
        config.target_finality_ms / achievable_finality_ms
    } else {
        0.0
    };
    let target_alignment = inclusion_ratio.min(finality_ratio).clamp(0.0, 1.0);
    let utilization_score = (1.0 - slot_utilization.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let feasibility_score =
        (0.55 * target_alignment + 0.30 * confidence + 0.15 * utilization_score).clamp(0.0, 1.0);
    let meets_target = achievable_inclusion_ms <= config.target_inclusion_ms
        && achievable_finality_ms <= config.target_finality_ms;
    let headroom_ms = (config.target_inclusion_ms - achievable_inclusion_ms)
        .min(config.target_finality_ms - achievable_finality_ms);

    FastL1Stats {
        achievable_inclusion_ms,
        achievable_finality_ms,
        slot_utilization,
        bottleneck_component,
        feasibility_score,
        meets_target,
        headroom_ms,
    }
}

pub fn compare_finality_targets(config: &FastL1Config) -> Vec<(String, FastL1Stats)> {
    let targets = [
        FinalityTarget::SingleSlot,
        FinalityTarget::FewSlots,
        FinalityTarget::SubMinute,
        FinalityTarget::CurrentBaseline,
    ];

    targets
        .iter()
        .map(|target| {
            let mut variant = config.clone();
            variant.finality_target = *target;
            (
                finality_target_name(*target).to_string(),
                compute_fast_l1_stats(&variant),
            )
        })
        .collect()
}

pub fn estimate_propagation_delay(validator_count: usize, network_diameter_ms: f64) -> f64 {
    if validator_count == 0 || !network_diameter_ms.is_finite() || network_diameter_ms <= 0.0 {
        return 0.0;
    }

    let fanout_depth_penalty = (validator_count as f64).log2().max(1.0) * 4.0;
    network_diameter_ms + fanout_depth_penalty
}

pub fn compute_fast_l1_commitment(config: &FastL1Config) -> [u8; 32] {
    let mut normalized_budgets = config.latency_budgets.clone();
    normalized_budgets.sort_by_key(|budget| latency_component_byte(budget.component));

    let mut hasher = Sha256::new();
    hasher.update(b"FAST-L1-V1");
    hasher.update([finality_target_byte(config.finality_target)]);
    hasher.update([inclusion_mode_byte(config.inclusion_mode)]);
    hasher.update(config.slot_time_ms.to_le_bytes());
    hasher.update(config.target_inclusion_ms.to_le_bytes());
    hasher.update(config.target_finality_ms.to_le_bytes());
    hasher.update((config.validator_count as u64).to_le_bytes());
    hasher.update(config.network_diameter_ms.to_le_bytes());
    hasher.update((normalized_budgets.len() as u64).to_le_bytes());

    for budget in normalized_budgets {
        hasher.update([latency_component_byte(budget.component)]);
        hasher.update(budget.budget_ms.to_le_bytes());
        hasher.update([if budget.measured_ms.is_some() { 1 } else { 0 }]);
        hasher.update(budget.measured_ms.unwrap_or_default().to_le_bytes());
        hasher.update(budget.confidence.to_le_bytes());
    }

    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn component_budget_map(config: &FastL1Config) -> (f64, f64, f64, f64, f64) {
    let mut propagation: f64 = 0.0;
    let mut execution: f64 = 0.0;
    let mut consensus: f64 = 0.0;
    let mut attestation: f64 = 0.0;
    let mut finality_voting: f64 = 0.0;

    for budget in &config.latency_budgets {
        let value = budget.measured_ms.unwrap_or(budget.budget_ms).max(0.0);
        match budget.component {
            LatencyComponent::PropagationDelay => propagation = propagation.max(value),
            LatencyComponent::ExecutionTime => execution = execution.max(value),
            LatencyComponent::ConsensusRounds => consensus = consensus.max(value),
            LatencyComponent::AttestationGathering => attestation = attestation.max(value),
            LatencyComponent::FinalityVoting => finality_voting = finality_voting.max(value),
        }
    }

    (
        propagation,
        execution,
        consensus,
        attestation,
        finality_voting,
    )
}

fn total_measured_latency(config: &FastL1Config) -> f64 {
    config
        .latency_budgets
        .iter()
        .map(|budget| budget.measured_ms.unwrap_or(budget.budget_ms).max(0.0))
        .sum()
}

fn bottleneck_component_name(config: &FastL1Config) -> String {
    let bottleneck = config.latency_budgets.iter().max_by(|a, b| {
        a.measured_ms
            .unwrap_or(a.budget_ms)
            .total_cmp(&b.measured_ms.unwrap_or(b.budget_ms))
    });

    match bottleneck.map(|budget| budget.component) {
        Some(component) => component_name(component).to_string(),
        None => "None".to_string(),
    }
}

fn avg_confidence(config: &FastL1Config) -> f64 {
    if config.latency_budgets.is_empty() {
        return 0.0;
    }

    config
        .latency_budgets
        .iter()
        .map(|budget| budget.confidence.clamp(0.0, 1.0))
        .sum::<f64>()
        / config.latency_budgets.len() as f64
}

fn slots_for_finality_target(target: FinalityTarget, slot_time_ms: f64) -> f64 {
    match target {
        FinalityTarget::SingleSlot => 1.0,
        FinalityTarget::FewSlots => 2.0,
        FinalityTarget::SubMinute => (60_000.0 / slot_time_ms.max(1.0)).clamp(3.0, 10.0),
        FinalityTarget::CurrentBaseline => 64.0,
    }
}

fn finality_target_name(target: FinalityTarget) -> &'static str {
    match target {
        FinalityTarget::SingleSlot => "SingleSlot",
        FinalityTarget::FewSlots => "FewSlots",
        FinalityTarget::SubMinute => "SubMinute",
        FinalityTarget::CurrentBaseline => "CurrentBaseline",
    }
}

fn component_name(component: LatencyComponent) -> &'static str {
    match component {
        LatencyComponent::PropagationDelay => "PropagationDelay",
        LatencyComponent::ExecutionTime => "ExecutionTime",
        LatencyComponent::ConsensusRounds => "ConsensusRounds",
        LatencyComponent::AttestationGathering => "AttestationGathering",
        LatencyComponent::FinalityVoting => "FinalityVoting",
    }
}

fn finality_target_byte(target: FinalityTarget) -> u8 {
    match target {
        FinalityTarget::SingleSlot => 0,
        FinalityTarget::FewSlots => 1,
        FinalityTarget::SubMinute => 2,
        FinalityTarget::CurrentBaseline => 3,
    }
}

fn inclusion_mode_byte(mode: InclusionMode) -> u8 {
    match mode {
        InclusionMode::Immediate => 0,
        InclusionMode::NextSlot => 1,
        InclusionMode::BestEffort => 2,
        InclusionMode::Guaranteed => 3,
    }
}

fn latency_component_byte(component: LatencyComponent) -> u8 {
    match component {
        LatencyComponent::PropagationDelay => 0,
        LatencyComponent::ExecutionTime => 1,
        LatencyComponent::ConsensusRounds => 2,
        LatencyComponent::AttestationGathering => 3,
        LatencyComponent::FinalityVoting => 4,
    }
}
