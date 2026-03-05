//! In-memory state database for EVM execution.

use std::collections::HashMap;

use alloy_primitives::{Address, B256, U256};

/// Account info stored in the state DB.
#[derive(Debug, Clone, Default)]
pub struct AccountInfo {
    pub balance: U256,
    pub nonce: u64,
    pub code_hash: B256,
    pub code: Option<Vec<u8>>,
}

/// In-memory Ethereum state database.
///
/// Stores account balances, nonces, code, and storage.
/// This is the backing store that revm reads from and writes to.
#[derive(Debug, Clone, Default)]
pub struct InMemoryStateDB {
    accounts: HashMap<Address, AccountInfo>,
    storage: HashMap<Address, HashMap<U256, U256>>,
    block_hashes: HashMap<u64, B256>,
}

impl InMemoryStateDB {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or update an account.
    pub fn insert_account(&mut self, address: Address, info: AccountInfo) {
        self.accounts.insert(address, info);
    }

    /// Get account info (returns default if not found).
    pub fn get_account(&self, address: &Address) -> AccountInfo {
        self.accounts.get(address).cloned().unwrap_or_default()
    }

    /// Set a storage slot.
    pub fn set_storage(&mut self, address: Address, slot: U256, value: U256) {
        self.storage.entry(address).or_default().insert(slot, value);
    }

    /// Get a storage slot value.
    pub fn get_storage(&self, address: &Address, slot: &U256) -> U256 {
        self.storage
            .get(address)
            .and_then(|s| s.get(slot))
            .copied()
            .unwrap_or(U256::ZERO)
    }

    /// Record a block hash.
    pub fn insert_block_hash(&mut self, number: u64, hash: B256) {
        self.block_hashes.insert(number, hash);
    }

    /// Get a block hash by number.
    pub fn get_block_hash(&self, number: u64) -> B256 {
        self.block_hashes
            .get(&number)
            .copied()
            .unwrap_or(B256::ZERO)
    }

    /// Check if an account exists.
    pub fn account_exists(&self, address: &Address) -> bool {
        self.accounts.contains_key(address)
    }

    /// Remove an account and all of its storage.
    pub fn remove_account(&mut self, address: &Address) {
        self.accounts.remove(address);
        self.storage.remove(address);
    }

    /// Clear all storage for an account.
    pub fn clear_storage(&mut self, address: &Address) {
        self.storage.remove(address);
    }

    /// Get all account addresses.
    pub fn accounts(&self) -> impl Iterator<Item = &Address> {
        self.accounts.keys()
    }
}
