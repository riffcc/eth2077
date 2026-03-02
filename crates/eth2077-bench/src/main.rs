use std::env;
use std::fs;

use eth2077_node::bootstrap;
use eth2077_types::{ScenarioConfig, ScenarioResult};

#[derive(Debug, Clone)]
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        let s = if seed == 0 { 0x9e3779b97f4a7c15 } else { seed };
        Self { state: s }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx]
}

fn scenario_hash(name: &str) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in name.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

fn simulate(cfg: &ScenarioConfig) -> ScenarioResult {
    let (execution_capacity_raw, oob_capacity_raw, _bridge_ok) = bootstrap(cfg);

    let ingress_capacity = (cfg.nodes as f64 * cfg.ingress_tps_per_node).max(1.0);

    let adversarial_penalty =
        (1.0 - (cfg.byzantine_fraction * 0.35) - (cfg.packet_loss_fraction * 0.5)).clamp(0.30, 1.0);

    let execution_capacity = (execution_capacity_raw * (0.9 + 0.1 * adversarial_penalty)).max(1.0);
    let oob_capacity = (oob_capacity_raw * adversarial_penalty).max(1.0);

    let mut latencies_ms = Vec::with_capacity(cfg.tx_count);

    let mut exec_cursor_s = 0.0f64;
    let mut oob_cursor_s = 0.0f64;

    let mut rng = XorShift64::new(cfg.seed ^ scenario_hash(&cfg.name));

    let mut start = 0usize;
    while start < cfg.tx_count {
        let end = (start + cfg.commit_batch_size).min(cfg.tx_count);
        let batch_len = end - start;

        let submit_last_s = (end.saturating_sub(1) as f64) / ingress_capacity;

        let exec_start = exec_cursor_s.max(submit_last_s);
        exec_cursor_s = exec_start + (batch_len as f64 / execution_capacity);

        let oob_start = oob_cursor_s.max(exec_cursor_s);
        oob_cursor_s = oob_start + (batch_len as f64 / oob_capacity);

        let fanout = 2.0 + 0.6 * (cfg.nodes as f64).log2();
        let base_net_delay_ms = cfg.base_rtt_ms * fanout * (1.0 + cfg.packet_loss_fraction * 0.8);
        let jitter = (rng.next_f64() * 2.0 - 1.0) * cfg.jitter_ms;
        let byz_drag_ms = cfg.byzantine_fraction * 150.0;

        let finality_delay_s = ((base_net_delay_ms + jitter + byz_drag_ms).max(1.0)) / 1000.0;
        let batch_final_s = oob_cursor_s + finality_delay_s;

        for tx in start..end {
            let submit_s = tx as f64 / ingress_capacity;
            let latency_ms = (batch_final_s - submit_s) * 1000.0;
            latencies_ms.push(latency_ms);
        }

        start = end;
    }

    latencies_ms.sort_by(|a, b| a.total_cmp(b));
    let sum_latency: f64 = latencies_ms.iter().copied().sum();
    let avg_latency = if latencies_ms.is_empty() {
        0.0
    } else {
        sum_latency / latencies_ms.len() as f64
    };

    let makespan_s = latencies_ms
        .last()
        .copied()
        .map(|ms| ms / 1000.0)
        .unwrap_or(0.0)
        + ((cfg.tx_count.saturating_sub(1) as f64) / ingress_capacity);

    let sustained_tps = if makespan_s > 0.0 {
        cfg.tx_count as f64 / makespan_s
    } else {
        0.0
    };

    let bottleneck = {
        let mut pairs = vec![
            ("ingress", ingress_capacity),
            ("execution", execution_capacity),
            ("oob_consensus", oob_capacity),
        ];
        pairs.sort_by(|a, b| a.1.total_cmp(&b.1));
        pairs[0].0.to_string()
    };

    ScenarioResult {
        name: cfg.name.clone(),
        nodes: cfg.nodes,
        tx_count: cfg.tx_count,
        sustained_tps,
        p50_finality_ms: percentile(&latencies_ms, 0.50),
        p95_finality_ms: percentile(&latencies_ms, 0.95),
        p99_finality_ms: percentile(&latencies_ms, 0.99),
        avg_finality_ms: avg_latency,
        makespan_s,
        ingress_capacity_tps: ingress_capacity,
        execution_capacity_tps: execution_capacity,
        oob_capacity_tps: oob_capacity,
        bottleneck,
    }
}

