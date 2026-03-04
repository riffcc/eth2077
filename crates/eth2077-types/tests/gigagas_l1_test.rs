use eth2077_types::gigagas_l1::*;
use std::collections::HashSet;

fn assert_close(left: f64, right: f64, eps: f64) {
    assert!(
        (left - right).abs() <= eps,
        "left={left}, right={right}, eps={eps}"
    );
}

fn high_capacity_config() -> GigagasConfig {
    GigagasConfig {
        backend: ExecutionBackend::PipelinedExecution,
        gas_model: GasAccountingModel::ComputeOnlyGas,
        scaling_approach: ScalingApproach::SpeculativeExecution,
        target_ggas_per_sec: 1.0,
        current_ggas_per_sec: 0.15,
        core_count: 192,
        state_size_gb: 512.0,
        io_bandwidth_gbps: 600.0,
        memory_gb: 2_048.0,
    }
}

#[test]
fn default_config_is_valid() {
    let config = default_gigagas_config();
    assert_eq!(validate_gigagas_config(&config), Ok(()));
}

#[test]
fn validation_rejects_target_below_current() {
    let mut config = default_gigagas_config();
    config.target_ggas_per_sec = 0.05;
    config.current_ggas_per_sec = 0.30;

    let errors = validate_gigagas_config(&config).unwrap_err();
    assert!(errors.contains(&GigagasValidationError::TargetBelowCurrent));
}

#[test]
fn validation_rejects_zero_cores() {
    let mut config = default_gigagas_config();
    config.core_count = 0;

    let errors = validate_gigagas_config(&config).unwrap_err();
    assert!(errors.contains(&GigagasValidationError::ZeroCores));
}

#[test]
fn validation_rejects_insufficient_memory() {
    let mut config = default_gigagas_config();
    config.state_size_gb = 700.0;
    config.memory_gb = 64.0;

    let errors = validate_gigagas_config(&config).unwrap_err();
    assert!(errors.iter().any(|error| matches!(
        error,
        GigagasValidationError::InsufficientMemory {
            required_gb: _,
            available_gb: 64.0
        }
    )));
}

#[test]
fn stats_show_scaling_factor_above_one() {
    let config = default_gigagas_config();
    let stats = compute_gigagas_stats(&config);

    assert!(stats.scaling_factor > 1.0);
    assert!(stats.projected_ggas_per_sec > config.current_ggas_per_sec);
    assert!(stats.estimated_tps_equivalent > 0.0);
}

#[test]
fn parallel_efficiency_decreases_with_contention() {
    let cores = 96;
    let low_contention = estimate_parallelism_efficiency(cores, 0.1);
    let medium_contention = estimate_parallelism_efficiency(cores, 0.4);
    let high_contention = estimate_parallelism_efficiency(cores, 0.8);

    assert!(low_contention > medium_contention);
    assert!(medium_contention > high_contention);
    assert!(high_contention >= 0.05);
}

#[test]
fn compare_approaches_returns_all_variants() {
    let config = default_gigagas_config();
    let comparisons = compare_scaling_approaches(&config);

    assert_eq!(comparisons.len(), 5);
    let names: HashSet<String> = comparisons.into_iter().map(|(name, _)| name).collect();
    let expected: HashSet<String> = vec![
        "VerticalScaling".to_string(),
        "HorizontalSharding".to_string(),
        "StatelessExecution".to_string(),
        "ParallelTransactions".to_string(),
        "SpeculativeExecution".to_string(),
    ]
    .into_iter()
    .collect();

    assert_eq!(names, expected);
}

#[test]
fn commitment_is_deterministic_and_sensitive() {
    let config = default_gigagas_config();
    let first = compute_gigagas_commitment(&config);
    let second = compute_gigagas_commitment(&config);
    assert_eq!(first, second);

    let mut changed = config.clone();
    changed.target_ggas_per_sec += 0.01;
    let third = compute_gigagas_commitment(&changed);
    assert_ne!(first, third);
}

#[test]
fn validation_rejects_insufficient_io() {
    let mut config = default_gigagas_config();
    config.io_bandwidth_gbps = 1.0;

    let errors = validate_gigagas_config(&config).unwrap_err();
    assert!(errors.iter().any(|error| matches!(
        error,
        GigagasValidationError::InsufficientIO {
            required_gbps: _,
            available_gbps: 1.0
        }
    )));
}

#[test]
fn validation_rejects_state_exceeding_memory() {
    let mut config = default_gigagas_config();
    config.state_size_gb = 900.0;
    config.memory_gb = 800.0;

    let errors = validate_gigagas_config(&config).unwrap_err();
    assert!(errors.contains(&GigagasValidationError::StateExceedsMemory));
}

#[test]
fn high_capacity_profile_meets_target_with_zero_gap() {
    let config = high_capacity_config();
    let stats = compute_gigagas_stats(&config);

    assert!(stats.meets_target);
    assert_close(stats.gap_to_target_pct, 0.0, 1e-9);
    assert!(stats.projected_ggas_per_sec >= config.target_ggas_per_sec);
}

#[test]
fn constrained_profile_reports_positive_gap() {
    let mut config = default_gigagas_config();
    config.target_ggas_per_sec = 1.5;
    config.current_ggas_per_sec = 0.08;
    config.core_count = 24;
    config.state_size_gb = 1_200.0;
    config.memory_gb = 1_300.0;
    config.io_bandwidth_gbps = 80.0;
    config.scaling_approach = ScalingApproach::VerticalScaling;
    config.backend = ExecutionBackend::SequentialEVM;
    config.gas_model = GasAccountingModel::CurrentEIP1559;

    let stats = compute_gigagas_stats(&config);

    assert!(!stats.meets_target);
    assert!(stats.gap_to_target_pct > 0.0);
    assert!(stats.bottleneck == "IOBandwidth" || stats.bottleneck == "MemoryCapacity");
}
