use eth2077_types::ScenarioConfig;

pub mod mock;
pub mod traits;
pub mod types;

#[derive(Debug, Clone)]
pub struct OobPlan {
    pub effective_tps: f64,
}

pub fn plan_oob(cfg: &ScenarioConfig) -> OobPlan {
    let base = cfg.nodes as f64 * cfg.oob_tps_per_node * cfg.mesh_efficiency;
    OobPlan {
        effective_tps: base.max(1.0),
    }
}
