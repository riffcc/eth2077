use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FaultType {
    NetworkPartition,
    MessageDelay,
    MessageDrop,
    MessageReorder,
    EquivocationAttempt,
    ReplayAttack,
    CrashFailure,
    ByzantineProposal,
    TimestampSkew,
    ResourceExhaustion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InjectionTiming {
    BeforeProposal,
    DuringVoting,
    AfterFinality,
    AtEpochBoundary,
    RandomInterval,
    Continuous,
    Bursty,
    Gradual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecoveryStrategy {
    AutoRecover,
    ManualIntervention,
    GracefulDegradation,
    Failover,
    Rollback,
    StateReconstruction,
    PeerAssisted,
    NoRecovery,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SeverityLevel {
    Minor,
    Moderate,
    Major,
    Critical,
    Catastrophic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FaultScenario {
    pub name: String,
    pub fault_type: FaultType,
    pub timing: InjectionTiming,
    pub severity: SeverityLevel,
    pub duration_slots: u64,
    pub affected_nodes_fraction: f64,
    pub expected_recovery: RecoveryStrategy,
    pub expected_impact: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FaultInjectionConfig {
    pub scenarios: Vec<FaultScenario>,
    pub total_nodes: usize,
    pub max_concurrent_faults: usize,
    pub byzantine_threshold: f64,
    pub min_recovery_time_slots: u64,
    pub require_safety_preservation: bool,
    pub require_liveness_recovery: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FaultInjectionValidationError {
    EmptyScenarios,
    NodesTooFew { count: usize },
    ConcurrentFaultsExceedNodes { faults: usize, nodes: usize },
    ByzantineThresholdOutOfRange { value: f64 },
    RecoveryTimeZero,
    AffectedFractionInvalid { scenario: String, value: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FaultInjectionStats {
    pub total_scenarios: usize,
    pub critical_scenarios: usize,
    pub avg_recovery_slots: f64,
    pub worst_case_impact: String,
    pub safety_violations: usize,
    pub liveness_violations: usize,
    pub coverage_score: f64,
    pub risk_summary: String,
}

pub fn default_fault_injection_config() -> FaultInjectionConfig {
    FaultInjectionConfig {
        scenarios: vec![
            FaultScenario {
                name: "Regional partition during committee voting".to_string(),
                fault_type: FaultType::NetworkPartition,
                timing: InjectionTiming::DuringVoting,
                severity: SeverityLevel::Major,
                duration_slots: 24,
                affected_nodes_fraction: 0.38,
                expected_recovery: RecoveryStrategy::PeerAssisted,
                expected_impact:
                    "Finality slows while isolated partitions rejoin and re-sync vote state."
                        .to_string(),
            },
            FaultScenario {
                name: "Equivocating proposer emits conflicting blocks".to_string(),
                fault_type: FaultType::EquivocationAttempt,
                timing: InjectionTiming::DuringVoting,
                severity: SeverityLevel::Critical,
                duration_slots: 8,
                affected_nodes_fraction: 0.14,
                expected_recovery: RecoveryStrategy::AutoRecover,
                expected_impact:
                    "Evidence propagation should slash offenders and preserve canonical safety."
                        .to_string(),
            },
            FaultScenario {
                name: "Burst network jitter delays attestations".to_string(),
                fault_type: FaultType::MessageDelay,
                timing: InjectionTiming::Bursty,
                severity: SeverityLevel::Moderate,
                duration_slots: 12,
                affected_nodes_fraction: 0.52,
                expected_recovery: RecoveryStrategy::GracefulDegradation,
                expected_impact:
                    "Head voting quality degrades temporarily and inclusion latency increases."
                        .to_string(),
            },
            FaultScenario {
                name: "Replay of stale attestation traffic".to_string(),
                fault_type: FaultType::ReplayAttack,
                timing: InjectionTiming::AfterFinality,
                severity: SeverityLevel::Major,
                duration_slots: 10,
                affected_nodes_fraction: 0.21,
                expected_recovery: RecoveryStrategy::StateReconstruction,
                expected_impact:
                    "Duplicate gossip should be filtered while stale signatures are rejected."
                        .to_string(),
            },
            FaultScenario {
                name: "Validator client crash at epoch transition".to_string(),
                fault_type: FaultType::CrashFailure,
                timing: InjectionTiming::AtEpochBoundary,
                severity: SeverityLevel::Critical,
                duration_slots: 16,
                affected_nodes_fraction: 0.11,
                expected_recovery: RecoveryStrategy::Failover,
                expected_impact:
                    "Missed duties spike until standby instances restore validator participation."
                        .to_string(),
            },
            FaultScenario {
                name: "Byzantine proposal storm with invalid payloads".to_string(),
                fault_type: FaultType::ByzantineProposal,
                timing: InjectionTiming::RandomInterval,
                severity: SeverityLevel::Catastrophic,
                duration_slots: 20,
                affected_nodes_fraction: 0.13,
                expected_recovery: RecoveryStrategy::ManualIntervention,
                expected_impact:
                    "Invalid proposal spam stresses fork-choice filters and incident workflows."
                        .to_string(),
            },
        ],
        total_nodes: 128,
        max_concurrent_faults: 3,
        byzantine_threshold: 0.33,
        min_recovery_time_slots: 4,
        require_safety_preservation: true,
        require_liveness_recovery: true,
    }
}

pub fn validate_fault_injection(
    config: &FaultInjectionConfig,
) -> Result<(), Vec<FaultInjectionValidationError>> {
    let mut errors = Vec::new();

    if config.scenarios.is_empty() {
        errors.push(FaultInjectionValidationError::EmptyScenarios);
    }

    if config.total_nodes == 0 {
        errors.push(FaultInjectionValidationError::NodesTooFew {
            count: config.total_nodes,
        });
    }

    if config.max_concurrent_faults > config.total_nodes {
        errors.push(FaultInjectionValidationError::ConcurrentFaultsExceedNodes {
            faults: config.max_concurrent_faults,
            nodes: config.total_nodes,
        });
    }

    if !(config.byzantine_threshold.is_finite()
        && config.byzantine_threshold >= 0.0
        && config.byzantine_threshold <= 1.0)
    {
        errors.push(
            FaultInjectionValidationError::ByzantineThresholdOutOfRange {
                value: config.byzantine_threshold,
            },
        );
    }

    if config.min_recovery_time_slots == 0 {
        errors.push(FaultInjectionValidationError::RecoveryTimeZero);
    }

    for scenario in &config.scenarios {
        if !(scenario.affected_nodes_fraction.is_finite()
            && scenario.affected_nodes_fraction >= 0.0
            && scenario.affected_nodes_fraction <= 1.0)
        {
            errors.push(FaultInjectionValidationError::AffectedFractionInvalid {
                scenario: scenario.name.clone(),
                value: scenario.affected_nodes_fraction,
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_fault_stats(config: &FaultInjectionConfig) -> FaultInjectionStats {
    let total_scenarios = config.scenarios.len();
    let critical_scenarios = config
        .scenarios
        .iter()
        .filter(|scenario| {
            scenario.severity == SeverityLevel::Critical
                || scenario.severity == SeverityLevel::Catastrophic
        })
        .count();

    let avg_recovery_slots = if total_scenarios == 0 {
        0.0
    } else {
        config
            .scenarios
            .iter()
            .map(|scenario| {
                let baseline = scenario.duration_slots.max(config.min_recovery_time_slots) as f64;
                baseline * recovery_multiplier(&scenario.expected_recovery)
            })
            .sum::<f64>()
            / total_scenarios as f64
    };

    let mut safety_violations = 0_usize;
    let mut liveness_violations = 0_usize;
    let mut worst_case_impact = "No scenarios configured.".to_string();
    let mut worst_case_score = -1.0_f64;

    for scenario in &config.scenarios {
        let severity_weight = severity_weight(&scenario.severity);
        let risk_score = severity_weight
            * scenario.affected_nodes_fraction
            * (1.0 + scenario.duration_slots as f64 / 64.0);

        if risk_score > worst_case_score {
            worst_case_score = risk_score;
            worst_case_impact = scenario.expected_impact.clone();
        }

        if is_safety_fault(&scenario.fault_type)
            && (scenario.affected_nodes_fraction > config.byzantine_threshold
                || scenario.severity == SeverityLevel::Critical
                || scenario.severity == SeverityLevel::Catastrophic)
        {
            safety_violations += 1;
        }

        if is_liveness_fault(&scenario.fault_type)
            && (scenario.affected_nodes_fraction >= 0.2
                || scenario.duration_slots >= config.min_recovery_time_slots.saturating_mul(2)
                || scenario.severity == SeverityLevel::Major
                || scenario.severity == SeverityLevel::Critical
                || scenario.severity == SeverityLevel::Catastrophic)
        {
            liveness_violations += 1;
        }
    }

    let mut covered_fault_types: Vec<FaultType> = Vec::new();
    let mut covered_timings: Vec<InjectionTiming> = Vec::new();
    for scenario in &config.scenarios {
        if !covered_fault_types.contains(&scenario.fault_type) {
            covered_fault_types.push(scenario.fault_type.clone());
        }
        if !covered_timings.contains(&scenario.timing) {
            covered_timings.push(scenario.timing.clone());
        }
    }

    let fault_coverage = covered_fault_types.len() as f64 / 10.0;
    let timing_coverage = covered_timings.len() as f64 / 8.0;
    let coverage_score = ((fault_coverage * 0.7) + (timing_coverage * 0.3)).min(1.0);

    let risk_summary = format!(
        "{critical_scenarios} critical/catastrophic scenarios, \
{safety_violations} safety alerts, {liveness_violations} liveness alerts, coverage {:.1}%.",
        coverage_score * 100.0
    );

    FaultInjectionStats {
        total_scenarios,
        critical_scenarios,
        avg_recovery_slots,
        worst_case_impact,
        safety_violations,
        liveness_violations,
        coverage_score,
        risk_summary,
    }
}

pub fn compute_fault_commitment(config: &FaultInjectionConfig) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ETH2077::FAULT_INJECTION::V1");
    let serialized = serde_json::to_vec(config).expect("fault injection config must serialize");
    hasher.update(serialized);

    let digest = hasher.finalize();
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn severity_weight(severity: &SeverityLevel) -> f64 {
    match severity {
        SeverityLevel::Minor => 0.2,
        SeverityLevel::Moderate => 0.4,
        SeverityLevel::Major => 0.65,
        SeverityLevel::Critical => 0.85,
        SeverityLevel::Catastrophic => 1.0,
    }
}

fn recovery_multiplier(strategy: &RecoveryStrategy) -> f64 {
    match strategy {
        RecoveryStrategy::AutoRecover => 1.0,
        RecoveryStrategy::GracefulDegradation => 1.1,
        RecoveryStrategy::Failover => 1.2,
        RecoveryStrategy::PeerAssisted => 1.3,
        RecoveryStrategy::StateReconstruction => 1.4,
        RecoveryStrategy::Rollback => 1.5,
        RecoveryStrategy::ManualIntervention => 1.8,
        RecoveryStrategy::NoRecovery => 2.0,
    }
}

fn is_safety_fault(fault_type: &FaultType) -> bool {
    matches!(
        fault_type,
        FaultType::EquivocationAttempt
            | FaultType::ReplayAttack
            | FaultType::ByzantineProposal
            | FaultType::TimestampSkew
    )
}

fn is_liveness_fault(fault_type: &FaultType) -> bool {
    matches!(
        fault_type,
        FaultType::NetworkPartition
            | FaultType::MessageDelay
            | FaultType::MessageDrop
            | FaultType::MessageReorder
            | FaultType::CrashFailure
            | FaultType::ResourceExhaustion
    )
}
