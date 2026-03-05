//! Gate 1 block validator.

use alloy_primitives::{Bloom, B256};
use eth2077_types::canonical::Block;

use crate::{
    block_builder::{compute_receipts_root, compute_state_root, compute_transactions_root},
    executor::{BlockExecutor, ExecutionError},
    state::InMemoryStateDB,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    TransactionExecutionFailed {
        index: usize,
        tx_hash: B256,
        source: ExecutionError,
    },
    StateRootMismatch {
        expected: B256,
        actual: B256,
    },
    TransactionsRootMismatch {
        expected: B256,
        actual: B256,
    },
    ReceiptsRootMismatch {
        expected: B256,
        actual: B256,
    },
    GasUsedMismatch {
        expected: u64,
        actual: u64,
    },
    LogsBloomMismatch {
        expected: Bloom,
        actual: Bloom,
    },
}

impl core::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TransactionExecutionFailed {
                index,
                tx_hash,
                source,
            } => write!(
                f,
                "failed to execute transaction at index {index} ({tx_hash:#x}): {source}"
            ),
            Self::StateRootMismatch { expected, actual } => {
                write!(
                    f,
                    "state root mismatch: expected {expected:#x}, got {actual:#x}"
                )
            }
            Self::TransactionsRootMismatch { expected, actual } => {
                write!(
                    f,
                    "transactions root mismatch: expected {expected:#x}, got {actual:#x}"
                )
            }
            Self::ReceiptsRootMismatch { expected, actual } => {
                write!(
                    f,
                    "receipts root mismatch: expected {expected:#x}, got {actual:#x}"
                )
            }
            Self::GasUsedMismatch { expected, actual } => {
                write!(f, "gas used mismatch: expected {expected}, got {actual}")
            }
            Self::LogsBloomMismatch { expected, actual } => {
                write!(
                    f,
                    "logs bloom mismatch: expected {expected:#x}, got {actual:#x}"
                )
            }
        }
    }
}

impl std::error::Error for ValidationError {}

#[derive(Debug, Clone, Default)]
pub struct BlockValidator;

impl BlockValidator {
    pub fn validate_block(
        block: &Block,
        pre_state: &InMemoryStateDB,
    ) -> Result<(), ValidationError> {
        let mut executor = BlockExecutor::new(pre_state.clone());
        let mut receipts = Vec::with_capacity(block.transactions.len());
        let mut gas_used = 0u64;
        let mut logs_bloom = Bloom::ZERO;

        for (index, tx) in block.transactions.iter().enumerate() {
            let receipt = executor.execute_tx(tx).map_err(|source| {
                ValidationError::TransactionExecutionFailed {
                    index,
                    tx_hash: tx.hash,
                    source,
                }
            })?;
            gas_used = gas_used.saturating_add(receipt.cumulative_gas_used);
            logs_bloom.accrue_bloom(&receipt.logs_bloom);
            receipts.push(receipt);
        }

        let post_state = executor.into_state();
        let state_root = compute_state_root(&post_state);
        if state_root != block.header.state_root {
            return Err(ValidationError::StateRootMismatch {
                expected: block.header.state_root,
                actual: state_root,
            });
        }

        let transactions_root = compute_transactions_root(&block.transactions);
        if transactions_root != block.header.transactions_root {
            return Err(ValidationError::TransactionsRootMismatch {
                expected: block.header.transactions_root,
                actual: transactions_root,
            });
        }

        let receipts_root = compute_receipts_root(&receipts);
        if receipts_root != block.header.receipts_root {
            return Err(ValidationError::ReceiptsRootMismatch {
                expected: block.header.receipts_root,
                actual: receipts_root,
            });
        }

        if gas_used != block.header.gas_used {
            return Err(ValidationError::GasUsedMismatch {
                expected: block.header.gas_used,
                actual: gas_used,
            });
        }

        if logs_bloom != block.header.logs_bloom {
            return Err(ValidationError::LogsBloomMismatch {
                expected: block.header.logs_bloom,
                actual: logs_bloom,
            });
        }

        Ok(())
    }
}
