pub mod traits;
pub mod ingress;

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
