use eth2077_types::hyperscale_state::{
    compute_hyperscale_stats, compute_state_commitment, default_hyperscale_config,
    estimate_monthly_pruning_savings, recommend_state_type, validate_hyperscale_config,
    HyperscaleConfig, HyperscaleValidationError, StateAllocation, StateType, StateUseCase,
};

fn allocation(
    state_type: StateType,
    use_case: StateUseCase,
    cost_per_slot_gas: u64,
    max_slots: u64,
    retention_period_days: u64,
    storage_size_gb: f64,
) -> StateAllocation {
    StateAllocation {
        state_type,
        use_case,
        cost_per_slot_gas,
        max_slots,
        retention_period_days,
        storage_size_gb,
    }
}

#[test]
fn default_config_valid() {
    let config = default_hyperscale_config();
    assert_eq!(validate_hyperscale_config(&config), Ok(()));
}

#[test]
fn empty_allocations_rejected() {
    let mut config = default_hyperscale_config();
    config.allocations.clear();

    let errors = validate_hyperscale_config(&config).unwrap_err();
    assert!(errors.contains(&HyperscaleValidationError::EmptyAllocations));
}

#[test]
fn no_permanent_state_rejected() {
    let mut config = default_hyperscale_config();
    config
        .allocations
        .retain(|allocation| allocation.state_type != StateType::Permanent);

    let errors = validate_hyperscale_config(&config).unwrap_err();
    assert!(errors.contains(&HyperscaleValidationError::NoPermanentState));
}

#[test]
fn recommend_state_type_for_user_account() {
    assert_eq!(
        recommend_state_type(StateUseCase::UserAccount),
        StateType::Permanent
    );
}

#[test]
fn recommend_state_type_for_temp_computation() {
    assert_eq!(
        recommend_state_type(StateUseCase::TempComputation),
        StateType::TemporaryWeekly
    );
}

#[test]
fn stats_computation_correct() {
    let config = HyperscaleConfig {
        allocations: vec![
            allocation(
                StateType::Permanent,
                StateUseCase::UserAccount,
                20_000,
                10,
                0,
                2.0,
            ),
            allocation(
                StateType::TemporaryMonthly,
                StateUseCase::TokenBalance,
                200,
                90,
                30,
                8.0,
            ),
        ],
        permanent_state_budget_gb: 10.0,
        temporary_state_budget_gb: 10.0,
        target_scaling_factor: 1_000.0,
        monthly_pruning_enabled: true,
    };

    let stats = compute_hyperscale_stats(&config);

    assert_eq!(stats.total_state_types, 2);
    assert_eq!(stats.permanent_storage_gb, 2.0);
    assert_eq!(stats.temporary_storage_gb, 8.0);
    assert_eq!(stats.total_storage_gb, 10.0);
    assert!((stats.avg_cost_per_slot - 2_180.0).abs() < 1e-9);
    assert!((stats.effective_scaling_factor - (20_000.0 / 2_180.0)).abs() < 1e-9);
    assert!((stats.cost_reduction_vs_permanent - 89.1).abs() < 1e-9);
}

#[test]
fn commitment_deterministic() {
    let mut allocations = default_hyperscale_config().allocations;
    let mut reversed = allocations.clone();
    reversed.reverse();

    let first = compute_state_commitment(&allocations);
    let second = compute_state_commitment(&reversed);

    allocations.swap(0, 1);
    let third = compute_state_commitment(&allocations);

    assert_eq!(first, second);
    assert_eq!(first, third);
}

#[test]
fn pruning_savings_estimation() {
    let config = HyperscaleConfig {
        allocations: vec![
            allocation(
                StateType::Permanent,
                StateUseCase::DeFiHub,
                20_000,
                1,
                0,
                3.0,
            ),
            allocation(
                StateType::TemporaryMonthly,
                StateUseCase::TokenBalance,
                200,
                1,
                30,
                10.0,
            ),
            allocation(
                StateType::TemporaryWeekly,
                StateUseCase::TempComputation,
                50,
                1,
                7,
                14.0,
            ),
        ],
        permanent_state_budget_gb: 16.0,
        temporary_state_budget_gb: 8_192.0,
        target_scaling_factor: 1_000.0,
        monthly_pruning_enabled: true,
    };

    let savings = estimate_monthly_pruning_savings(&config);
    assert!((savings - 70.0).abs() < 1e-9);
}
