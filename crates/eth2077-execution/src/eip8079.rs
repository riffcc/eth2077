use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const EXECUTE_PRECOMPILE_ADDRESS: [u8; 20] = {
    let mut addr = [0u8; 20];
    addr[19] = 0x20;
    addr
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RollupAnchor {
    pub l1_state_root: [u8; 32],
    pub message_root: [u8; 32],
    pub rolling_hash: [u8; 32],
    pub l1_block_number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeRollupConfig {
    pub rollup_id: [u8; 32],
    pub anchor_frequency: u64,
    pub max_gas_per_execution: u64,
    pub precompile_address: [u8; 20],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutePrecompileInput {
    pub pre_state_root: [u8; 32],
    pub transactions: Vec<Vec<u8>>,
    pub anchor: RollupAnchor,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutePrecompileOutput {
    pub post_state_root: [u8; 32],
    pub gas_used: u64,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NativeRollupError {
    InvalidAnchor { reason: String },
    ExceedsGasLimit { used: u64, limit: u64 },
    EmptyTransactions,
    InvalidPreStateRoot,
    AnchorFrequencyViolation { expected_block: u64, actual_block: u64 },
    RollupIdMismatch { expected: [u8; 32], actual: [u8; 32] },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RollupExecutionStats {
    pub rollup_id: [u8; 32],
    pub executions_count: usize,
    pub total_gas_used: u64,
    pub total_transactions: usize,
    pub avg_gas_per_execution: f64,
}

pub fn default_rollup_config(rollup_id: [u8; 32]) -> NativeRollupConfig {
    NativeRollupConfig {
        rollup_id,
        anchor_frequency: 32,
        max_gas_per_execution: 30_000_000,
        precompile_address: EXECUTE_PRECOMPILE_ADDRESS,
    }
}

pub fn validate_execute_input(
    input: &ExecutePrecompileInput,
    config: &NativeRollupConfig,
) -> Result<(), Vec<NativeRollupError>> {
    let mut errors = Vec::new();

    if input.transactions.is_empty() {
        errors.push(NativeRollupError::EmptyTransactions);
    }

    if input.pre_state_root == [0u8; 32] {
        errors.push(NativeRollupError::InvalidPreStateRoot);
    }

    let frequency = config.anchor_frequency;
    if frequency == 0 {
        errors.push(NativeRollupError::AnchorFrequencyViolation {
            expected_block: 0,
            actual_block: input.anchor.l1_block_number,
        });
    } else if input.anchor.l1_block_number % frequency != 0 {
        errors.push(NativeRollupError::AnchorFrequencyViolation {
            expected_block: (input.anchor.l1_block_number / frequency) * frequency,
            actual_block: input.anchor.l1_block_number,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn mock_execute(
    input: &ExecutePrecompileInput,
    config: &NativeRollupConfig,
) -> Result<ExecutePrecompileOutput, NativeRollupError> {
    if input.transactions.is_empty() {
        return Err(NativeRollupError::EmptyTransactions);
    }

    if input.pre_state_root == [0u8; 32] {
        return Err(NativeRollupError::InvalidPreStateRoot);
    }

    let tx_count = u64::try_from(input.transactions.len()).unwrap_or(u64::MAX);
    let gas_used = 21_000u64.saturating_mul(tx_count);

    if gas_used > config.max_gas_per_execution {
        return Err(NativeRollupError::ExceedsGasLimit {
            used: gas_used,
            limit: config.max_gas_per_execution,
        });
    }

    let mut hasher = Sha256::new();
    hasher.update(input.pre_state_root);
    hasher.update(input.anchor.l1_state_root);
    hasher.update(tx_count.to_be_bytes());

    Ok(ExecutePrecompileOutput {
        post_state_root: hasher.finalize().into(),
        gas_used,
        success: true,
    })
}

pub fn compute_anchor_hash(anchor: &RollupAnchor) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(anchor.l1_state_root);
    hasher.update(anchor.message_root);
    hasher.update(anchor.rolling_hash);
    hasher.update(anchor.l1_block_number.to_be_bytes());
    hasher.finalize().into()
}

pub fn compute_execution_stats(
    executions: &[(ExecutePrecompileInput, ExecutePrecompileOutput)],
) -> RollupExecutionStats {
    let executions_count = executions.len();
    let total_gas_used = executions
        .iter()
        .fold(0u64, |acc, (_, output)| acc.saturating_add(output.gas_used));
    let total_transactions = executions
        .iter()
        .fold(0usize, |acc, (input, _)| acc.saturating_add(input.transactions.len()));
    let avg_gas_per_execution = if executions_count == 0 {
        0.0
    } else {
        total_gas_used as f64 / executions_count as f64
    };

    RollupExecutionStats {
        rollup_id: [0u8; 32],
        executions_count,
        total_gas_used,
        total_transactions,
        avg_gas_per_execution,
    }
}
