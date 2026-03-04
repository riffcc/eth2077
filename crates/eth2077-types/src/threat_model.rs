use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AdversaryCapability {
    NetworkDelay,
    MessageReorder,
    PartitionAttack,
    EclipseAttack,
    SybilAttack,
    ByzantineFault,
    MEVExtraction,
    LongRangeAttack,
    GrindingAttack,
    TimestampManipulation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrustAssumption {
    HonestMajority,
    SynchronousNetwork,
    PartialSynchrony,
    WeakSubjectivity,
    EconomicRationality,
    CryptographicHardness,
    RandomOracleModel,
    PKIAvailability,
    NoAdaptiveCorruption,
    BoundedResourceAdversary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClaimClass {
    SafetyProperty,
    LivenessProperty,
    FinalityGuarantee,
    CensorshipResistance,
    DataAvailability,
    ValidatorAccountability,
    ForkChoiceCorrectness,
    SlashingCompleteness,
    MerkleIntegrity,
    CrossChainSoundness,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThreatSeverity {
    Critical,
    High,
    Medium,
    Low,
    Informational,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreatEntry {
    pub name: String,
    pub capability: AdversaryCapability,
    pub severity: ThreatSeverity,
    pub affected_claims: Vec<ClaimClass>,
    pub mitigations: Vec<String>,
    pub residual_risk: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssumptionMapping {
    pub assumption: TrustAssumption,
    pub depends_on_claims: Vec<ClaimClass>,
    pub violation_impact: ThreatSeverity,
    pub testable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreatModelConfig {
    pub threats: Vec<ThreatEntry>,
    pub assumptions: Vec<AssumptionMapping>,
    pub byzantine_threshold: f64,
    pub network_model: String,
    pub max_acceptable_residual_risk: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThreatModelValidationError {
    EmptyThreats,
    EmptyAssumptions,
    ByzantineThresholdOutOfRange { value: f64 },
    ResidualRiskOutOfRange { value: f64 },
    OrphanedClaim { claim: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreatModelStats {
    pub total_threats: usize,
    pub critical_threats: usize,
    pub high_threats: usize,
    pub assumption_count: usize,
    pub testable_assumptions: usize,
    pub coverage_score: f64,
    pub avg_residual_risk: f64,
    pub uncovered_claims: Vec<String>,
    pub summary: String,
}

fn all_claim_classes() -> [ClaimClass; 10] {
    [
        ClaimClass::SafetyProperty,
        ClaimClass::LivenessProperty,
        ClaimClass::FinalityGuarantee,
        ClaimClass::CensorshipResistance,
        ClaimClass::DataAvailability,
        ClaimClass::ValidatorAccountability,
        ClaimClass::ForkChoiceCorrectness,
        ClaimClass::SlashingCompleteness,
        ClaimClass::MerkleIntegrity,
        ClaimClass::CrossChainSoundness,
    ]
}

fn collect_covered_claims(config: &ThreatModelConfig) -> Vec<ClaimClass> {
    let mut covered_claims = Vec::new();

    for threat in &config.threats {
        for claim in &threat.affected_claims {
            if !covered_claims.contains(claim) {
                covered_claims.push(claim.clone());
            }
        }
    }

    for assumption in &config.assumptions {
        for claim in &assumption.depends_on_claims {
            if !covered_claims.contains(claim) {
                covered_claims.push(claim.clone());
            }
        }
    }

    covered_claims
}

pub fn default_threat_model_config() -> ThreatModelConfig {
    ThreatModelConfig {
        threats: vec![
            ThreatEntry {
                name: "Sustained eclipse campaign on proposer set".to_string(),
                capability: AdversaryCapability::EclipseAttack,
                severity: ThreatSeverity::Critical,
                affected_claims: vec![
                    ClaimClass::ForkChoiceCorrectness,
                    ClaimClass::LivenessProperty,
                    ClaimClass::CensorshipResistance,
                ],
                mitigations: vec![
                    "Peer diversity requirements and anti-eclipse scoring".to_string(),
                    "Periodic peer rotation with stake-weighted sampling".to_string(),
                ],
                residual_risk: 0.27,
            },
            ThreatEntry {
                name: "Regional partition causing delayed quorum convergence".to_string(),
                capability: AdversaryCapability::PartitionAttack,
                severity: ThreatSeverity::High,
                affected_claims: vec![
                    ClaimClass::DataAvailability,
                    ClaimClass::LivenessProperty,
                    ClaimClass::FinalityGuarantee,
                ],
                mitigations: vec![
                    "Multi-region validator placement and relay mesh".to_string(),
                    "Adaptive timeout escalation under partition signals".to_string(),
                ],
                residual_risk: 0.22,
            },
            ThreatEntry {
                name: "MEV cartel censorship of specific transaction classes".to_string(),
                capability: AdversaryCapability::MEVExtraction,
                severity: ThreatSeverity::High,
                affected_claims: vec![
                    ClaimClass::CensorshipResistance,
                    ClaimClass::ValidatorAccountability,
                    ClaimClass::FinalityGuarantee,
                ],
                mitigations: vec![
                    "Encrypted mempool path with delayed reveal".to_string(),
                    "Builder diversity and inclusion-list enforcement".to_string(),
                ],
                residual_risk: 0.19,
            },
            ThreatEntry {
                name: "Byzantine validator coalition at super-minority scale".to_string(),
                capability: AdversaryCapability::ByzantineFault,
                severity: ThreatSeverity::Critical,
                affected_claims: vec![
                    ClaimClass::SafetyProperty,
                    ClaimClass::FinalityGuarantee,
                    ClaimClass::SlashingCompleteness,
                    ClaimClass::ForkChoiceCorrectness,
                ],
                mitigations: vec![
                    "Slashable equivocation evidence pipeline".to_string(),
                    "Rapid exit and inactivity leak controls".to_string(),
                ],
                residual_risk: 0.24,
            },
            ThreatEntry {
                name: "Long-range history rewrite against stale clients".to_string(),
                capability: AdversaryCapability::LongRangeAttack,
                severity: ThreatSeverity::High,
                affected_claims: vec![
                    ClaimClass::FinalityGuarantee,
                    ClaimClass::CrossChainSoundness,
                ],
                mitigations: vec![
                    "Weak subjectivity checkpoints and client freshness policy".to_string(),
                    "Checkpoint gossip signed by diverse operators".to_string(),
                ],
                residual_risk: 0.17,
            },
            ThreatEntry {
                name: "Hash grinding against Merkle inclusion constraints".to_string(),
                capability: AdversaryCapability::GrindingAttack,
                severity: ThreatSeverity::Medium,
                affected_claims: vec![
                    ClaimClass::MerkleIntegrity,
                    ClaimClass::CrossChainSoundness,
                    ClaimClass::ValidatorAccountability,
                ],
                mitigations: vec![
                    "Domain-separated hashing and strict tree format validation".to_string(),
                    "Cross-client proof vector differential tests".to_string(),
                ],
                residual_risk: 0.11,
            },
        ],
        assumptions: vec![
            AssumptionMapping {
                assumption: TrustAssumption::HonestMajority,
                depends_on_claims: vec![
                    ClaimClass::SafetyProperty,
                    ClaimClass::FinalityGuarantee,
                    ClaimClass::ForkChoiceCorrectness,
                ],
                violation_impact: ThreatSeverity::Critical,
                testable: true,
            },
            AssumptionMapping {
                assumption: TrustAssumption::PartialSynchrony,
                depends_on_claims: vec![
                    ClaimClass::LivenessProperty,
                    ClaimClass::FinalityGuarantee,
                ],
                violation_impact: ThreatSeverity::High,
                testable: true,
            },
            AssumptionMapping {
                assumption: TrustAssumption::WeakSubjectivity,
                depends_on_claims: vec![
                    ClaimClass::FinalityGuarantee,
                    ClaimClass::CrossChainSoundness,
                ],
                violation_impact: ThreatSeverity::High,
                testable: false,
            },
            AssumptionMapping {
                assumption: TrustAssumption::CryptographicHardness,
                depends_on_claims: vec![
                    ClaimClass::MerkleIntegrity,
                    ClaimClass::CrossChainSoundness,
                ],
                violation_impact: ThreatSeverity::Critical,
                testable: true,
            },
            AssumptionMapping {
                assumption: TrustAssumption::PKIAvailability,
                depends_on_claims: vec![
                    ClaimClass::ValidatorAccountability,
                    ClaimClass::CensorshipResistance,
                    ClaimClass::SlashingCompleteness,
                ],
                violation_impact: ThreatSeverity::Medium,
                testable: true,
            },
            AssumptionMapping {
                assumption: TrustAssumption::NoAdaptiveCorruption,
                depends_on_claims: vec![
                    ClaimClass::SlashingCompleteness,
                    ClaimClass::DataAvailability,
                    ClaimClass::SafetyProperty,
                ],
                violation_impact: ThreatSeverity::High,
                testable: false,
            },
        ],
        byzantine_threshold: 0.33,
        network_model: "Partially synchronous gossip with eventual delivery".to_string(),
        max_acceptable_residual_risk: 0.25,
    }
}

pub fn validate_threat_model(
    config: &ThreatModelConfig,
) -> Result<(), Vec<ThreatModelValidationError>> {
    let mut errors = Vec::new();

    if config.threats.is_empty() {
        errors.push(ThreatModelValidationError::EmptyThreats);
    }

    if config.assumptions.is_empty() {
        errors.push(ThreatModelValidationError::EmptyAssumptions);
    }

    if !(config.byzantine_threshold >= 0.0 && config.byzantine_threshold <= 1.0) {
        errors.push(ThreatModelValidationError::ByzantineThresholdOutOfRange {
            value: config.byzantine_threshold,
        });
    }

    if !(config.max_acceptable_residual_risk >= 0.0 && config.max_acceptable_residual_risk <= 1.0) {
        errors.push(ThreatModelValidationError::ResidualRiskOutOfRange {
            value: config.max_acceptable_residual_risk,
        });
    }

    for threat in &config.threats {
        if !(threat.residual_risk >= 0.0 && threat.residual_risk <= 1.0) {
            errors.push(ThreatModelValidationError::ResidualRiskOutOfRange {
                value: threat.residual_risk,
            });
        }
    }

    let covered_claims = collect_covered_claims(config);
    for claim in all_claim_classes() {
        if !covered_claims.contains(&claim) {
            errors.push(ThreatModelValidationError::OrphanedClaim {
                claim: format!("{claim:?}"),
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_threat_model_stats(config: &ThreatModelConfig) -> ThreatModelStats {
    let total_threats = config.threats.len();
    let critical_threats = config
        .threats
        .iter()
        .filter(|t| t.severity == ThreatSeverity::Critical)
        .count();
    let high_threats = config
        .threats
        .iter()
        .filter(|t| t.severity == ThreatSeverity::High)
        .count();
    let assumption_count = config.assumptions.len();
    let testable_assumptions = config.assumptions.iter().filter(|a| a.testable).count();

    let avg_residual_risk = if total_threats == 0 {
        0.0
    } else {
        config
            .threats
            .iter()
            .map(|threat| threat.residual_risk)
            .sum::<f64>()
            / total_threats as f64
    };

    let covered_claims = collect_covered_claims(config);
    let all_claims = all_claim_classes();
    let uncovered_claims: Vec<String> = all_claims
        .iter()
        .filter(|claim| !covered_claims.contains(claim))
        .map(|claim| format!("{claim:?}"))
        .collect();

    let covered_count = all_claims.len() - uncovered_claims.len();
    let coverage_score = covered_count as f64 / all_claims.len() as f64;

    let summary = format!(
        "{total_threats} threats ({critical_threats} critical, {high_threats} high), \
{assumption_count} assumptions ({testable_assumptions} testable), \
coverage {:.1}%, average residual risk {:.3}.",
        coverage_score * 100.0,
        avg_residual_risk
    );

    ThreatModelStats {
        total_threats,
        critical_threats,
        high_threats,
        assumption_count,
        testable_assumptions,
        coverage_score,
        avg_residual_risk,
        uncovered_claims,
        summary,
    }
}

pub fn compute_threat_commitment(config: &ThreatModelConfig) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ETH2077::THREAT_MODEL::V1");
    let serialized = serde_json::to_vec(config).expect("threat model config must serialize");
    hasher.update(serialized);

    let digest = hasher.finalize();
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest);
    out
}
