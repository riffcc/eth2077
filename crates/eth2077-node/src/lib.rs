use eth2077_bridge::plan_bridge;
use eth2077_execution::plan_execution;
use eth2077_oob_consensus::plan_oob;
use eth2077_types::ScenarioConfig;

pub fn bootstrap(cfg: &ScenarioConfig) -> (f64, f64, bool) {
    let exec = plan_execution(cfg);
    let oob = plan_oob(cfg);
    let bridge = plan_bridge(cfg);
    (exec.effective_tps, oob.effective_tps, bridge.replay_safe)
}
