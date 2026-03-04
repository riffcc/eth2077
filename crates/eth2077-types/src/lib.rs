use serde::{Deserialize, Serialize};

pub mod aggregate_throughput;
pub mod assumption_checker;
pub mod aps_integration;
pub mod attester_includer;
pub mod bals_integration;
pub mod blob_streaming;
pub mod claim_integrity;
pub mod da_throughput;
pub mod ec_broadcast;
pub mod ef_architecture;
pub mod eip7919;
pub mod eip7938;
pub mod eip7999;
pub mod eip8025;
pub mod eip8077;
pub mod eip8141;
pub mod eip_delta_review;
pub mod eip_portability;
pub mod epbs;
pub mod epbs_integration;
pub mod fast_l1;
pub mod focil;
pub mod focil_integration;
pub mod formal_verification;
pub mod gigagas_l1;
pub mod hyperscale_state;
pub mod lucid_mempool;
pub mod million_tps;
pub mod ntt_crypto;
pub mod one_round_finality;
pub mod post_quantum;
pub mod quick_slots;
pub mod riscv_migration;
pub mod shielded_transfers;
pub mod spore_sync;
pub mod teragas_l2;
pub mod theorem_registry;
pub mod threat_model;
pub mod vops;
pub mod whisk_ssle;
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
