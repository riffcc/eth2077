use eth2077_types::riscv_migration::{
    compute_migration_stats, default_evm_comparison, default_migration_strategy,
    default_riscv_comparison, estimate_zk_improvement, validate_migration_strategy,
    ExecutionTarget, MigrationPhase, MigrationValidationError,
};

#[test]
fn default_evm_comparison_values() {
    let comparison = default_evm_comparison();

    assert_eq!(comparison.target, ExecutionTarget::EvmBytecode);
    assert_eq!(comparison.zk_proving_cost_relative, 1.0);
    assert_eq!(comparison.native_execution_speed_relative, 1.0);
    assert_eq!(comparison.compiler_maturity, 1.0);
    assert!(comparison.backwards_compatible);
    assert_eq!(comparison.syscall_overhead_ns, 0);
}

#[test]
fn default_riscv_comparison_values() {
    let comparison = default_riscv_comparison();

    assert_eq!(comparison.target, ExecutionTarget::RiscV64);
    assert_eq!(comparison.zk_proving_cost_relative, 0.01);
    assert_eq!(comparison.native_execution_speed_relative, 3.0);
    assert_eq!(comparison.compiler_maturity, 0.3);
    assert!(!comparison.backwards_compatible);
    assert_eq!(comparison.syscall_overhead_ns, 50);
}

#[test]
fn default_strategy_valid() {
    let strategy = default_migration_strategy();

    assert_eq!(strategy.phases.len(), 5);
    assert_eq!(strategy.estimated_years_per_phase, vec![1.0, 2.0, 2.0, 3.0, 2.0]);
    assert_eq!(strategy.total_estimated_years, 10.0);
    assert_eq!(strategy.risks.len(), 3);
    assert_eq!(strategy.breaking_changes.len(), 4);
    assert_eq!(validate_migration_strategy(&strategy), Ok(()));
}

#[test]
fn empty_phases_rejected() {
    let mut strategy = default_migration_strategy();
    strategy.phases.clear();
    strategy.estimated_years_per_phase.clear();
    strategy.total_estimated_years = 0.0;

    let errors = validate_migration_strategy(&strategy).unwrap_err();
    assert!(errors.contains(&MigrationValidationError::EmptyPhases));
}

#[test]
fn phases_out_of_order_rejected() {
    let mut strategy = default_migration_strategy();
    strategy.phases = vec![
        MigrationPhase::Phase0_Research,
        MigrationPhase::Phase2_EvmWrapped,
        MigrationPhase::Phase1_DualDeploy,
        MigrationPhase::Phase3_EvmDeprecated,
        MigrationPhase::Phase4_RiscVNative,
    ];

    let errors = validate_migration_strategy(&strategy).unwrap_err();
    assert!(errors.contains(&MigrationValidationError::PhasesOutOfOrder));
}

#[test]
fn mismatched_years_detected() {
    let mut strategy = default_migration_strategy();
    strategy.estimated_years_per_phase.pop();

    let errors = validate_migration_strategy(&strategy).unwrap_err();
    assert!(errors.iter().any(|error| {
        matches!(
            error,
            MigrationValidationError::MismatchedPhaseYears { phases: 5, years: 4 }
        )
    }));
}

#[test]
fn stats_computation_correct() {
    let strategy = default_migration_strategy();
    let comparisons = vec![default_evm_comparison(), default_riscv_comparison()];

    let stats = compute_migration_stats(&strategy, &comparisons);

    assert_eq!(stats.total_phases, 5);
    assert_eq!(stats.total_years, 10.0);
    assert_eq!(stats.total_risks, 3);
    assert!((stats.avg_risk_severity - 0.6).abs() < f64::EPSILON);
    assert!((stats.max_zk_improvement - 100.0).abs() < f64::EPSILON);
    assert_eq!(stats.breaking_change_count, 4);
}

#[test]
fn zk_improvement_ratio_calculation() {
    let current = default_evm_comparison();
    let target = default_riscv_comparison();

    let ratio = estimate_zk_improvement(&current, &target);
    assert!((ratio - 100.0).abs() < f64::EPSILON);
}
