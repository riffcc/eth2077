pub mod engine_api;
pub mod eth_rpc;
pub mod forkchoice;
pub mod payload_converter;
pub mod traits;

use eth2077_types::ScenarioConfig;

#[derive(Debug, Clone)]
pub struct BridgePlan {
    pub replay_safe: bool,
}

pub fn plan_bridge(_cfg: &ScenarioConfig) -> BridgePlan {
    BridgePlan { replay_safe: true }
}
