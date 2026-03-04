use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum StateType {
    Permanent,
    TemporaryMonthly,
    TemporaryWeekly,
    UtxoBased,
    Ephemeral,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum StateUseCase {
    UserAccount,
    DeFiHub,
    TokenBalance,
    NftOwnership,
    TempComputation,
    CrossTxData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateAllocation {
    pub state_type: StateType,
    pub use_case: StateUseCase,
    pub cost_per_slot_gas: u64,
    pub max_slots: u64,
    pub retention_period_days: u64,
    pub storage_size_gb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HyperscaleConfig {
    pub allocations: Vec<StateAllocation>,
    pub permanent_state_budget_gb: f64,
    pub temporary_state_budget_gb: f64,
    pub target_scaling_factor: f64,
    pub monthly_pruning_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HyperscaleValidationError {
    EmptyAllocations,
    NoPermanentState,
    ExceedsPermanentBudget { used_gb: String, budget_gb: String },
    ExceedsTempBudget { used_gb: String, budget_gb: String },
    UnrealisticScaling { factor: String },
    ZeroCostAllocation { state_type: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HyperscaleStats {
    pub total_state_types: usize,
    pub permanent_storage_gb: f64,
    pub temporary_storage_gb: f64,
    pub total_storage_gb: f64,
    pub avg_cost_per_slot: f64,
    pub effective_scaling_factor: f64,
    pub cost_reduction_vs_permanent: f64,
}

fn slots_for_gb(storage_size_gb: f64) -> u64 {
    ((storage_size_gb * 1_000_000_000.0) / 32.0) as u64
}

fn state_type_discriminant(state_type: StateType) -> u8 {
    match state_type {
        StateType::Permanent => 0,
        StateType::TemporaryMonthly => 1,
        StateType::TemporaryWeekly => 2,
        StateType::UtxoBased => 3,
        StateType::Ephemeral => 4,
    }
}

fn state_use_case_discriminant(use_case: StateUseCase) -> u8 {
    match use_case {
        StateUseCase::UserAccount => 0,
        StateUseCase::DeFiHub => 1,
        StateUseCase::TokenBalance => 2,
        StateUseCase::NftOwnership => 3,
        StateUseCase::TempComputation => 4,
        StateUseCase::CrossTxData => 5,
    }
}

fn format_decimal(value: f64) -> String {
    format!("{value:.3}")
}

pub fn default_hyperscale_config() -> HyperscaleConfig {
    let allocations = vec![
        StateAllocation {
            state_type: StateType::Permanent,
            use_case: StateUseCase::UserAccount,
            cost_per_slot_gas: 20_000,
            max_slots: slots_for_gb(12.0),
            retention_period_days: 0,
            storage_size_gb: 12.0,
        },
        StateAllocation {
            state_type: StateType::TemporaryMonthly,
            use_case: StateUseCase::TokenBalance,
            cost_per_slot_gas: 200,
            max_slots: slots_for_gb(4_096.0),
            retention_period_days: 30,
            storage_size_gb: 4_096.0,
        },
        StateAllocation {
            state_type: StateType::TemporaryWeekly,
            use_case: StateUseCase::TempComputation,
            cost_per_slot_gas: 50,
            max_slots: slots_for_gb(2_048.0),
            retention_period_days: 7,
            storage_size_gb: 2_048.0,
        },
        StateAllocation {
            state_type: StateType::UtxoBased,
            use_case: StateUseCase::NftOwnership,
            cost_per_slot_gas: 100,
            max_slots: slots_for_gb(1_024.0),
            retention_period_days: 30,
            storage_size_gb: 1_024.0,
        },
        StateAllocation {
            state_type: StateType::Ephemeral,
            use_case: StateUseCase::CrossTxData,
            cost_per_slot_gas: 10,
            max_slots: slots_for_gb(512.0),
            retention_period_days: 1,
            storage_size_gb: 512.0,
        },
    ];

    HyperscaleConfig {
        allocations,
        permanent_state_budget_gb: 16.0,
        temporary_state_budget_gb: 8_192.0,
        target_scaling_factor: 1_000.0,
        monthly_pruning_enabled: true,
    }
}

pub fn validate_hyperscale_config(
    config: &HyperscaleConfig,
) -> Result<(), Vec<HyperscaleValidationError>> {
    let mut errors = Vec::new();

    if config.allocations.is_empty() {
        errors.push(HyperscaleValidationError::EmptyAllocations);
    }

    let has_permanent_state = config
        .allocations
        .iter()
        .any(|allocation| allocation.state_type == StateType::Permanent);
    if !has_permanent_state {
        errors.push(HyperscaleValidationError::NoPermanentState);
    }

    let permanent_used_gb: f64 = config
        .allocations
        .iter()
        .filter(|allocation| allocation.state_type == StateType::Permanent)
        .map(|allocation| allocation.storage_size_gb)
        .sum();

    let temp_used_gb: f64 = config
        .allocations
        .iter()
        .filter(|allocation| allocation.state_type != StateType::Permanent)
        .map(|allocation| allocation.storage_size_gb)
        .sum();

    if permanent_used_gb > config.permanent_state_budget_gb {
        errors.push(HyperscaleValidationError::ExceedsPermanentBudget {
            used_gb: format_decimal(permanent_used_gb),
            budget_gb: format_decimal(config.permanent_state_budget_gb),
        });
    }

    if temp_used_gb > config.temporary_state_budget_gb {
        errors.push(HyperscaleValidationError::ExceedsTempBudget {
            used_gb: format_decimal(temp_used_gb),
            budget_gb: format_decimal(config.temporary_state_budget_gb),
        });
    }

    if config.target_scaling_factor <= 0.0 || config.target_scaling_factor >= 10_000.0 {
        errors.push(HyperscaleValidationError::UnrealisticScaling {
            factor: format_decimal(config.target_scaling_factor),
        });
    }

    for allocation in &config.allocations {
        if allocation.cost_per_slot_gas == 0 {
            errors.push(HyperscaleValidationError::ZeroCostAllocation {
                state_type: format!("{:?}", allocation.state_type),
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_hyperscale_stats(config: &HyperscaleConfig) -> HyperscaleStats {
    let mut by_type: HashMap<StateType, usize> = HashMap::new();
    let mut permanent_storage_gb = 0.0;
    let mut temporary_storage_gb = 0.0;
    let mut weighted_cost_total = 0.0;
    let mut total_slots = 0u128;

    for allocation in &config.allocations {
        *by_type.entry(allocation.state_type).or_insert(0) += 1;
        weighted_cost_total += allocation.cost_per_slot_gas as f64 * allocation.max_slots as f64;
        total_slots += allocation.max_slots as u128;

        if allocation.state_type == StateType::Permanent {
            permanent_storage_gb += allocation.storage_size_gb;
        } else {
            temporary_storage_gb += allocation.storage_size_gb;
        }
    }

    let total_storage_gb = permanent_storage_gb + temporary_storage_gb;
    let avg_cost_per_slot = if total_slots == 0 {
        0.0
    } else {
        weighted_cost_total / total_slots as f64
    };

    let effective_scaling_factor = if avg_cost_per_slot == 0.0 {
        0.0
    } else {
        20_000.0 / avg_cost_per_slot
    };

    let cost_reduction_vs_permanent = if avg_cost_per_slot == 0.0 {
        100.0
    } else {
        (1.0 - (avg_cost_per_slot / 20_000.0)) * 100.0
    };

    HyperscaleStats {
        total_state_types: by_type.len(),
        permanent_storage_gb,
        temporary_storage_gb,
        total_storage_gb,
        avg_cost_per_slot,
        effective_scaling_factor,
        cost_reduction_vs_permanent,
    }
}

pub fn recommend_state_type(use_case: StateUseCase) -> StateType {
    let recommendations: HashMap<StateUseCase, StateType> = HashMap::from([
        (StateUseCase::UserAccount, StateType::Permanent),
        (StateUseCase::DeFiHub, StateType::Permanent),
        (StateUseCase::TokenBalance, StateType::TemporaryMonthly),
        (StateUseCase::NftOwnership, StateType::TemporaryMonthly),
        (StateUseCase::TempComputation, StateType::TemporaryWeekly),
        (StateUseCase::CrossTxData, StateType::Ephemeral),
    ]);

    recommendations
        .get(&use_case)
        .copied()
        .unwrap_or(StateType::Permanent)
}

fn allocation_digest(allocation: &StateAllocation) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([state_type_discriminant(allocation.state_type)]);
    hasher.update([state_use_case_discriminant(allocation.use_case)]);
    hasher.update(allocation.cost_per_slot_gas.to_be_bytes());
    hasher.update(allocation.max_slots.to_be_bytes());
    hasher.update(allocation.retention_period_days.to_be_bytes());
    hasher.update(allocation.storage_size_gb.to_bits().to_be_bytes());

    let digest = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

pub fn compute_state_commitment(allocations: &[StateAllocation]) -> [u8; 32] {
    let mut allocation_hashes: Vec<[u8; 32]> = allocations.iter().map(allocation_digest).collect();
    allocation_hashes.sort_unstable();

    let mut root_hasher = Sha256::new();
    for allocation_hash in allocation_hashes {
        root_hasher.update(allocation_hash);
    }

    let digest = root_hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

pub fn estimate_monthly_pruning_savings(config: &HyperscaleConfig) -> f64 {
    if !config.monthly_pruning_enabled {
        return 0.0;
    }

    config
        .allocations
        .iter()
        .filter(|allocation| allocation.retention_period_days > 0)
        .map(|allocation| {
            let pruning_cycles_per_month = 30.0 / allocation.retention_period_days as f64;
            allocation.storage_size_gb * pruning_cycles_per_month
        })
        .sum()
}
