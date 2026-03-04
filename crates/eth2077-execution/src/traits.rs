#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    pub state_root: [u8; 32],
    pub receipts_root: [u8; 32],
    pub gas_used: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    InvalidBlock,
    StateUnavailable,
    InternalError(String),
}

#[allow(async_fn_in_trait)]
pub trait ExecutionEngine: Send + Sync {
    async fn execute_block(&self, block: &[u8]) -> Result<ExecutionResult, ExecutionError>;

    async fn validate_block(&self, block: &[u8]) -> Result<bool, ExecutionError>;

    async fn get_head_block_number(&self) -> Result<u64, ExecutionError>;
}
