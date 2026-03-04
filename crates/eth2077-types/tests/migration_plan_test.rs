use eth2077_types::migration_plan::{
    compute_migration_plan_commitment, compute_migration_plan_stats, default_migration_plan_config,
    validate_migration_plan_config, DependencyKind, MigrationPhase, MigrationStep, RiskLevel,
    RollbackStrategy,
};
use std::collections::HashMap;

#[test]
fn all_enum_variants_are_covered() {
    let phases = [
        MigrationPhase::Discovery,
        MigrationPhase::Analysis,
        MigrationPhase::Implementation,
        MigrationPhase::Testing,
        MigrationPhase::Staging,
        MigrationPhase::Production,
    ];
    let risks = [
        RiskLevel::Negligible,
        RiskLevel::Low,
        RiskLevel::Medium,
        RiskLevel::High,
        RiskLevel::Critical,
        RiskLevel::Blocking,
    ];
    let rollbacks = [
        RollbackStrategy::Automatic,
        RollbackStrategy::ManualApproval,
        RollbackStrategy::FeatureFlag,
        RollbackStrategy::BlueGreen,
        RollbackStrategy::Canary,
        RollbackStrategy::NoRollback,
    ];
    let deps = [
        DependencyKind::HardBlock,
        DependencyKind::SoftBlock,
        DependencyKind::Advisory,
        DependencyKind::TestOnly,
        DependencyKind::BuildTime,
        DependencyKind::Runtime,
    ];
    assert_eq!(phases.len(), 6);
    assert_eq!(risks.len(), 6);
    assert_eq!(rollbacks.len(), 6);
    assert_eq!(deps.len(), 6);
}

#[test]
fn default_config_and_validation() {
    let config = default_migration_plan_config();
    assert_eq!(config.max_parallel_migrations, 2);
    assert_eq!(config.risk_tolerance, RiskLevel::Medium);
    assert_eq!(validate_migration_plan_config(&config), Ok(()));
}

#[test]
fn validation_reports_multiple_problems() {
    let mut config = default_migration_plan_config();
    config.max_parallel_migrations = 0;
    config.staging_duration_hours = -1.0;
    config.test_coverage_min_pct = 101.0;
    config.risk_tolerance = RiskLevel::Critical;
    config.require_rollback = false;
    config.approval_required = false;
    config.metadata.insert(" ".to_string(), "bad".to_string());

    let errors = validate_migration_plan_config(&config).unwrap_err();
    for field in [
        "max_parallel_migrations",
        "staging_duration_hours",
        "test_coverage_min_pct",
        "require_rollback",
        "approval_required",
        "metadata",
    ] {
        assert!(errors.iter().any(|error| error.field == field));
    }
}

fn mk_step(
    id: &str,
    phase: MigrationPhase,
    risk: RiskLevel,
    deps: Vec<&str>,
    hours: f64,
) -> MigrationStep {
    MigrationStep {
        id: id.to_string(),
        module_name: format!("module-{id}"),
        phase,
        risk,
        rollback: RollbackStrategy::Automatic,
        dependencies: deps.into_iter().map(str::to_string).collect(),
        estimated_hours: hours,
        metadata: HashMap::new(),
    }
}

#[test]
fn stats_aggregate_expected_values() {
    let steps = vec![
        mk_step("s1", MigrationPhase::Discovery, RiskLevel::Low, vec![], 5.0),
        mk_step(
            "s2",
            MigrationPhase::Testing,
            RiskLevel::Blocking,
            vec!["s1", "missing"],
            8.0,
        ),
        mk_step(
            "s3",
            MigrationPhase::Production,
            RiskLevel::Medium,
            vec!["s1"],
            -2.0,
        ),
    ];
    let stats = compute_migration_plan_stats(&steps);

    assert_eq!(stats.total_steps, 3);
    assert_eq!(stats.by_phase.get("Discovery"), Some(&1));
    assert_eq!(stats.by_phase.get("Testing"), Some(&1));
    assert_eq!(stats.by_phase.get("Production"), Some(&1));
    assert!((stats.avg_risk_score - (8.0 / 3.0)).abs() < 1e-12);
    assert!((stats.total_estimated_hours - 13.0).abs() < 1e-12);
    assert_eq!(stats.blocked_count, 1);
    assert!((stats.completion_pct - 60.0).abs() < 1e-12);
}

#[test]
fn commitment_is_deterministic_and_sensitive() {
    let config = default_migration_plan_config();
    let baseline = compute_migration_plan_commitment(&config);
    assert_eq!(baseline, compute_migration_plan_commitment(&config));

    let mut reordered = config.clone();
    reordered.metadata = HashMap::from([
        ("owner".to_string(), "consensus-core".to_string()),
        ("mode".to_string(), "safety-first".to_string()),
        ("program".to_string(), "ETH2077-Citadel".to_string()),
    ]);
    assert_eq!(baseline, compute_migration_plan_commitment(&reordered));

    let mut changed = config.clone();
    changed.staging_duration_hours = 96.0;
    assert_ne!(baseline, compute_migration_plan_commitment(&changed));
}
