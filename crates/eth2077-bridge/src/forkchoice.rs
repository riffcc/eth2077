use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::engine_api::{Bytes32, ForkchoiceStateV1};

#[derive(Debug, Clone, Default)]
pub struct ForkchoiceManager {
    state: Arc<RwLock<ForkchoiceState>>,
}

#[derive(Debug, Clone, Default)]
struct ForkchoiceState {
    head: Bytes32,
    safe: Bytes32,
    finalized: Bytes32,
}

impl ForkchoiceManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_head(&self, head_block_hash: Bytes32) {
        let mut state = self.write_state();
        state.head = head_block_hash;
    }

    pub fn mark_safe(&self, safe_block_hash: Bytes32) {
        let mut state = self.write_state();
        state.safe = safe_block_hash;
    }

    pub fn mark_finalized(&self, finalized_block_hash: Bytes32) {
        let mut state = self.write_state();
        state.finalized = finalized_block_hash;
    }

    pub fn get_state(&self) -> ForkchoiceStateV1 {
        let state = self.read_state();
        ForkchoiceStateV1 {
            head_block_hash: state.head.clone(),
            safe_block_hash: state.safe.clone(),
            finalized_block_hash: state.finalized.clone(),
        }
    }

    fn read_state(&self) -> RwLockReadGuard<'_, ForkchoiceState> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_state(&self) -> RwLockWriteGuard<'_, ForkchoiceState> {
        self.state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
