use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum StatelessnessLevel {
    /// Fully stateless: witnesses on everything.
    Full,
    /// VOPS: store validation-critical state, witnesses for the rest.
    PartialVops,
    /// Weak statelessness: proposers have state, validators don't.
    WeakStateless,
    /// Current: everyone has full state.
    CurrentStateful,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum StateCategory {
    /// ETH balance.
    AccountBalance,
    /// Transaction nonce.
    AccountNonce,
    /// Contract bytecode.
    ContractCode,
    /// Individual storage slot.
    StorageSlot,
    /// AA paymaster validation state.
    PaymasterState,
    /// Account abstraction validation logic.
    ValidationCode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VopsConfig {
    pub retained_categories: HashSet<StateCategory>,
    /// Max witness size per block.
    pub witness_size_budget_bytes: usize,
    /// Estimated partial state storage.
    pub partial_state_size_mb: usize,
    /// Full state for comparison.
    pub full_state_size_mb: usize,
    pub max_witness_items_per_tx: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WitnessRequirement {
    pub tx_hash: [u8; 32],
    pub needed_state_keys: Vec<[u8; 32]>,
    pub witness_size_bytes: usize,
    pub categories_accessed: HashSet<StateCategory>,
    /// False if all accessed state is retained.
    pub requires_witness: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VopsValidationError {
    WitnessTooLarge { size: usize, max: usize },
    TooManyWitnessItems { count: usize, max: usize },
    MissingRetainedState { category: StateCategory },
    EmptyRetainedCategories,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VopsImpactStats {
    pub total_transactions: usize,
    pub witness_required_count: usize,
    pub witness_free_count: usize,
    pub total_witness_bytes: usize,
    pub avg_witness_bytes_per_tx: f64,
    /// Vs fully stateless.
    pub bandwidth_reduction_percent: f64,
    /// Vs fully stateful.
    pub state_storage_reduction_percent: f64,
}

pub fn default_vops_config() -> VopsConfig {
    VopsConfig {
        retained_categories: HashSet::from([
            StateCategory::AccountBalance,
            StateCategory::AccountNonce,
            StateCategory::ContractCode,
            StateCategory::PaymasterState,
        ]),
        witness_size_budget_bytes: 1024 * 1024,
        partial_state_size_mb: 16 * 1024,
        full_state_size_mb: 256 * 1024,
        max_witness_items_per_tx: 64,
    }
}

pub fn validate_witness_requirement(
    req: &WitnessRequirement,
    config: &VopsConfig,
) -> Result<(), Vec<VopsValidationError>> {
    let mut errors = Vec::new();

    if req.witness_size_bytes > config.witness_size_budget_bytes {
        errors.push(VopsValidationError::WitnessTooLarge {
            size: req.witness_size_bytes,
            max: config.witness_size_budget_bytes,
        });
    }

    if req.needed_state_keys.len() > config.max_witness_items_per_tx {
        errors.push(VopsValidationError::TooManyWitnessItems {
            count: req.needed_state_keys.len(),
            max: config.max_witness_items_per_tx,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn classify_witness_need(categories: &HashSet<StateCategory>, config: &VopsConfig) -> bool {
    categories
        .iter()
        .any(|category| !config.retained_categories.contains(category))
}

pub fn compute_vops_stats(
    requirements: &[WitnessRequirement],
    config: &VopsConfig,
) -> VopsImpactStats {
    let total_transactions = requirements.len();
    let mut witness_required_count = 0_usize;
    let mut total_witness_bytes = 0_usize;
    let mut full_witness_bytes = 0_usize;
    let mut _category_access_counts: HashMap<StateCategory, usize> = HashMap::new();

    for req in requirements {
        full_witness_bytes += req.witness_size_bytes;

        for category in &req.categories_accessed {
            *_category_access_counts.entry(*category).or_insert(0) += 1;
        }

        if classify_witness_need(&req.categories_accessed, config) {
            witness_required_count += 1;
            total_witness_bytes += req.witness_size_bytes;
        }
    }

    let witness_free_count = total_transactions.saturating_sub(witness_required_count);
    let avg_witness_bytes_per_tx = if total_transactions == 0 {
        0.0
    } else {
        total_witness_bytes as f64 / total_transactions as f64
    };

    VopsImpactStats {
        total_transactions,
        witness_required_count,
        witness_free_count,
        total_witness_bytes,
        avg_witness_bytes_per_tx,
        bandwidth_reduction_percent: estimate_bandwidth_savings(
            full_witness_bytes,
            total_witness_bytes,
        ),
        state_storage_reduction_percent: estimate_bandwidth_savings(
            config.full_state_size_mb,
            config.partial_state_size_mb,
        ),
    }
}

pub fn compute_witness_commitment(req: &WitnessRequirement) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(req.tx_hash);

    let mut sorted_state_keys = req.needed_state_keys.clone();
    sorted_state_keys.sort_unstable();
    for key in sorted_state_keys {
        hasher.update(key);
    }

    hasher.update((req.witness_size_bytes as u64).to_le_bytes());

    let digest = hasher.finalize();
    let mut commitment = [0_u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

pub fn estimate_bandwidth_savings(full_witness_bytes: usize, vops_witness_bytes: usize) -> f64 {
    if full_witness_bytes == 0 {
        return 0.0;
    }

    let savings = ((full_witness_bytes as f64 - vops_witness_bytes as f64)
        / full_witness_bytes as f64)
        * 100.0;
    savings.clamp(0.0, 100.0)
}
