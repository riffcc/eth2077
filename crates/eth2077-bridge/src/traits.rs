use crate::engine_api::{ExecutionPayloadV3, ForkchoiceStateV1};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadStatus {
    Valid,
    Invalid(String),
    Syncing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForkchoiceStatus {
    Success,
    Reorged,
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    EngineError(String),
    Timeout,
    NotReady,
}

#[allow(async_fn_in_trait)]
pub trait BridgeService: Send + Sync {
    async fn submit_payload(&self, payload: ExecutionPayloadV3)
        -> Result<PayloadStatus, BridgeError>;

    async fn update_forkchoice(
        &self,
        state: ForkchoiceStateV1,
    ) -> Result<ForkchoiceStatus, BridgeError>;
}
