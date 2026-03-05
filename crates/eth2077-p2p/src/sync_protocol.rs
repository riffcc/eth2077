use eth2077_types::canonical::Block;

use crate::codec::WireMessage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncState {
    Idle,
    Syncing {
        target_height: u64,
        current_height: u64,
    },
    Synced,
}

#[derive(Debug, Clone)]
pub struct SyncEngine {
    pub local_height: u64,
    pub state: SyncState,
    pub batch_size: u32,
}

impl SyncEngine {
    pub fn new(local_height: u64) -> Self {
        Self {
            local_height,
            state: SyncState::Idle,
            batch_size: 64,
        }
    }

    pub fn needs_sync(&self, network_best_height: u64) -> bool {
        network_best_height > self.local_height
    }

    pub fn start_sync(&mut self, target_height: u64) {
        if target_height <= self.local_height {
            self.state = SyncState::Synced;
            return;
        }

        self.state = SyncState::Syncing {
            target_height,
            current_height: self.local_height,
        };
    }

    pub fn next_request(&self) -> Option<WireMessage> {
        match self.state {
            SyncState::Syncing {
                target_height,
                current_height,
            } if current_height < target_height => Some(WireMessage::GetBlocks {
                from_height: current_height.saturating_add(1),
                count: self.batch_size,
            }),
            _ => None,
        }
    }

    pub fn on_blocks_received(&mut self, blocks: &[Block]) {
        if blocks.is_empty() {
            return;
        }

        if let Some(best_height) = blocks.iter().map(Block::number).max() {
            self.local_height = self.local_height.max(best_height);
        }

        if let SyncState::Syncing {
            target_height,
            current_height,
        } = &mut self.state
        {
            *current_height = self.local_height;
            if *current_height >= *target_height {
                self.state = SyncState::Synced;
            }
        }
    }

    pub fn sync_progress(&self) -> (u64, u64) {
        match self.state {
            SyncState::Syncing {
                target_height,
                current_height,
            } => (current_height, target_height),
            SyncState::Idle | SyncState::Synced => (self.local_height, self.local_height),
        }
    }
}

impl Default for SyncEngine {
    fn default() -> Self {
        Self::new(0)
    }
}
