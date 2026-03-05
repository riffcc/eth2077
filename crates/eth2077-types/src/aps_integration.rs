use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AttesterRole {
    FullAttester,
    LightAttester,
    CommitteeAttester,
    SyncCommittee,
    RandomSampled,
    DelegatedAttester,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProposerMode {
    SoloProposer,
    RotatingProposer,
    AuctionedProposer,
    CommitteeProposer,
    DelegatedProposer,
    HybridMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SeparationPhase {
    PreSeparation,
    InitialSplit,
    PartialMigration,
    FullSeparation,
    OptimizedSeparation,
    PostSeparation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SafetyCheck {
    LivenessGuarantee,
    FinalityBound,
    CensorshipResistance,
    MEVFairness,
    ValidatorEconomics,
    NetworkStability,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApsIntegrationConfig {
    pub proposer_mode: ProposerMode,
    pub attester_role: AttesterRole,
    pub current_phase: SeparationPhase,
    pub required_safety_checks: Vec<SafetyCheck>,
    pub validator_set_size: usize,
    pub committee_size: usize,
    pub rotation_period_epochs: u64,
    pub attestation_deadline_slots: u64,
    pub min_attester_participation: f64,
    pub proposer_collateral_eth: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ApsValidationError {
    ValidatorSetTooSmall { size: usize },
    CommitteeTooLarge { size: usize, max: usize },
    RotationTooFrequent { epochs: u64 },
    AttestationDeadlineZero,
    ParticipationOutOfRange { value: f64 },
    NoSafetyChecks,
    CollateralNonPositive { value: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApsIntegrationStats {
    pub effective_attestation_rate: f64,
    pub proposer_utilization: f64,
    pub separation_completeness: f64,
    pub safety_coverage: f64,
    pub validator_overhead: f64,
    pub migration_risk_score: f64,
    pub bottleneck: String,
    pub milestones: Vec<String>,
}

pub fn default_aps_integration_config() -> ApsIntegrationConfig {
    ApsIntegrationConfig {
        proposer_mode: ProposerMode::RotatingProposer,
        attester_role: AttesterRole::CommitteeAttester,
        current_phase: SeparationPhase::InitialSplit,
        required_safety_checks: vec![
            SafetyCheck::LivenessGuarantee,
            SafetyCheck::FinalityBound,
            SafetyCheck::CensorshipResistance,
        ],
        validator_set_size: 8192,
        committee_size: 512,
        rotation_period_epochs: 8,
        attestation_deadline_slots: 8,
        min_attester_participation: 0.85,
        proposer_collateral_eth: 64.0,
    }
}

pub fn validate_aps_config(config: &ApsIntegrationConfig) -> Result<(), Vec<ApsValidationError>> {
    let mut errors = Vec::new();

    if config.validator_set_size < 1024 {
        errors.push(ApsValidationError::ValidatorSetTooSmall {
            size: config.validator_set_size,
        });
    }

    let max_committee = config.validator_set_size / 4;
    if config.committee_size > max_committee {
        errors.push(ApsValidationError::CommitteeTooLarge {
            size: config.committee_size,
            max: max_committee,
        });
    }

    if config.rotation_period_epochs < 4 {
        errors.push(ApsValidationError::RotationTooFrequent {
            epochs: config.rotation_period_epochs,
        });
    }

    if config.attestation_deadline_slots == 0 {
        errors.push(ApsValidationError::AttestationDeadlineZero);
    }

    if config.min_attester_participation < 0.0 || config.min_attester_participation > 1.0 {
        errors.push(ApsValidationError::ParticipationOutOfRange {
            value: config.min_attester_participation,
        });
    }

    if config.required_safety_checks.is_empty() {
        errors.push(ApsValidationError::NoSafetyChecks);
    }

    if config.proposer_collateral_eth <= 0.0 {
        errors.push(ApsValidationError::CollateralNonPositive {
            value: config.proposer_collateral_eth,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_aps_stats(config: &ApsIntegrationConfig) -> ApsIntegrationStats {
    let (
        mut effective_attestation_rate,
        mut proposer_utilization,
        mut separation_completeness,
        mut migration_risk_score,
        phase_bottleneck,
        mut milestones,
    ) = match config.current_phase {
        SeparationPhase::PreSeparation => (
            0.94,
            0.80,
            0.08,
            0.62,
            "design-finalization lag",
            vec![
                "Finalize APS design and threat model.".to_string(),
                "Instrument baseline attestation telemetry.".to_string(),
            ],
        ),
        SeparationPhase::InitialSplit => (
            0.89,
            0.74,
            0.28,
            0.55,
            "split-duty handoff instability",
            vec![
                "Launch initial proposer/attester duty split.".to_string(),
                "Validate liveness and finality under split roles.".to_string(),
            ],
        ),
        SeparationPhase::PartialMigration => (
            0.87,
            0.70,
            0.52,
            0.49,
            "mixed-mode orchestration complexity",
            vec![
                "Migrate a minority of validators to APS duties.".to_string(),
                "Backtest attestation deadlines and timeout policy.".to_string(),
            ],
        ),
        SeparationPhase::FullSeparation => (
            0.84,
            0.66,
            0.78,
            0.43,
            "committee scheduling pressure",
            vec![
                "Complete validator migration to APS.".to_string(),
                "Enforce proposer collateral and penalties.".to_string(),
            ],
        ),
        SeparationPhase::OptimizedSeparation => (
            0.92,
            0.61,
            0.93,
            0.29,
            "cross-client optimization variance",
            vec![
                "Tune proposer rotations for network efficiency.".to_string(),
                "Benchmark safety-check pass rates across clients.".to_string(),
            ],
        ),
        SeparationPhase::PostSeparation => (
            0.95,
            0.57,
            0.98,
            0.21,
            "long-tail economics drift",
            vec![
                "Publish APS stability reports.".to_string(),
                "Operationalize long-horizon validator economics audits.".to_string(),
            ],
        ),
    };

    let mut validator_overhead =
        0.10 + ((config.committee_size as f64 / config.validator_set_size.max(1) as f64) * 0.45);

    match config.attester_role {
        AttesterRole::FullAttester => {
            effective_attestation_rate += 0.05;
            validator_overhead += 0.10;
            migration_risk_score -= 0.03;
        }
        AttesterRole::LightAttester => {
            effective_attestation_rate -= 0.04;
            validator_overhead -= 0.03;
            migration_risk_score += 0.08;
        }
        AttesterRole::CommitteeAttester => {
            effective_attestation_rate += 0.02;
            validator_overhead += 0.04;
        }
        AttesterRole::SyncCommittee => {
            effective_attestation_rate += 0.01;
            proposer_utilization += 0.03;
            migration_risk_score += 0.04;
        }
        AttesterRole::RandomSampled => {
            effective_attestation_rate -= 0.02;
            validator_overhead -= 0.01;
            migration_risk_score += 0.06;
        }
        AttesterRole::DelegatedAttester => {
            effective_attestation_rate -= 0.03;
            proposer_utilization += 0.05;
            migration_risk_score += 0.09;
            milestones.push("Audit delegated-attester trust assumptions.".to_string());
        }
    }

    match config.proposer_mode {
        ProposerMode::SoloProposer => {
            proposer_utilization += 0.18;
            migration_risk_score += 0.10;
        }
        ProposerMode::RotatingProposer => {
            proposer_utilization += 0.08;
            migration_risk_score -= 0.02;
        }
        ProposerMode::AuctionedProposer => {
            proposer_utilization += 0.13;
            migration_risk_score += 0.07;
            milestones.push("Define auction anti-concentration guardrails.".to_string());
        }
        ProposerMode::CommitteeProposer => {
            proposer_utilization -= 0.03;
            migration_risk_score -= 0.04;
            validator_overhead += 0.06;
        }
        ProposerMode::DelegatedProposer => {
            proposer_utilization += 0.07;
            migration_risk_score += 0.08;
            milestones.push("Monitor delegated proposer market concentration.".to_string());
        }
        ProposerMode::HybridMode => {
            proposer_utilization += 0.02;
            migration_risk_score -= 0.01;
            separation_completeness += 0.03;
        }
    }

    let rotation_factor = (config.rotation_period_epochs as f64 / 16.0).clamp(0.25, 2.0);
    proposer_utilization *= 1.0 + (0.12 / rotation_factor);

    let deadline_factor = (config.attestation_deadline_slots as f64 / 8.0).clamp(0.25, 2.0);
    effective_attestation_rate *= 0.90 + (0.10 * deadline_factor);

    effective_attestation_rate *= config.min_attester_participation.clamp(0.0, 1.0);

    let collateral_factor = (config.proposer_collateral_eth / 64.0).clamp(0.0, 2.0);
    proposer_utilization *= 1.0 - (0.08 * (collateral_factor - 1.0).max(0.0));
    migration_risk_score -= 0.08 * collateral_factor;

    let safety_coverage =
        clamp01(config.required_safety_checks.len() as f64 / all_safety_checks().len() as f64);
    migration_risk_score += (1.0 - safety_coverage) * 0.20;

    separation_completeness += (config.required_safety_checks.len() as f64 * 0.01).min(0.06);

    if config.validator_set_size < 2048 {
        milestones.push("Scale validator set before broad APS rollout.".to_string());
    }
    if config.committee_size < 128 {
        milestones.push("Increase committee size to reduce sampling variance.".to_string());
    }

    let bottleneck = if config.rotation_period_epochs < 6 {
        "high-frequency proposer churn".to_string()
    } else if config.committee_size * 4 > config.validator_set_size {
        "committee over-allocation".to_string()
    } else if config.required_safety_checks.len() < 3 {
        "insufficient safety-check coverage".to_string()
    } else {
        phase_bottleneck.to_string()
    };

    ApsIntegrationStats {
        effective_attestation_rate: clamp01(effective_attestation_rate).max(0.01),
        proposer_utilization: clamp01(proposer_utilization).max(0.01),
        separation_completeness: clamp01(separation_completeness).max(0.01),
        safety_coverage: safety_coverage.max(0.01),
        validator_overhead: validator_overhead.max(0.01),
        migration_risk_score: clamp01(migration_risk_score).max(0.01),
        bottleneck,
        milestones,
    }
}

pub fn compare_proposer_modes(config: &ApsIntegrationConfig) -> Vec<(String, ApsIntegrationStats)> {
    all_proposer_modes()
        .iter()
        .map(|mode| {
            let mut modeled = config.clone();
            modeled.proposer_mode = mode.clone();
            (
                proposer_mode_name(mode).to_string(),
                compute_aps_stats(&modeled),
            )
        })
        .collect()
}

pub fn compute_aps_commitment(config: &ApsIntegrationConfig) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ETH2077::APS_INTEGRATION::V1");
    hasher.update([proposer_mode_tag(&config.proposer_mode)]);
    hasher.update([attester_role_tag(&config.attester_role)]);
    hasher.update([separation_phase_tag(&config.current_phase)]);
    hasher.update((config.required_safety_checks.len() as u64).to_le_bytes());
    for check in &config.required_safety_checks {
        hasher.update([safety_check_tag(check)]);
    }
    hasher.update((config.validator_set_size as u64).to_le_bytes());
    hasher.update((config.committee_size as u64).to_le_bytes());
    hasher.update(config.rotation_period_epochs.to_le_bytes());
    hasher.update(config.attestation_deadline_slots.to_le_bytes());
    hasher.update(config.min_attester_participation.to_le_bytes());
    hasher.update(config.proposer_collateral_eth.to_le_bytes());

    let digest = hasher.finalize();
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn all_proposer_modes() -> [ProposerMode; 6] {
    [
        ProposerMode::SoloProposer,
        ProposerMode::RotatingProposer,
        ProposerMode::AuctionedProposer,
        ProposerMode::CommitteeProposer,
        ProposerMode::DelegatedProposer,
        ProposerMode::HybridMode,
    ]
}

fn all_safety_checks() -> [SafetyCheck; 6] {
    [
        SafetyCheck::LivenessGuarantee,
        SafetyCheck::FinalityBound,
        SafetyCheck::CensorshipResistance,
        SafetyCheck::MEVFairness,
        SafetyCheck::ValidatorEconomics,
        SafetyCheck::NetworkStability,
    ]
}

fn proposer_mode_name(mode: &ProposerMode) -> &'static str {
    match mode {
        ProposerMode::SoloProposer => "SoloProposer",
        ProposerMode::RotatingProposer => "RotatingProposer",
        ProposerMode::AuctionedProposer => "AuctionedProposer",
        ProposerMode::CommitteeProposer => "CommitteeProposer",
        ProposerMode::DelegatedProposer => "DelegatedProposer",
        ProposerMode::HybridMode => "HybridMode",
    }
}

fn proposer_mode_tag(mode: &ProposerMode) -> u8 {
    match mode {
        ProposerMode::SoloProposer => 0,
        ProposerMode::RotatingProposer => 1,
        ProposerMode::AuctionedProposer => 2,
        ProposerMode::CommitteeProposer => 3,
        ProposerMode::DelegatedProposer => 4,
        ProposerMode::HybridMode => 5,
    }
}

fn attester_role_tag(role: &AttesterRole) -> u8 {
    match role {
        AttesterRole::FullAttester => 0,
        AttesterRole::LightAttester => 1,
        AttesterRole::CommitteeAttester => 2,
        AttesterRole::SyncCommittee => 3,
        AttesterRole::RandomSampled => 4,
        AttesterRole::DelegatedAttester => 5,
    }
}

fn separation_phase_tag(phase: &SeparationPhase) -> u8 {
    match phase {
        SeparationPhase::PreSeparation => 0,
        SeparationPhase::InitialSplit => 1,
        SeparationPhase::PartialMigration => 2,
        SeparationPhase::FullSeparation => 3,
        SeparationPhase::OptimizedSeparation => 4,
        SeparationPhase::PostSeparation => 5,
    }
}

fn safety_check_tag(check: &SafetyCheck) -> u8 {
    match check {
        SafetyCheck::LivenessGuarantee => 0,
        SafetyCheck::FinalityBound => 1,
        SafetyCheck::CensorshipResistance => 2,
        SafetyCheck::MEVFairness => 3,
        SafetyCheck::ValidatorEconomics => 4,
        SafetyCheck::NetworkStability => 5,
    }
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}
