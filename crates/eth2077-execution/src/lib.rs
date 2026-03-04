pub mod blob_sidecar;
pub mod eip7702;
pub mod eip7864;
pub mod eip7928;
pub mod eip8007;
pub mod eip8070;
pub mod eip8079;
pub mod ingress;
pub mod traits;

use eth2077_types::ScenarioConfig;

#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub effective_tps: f64,
}

pub fn plan_execution(cfg: &ScenarioConfig) -> ExecutionPlan {
    let base = cfg.nodes as f64 * cfg.execution_tps_per_node;
    ExecutionPlan {
        effective_tps: base.max(1.0),
    }
}