fn default_scenarios(seed: u64, tx_count: usize) -> Vec<ScenarioConfig> {
    vec![
        ScenarioConfig {
            name: "mesh-8n-baseline".to_string(),
            nodes: 8,
            tx_count,
            seed,
            ingress_tps_per_node: 55_000.0,
            execution_tps_per_node: 38_000.0,
            oob_tps_per_node: 62_000.0,
            mesh_efficiency: 0.82,
            base_rtt_ms: 18.0,
            jitter_ms: 4.0,
            commit_batch_size: 1024,
            byzantine_fraction: 0.00,
            packet_loss_fraction: 0.01,
        },
        ScenarioConfig {
            name: "mesh-16n-baseline".to_string(),
            nodes: 16,
            tx_count,
            seed,
            ingress_tps_per_node: 55_000.0,
            execution_tps_per_node: 38_000.0,
            oob_tps_per_node: 62_000.0,
            mesh_efficiency: 0.78,
            base_rtt_ms: 22.0,
            jitter_ms: 6.0,
            commit_batch_size: 1024,
            byzantine_fraction: 0.02,
            packet_loss_fraction: 0.015,
        },
        ScenarioConfig {
            name: "mesh-32n-baseline".to_string(),
            nodes: 32,
            tx_count,
            seed,
            ingress_tps_per_node: 55_000.0,
            execution_tps_per_node: 38_000.0,
            oob_tps_per_node: 62_000.0,
            mesh_efficiency: 0.73,
            base_rtt_ms: 28.0,
            jitter_ms: 9.0,
            commit_batch_size: 1024,
            byzantine_fraction: 0.03,
            packet_loss_fraction: 0.02,
        },
        ScenarioConfig {
            name: "mesh-48n-scale".to_string(),
            nodes: 48,
            tx_count,
            seed,
            ingress_tps_per_node: 55_000.0,
            execution_tps_per_node: 38_000.0,
            oob_tps_per_node: 62_000.0,
            mesh_efficiency: 0.69,
            base_rtt_ms: 33.0,
            jitter_ms: 11.0,
            commit_batch_size: 1024,
            byzantine_fraction: 0.03,
            packet_loss_fraction: 0.025,
        },
        ScenarioConfig {
            name: "mesh-32n-adversarial".to_string(),
            nodes: 32,
            tx_count,
            seed,
            ingress_tps_per_node: 55_000.0,
            execution_tps_per_node: 38_000.0,
            oob_tps_per_node: 62_000.0,
            mesh_efficiency: 0.73,
            base_rtt_ms: 35.0,
            jitter_ms: 14.0,
            commit_batch_size: 1024,
            byzantine_fraction: 0.10,
            packet_loss_fraction: 0.05,
        },
    ]
}

fn arg_value(args: &[String], flag: &str, default: &str) -> String {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
        .unwrap_or_else(|| default.to_string())
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let seed: u64 = arg_value(&args, "--seed", "2077").parse().unwrap_or(2077);
    let tx_count: usize = arg_value(&args, "--tx-count", "500000")
        .parse()
        .unwrap_or(500000);
    let output_json = arg_value(
        &args,
        "--output-json",
        "reports/eth2077-mesh-bench-2026-03-02.json",
    );
    let output_md = arg_value(
        &args,
        "--output-md",
        "reports/eth2077-mesh-bench-2026-03-02.md",
    );

    let scenarios = default_scenarios(seed, tx_count);
    let results: Vec<ScenarioResult> = scenarios.iter().map(simulate).collect();

    let json = serde_json::to_string_pretty(&results).expect("serialize results");
    fs::write(&output_json, json).expect("write json report");

    let mut md = String::new();
    md.push_str("# ETH2077 Deterministic Mesh Benchmark\n\n");
    md.push_str("This is a deterministic synthetic benchmark for ETH-like workload flow over a Citadel-mesh-style model.\n");
    md.push_str("It is a planning baseline, not yet a live full-client throughput claim.\n\n");
    md.push_str("## Parameters\n\n");
    md.push_str(&format!("- seed: `{}`\n", seed));
    md.push_str(&format!("- tx_count per scenario: `{}`\n", tx_count));
    md.push_str("- commit_batch_size: `1024`\n\n");

    md.push_str("## Results\n\n");
    md.push_str("| Scenario | Nodes | Sustained TPS | p50 Finality (ms) | p95 Finality (ms) | p99 Finality (ms) | Bottleneck |\n");
    md.push_str("|---|---:|---:|---:|---:|---:|---|\n");

    for r in &results {
        md.push_str(&format!(
            "| {} | {} | {:.0} | {:.1} | {:.1} | {:.1} | {} |\n",
            r.name,
            r.nodes,
            r.sustained_tps,
            r.p50_finality_ms,
            r.p95_finality_ms,
            r.p99_finality_ms,
            r.bottleneck
        ));
    }

    md.push_str("\n## Notes\n\n");
    md.push_str("1. Numbers are deterministic under fixed seed and scenario definitions.\n");
    md.push_str("2. Next step is wiring the same harness shape to live node runtime paths for empirical multi-node validation.\n");

    fs::write(&output_md, md).expect("write markdown report");

    println!("Wrote JSON report: {}", output_json);
    println!("Wrote Markdown report: {}", output_md);
    for r in &results {
        println!(
            "{} => TPS {:.0}, finality p50/p95/p99 {:.1}/{:.1}/{:.1} ms (bottleneck: {})",
            r.name,
            r.sustained_tps,
            r.p50_finality_ms,
            r.p95_finality_ms,
            r.p99_finality_ms,
            r.bottleneck
        );
    }
}
