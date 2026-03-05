use std::collections::{HashSet, VecDeque};

use eth2077_types::canonical::{Block, Transaction};
use tracing::debug;

use crate::codec::WireMessage;

#[derive(Debug, Clone, Copy)]
pub struct GossipConfig {
    pub seen_cache_size: usize,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            seen_cache_size: 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GossipEngine {
    config: GossipConfig,
    pub seen_blocks: HashSet<[u8; 32]>,
    pub seen_txs: HashSet<[u8; 32]>,
    seen_block_order: VecDeque<[u8; 32]>,
    seen_tx_order: VecDeque<[u8; 32]>,
}

impl GossipEngine {
    pub fn new(config: GossipConfig) -> Self {
        Self {
            config,
            seen_blocks: HashSet::new(),
            seen_txs: HashSet::new(),
            seen_block_order: VecDeque::new(),
            seen_tx_order: VecDeque::new(),
        }
    }

    pub fn should_relay_block(&mut self, hash: [u8; 32]) -> bool {
        if self.seen_blocks.contains(&hash) {
            return false;
        }

        self.seen_blocks.insert(hash);
        self.seen_block_order.push_back(hash);
        self.evict_old_blocks();
        true
    }

    pub fn should_relay_tx(&mut self, hash: [u8; 32]) -> bool {
        if self.seen_txs.contains(&hash) {
            return false;
        }

        self.seen_txs.insert(hash);
        self.seen_tx_order.push_back(hash);
        self.evict_old_txs();
        true
    }

    pub fn on_new_block(&mut self, block: &Block) -> Option<WireMessage> {
        let hash: [u8; 32] = block.hash().into();
        if !self.should_relay_block(hash) {
            return None;
        }

        let block_data = match serde_json::to_vec(block) {
            Ok(bytes) => bytes,
            Err(error) => {
                debug!(%error, "failed to serialize block for gossip");
                return None;
            }
        };

        Some(WireMessage::NewBlock {
            height: block.number(),
            block_data,
        })
    }

    pub fn on_new_tx(&mut self, tx: &Transaction) -> Option<WireMessage> {
        let hash: [u8; 32] = tx.hash.into();
        if !self.should_relay_tx(hash) {
            return None;
        }

        let tx_data = match serde_json::to_vec(tx) {
            Ok(bytes) => bytes,
            Err(error) => {
                debug!(%error, "failed to serialize transaction for gossip");
                return None;
            }
        };

        Some(WireMessage::NewTransaction { tx_data })
    }

    fn evict_old_blocks(&mut self) {
        while self.seen_blocks.len() > self.config.seen_cache_size {
            if let Some(oldest) = self.seen_block_order.pop_front() {
                self.seen_blocks.remove(&oldest);
            }
        }
    }

    fn evict_old_txs(&mut self) {
        while self.seen_txs.len() > self.config.seen_cache_size {
            if let Some(oldest) = self.seen_tx_order.pop_front() {
                self.seen_txs.remove(&oldest);
            }
        }
    }
}

impl Default for GossipEngine {
    fn default() -> Self {
        Self::new(GossipConfig::default())
    }
}
