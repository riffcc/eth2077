//! Canonical Ethereum types for block execution.
//!
//! These types use alloy-primitives for Ethereum-native representations
//! and are the source of truth for block processing.

use alloy_primitives::{Address, Bloom, Bytes, B256, U256};
use alloy_rlp::{RlpDecodable, RlpEncodable};
use serde::{Deserialize, Serialize};

/// Canonical Ethereum block header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, RlpEncodable, RlpDecodable)]
#[rlp(trailing)]
pub struct Header {
    pub parent_hash: B256,
    pub ommers_hash: B256,
    pub beneficiary: Address,
    pub state_root: B256,
    pub transactions_root: B256,
    pub receipts_root: B256,
    pub logs_bloom: Bloom,
    pub difficulty: U256,
    pub number: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    pub extra_data: Bytes,
    pub mix_hash: B256,
    pub nonce: u64,
    /// EIP-1559 base fee
    pub base_fee_per_gas: Option<u64>,
    /// EIP-4844 blob gas used
    pub blob_gas_used: Option<u64>,
    /// EIP-4844 excess blob gas
    pub excess_blob_gas: Option<u64>,
    /// EIP-4788 parent beacon block root
    pub parent_beacon_block_root: Option<B256>,
}

/// Canonical Ethereum transaction (Type 2 / EIP-1559).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub hash: B256,
    pub nonce: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub gas_limit: u64,
    pub max_fee_per_gas: u64,
    pub max_priority_fee_per_gas: u64,
    pub input: Bytes,
    pub chain_id: u64,
    pub signature_r: U256,
    pub signature_s: U256,
    pub signature_v: u64,
}

/// Canonical Ethereum block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub header: Header,
    pub transactions: Vec<Transaction>,
    pub ommers: Vec<Header>,
}

/// Transaction receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub tx_hash: B256,
    pub status: bool,
    pub cumulative_gas_used: u64,
    pub logs_bloom: Bloom,
    pub logs: Vec<Log>,
}

/// Event log entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Log {
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: Bytes,
}

/// Account state in the world state trie.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AccountState {
    pub nonce: u64,
    pub balance: U256,
    pub storage_root: B256,
    pub code_hash: B256,
}

impl Header {
    /// Compute the hash of this header (keccak256 of RLP encoding).
    pub fn hash(&self) -> B256 {
        use alloy_rlp::Encodable;
        let mut buf = Vec::new();
        self.encode(&mut buf);
        alloy_primitives::keccak256(&buf)
    }
}

impl Block {
    /// Convenience: the block hash is the header hash.
    pub fn hash(&self) -> B256 {
        self.header.hash()
    }

    /// The block number.
    pub fn number(&self) -> u64 {
        self.header.number
    }
}
