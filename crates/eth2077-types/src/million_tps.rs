use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BottleneckLayer {
    Execution,
    Ingress,
    Consensus,
    DataAvailability,
    Networking,
    Storage,
    Verification,
    CrossLayer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScalingStrategy {
    VerticalScale,
    HorizontalShard,
    PipelinedExecution,
    BatchOptimization,
    CompressionGains,
    ProtocolSimplification,
    HardwareAcceleration,
    ParallelVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MilestoneGate {
    BenchmarkPassed,
    FormalProofComplete,
    SecurityAuditClear,
    InteropVerified,
    StressTestPassed,
    RegressionFree,
    PeerReviewed,
    ProductionReady,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TpsMilestone {
    pub name: String,
    pub target_tps: u64,
    pub bottleneck: BottleneckLayer,
    pub strategy: ScalingStrategy,
    pub required_gates: Vec<MilestoneGate>,
    pub gates_passed: Vec<MilestoneGate>,
    pub reproducible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MillionTpsConfig {
    pub milestones: Vec<TpsMilestone>,
    pub l1_target_tps: u64,
    pub l2_target_tps: u64,
    pub combined_target_tps: u64,
    pub max_latency_ms: f64,
    pub min_finality_confidence: f64,
    pub benchmark_reproducibility_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MillionTpsValidationError {
    EmptyMilestones,
    TargetMismatch { l1: u64, l2: u64, combined: u64 },
    LatencyNonPositive { value: f64 },
    ConfidenceOutOfRange { value: f64 },
    ReproducibilityOutOfRange { value: f64 },
    MilestoneWithoutGates { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MillionTpsStats {
    pub total_milestones: usize,
    pub milestones_complete: usize,
    pub overall_progress: f64,
    pub l1_achieved_tps: u64,
    pub l2_achieved_tps: u64,
    pub primary_bottleneck: String,
    pub secondary_bottleneck: String,
    pub gate_coverage: f64,
    pub reproducibility_score: f64,
    pub risk_assessment: String,
}

pub fn default_million_tps_config() -> MillionTpsConfig {
    MillionTpsConfig {
        milestones: vec![
            TpsMilestone {
                name: "10K TPS: Executor Headroom".to_string(),
                target_tps: 10_000,
                bottleneck: BottleneckLayer::Execution,
                strategy: ScalingStrategy::VerticalScale,
                required_gates: vec![
                    MilestoneGate::BenchmarkPassed,
                    MilestoneGate::RegressionFree,
                    MilestoneGate::PeerReviewed,
                ],
                gates_passed: vec![
                    MilestoneGate::BenchmarkPassed,
                    MilestoneGate::RegressionFree,
                ],
                reproducible: true,
            },
            TpsMilestone {
                name: "50K TPS: Ingress Saturation".to_string(),
                target_tps: 50_000,
                bottleneck: BottleneckLayer::Ingress,
                strategy: ScalingStrategy::PipelinedExecution,
                required_gates: vec![
                    MilestoneGate::BenchmarkPassed,
                    MilestoneGate::StressTestPassed,
                    MilestoneGate::RegressionFree,
                ],
                gates_passed: vec![MilestoneGate::BenchmarkPassed],
                reproducible: true,
            },
            TpsMilestone {
                name: "100K TPS: Consensus Tightening".to_string(),
                target_tps: 100_000,
                bottleneck: BottleneckLayer::Consensus,
                strategy: ScalingStrategy::ProtocolSimplification,
                required_gates: vec![
                    MilestoneGate::BenchmarkPassed,
                    MilestoneGate::FormalProofComplete,
                    MilestoneGate::InteropVerified,
                    MilestoneGate::RegressionFree,
                ],
                gates_passed: vec![
                    MilestoneGate::BenchmarkPassed,
                    MilestoneGate::InteropVerified,
                ],
                reproducible: false,
            },
            TpsMilestone {
                name: "500K TPS: DA Compression".to_string(),
                target_tps: 500_000,
                bottleneck: BottleneckLayer::DataAvailability,
                strategy: ScalingStrategy::CompressionGains,
                required_gates: vec![
                    MilestoneGate::BenchmarkPassed,
                    MilestoneGate::FormalProofComplete,
                    MilestoneGate::SecurityAuditClear,
                    MilestoneGate::StressTestPassed,
                ],
                gates_passed: vec![MilestoneGate::BenchmarkPassed],
                reproducible: false,
            },
            TpsMilestone {
                name: "1M TPS: Cross-Layer Fusion".to_string(),
                target_tps: 1_000_000,
                bottleneck: BottleneckLayer::CrossLayer,
                strategy: ScalingStrategy::HorizontalShard,
                required_gates: vec![
                    MilestoneGate::BenchmarkPassed,
                    MilestoneGate::FormalProofComplete,
                    MilestoneGate::SecurityAuditClear,
                    MilestoneGate::InteropVerified,
                    MilestoneGate::StressTestPassed,
                    MilestoneGate::RegressionFree,
                    MilestoneGate::PeerReviewed,
                    MilestoneGate::ProductionReady,
                ],
                gates_passed: vec![MilestoneGate::BenchmarkPassed],
                reproducible: false,
            },
        ],
        l1_target_tps: 120_000,
        l2_target_tps: 880_000,
        combined_target_tps: 1_000_000,
        max_latency_ms: 250.0,
        min_finality_confidence: 0.98,
        benchmark_reproducibility_threshold: 0.90,
    }
}

pub fn validate_million_tps(
    config: &MillionTpsConfig,
) -> Result<(), Vec<MillionTpsValidationError>> {
    let mut errors = Vec::new();

    if config.milestones.is_empty() {
        errors.push(MillionTpsValidationError::EmptyMilestones);
    }

    if (config.l1_target_tps as u128 + config.l2_target_tps as u128)
        > config.combined_target_tps as u128
    {
        errors.push(MillionTpsValidationError::TargetMismatch {
            l1: config.l1_target_tps,
            l2: config.l2_target_tps,
            combined: config.combined_target_tps,
        });
    }

    if !config.max_latency_ms.is_finite() || config.max_latency_ms <= 0.0 {
        errors.push(MillionTpsValidationError::LatencyNonPositive {
            value: config.max_latency_ms,
        });
    }

    if !config.min_finality_confidence.is_finite()
        || !(0.0..=1.0).contains(&config.min_finality_confidence)
    {
        errors.push(MillionTpsValidationError::ConfidenceOutOfRange {
            value: config.min_finality_confidence,
        });
    }

    if !config.benchmark_reproducibility_threshold.is_finite()
        || !(0.0..=1.0).contains(&config.benchmark_reproducibility_threshold)
    {
        errors.push(MillionTpsValidationError::ReproducibilityOutOfRange {
            value: config.benchmark_reproducibility_threshold,
        });
    }

    for milestone in &config.milestones {
        if milestone.required_gates.is_empty() {
            errors.push(MillionTpsValidationError::MilestoneWithoutGates {
                name: milestone.name.clone(),
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_million_tps_stats(config: &MillionTpsConfig) -> MillionTpsStats {
    let total_milestones = config.milestones.len();
    let mut milestones_complete = 0_usize;
    let mut total_required_gates = 0_usize;
    let mut total_passed_required_gates = 0_usize;
    let mut reproducible_count = 0_usize;
    let mut weighted_progress_numerator = 0.0_f64;
    let mut weighted_progress_denominator = 0_u64;
    let mut bottleneck_counts: Vec<(BottleneckLayer, usize)> = Vec::new();

    for milestone in &config.milestones {
        if milestone.reproducible {
            reproducible_count += 1;
        }

        let mut unique_required = Vec::new();
        for gate in &milestone.required_gates {
            if !unique_required.contains(gate) {
                unique_required.push(gate.clone());
            }
        }

        let mut passed_required = 0_usize;
        for gate in &unique_required {
            if milestone.gates_passed.contains(gate) {
                passed_required += 1;
            }
        }

        if !unique_required.is_empty() && passed_required == unique_required.len() {
            milestones_complete += 1;
        }

        total_required_gates += unique_required.len();
        total_passed_required_gates += passed_required;

        let milestone_progress = if unique_required.is_empty() {
            0.0
        } else {
            passed_required as f64 / unique_required.len() as f64
        };
        weighted_progress_numerator += milestone_progress * milestone.target_tps as f64;
        weighted_progress_denominator =
            weighted_progress_denominator.saturating_add(milestone.target_tps);

        let mut found = false;
        for (layer, count) in &mut bottleneck_counts {
            if *layer == milestone.bottleneck {
                *count += 1;
                found = true;
                break;
            }
        }
        if !found {
            bottleneck_counts.push((milestone.bottleneck.clone(), 1));
        }
    }

    bottleneck_counts.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| bottleneck_label(&a.0).cmp(bottleneck_label(&b.0)))
    });

    let primary_bottleneck = bottleneck_counts
        .first()
        .map(|(layer, _)| bottleneck_label(layer).to_string())
        .unwrap_or_else(|| "None".to_string());
    let secondary_bottleneck = bottleneck_counts
        .get(1)
        .map(|(layer, _)| bottleneck_label(layer).to_string())
        .unwrap_or_else(|| "None".to_string());

    let gate_coverage = if total_required_gates == 0 {
        0.0
    } else {
        (total_passed_required_gates as f64 / total_required_gates as f64).clamp(0.0, 1.0)
    };

    let overall_progress = if weighted_progress_denominator == 0 {
        0.0
    } else {
        (weighted_progress_numerator / weighted_progress_denominator as f64).clamp(0.0, 1.0)
    };

    let reproducibility_score = if total_milestones == 0 {
        0.0
    } else {
        reproducible_count as f64 / total_milestones as f64
    };

    let l1_achieved_tps = (config.l1_target_tps as f64 * overall_progress).round() as u64;
    let l2_achieved_tps = (config.l2_target_tps as f64 * overall_progress).round() as u64;
    let risk_assessment = assess_risk(
        overall_progress,
        gate_coverage,
        reproducibility_score,
        config.benchmark_reproducibility_threshold,
    );

    MillionTpsStats {
        total_milestones,
        milestones_complete,
        overall_progress,
        l1_achieved_tps,
        l2_achieved_tps,
        primary_bottleneck,
        secondary_bottleneck,
        gate_coverage,
        reproducibility_score,
        risk_assessment,
    }
}

pub fn compute_million_tps_commitment(config: &MillionTpsConfig) -> [u8; 32] {
    let payload =
        serde_json::to_vec(config).expect("MillionTpsConfig must serialize deterministically");
    let mut hasher = Sha256::new();
    hasher.update(b"ETH2077::MILLION_TPS::V1");
    hasher.update(payload);

    let digest = hasher.finalize();
    let mut commitment = [0_u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn bottleneck_label(layer: &BottleneckLayer) -> &'static str {
    match layer {
        BottleneckLayer::Execution => "Execution",
        BottleneckLayer::Ingress => "Ingress",
        BottleneckLayer::Consensus => "Consensus",
        BottleneckLayer::DataAvailability => "DataAvailability",
        BottleneckLayer::Networking => "Networking",
        BottleneckLayer::Storage => "Storage",
        BottleneckLayer::Verification => "Verification",
        BottleneckLayer::CrossLayer => "CrossLayer",
    }
}

fn assess_risk(
    overall_progress: f64,
    gate_coverage: f64,
    reproducibility_score: f64,
    reproducibility_threshold: f64,
) -> String {
    if overall_progress >= 0.90
        && gate_coverage >= 0.90
        && reproducibility_score >= reproducibility_threshold
    {
        "Low".to_string()
    } else if overall_progress >= 0.60
        && gate_coverage >= 0.60
        && reproducibility_score >= (reproducibility_threshold * 0.80)
    {
        "Moderate".to_string()
    } else {
        "High".to_string()
    }
}
