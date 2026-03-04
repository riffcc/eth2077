use serde::{Deserialize, Serialize};

pub mod assumption_checker;
pub mod eip7919;
pub mod epbs;
pub mod focil;
pub mod spore_sync;
pub mod theorem_registry;
pub mod witness;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioConfig {
    pub name: String,
    pub nodes: usize,
    pub tx_count: usize,
    pub seed: u64,
    pub ingress_tps_per_node: f64,
    pub execution_tps_per_node: f64,
    pub oob_tps_per_node: f64,
    pub mesh_efficiency: f64,
    pub base_rtt_ms: f64,
    pub jitter_ms: f64,
    pub commit_batch_size: usize,
    pub byzantine_fraction: f64,
    pub packet_loss_fraction: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub name: String,
    pub nodes: usize,
    pub tx_count: usize,
    pub sustained_tps: f64,
    pub p50_finality_ms: f64,
    pub p95_finality_ms: f64,
    pub p99_finality_ms: f64,
    pub avg_finality_ms: f64,
    pub makespan_s: f64,
    pub ingress_capacity_tps: f64,
    pub execution_capacity_tps: f64,
    pub oob_capacity_tps: f64,
    pub bottleneck: String,
}
