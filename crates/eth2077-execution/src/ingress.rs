use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub hash: [u8; 32],
    pub from: [u8; 20],
    pub to: [u8; 20],
    pub nonce: u64,
    pub gas_limit: u64,
    pub gas_price: u64,
    pub value: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngressError {
    DuplicateTransaction,
    InvalidNonce,
    InsufficientGas,
    MempoolFull,
    ValidationFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolConfig {
    pub max_size: usize,
    pub max_per_account: usize,
    pub min_gas_price: u64,
    pub eviction_threshold_percent: u64,
}

#[derive(Debug, Clone)]
pub struct Mempool {
    config: MempoolConfig,
    transactions: BTreeMap<[u8; 32], Transaction>,
    account_counts: BTreeMap<[u8; 20], usize>,
    account_nonces: BTreeMap<[u8; 20], BTreeSet<u64>>,
}

impl Mempool {
    pub fn new(config: MempoolConfig) -> Self {
        Self {
            config,
            transactions: BTreeMap::new(),
            account_counts: BTreeMap::new(),
            account_nonces: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, tx: Transaction) -> Result<(), IngressError> {
        self.validate_candidate(&tx)?;

        if self.is_full() {
            let evictions = self.suggested_eviction_count();
            if evictions == 0 {
                return Err(IngressError::MempoolFull);
            }
            self.evict_lowest_gas(evictions);
        }

        if self.is_full() {
            return Err(IngressError::MempoolFull);
        }

        self.account_counts
            .entry(tx.from)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        self.account_nonces
            .entry(tx.from)
            .or_default()
            .insert(tx.nonce);
        self.transactions.insert(tx.hash, tx);
        Ok(())
    }

    pub fn remove(&mut self, tx_hash: &[u8; 32]) -> Option<Transaction> {
        let tx = self.transactions.remove(tx_hash)?;
        self.decrement_account_state(&tx);
        Some(tx)
    }

    pub fn peek_best(&self, count: usize) -> Vec<&Transaction> {
        let mut sorted: Vec<&Transaction> = self.transactions.values().collect();
        sorted.sort_unstable_by(|a, b| {
            b.gas_price
                .cmp(&a.gas_price)
                .then_with(|| a.nonce.cmp(&b.nonce))
                .then_with(|| a.hash.cmp(&b.hash))
        });
        sorted.truncate(count);
        sorted
    }

    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn is_full(&self) -> bool {
        self.len() >= self.config.max_size
    }

    pub fn clear(&mut self) {
        self.transactions.clear();
        self.account_counts.clear();
        self.account_nonces.clear();
    }

    fn validate_candidate(&self, tx: &Transaction) -> Result<(), IngressError> {
        if self.transactions.contains_key(&tx.hash) {
            return Err(IngressError::DuplicateTransaction);
        }

        if tx.gas_limit == 0 || tx.gas_price < self.config.min_gas_price {
            return Err(IngressError::InsufficientGas);
        }

        if self
            .account_nonces
            .get(&tx.from)
            .is_some_and(|nonces| nonces.contains(&tx.nonce))
        {
            return Err(IngressError::InvalidNonce);
        }

        if self
            .account_counts
            .get(&tx.from)
            .copied()
            .unwrap_or_default()
            >= self.config.max_per_account
        {
            return Err(IngressError::ValidationFailed(
                "account mempool limit reached".to_string(),
            ));
        }

        Ok(())
    }

    fn suggested_eviction_count(&self) -> usize {
        let percent = self.config.eviction_threshold_percent.min(100) as usize;
        if percent == 0 {
            return 0;
        }

        let max_size = self.config.max_size.max(1);
        ((max_size * percent) / 100).max(1)
    }

    fn evict_lowest_gas(&mut self, mut to_evict: usize) {
        if to_evict == 0 || self.transactions.is_empty() {
            return;
        }

        let mut candidates: Vec<([u8; 32], u64, u64)> = self
            .transactions
            .values()
            .map(|tx| (tx.hash, tx.gas_price, tx.nonce))
            .collect();
        candidates.sort_unstable_by(|a, b| a.1.cmp(&b.1).then_with(|| b.2.cmp(&a.2)));

        for (hash, _, _) in candidates {
            if !self.is_full() || to_evict == 0 {
                break;
            }
            let _ = self.remove(&hash);
            to_evict -= 1;
        }
    }

    fn decrement_account_state(&mut self, tx: &Transaction) {
        let mut remove_count_entry = false;
        if let Some(count) = self.account_counts.get_mut(&tx.from) {
            *count = count.saturating_sub(1);
            remove_count_entry = *count == 0;
        }
        if remove_count_entry {
            self.account_counts.remove(&tx.from);
        }

        let mut remove_nonce_entry = false;
        if let Some(nonces) = self.account_nonces.get_mut(&tx.from) {
            nonces.remove(&tx.nonce);
            remove_nonce_entry = nonces.is_empty();
        }
        if remove_nonce_entry {
            self.account_nonces.remove(&tx.from);
        }
    }
}

#[derive(Debug, Clone)]
pub struct IngressPipeline {
    mempool: Mempool,
}

impl IngressPipeline {
    pub fn new(mempool: Mempool) -> Self {
        Self { mempool }
    }

    pub fn validate_transaction(&self, tx: &Transaction) -> Result<(), IngressError> {
        self.mempool.validate_candidate(tx)
    }

    pub fn submit(&mut self, tx: Transaction) -> Result<(), IngressError> {
        self.validate_transaction(&tx)?;
        self.mempool.insert(tx)
    }

    pub fn drain_batch(&mut self, max_count: usize) -> Vec<Transaction> {
        let best_hashes: Vec<[u8; 32]> = self
            .mempool
            .peek_best(max_count)
            .into_iter()
            .map(|tx| tx.hash)
            .collect();

        best_hashes
            .into_iter()
            .filter_map(|hash| self.mempool.remove(&hash))
            .collect()
    }

    pub fn pending_count(&self) -> usize {
        self.mempool.len()
    }
}
