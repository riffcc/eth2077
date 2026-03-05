use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ExecutionTarget {
    EvmBytecode,
    RiscV32,
    RiscV64,
    EvmInRiscV,
    DualMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MigrationPhase {
    Phase0Research,
    Phase1DualDeploy,
    Phase2EvmWrapped,
    Phase3EvmDeprecated,
    Phase4RiscVNative,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArchitectureComparison {
    pub target: ExecutionTarget,
    pub zk_proving_cost_relative: f64,
    pub native_execution_speed_relative: f64,
    pub compiler_maturity: f64,
    pub backwards_compatible: bool,
    pub syscall_overhead_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MigrationRisk {
    pub phase: MigrationPhase,
    pub risk_description: String,
    pub severity: f64,
    pub mitigation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MigrationStrategy {
    pub phases: Vec<MigrationPhase>,
    pub estimated_years_per_phase: Vec<f64>,
    pub total_estimated_years: f64,
    pub risks: Vec<MigrationRisk>,
    pub breaking_changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MigrationValidationError {
    EmptyPhases,
    PhasesOutOfOrder,
    DuplicatePhase,
    MismatchedPhaseYears { phases: usize, years: usize },
    UnrealisticTimeline { total_years: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MigrationEvalStats {
    pub total_phases: usize,
    pub total_years: f64,
    pub total_risks: usize,
    pub avg_risk_severity: f64,
    pub max_zk_improvement: f64,
    pub breaking_change_count: usize,
}

fn phase_index(phase: MigrationPhase) -> usize {
    match phase {
        MigrationPhase::Phase0Research => 0,
        MigrationPhase::Phase1DualDeploy => 1,
        MigrationPhase::Phase2EvmWrapped => 2,
        MigrationPhase::Phase3EvmDeprecated => 3,
        MigrationPhase::Phase4RiscVNative => 4,
    }
}

pub fn default_evm_comparison() -> ArchitectureComparison {
    ArchitectureComparison {
        target: ExecutionTarget::EvmBytecode,
        zk_proving_cost_relative: 1.0,
        native_execution_speed_relative: 1.0,
        compiler_maturity: 1.0,
        backwards_compatible: true,
        syscall_overhead_ns: 0,
    }
}

pub fn default_riscv_comparison() -> ArchitectureComparison {
    ArchitectureComparison {
        target: ExecutionTarget::RiscV64,
        zk_proving_cost_relative: 0.01,
        native_execution_speed_relative: 3.0,
        compiler_maturity: 0.3,
        backwards_compatible: false,
        syscall_overhead_ns: 50,
    }
}

pub fn default_migration_strategy() -> MigrationStrategy {
    let phases = vec![
        MigrationPhase::Phase0Research,
        MigrationPhase::Phase1DualDeploy,
        MigrationPhase::Phase2EvmWrapped,
        MigrationPhase::Phase3EvmDeprecated,
        MigrationPhase::Phase4RiscVNative,
    ];
    let estimated_years_per_phase = vec![1.0, 2.0, 2.0, 3.0, 2.0];
    let total_estimated_years = 10.0;
    let risks = vec![
        MigrationRisk {
            phase: MigrationPhase::Phase1DualDeploy,
            risk_description: "Dual runtime divergence could create consensus edge-cases.".into(),
            severity: 0.6,
            mitigation: "Use differential execution traces and client conformance test vectors.".into(),
        },
        MigrationRisk {
            phase: MigrationPhase::Phase2EvmWrapped,
            risk_description: "Wrapper semantics may mis-handle EVM corner-cases and precompiles.".into(),
            severity: 0.8,
            mitigation: "Ship compatibility suites and phase-gate legacy opcode parity.".into(),
        },
        MigrationRisk {
            phase: MigrationPhase::Phase3EvmDeprecated,
            risk_description: "Developer tooling lag can slow migration of production contracts.".into(),
            severity: 0.4,
            mitigation: "Fund compiler/tooling grants and provide long-lived migration SDKs.".into(),
        },
    ];
    let breaking_changes = vec![
        "Contract deployment defaults to RISC-V object format.".into(),
        "Gas accounting changes for memory and syscall-heavy workloads.".into(),
        "Low-level EVM bytecode introspection no longer available natively.".into(),
        "Legacy opcodes only available through compatibility wrapper entrypoints.".into(),
    ];

    MigrationStrategy {
        phases,
        estimated_years_per_phase,
        total_estimated_years,
        risks,
        breaking_changes,
    }
}

pub fn validate_migration_strategy(
    strategy: &MigrationStrategy,
) -> Result<(), Vec<MigrationValidationError>> {
    let mut errors = Vec::new();

    if strategy.phases.is_empty() {
        errors.push(MigrationValidationError::EmptyPhases);
    }

    if strategy.phases.len() != strategy.estimated_years_per_phase.len() {
        errors.push(MigrationValidationError::MismatchedPhaseYears {
            phases: strategy.phases.len(),
            years: strategy.estimated_years_per_phase.len(),
        });
    }

    let mut seen_counts: HashMap<MigrationPhase, usize> = HashMap::new();
    let mut previous_phase: Option<usize> = None;
    let mut out_of_order = false;

    for phase in &strategy.phases {
        let count = seen_counts.entry(*phase).or_insert(0);
        *count += 1;
        if *count > 1 {
            errors.push(MigrationValidationError::DuplicatePhase);
        }

        let current = phase_index(*phase);
        if let Some(previous) = previous_phase {
            if current < previous {
                out_of_order = true;
            }
        }
        previous_phase = Some(current);
    }

    if out_of_order {
        errors.push(MigrationValidationError::PhasesOutOfOrder);
    }

    if strategy.total_estimated_years >= 20.0 {
        errors.push(MigrationValidationError::UnrealisticTimeline {
            total_years: format!("{:.2}", strategy.total_estimated_years),
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_migration_stats(
    strategy: &MigrationStrategy,
    comparisons: &[ArchitectureComparison],
) -> MigrationEvalStats {
    let total_risks = strategy.risks.len();
    let avg_risk_severity = if total_risks == 0 {
        0.0
    } else {
        strategy.risks.iter().map(|risk| risk.severity).sum::<f64>() / total_risks as f64
    };

    let mut by_target: HashMap<ExecutionTarget, &ArchitectureComparison> = HashMap::new();
    for comparison in comparisons {
        by_target
            .entry(comparison.target)
            .and_modify(|existing| {
                if comparison.zk_proving_cost_relative < existing.zk_proving_cost_relative {
                    *existing = comparison;
                }
            })
            .or_insert(comparison);
    }

    let baseline = by_target
        .get(&ExecutionTarget::EvmBytecode)
        .copied()
        .or_else(|| comparisons.first());
    let max_zk_improvement = if let Some(current) = baseline {
        by_target
            .values()
            .copied()
            .map(|target| estimate_zk_improvement(current, target))
            .fold(1.0, f64::max)
    } else {
        1.0
    };

    MigrationEvalStats {
        total_phases: strategy.phases.len(),
        total_years: strategy.total_estimated_years,
        total_risks,
        avg_risk_severity,
        max_zk_improvement,
        breaking_change_count: strategy.breaking_changes.len(),
    }
}

pub fn estimate_zk_improvement(
    current: &ArchitectureComparison,
    target: &ArchitectureComparison,
) -> f64 {
    if target.zk_proving_cost_relative <= 0.0 {
        f64::INFINITY
    } else {
        current.zk_proving_cost_relative / target.zk_proving_cost_relative
    }
}
