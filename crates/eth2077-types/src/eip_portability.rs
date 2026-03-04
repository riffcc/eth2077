use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ClientArchitecture {
    Geth,
    Reth,
    Nethermind,
    Besu,
    Erigon,
    Eth2077Citadel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PortabilityRisk {
    None,
    TightCoupling,        // EIP assumes specific internal architecture
    UndefinedBehavior,    // Spec leaves behavior undefined
    PerformanceSensitive, // Implementation perf-critical
    ConsensusDeviation,   // Risk of consensus divergence
    HardwareDependent,    // Assumes specific hardware features
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PortabilityAssessment {
    pub eip_number: u64,
    pub target_client: ClientArchitecture,
    pub complexity_score: f64, // 0.0-1.0
    pub dependency_count: usize,
    pub risks: Vec<PortabilityRisk>,
    pub estimated_dev_days: f64,
    pub requires_consensus_changes: bool,
    pub requires_networking_changes: bool,
    pub test_coverage_available: f64, // 0.0-1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PortabilityConfig {
    pub target_client: ClientArchitecture,
    pub max_acceptable_complexity: f64,
    pub min_test_coverage: f64,
    pub weight_complexity: f64,
    pub weight_risk: f64,
    pub weight_dependencies: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PortabilityError {
    EmptyAssessments,
    InvalidComplexity {
        eip: u64,
        score: String,
    },
    DuplicateEip {
        eip: u64,
    },
    MissingRiskAnalysis {
        eip: u64,
    },
    InsufficientTestCoverage {
        eip: u64,
        coverage: String,
        required: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PortabilityStats {
    pub total_eips_assessed: usize,
    pub avg_complexity: f64,
    pub total_dev_days: f64,
    pub high_risk_count: usize,
    pub consensus_change_count: usize,
    pub risk_distribution: Vec<(String, usize)>,
    pub client_readiness_score: f64,
}

pub fn default_portability_config() -> PortabilityConfig {
    PortabilityConfig {
        target_client: ClientArchitecture::Eth2077Citadel,
        max_acceptable_complexity: 0.7,
        min_test_coverage: 0.6,
        weight_complexity: 1.0 / 3.0,
        weight_risk: 1.0 / 3.0,
        weight_dependencies: 1.0 / 3.0,
    }
}

pub fn validate_assessments(
    assessments: &[PortabilityAssessment],
    config: &PortabilityConfig,
) -> Result<(), Vec<PortabilityError>> {
    let mut errors = Vec::new();
    if assessments.is_empty() {
        errors.push(PortabilityError::EmptyAssessments);
        return Err(errors);
    }

    let mut seen_eips: HashMap<u64, usize> = HashMap::new();

    for assessment in assessments {
        if !is_unit_interval(assessment.complexity_score) {
            errors.push(PortabilityError::InvalidComplexity {
                eip: assessment.eip_number,
                score: format_float(assessment.complexity_score),
            });
        }

        if seen_eips.insert(assessment.eip_number, 1).is_some() {
            errors.push(PortabilityError::DuplicateEip {
                eip: assessment.eip_number,
            });
        }

        if assessment.risks.is_empty() {
            errors.push(PortabilityError::MissingRiskAnalysis {
                eip: assessment.eip_number,
            });
        }

        if !is_unit_interval(assessment.test_coverage_available)
            || assessment.test_coverage_available < config.min_test_coverage
        {
            errors.push(PortabilityError::InsufficientTestCoverage {
                eip: assessment.eip_number,
                coverage: format_float(assessment.test_coverage_available),
                required: format_float(config.min_test_coverage),
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_portability_stats(assessments: &[PortabilityAssessment]) -> PortabilityStats {
    if assessments.is_empty() {
        return PortabilityStats {
            total_eips_assessed: 0,
            avg_complexity: 0.0,
            total_dev_days: 0.0,
            high_risk_count: 0,
            consensus_change_count: 0,
            risk_distribution: Vec::new(),
            client_readiness_score: 0.0,
        };
    }

    let mut complexity_sum = 0.0;
    let mut total_dev_days = 0.0;
    let mut high_risk_count = 0usize;
    let mut consensus_change_count = 0usize;
    let mut risk_counts: HashMap<String, usize> = HashMap::new();

    for assessment in assessments {
        complexity_sum += clamp_unit(assessment.complexity_score);
        total_dev_days += assessment.estimated_dev_days.max(0.0);

        if is_high_risk(assessment) {
            high_risk_count = high_risk_count.saturating_add(1);
        }
        if assessment.requires_consensus_changes {
            consensus_change_count = consensus_change_count.saturating_add(1);
        }

        for risk in &assessment.risks {
            let key = risk_name(*risk).to_owned();
            let entry = risk_counts.entry(key).or_insert(0);
            *entry = entry.saturating_add(1);
        }
    }

    let mut risk_distribution: Vec<(String, usize)> = risk_counts.into_iter().collect();
    risk_distribution.sort_by(|a, b| a.0.cmp(&b.0));

    let mut score_config = default_portability_config();
    score_config.target_client = assessments[0].target_client;
    let readiness_sum: f64 = assessments
        .iter()
        .map(|assessment| score_portability(assessment, &score_config))
        .sum();
    let client_readiness_score = clamp_unit(readiness_sum / assessments.len() as f64);

    PortabilityStats {
        total_eips_assessed: assessments.len(),
        avg_complexity: complexity_sum / assessments.len() as f64,
        total_dev_days,
        high_risk_count,
        consensus_change_count,
        risk_distribution,
        client_readiness_score,
    }
}

pub fn score_portability(assessment: &PortabilityAssessment, config: &PortabilityConfig) -> f64 {
    let complexity_component = 1.0 - clamp_unit(assessment.complexity_score);
    let risk_component = 1.0 - assessment_risk_load(assessment);
    let dependency_component = 1.0 / (1.0 + (assessment.dependency_count as f64 / 5.0));

    let (weight_complexity, weight_risk, weight_dependencies) = normalized_weights(config);
    let base_score = complexity_component * weight_complexity
        + risk_component * weight_risk
        + dependency_component * weight_dependencies;

    let coverage_factor = clamp_unit(assessment.test_coverage_available);
    let architecture_factor = if assessment.target_client == config.target_client {
        1.0
    } else {
        0.9
    };
    let mut change_factor = 1.0;
    if assessment.requires_consensus_changes {
        change_factor -= 0.15;
    }
    if assessment.requires_networking_changes {
        change_factor -= 0.08;
    }

    clamp_unit(base_score * coverage_factor * architecture_factor * change_factor.max(0.0))
}

pub fn prioritize_eips(assessments: &mut [PortabilityAssessment], config: &PortabilityConfig) {
    assessments.sort_by(|a, b| {
        let b_score = score_portability(b, config);
        let a_score = score_portability(a, config);
        b_score
            .partial_cmp(&a_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.eip_number.cmp(&b.eip_number))
    });
}

pub fn estimate_total_effort(assessments: &[PortabilityAssessment]) -> f64 {
    assessments
        .iter()
        .map(|assessment| {
            let base_days = assessment.estimated_dev_days.max(0.0);
            let risk_multiplier = 1.0 + (assessment_risk_load(assessment) * 0.8);
            let consensus_multiplier = if assessment.requires_consensus_changes {
                1.35
            } else {
                1.0
            };
            let networking_multiplier = if assessment.requires_networking_changes {
                1.15
            } else {
                1.0
            };
            let dependency_multiplier = 1.0 + ((assessment.dependency_count as f64).ln_1p() * 0.05);
            base_days
                * risk_multiplier
                * consensus_multiplier
                * networking_multiplier
                * dependency_multiplier
        })
        .sum()
}

pub fn compute_portability_commitment(assessments: &[PortabilityAssessment]) -> [u8; 32] {
    let mut ordered: Vec<&PortabilityAssessment> = assessments.iter().collect();
    ordered.sort_by(|a, b| {
        a.eip_number
            .cmp(&b.eip_number)
            .then_with(|| {
                client_arch_discriminant(a.target_client)
                    .cmp(&client_arch_discriminant(b.target_client))
            })
            .then_with(|| a.dependency_count.cmp(&b.dependency_count))
    });

    let mut hasher = Sha256::new();
    hasher.update((ordered.len() as u64).to_be_bytes());

    for assessment in ordered {
        hasher.update(assessment.eip_number.to_be_bytes());
        hasher.update([client_arch_discriminant(assessment.target_client)]);
        hasher.update(assessment.complexity_score.to_bits().to_be_bytes());
        hasher.update((assessment.dependency_count as u64).to_be_bytes());

        let mut ordered_risks = assessment.risks.clone();
        ordered_risks.sort_by_key(|risk| risk_discriminant(*risk));
        hasher.update((ordered_risks.len() as u64).to_be_bytes());
        for risk in ordered_risks {
            hasher.update([risk_discriminant(risk)]);
        }

        hasher.update(assessment.estimated_dev_days.to_bits().to_be_bytes());
        hasher.update([assessment.requires_consensus_changes as u8]);
        hasher.update([assessment.requires_networking_changes as u8]);
        hasher.update(assessment.test_coverage_available.to_bits().to_be_bytes());
    }

    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn is_unit_interval(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn clamp_unit(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.clamp(0.0, 1.0)
}

fn format_float(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.3}")
    } else if value.is_nan() {
        "NaN".to_owned()
    } else if value.is_sign_positive() {
        "inf".to_owned()
    } else {
        "-inf".to_owned()
    }
}

fn risk_name(risk: PortabilityRisk) -> &'static str {
    match risk {
        PortabilityRisk::None => "None",
        PortabilityRisk::TightCoupling => "TightCoupling",
        PortabilityRisk::UndefinedBehavior => "UndefinedBehavior",
        PortabilityRisk::PerformanceSensitive => "PerformanceSensitive",
        PortabilityRisk::ConsensusDeviation => "ConsensusDeviation",
        PortabilityRisk::HardwareDependent => "HardwareDependent",
    }
}

fn risk_penalty(risk: PortabilityRisk) -> f64 {
    match risk {
        PortabilityRisk::None => 0.0,
        PortabilityRisk::PerformanceSensitive => 0.3,
        PortabilityRisk::TightCoupling => 0.5,
        PortabilityRisk::HardwareDependent => 0.55,
        PortabilityRisk::UndefinedBehavior => 0.7,
        PortabilityRisk::ConsensusDeviation => 0.9,
    }
}

fn assessment_risk_load(assessment: &PortabilityAssessment) -> f64 {
    if assessment.risks.is_empty() {
        return 1.0;
    }

    let total_penalty: f64 = assessment
        .risks
        .iter()
        .map(|risk| risk_penalty(*risk))
        .sum();
    clamp_unit(total_penalty / assessment.risks.len() as f64)
}

fn is_high_risk(assessment: &PortabilityAssessment) -> bool {
    assessment.requires_consensus_changes
        || assessment
            .risks
            .iter()
            .any(|risk| risk_penalty(*risk) >= 0.7)
}

fn normalized_weights(config: &PortabilityConfig) -> (f64, f64, f64) {
    let complexity = config.weight_complexity.max(0.0);
    let risk = config.weight_risk.max(0.0);
    let dependencies = config.weight_dependencies.max(0.0);
    let sum = complexity + risk + dependencies;

    if sum == 0.0 {
        (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0)
    } else {
        (complexity / sum, risk / sum, dependencies / sum)
    }
}

fn client_arch_discriminant(architecture: ClientArchitecture) -> u8 {
    match architecture {
        ClientArchitecture::Geth => 0,
        ClientArchitecture::Reth => 1,
        ClientArchitecture::Nethermind => 2,
        ClientArchitecture::Besu => 3,
        ClientArchitecture::Erigon => 4,
        ClientArchitecture::Eth2077Citadel => 5,
    }
}

fn risk_discriminant(risk: PortabilityRisk) -> u8 {
    match risk {
        PortabilityRisk::None => 0,
        PortabilityRisk::TightCoupling => 1,
        PortabilityRisk::UndefinedBehavior => 2,
        PortabilityRisk::PerformanceSensitive => 3,
        PortabilityRisk::ConsensusDeviation => 4,
        PortabilityRisk::HardwareDependent => 5,
    }
}
