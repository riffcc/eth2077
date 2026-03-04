use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum InclusionListMode {
    Mandatory,
    Advisory,
    Hybrid,
    ConditionalEnforcement,
    GradualRollout,
    FullEnforcement,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProposerConstraint {
    MustInclude,
    ShouldInclude,
    BestEffort,
    TimeBounded,
    FeeThreshold,
    NoConstraint,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EnforcementMechanism {
    ForkChoiceFilter,
    SlashingPenalty,
    ReputationScore,
    EconomicIncentive,
    SocialConsensus,
    HybridEnforcement,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CensorshipMetric {
    TransactionDelay,
    InclusionRate,
    ProposerCompliance,
    NetworkFairness,
    MEVExtraction,
    UserExperience,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FocilIntegrationConfig {
    pub inclusion_mode: InclusionListMode,
    pub proposer_constraint: ProposerConstraint,
    pub enforcement_mechanism: EnforcementMechanism,
    pub censorship_metric: CensorshipMetric,
    pub max_inclusion_list_txs: usize,
    pub inclusion_deadline_slots: u64,
    pub enforcement_penalty_gwei: u64,
    pub min_proposer_compliance: f64,
    pub network_participation_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FocilValidationError {
    ZeroInclusionListSize,
    DeadlineTooShort { slots: u64 },
    PenaltyTooLow { value: u64 },
    ComplianceOutOfRange { value: f64 },
    ParticipationOutOfRange { value: f64 },
    IncompatibleModeAndConstraint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FocilIntegrationStats {
    pub effective_inclusion_rate: f64,
    pub proposer_compliance_score: f64,
    pub censorship_resistance_index: f64,
    pub enforcement_cost_gwei: f64,
    pub network_overhead_fraction: f64,
    pub fork_complexity_score: f64,
    pub bottleneck: String,
    pub deployment_caveats: Vec<String>,
}

pub fn default_focil_integration_config() -> FocilIntegrationConfig {
    FocilIntegrationConfig {
        inclusion_mode: InclusionListMode::Hybrid,
        proposer_constraint: ProposerConstraint::TimeBounded,
        enforcement_mechanism: EnforcementMechanism::ForkChoiceFilter,
        censorship_metric: CensorshipMetric::InclusionRate,
        max_inclusion_list_txs: 128,
        inclusion_deadline_slots: 4,
        enforcement_penalty_gwei: 2_000,
        min_proposer_compliance: 0.90,
        network_participation_threshold: 0.67,
    }
}

pub fn validate_focil_config(
    config: &FocilIntegrationConfig,
) -> Result<(), Vec<FocilValidationError>> {
    let mut errors = Vec::new();

    if config.max_inclusion_list_txs == 0 {
        errors.push(FocilValidationError::ZeroInclusionListSize);
    }

    if config.inclusion_deadline_slots < 2 {
        errors.push(FocilValidationError::DeadlineTooShort {
            slots: config.inclusion_deadline_slots,
        });
    }

    if config.enforcement_penalty_gwei < 1_000 {
        errors.push(FocilValidationError::PenaltyTooLow {
            value: config.enforcement_penalty_gwei,
        });
    }

    if !config.min_proposer_compliance.is_finite()
        || config.min_proposer_compliance < 0.0
        || config.min_proposer_compliance > 1.0
    {
        errors.push(FocilValidationError::ComplianceOutOfRange {
            value: config.min_proposer_compliance,
        });
    }

    if !config.network_participation_threshold.is_finite()
        || config.network_participation_threshold < 0.0
        || config.network_participation_threshold > 1.0
    {
        errors.push(FocilValidationError::ParticipationOutOfRange {
            value: config.network_participation_threshold,
        });
    }

    if !is_mode_constraint_compatible(config.inclusion_mode, config.proposer_constraint) {
        errors.push(FocilValidationError::IncompatibleModeAndConstraint);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_focil_stats(config: &FocilIntegrationConfig) -> FocilIntegrationStats {
    let mode_base = mode_base_rate(config.inclusion_mode);
    let constraint_factor = constraint_factor(config.proposer_constraint);
    let enforcement_factor = enforcement_strength(config.enforcement_mechanism);
    let metric_factor = metric_adjustment(config.censorship_metric);
    let participation = config.network_participation_threshold.clamp(0.0, 1.0);

    let effective_inclusion_rate = (mode_base
        * constraint_factor
        * enforcement_factor
        * metric_factor
        * (0.75 + (0.25 * participation)))
        .clamp(0.0, 1.0);

    let proposer_compliance_score = (0.60 * config.min_proposer_compliance.clamp(0.0, 1.0)
        + 0.40 * effective_inclusion_rate)
        .clamp(0.0, 1.0);

    let censorship_resistance_index =
        (0.50 * effective_inclusion_rate + 0.30 * proposer_compliance_score + 0.20 * participation)
            .clamp(0.0, 1.0);

    let mode_overhead = mode_overhead(config.inclusion_mode);
    let mechanism_cost = mechanism_cost(config.enforcement_mechanism);
    let volume_factor = 1.0 + (config.max_inclusion_list_txs as f64 / 256.0);
    let deadline_pressure = if config.inclusion_deadline_slots == 0 {
        2.5
    } else {
        (2.5 / config.inclusion_deadline_slots as f64).clamp(0.4, 2.5)
    };

    let enforcement_cost_gwei = config.enforcement_penalty_gwei as f64
        * mechanism_cost
        * mode_overhead
        * volume_factor
        * deadline_pressure;

    let network_overhead_fraction = (0.08 * mode_overhead
        + (config.max_inclusion_list_txs as f64 / 4_000.0)
        + 0.05 * (deadline_pressure - 0.4))
        .clamp(0.0, 1.0);

    let fork_complexity_score = (mode_overhead
        * mechanism_complexity(config.enforcement_mechanism)
        * constraint_complexity(config.proposer_constraint)
        * (1.0 + 0.20 * deadline_pressure))
        .clamp(0.0, 10.0);

    let bottleneck = if network_overhead_fraction > 0.35 {
        "P2PPropagation".to_string()
    } else if proposer_compliance_score < config.min_proposer_compliance {
        "ProposerIncentives".to_string()
    } else if enforcement_cost_gwei > (config.enforcement_penalty_gwei as f64 * 2.5) {
        "EnforcementCost".to_string()
    } else if config.inclusion_deadline_slots <= 2 {
        "DeadlineTightness".to_string()
    } else {
        "Balanced".to_string()
    };

    let mut deployment_caveats = Vec::new();
    if matches!(
        config.inclusion_mode,
        InclusionListMode::Mandatory | InclusionListMode::FullEnforcement
    ) {
        deployment_caveats.push(
            "Strict mode requires robust fallback handling for missed inclusions.".to_string(),
        );
    }
    if matches!(
        config.enforcement_mechanism,
        EnforcementMechanism::SlashingPenalty | EnforcementMechanism::HybridEnforcement
    ) {
        deployment_caveats.push(
            "Penalty governance and appeals process must be finalized before rollout.".to_string(),
        );
    }
    if config.network_participation_threshold < 0.5 {
        deployment_caveats.push(
            "Low participation threshold may weaken censorship-resistance guarantees.".to_string(),
        );
    }
    if config.max_inclusion_list_txs > 512 {
        deployment_caveats.push(
            "Large inclusion lists can increase block propagation time and uncle risk.".to_string(),
        );
    }
    if config.inclusion_deadline_slots <= 2 {
        deployment_caveats.push(
            "Tight inclusion deadlines may cause transient non-compliance under network latency."
                .to_string(),
        );
    }
    if matches!(config.proposer_constraint, ProposerConstraint::NoConstraint) {
        deployment_caveats.push(
            "NoConstraint weakens enforceability and should only be used during early rollout."
                .to_string(),
        );
    }
    if deployment_caveats.is_empty() {
        deployment_caveats.push("No major caveats identified for this configuration.".to_string());
    }

    FocilIntegrationStats {
        effective_inclusion_rate,
        proposer_compliance_score,
        censorship_resistance_index,
        enforcement_cost_gwei,
        network_overhead_fraction,
        fork_complexity_score,
        bottleneck,
        deployment_caveats,
    }
}

pub fn compare_inclusion_modes(
    config: &FocilIntegrationConfig,
) -> Vec<(String, FocilIntegrationStats)> {
    all_inclusion_modes()
        .into_iter()
        .map(|mode| {
            let mut variant = config.clone();
            variant.inclusion_mode = mode;
            (format!("{mode:?}"), compute_focil_stats(&variant))
        })
        .collect()
}

pub fn compute_focil_commitment(config: &FocilIntegrationConfig) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ETH2077::FOCIL_INTEGRATION::V1");
    hasher.update([inclusion_mode_discriminant(config.inclusion_mode)]);
    hasher.update([constraint_discriminant(config.proposer_constraint)]);
    hasher.update([enforcement_discriminant(config.enforcement_mechanism)]);
    hasher.update([metric_discriminant(config.censorship_metric)]);
    hasher.update((config.max_inclusion_list_txs as u64).to_be_bytes());
    hasher.update(config.inclusion_deadline_slots.to_be_bytes());
    hasher.update(config.enforcement_penalty_gwei.to_be_bytes());
    hasher.update(config.min_proposer_compliance.to_bits().to_be_bytes());
    hasher.update(
        config
            .network_participation_threshold
            .to_bits()
            .to_be_bytes(),
    );

    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn all_inclusion_modes() -> [InclusionListMode; 6] {
    [
        InclusionListMode::Mandatory,
        InclusionListMode::Advisory,
        InclusionListMode::Hybrid,
        InclusionListMode::ConditionalEnforcement,
        InclusionListMode::GradualRollout,
        InclusionListMode::FullEnforcement,
    ]
}

fn is_mode_constraint_compatible(mode: InclusionListMode, constraint: ProposerConstraint) -> bool {
    match mode {
        InclusionListMode::Mandatory => matches!(
            constraint,
            ProposerConstraint::MustInclude | ProposerConstraint::TimeBounded
        ),
        InclusionListMode::Advisory => matches!(
            constraint,
            ProposerConstraint::ShouldInclude
                | ProposerConstraint::BestEffort
                | ProposerConstraint::NoConstraint
        ),
        InclusionListMode::Hybrid => matches!(
            constraint,
            ProposerConstraint::MustInclude
                | ProposerConstraint::ShouldInclude
                | ProposerConstraint::TimeBounded
                | ProposerConstraint::FeeThreshold
        ),
        InclusionListMode::ConditionalEnforcement => matches!(
            constraint,
            ProposerConstraint::ShouldInclude
                | ProposerConstraint::TimeBounded
                | ProposerConstraint::FeeThreshold
        ),
        InclusionListMode::GradualRollout => matches!(
            constraint,
            ProposerConstraint::ShouldInclude
                | ProposerConstraint::BestEffort
                | ProposerConstraint::TimeBounded
                | ProposerConstraint::FeeThreshold
        ),
        InclusionListMode::FullEnforcement => matches!(
            constraint,
            ProposerConstraint::MustInclude
                | ProposerConstraint::TimeBounded
                | ProposerConstraint::FeeThreshold
        ),
    }
}

fn mode_base_rate(mode: InclusionListMode) -> f64 {
    match mode {
        InclusionListMode::Mandatory => 0.96,
        InclusionListMode::Advisory => 0.74,
        InclusionListMode::Hybrid => 0.89,
        InclusionListMode::ConditionalEnforcement => 0.85,
        InclusionListMode::GradualRollout => 0.81,
        InclusionListMode::FullEnforcement => 0.99,
    }
}

fn mode_overhead(mode: InclusionListMode) -> f64 {
    match mode {
        InclusionListMode::Mandatory => 1.25,
        InclusionListMode::Advisory => 0.70,
        InclusionListMode::Hybrid => 1.00,
        InclusionListMode::ConditionalEnforcement => 0.95,
        InclusionListMode::GradualRollout => 0.85,
        InclusionListMode::FullEnforcement => 1.45,
    }
}

fn constraint_factor(constraint: ProposerConstraint) -> f64 {
    match constraint {
        ProposerConstraint::MustInclude => 1.00,
        ProposerConstraint::ShouldInclude => 0.94,
        ProposerConstraint::BestEffort => 0.82,
        ProposerConstraint::TimeBounded => 0.91,
        ProposerConstraint::FeeThreshold => 0.88,
        ProposerConstraint::NoConstraint => 0.70,
    }
}

fn enforcement_strength(mechanism: EnforcementMechanism) -> f64 {
    match mechanism {
        EnforcementMechanism::ForkChoiceFilter => 1.00,
        EnforcementMechanism::SlashingPenalty => 0.97,
        EnforcementMechanism::ReputationScore => 0.90,
        EnforcementMechanism::EconomicIncentive => 0.92,
        EnforcementMechanism::SocialConsensus => 0.87,
        EnforcementMechanism::HybridEnforcement => 1.03,
    }
}

fn metric_adjustment(metric: CensorshipMetric) -> f64 {
    match metric {
        CensorshipMetric::TransactionDelay => 0.95,
        CensorshipMetric::InclusionRate => 1.00,
        CensorshipMetric::ProposerCompliance => 0.98,
        CensorshipMetric::NetworkFairness => 0.97,
        CensorshipMetric::MEVExtraction => 0.93,
        CensorshipMetric::UserExperience => 0.96,
    }
}

fn mechanism_cost(mechanism: EnforcementMechanism) -> f64 {
    match mechanism {
        EnforcementMechanism::ForkChoiceFilter => 1.10,
        EnforcementMechanism::SlashingPenalty => 1.40,
        EnforcementMechanism::ReputationScore => 0.80,
        EnforcementMechanism::EconomicIncentive => 1.00,
        EnforcementMechanism::SocialConsensus => 0.65,
        EnforcementMechanism::HybridEnforcement => 1.60,
    }
}

fn mechanism_complexity(mechanism: EnforcementMechanism) -> f64 {
    match mechanism {
        EnforcementMechanism::ForkChoiceFilter => 1.10,
        EnforcementMechanism::SlashingPenalty => 1.30,
        EnforcementMechanism::ReputationScore => 0.90,
        EnforcementMechanism::EconomicIncentive => 1.00,
        EnforcementMechanism::SocialConsensus => 0.85,
        EnforcementMechanism::HybridEnforcement => 1.45,
    }
}

fn constraint_complexity(constraint: ProposerConstraint) -> f64 {
    match constraint {
        ProposerConstraint::MustInclude => 1.20,
        ProposerConstraint::ShouldInclude => 0.90,
        ProposerConstraint::BestEffort => 0.75,
        ProposerConstraint::TimeBounded => 1.10,
        ProposerConstraint::FeeThreshold => 1.05,
        ProposerConstraint::NoConstraint => 0.60,
    }
}

fn inclusion_mode_discriminant(mode: InclusionListMode) -> u8 {
    match mode {
        InclusionListMode::Mandatory => 0,
        InclusionListMode::Advisory => 1,
        InclusionListMode::Hybrid => 2,
        InclusionListMode::ConditionalEnforcement => 3,
        InclusionListMode::GradualRollout => 4,
        InclusionListMode::FullEnforcement => 5,
    }
}

fn constraint_discriminant(constraint: ProposerConstraint) -> u8 {
    match constraint {
        ProposerConstraint::MustInclude => 0,
        ProposerConstraint::ShouldInclude => 1,
        ProposerConstraint::BestEffort => 2,
        ProposerConstraint::TimeBounded => 3,
        ProposerConstraint::FeeThreshold => 4,
        ProposerConstraint::NoConstraint => 5,
    }
}

fn enforcement_discriminant(mechanism: EnforcementMechanism) -> u8 {
    match mechanism {
        EnforcementMechanism::ForkChoiceFilter => 0,
        EnforcementMechanism::SlashingPenalty => 1,
        EnforcementMechanism::ReputationScore => 2,
        EnforcementMechanism::EconomicIncentive => 3,
        EnforcementMechanism::SocialConsensus => 4,
        EnforcementMechanism::HybridEnforcement => 5,
    }
}

fn metric_discriminant(metric: CensorshipMetric) -> u8 {
    match metric {
        CensorshipMetric::TransactionDelay => 0,
        CensorshipMetric::InclusionRate => 1,
        CensorshipMetric::ProposerCompliance => 2,
        CensorshipMetric::NetworkFairness => 3,
        CensorshipMetric::MEVExtraction => 4,
        CensorshipMetric::UserExperience => 5,
    }
}
