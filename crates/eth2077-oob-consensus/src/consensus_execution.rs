use eth2077_types::canonical::{Block, Header};

pub trait ConsensusExecutionBridge: Send + Sync {
    fn propose_block(&self, parent: &Header, height: u64) -> Result<Block, String>;
    fn validate_proposed_block(
        &self,
        block: &Block,
        pre_state_hint: &[u8],
    ) -> Result<[u8; 32], String>;
    fn finalize_block(&self, block_hash: [u8; 32], height: u64) -> Result<(), String>;
}

#[derive(Debug, Clone, Default)]
pub struct MockConsensusExecutionBridge {
    proposed_block: Option<Block>,
}

impl MockConsensusExecutionBridge {
    pub fn new(proposed_block: Option<Block>) -> Self {
        Self { proposed_block }
    }
}

impl ConsensusExecutionBridge for MockConsensusExecutionBridge {
    fn propose_block(&self, _parent: &Header, _height: u64) -> Result<Block, String> {
        self.proposed_block
            .clone()
            .ok_or_else(|| "mock propose_block: no block configured".to_string())
    }

    fn validate_proposed_block(
        &self,
        block: &Block,
        _pre_state_hint: &[u8],
    ) -> Result<[u8; 32], String> {
        let mut hash = [0u8; 32];
        hash.copy_from_slice(block.hash().as_slice());
        Ok(hash)
    }

    fn finalize_block(&self, _block_hash: [u8; 32], _height: u64) -> Result<(), String> {
        Ok(())
    }
}
