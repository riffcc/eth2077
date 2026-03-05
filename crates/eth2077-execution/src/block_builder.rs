//! Gate 1 block builder.

use alloy_primitives::{keccak256, Address, Bloom, B256};
use eth2077_types::canonical::{Block, Header, Receipt, Transaction};

use crate::{
    executor::{BlockExecutor, ExecutionError},
    state::InMemoryStateDB,
};

#[derive(Debug, Clone)]
pub struct BlockBuilderConfig {
    pub beneficiary: Address,
    pub timestamp: Option<u64>,
    pub gas_limit: Option<u64>,
}

impl Default for BlockBuilderConfig {
    fn default() -> Self {
        Self {
            beneficiary: Address::ZERO,
            timestamp: None,
            gas_limit: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlockBuilder {
    config: BlockBuilderConfig,
}

#[derive(Debug, Clone)]
pub struct BuiltBlock {
    pub block: Block,
    pub receipts: Vec<Receipt>,
    pub state: InMemoryStateDB,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockBuildError {
    TransactionExecutionFailed {
        index: usize,
        tx_hash: B256,
        source: ExecutionError,
    },
}

impl core::fmt::Display for BlockBuildError {
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
        }
    }
}

impl std::error::Error for BlockBuildError {}

impl BlockBuilder {
    pub fn new(config: BlockBuilderConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &BlockBuilderConfig {
        &self.config
    }

    pub fn build_block(
        &self,
        parent: &Header,
        pending_transactions: Vec<Transaction>,
        state: InMemoryStateDB,
    ) -> Result<BuiltBlock, BlockBuildError> {
        let mut executor = BlockExecutor::new(state);
        let mut receipts = Vec::with_capacity(pending_transactions.len());
        let mut gas_used = 0u64;
        let mut logs_bloom = Bloom::ZERO;

        for (index, tx) in pending_transactions.iter().enumerate() {
            let receipt = executor.execute_tx(tx).map_err(|source| {
                BlockBuildError::TransactionExecutionFailed {
                    index,
                    tx_hash: tx.hash,
                    source,
                }
            })?;
            gas_used = gas_used.saturating_add(receipt.cumulative_gas_used);
            logs_bloom.accrue_bloom(&receipt.logs_bloom);
            receipts.push(receipt);
        }

        let mut post_state = executor.into_state();
        let state_root = compute_state_root(&post_state);
        let transactions_root = compute_transactions_root(&pending_transactions);
        let receipts_root = compute_receipts_root(&receipts);

        let block = Block {
            header: Header {
                parent_hash: parent.hash(),
                ommers_hash: empty_ommers_hash(),
                beneficiary: self.config.beneficiary,
                state_root,
                transactions_root,
                receipts_root,
                logs_bloom,
                difficulty: parent.difficulty,
                number: parent.number.saturating_add(1),
                gas_limit: self.config.gas_limit.unwrap_or(parent.gas_limit),
                gas_used,
                timestamp: self
                    .config
                    .timestamp
                    .unwrap_or(parent.timestamp.saturating_add(12)),
                extra_data: Default::default(),
                mix_hash: B256::ZERO,
                nonce: 0,
                base_fee_per_gas: parent.base_fee_per_gas,
                blob_gas_used: None,
                excess_blob_gas: None,
                parent_beacon_block_root: None,
            },
            transactions: pending_transactions,
            ommers: Vec::new(),
        };

        post_state.insert_block_hash(block.header.number, block.hash());

        Ok(BuiltBlock {
            block,
            receipts,
            state: post_state,
        })
    }
}

/// Placeholder state root for Gate 1.
/// Hashes all accounts sorted by address for deterministic output.
pub fn compute_state_root(state: &InMemoryStateDB) -> B256 {
    let mut addresses = state.accounts().copied().collect::<Vec<_>>();
    addresses.sort_unstable();

    let mut payload = Vec::new();
    payload.extend_from_slice(&(addresses.len() as u64).to_be_bytes());

    for address in addresses {
        let account = state.get_account(&address);
        payload.extend_from_slice(address.as_slice());
        payload.extend_from_slice(&account.nonce.to_be_bytes());
        payload.extend_from_slice(&account.balance.to_be_bytes::<32>());
        payload.extend_from_slice(account.code_hash.as_slice());

        match account.code {
            Some(code) => {
                payload.push(1);
                payload.extend_from_slice(&(code.len() as u64).to_be_bytes());
                payload.extend_from_slice(&code);
            }
            None => payload.push(0),
        }
    }

    keccak256(payload)
}

/// Placeholder transactions root for Gate 1.
/// Hashes ordered transaction hashes.
pub fn compute_transactions_root(transactions: &[Transaction]) -> B256 {
    let mut payload = Vec::new();
    payload.extend_from_slice(&(transactions.len() as u64).to_be_bytes());
    for tx in transactions {
        payload.extend_from_slice(tx.hash.as_slice());
    }
    keccak256(payload)
}

/// Placeholder receipts root for Gate 1.
/// Hashes ordered receipt payload hashes.
pub fn compute_receipts_root(receipts: &[Receipt]) -> B256 {
    let mut payload = Vec::new();
    payload.extend_from_slice(&(receipts.len() as u64).to_be_bytes());
    for receipt in receipts {
        let encoded = encode_receipt(receipt);
        let leaf = keccak256(encoded);
        payload.extend_from_slice(leaf.as_slice());
    }
    keccak256(payload)
}

pub fn empty_ommers_hash() -> B256 {
    keccak256([0xc0])
}

fn encode_receipt(receipt: &Receipt) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(receipt.tx_hash.as_slice());
    out.push(u8::from(receipt.status));
    out.extend_from_slice(&receipt.cumulative_gas_used.to_be_bytes());
    out.extend_from_slice(receipt.logs_bloom.data());
    out.extend_from_slice(&(receipt.logs.len() as u64).to_be_bytes());

    for log in &receipt.logs {
        out.extend_from_slice(log.address.as_slice());
        out.extend_from_slice(&(log.topics.len() as u64).to_be_bytes());
        for topic in &log.topics {
            out.extend_from_slice(topic.as_slice());
        }
        out.extend_from_slice(&(log.data.len() as u64).to_be_bytes());
        out.extend_from_slice(log.data.as_ref());
    }

    out
}
