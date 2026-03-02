use eth2077_types::ScenarioConfig;

#[derive(Debug, Clone)]
pub struct BridgePlan {
    pub replay_safe: bool,
}

pub fn plan_bridge(_cfg: &ScenarioConfig) -> BridgePlan {
    BridgePlan { replay_safe: true }
}
