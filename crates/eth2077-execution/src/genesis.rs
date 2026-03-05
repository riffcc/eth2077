//! Genesis block helpers for Gate 1.

use alloy_primitives::{Address, Bloom, B256, U256};
use eth2077_types::canonical::{Block, Header};

use crate::{
    block_builder::{
        compute_receipts_root, compute_state_root, compute_transactions_root, empty_ommers_hash,
    },
    state::{AccountInfo, InMemoryStateDB},
};

pub const DEFAULT_GENESIS_GAS_LIMIT: u64 = 30_000_000;

pub fn create_genesis_block(allocs: &[(Address, U256)]) -> (Block, InMemoryStateDB) {
    let mut state = InMemoryStateDB::new();
    for (address, balance) in allocs {
        state.insert_account(
            *address,
            AccountInfo {
                balance: *balance,
                nonce: 0,
                code_hash: B256::ZERO,
                code: None,
            },
        );
    }

    let transactions = Vec::new();
    let receipts = Vec::new();

    let block = Block {
        header: Header {
            parent_hash: B256::ZERO,
            ommers_hash: empty_ommers_hash(),
            beneficiary: Address::ZERO,
            state_root: compute_state_root(&state),
            transactions_root: compute_transactions_root(&transactions),
            receipts_root: compute_receipts_root(&receipts),
            logs_bloom: Bloom::ZERO,
            difficulty: U256::ZERO,
            number: 0,
            gas_limit: DEFAULT_GENESIS_GAS_LIMIT,
            gas_used: 0,
            timestamp: 0,
            extra_data: Default::default(),
            mix_hash: B256::ZERO,
            nonce: 0,
            base_fee_per_gas: Some(1_000_000_000),
            blob_gas_used: Some(0),
            excess_blob_gas: Some(0),
            parent_beacon_block_root: None,
        },
        transactions,
        ommers: Vec::new(),
    };

    state.insert_block_hash(0, block.hash());
    (block, state)
}
