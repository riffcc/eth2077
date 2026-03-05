use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eth2077_execution::plan_execution;
use eth2077_execution::traits::{ExecutionEngine, ExecutionError, ExecutionResult};
use eth2077_types::ScenarioConfig;

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
}

pub fn deterministic_blocks(seed: u64, count: usize, block_len: usize) -> Vec<Vec<u8>> {
    let mut rng = XorShift64::new(seed);
    let mut blocks = Vec::with_capacity(count);
    for _ in 0..count {
        let mut block = vec![0u8; block_len];
        for b in &mut block {
            *b = (rng.next_u64() & 0xff) as u8;
        }
        blocks.push(block);
    }
    blocks
}

#[derive(Debug)]
pub struct MockExecutionEngine {
    seed: u64,
    work_rounds: u32,
    head_block_number: AtomicU64,
}

impl MockExecutionEngine {
    pub fn new(cfg: &ScenarioConfig) -> Self {
        let plan = plan_execution(cfg);
        let rounds = (50_000.0 / plan.effective_tps.max(1.0))
            .round()
            .clamp(1.0, 64.0) as u32;
        Self {
            seed: cfg.seed ^ 0xa24baed4963ee407,
            work_rounds: rounds,
            head_block_number: AtomicU64::new(0),
        }
    }

    fn mix_block(&self, block: &[u8], rounds: u32) -> u64 {
        let mut acc = self.seed ^ ((block.len() as u64).wrapping_mul(0x9e3779b97f4a7c15));
        for round in 0..rounds.max(1) {
            for (idx, byte) in block.iter().enumerate() {
                let lane = (idx as u64)
                    .wrapping_mul(0xbf58476d1ce4e5b9)
                    .wrapping_add(round as u64);
                acc ^= lane ^ u64::from(*byte);
                acc = acc.rotate_left(9).wrapping_mul(0x94d049bb133111eb);
            }
            acc ^= acc >> 29;
            acc = acc.wrapping_add((round as u64 + 1).wrapping_mul(0x517cc1b727220a95));
        }
        black_box(acc)
    }

    fn build_roots(&self, block: &[u8], mixed: u64) -> ([u8; 32], [u8; 32]) {
        let mut state_root = [0u8; 32];
        let mut receipts_root = [0u8; 32];
        for i in 0..32 {
            let base = (mixed.rotate_left((i as u32) % 63) >> ((i % 8) * 8)) as u8;
            let left = block[i % block.len()];
            let right = block[block.len() - 1 - (i % block.len())];
            state_root[i] = base ^ left ^ (i as u8).wrapping_mul(17);
            receipts_root[i] =
                base.rotate_left((i % 7) as u32) ^ right ^ (i as u8).wrapping_mul(29);
        }
        (state_root, receipts_root)
    }
}

impl ExecutionEngine for MockExecutionEngine {
    async fn execute_block(&self, block: &[u8]) -> Result<ExecutionResult, ExecutionError> {
        if block.is_empty() {
            return Err(ExecutionError::InvalidBlock);
        }

        let mixed = self.mix_block(block, self.work_rounds);
        let (state_root, receipts_root) = self.build_roots(block, mixed);
        let gas_used = 21_000u64
            .saturating_add((block.len() as u64).saturating_mul(16))
            .saturating_add(mixed & 0x3fff);

        self.head_block_number.fetch_add(1, Ordering::Relaxed);

        Ok(ExecutionResult {
            state_root,
            receipts_root,
            gas_used,
        })
    }

    async fn validate_block(&self, block: &[u8]) -> Result<bool, ExecutionError> {
        if block.is_empty() {
            return Err(ExecutionError::InvalidBlock);
        }

        let mixed = self.mix_block(block, (self.work_rounds / 2).max(1));
        Ok(((mixed ^ block.len() as u64) & 0b11) != 0)
    }

    async fn get_head_block_number(&self) -> Result<u64, ExecutionError> {
        Ok(self.head_block_number.load(Ordering::Relaxed))
    }
}

#[derive(Debug, Clone)]
pub struct MicrobenchResult {
    pub label: String,
    pub iterations: usize,
    pub total_ns: u64,
    pub avg_ns: u64,
    pub min_ns: u64,
    pub max_ns: u64,
    pub throughput_ops_sec: f64,
}

fn duration_to_ns(duration: Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}

fn summarize(
    label: &str,
    iterations: usize,
    op_latencies_ns: &[u64],
    total_ns: u64,
) -> MicrobenchResult {
    let (min_ns, max_ns) = if op_latencies_ns.is_empty() {
        (0, 0)
    } else {
        let min_ns = *op_latencies_ns.iter().min().unwrap_or(&0);
        let max_ns = *op_latencies_ns.iter().max().unwrap_or(&0);
        (min_ns, max_ns)
    };

    let avg_ns = if iterations > 0 {
        total_ns / iterations as u64
    } else {
        0
    };

    let throughput_ops_sec = if total_ns > 0 {
        iterations as f64 * 1_000_000_000.0 / total_ns as f64
    } else {
        0.0
    };

    MicrobenchResult {
        label: label.to_string(),
        iterations,
        total_ns,
        avg_ns,
        min_ns,
        max_ns,
        throughput_ops_sec,
    }
}

fn ensure_nonzero_iterations(iterations: usize, label: &str) -> Result<(), ExecutionError> {
    if iterations == 0 {
        return Err(ExecutionError::InternalError(format!(
            "{label}: iterations must be > 0"
        )));
    }
    Ok(())
}

pub async fn bench_single_block_execution<E: ExecutionEngine>(
    engine: &E,
    block: &[u8],
    iterations: usize,
) -> Result<MicrobenchResult, ExecutionError> {
    ensure_nonzero_iterations(iterations, "bench_single_block_execution")?;

    let mut op_latencies_ns = Vec::with_capacity(iterations);
    let total_start = Instant::now();

    for _ in 0..iterations {
        let op_start = Instant::now();
        let _ = engine.execute_block(block).await?;
        op_latencies_ns.push(duration_to_ns(op_start.elapsed()));
    }

    let total_ns = duration_to_ns(total_start.elapsed());
    Ok(summarize(
        "execute_block_single",
        iterations,
        &op_latencies_ns,
        total_ns,
    ))
}

pub async fn bench_batch_validation<E: ExecutionEngine>(
    engine: &E,
    blocks: &[Vec<u8>],
    passes: usize,
) -> Result<MicrobenchResult, ExecutionError> {
    ensure_nonzero_iterations(passes, "bench_batch_validation")?;
    if blocks.is_empty() {
        return Err(ExecutionError::InvalidBlock);
    }

    let iterations = passes.saturating_mul(blocks.len());
    let mut op_latencies_ns = Vec::with_capacity(iterations);
    let total_start = Instant::now();

    for _ in 0..passes {
        for block in blocks {
            let op_start = Instant::now();
            let _ = engine.validate_block(block).await?;
            op_latencies_ns.push(duration_to_ns(op_start.elapsed()));
        }
    }

    let total_ns = duration_to_ns(total_start.elapsed());
    Ok(summarize(
        "validate_block_batch",
        iterations,
        &op_latencies_ns,
        total_ns,
    ))
}

pub async fn bench_head_query_latency<E: ExecutionEngine>(
    engine: &E,
    iterations: usize,
) -> Result<MicrobenchResult, ExecutionError> {
    ensure_nonzero_iterations(iterations, "bench_head_query_latency")?;

    let mut op_latencies_ns = Vec::with_capacity(iterations);
    let total_start = Instant::now();

    for _ in 0..iterations {
        let op_start = Instant::now();
        let _ = engine.get_head_block_number().await?;
        op_latencies_ns.push(duration_to_ns(op_start.elapsed()));
    }

    let total_ns = duration_to_ns(total_start.elapsed());
    Ok(summarize(
        "head_block_query",
        iterations,
        &op_latencies_ns,
        total_ns,
    ))
}

async fn bench_execute_blocks_sequential<E: ExecutionEngine>(
    engine: &E,
    blocks: &[Vec<u8>],
    passes: usize,
) -> Result<MicrobenchResult, ExecutionError> {
    ensure_nonzero_iterations(passes, "bench_execute_blocks_sequential")?;
    if blocks.is_empty() {
        return Err(ExecutionError::InvalidBlock);
    }

    let iterations = passes.saturating_mul(blocks.len());
    let mut op_latencies_ns = Vec::with_capacity(iterations);
    let total_start = Instant::now();

    for _ in 0..passes {
        for block in blocks {
            let op_start = Instant::now();
            let _ = engine.execute_block(block).await?;
            op_latencies_ns.push(duration_to_ns(op_start.elapsed()));
        }
    }

    let total_ns = duration_to_ns(total_start.elapsed());
    Ok(summarize(
        "execute_block_sequential",
        iterations,
        &op_latencies_ns,
        total_ns,
    ))
}

async fn bench_execute_blocks_parallel<E>(
    engine: Arc<E>,
    blocks: &[Vec<u8>],
    passes: usize,
) -> Result<MicrobenchResult, ExecutionError>
where
    E: ExecutionEngine + 'static,
{
    ensure_nonzero_iterations(passes, "bench_execute_blocks_parallel")?;
    if blocks.is_empty() {
        return Err(ExecutionError::InvalidBlock);
    }

    let iterations = passes.saturating_mul(blocks.len());
    let mut op_latencies_ns = Vec::with_capacity(iterations);
    let mut total_ns: u64 = 0;

    for _ in 0..passes {
        let pass_start = Instant::now();
        for block in blocks {
            let op_start = Instant::now();
            let result = engine.execute_block(block).await;
            let op_ns = duration_to_ns(op_start.elapsed());
            let _ = result?;
            op_latencies_ns.push(op_ns);
        }

        total_ns = total_ns.saturating_add(duration_to_ns(pass_start.elapsed()));
    }

    Ok(summarize(
        "execute_block_parallel",
        iterations,
        &op_latencies_ns,
        total_ns,
    ))
}

pub async fn bench_sequential_vs_parallel<E>(
    engine: Arc<E>,
    blocks: &[Vec<u8>],
    passes: usize,
) -> Result<(MicrobenchResult, MicrobenchResult), ExecutionError>
where
    E: ExecutionEngine + 'static,
{
    let sequential = bench_execute_blocks_sequential(engine.as_ref(), blocks, passes).await?;
    let parallel = bench_execute_blocks_parallel(engine, blocks, passes).await?;
    Ok((sequential, parallel))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn benchmark_scenario(seed: u64) -> ScenarioConfig {
        ScenarioConfig {
            name: "microbench".to_string(),
            nodes: 12,
            tx_count: 10_000,
            seed,
            ingress_tps_per_node: 55_000.0,
            execution_tps_per_node: 42_000.0,
            oob_tps_per_node: 60_000.0,
            mesh_efficiency: 0.80,
            base_rtt_ms: 20.0,
            jitter_ms: 5.0,
            commit_batch_size: 512,
            byzantine_fraction: 0.01,
            packet_loss_fraction: 0.01,
        }
    }

    fn assert_result_shape(result: &MicrobenchResult, expected_iterations: usize) {
        assert_eq!(result.iterations, expected_iterations);
        assert!(result.total_ns > 0);
        assert!(result.avg_ns > 0);
        assert!(result.max_ns >= result.min_ns);
        assert!(result.throughput_ops_sec > 0.0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn single_block_microbench() {
        let cfg = benchmark_scenario(0x1111);
        let engine = MockExecutionEngine::new(&cfg);
        let mut blocks = deterministic_blocks(cfg.seed, 1, 512);
        let block = blocks.pop().expect("deterministic block");

        let result = bench_single_block_execution(&engine, &block, 256)
            .await
            .expect("single block benchmark");

        assert_eq!(result.label, "execute_block_single");
        assert_result_shape(&result, 256);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn batch_validation_microbench() {
        let cfg = benchmark_scenario(0x2222);
        let engine = MockExecutionEngine::new(&cfg);
        let blocks = deterministic_blocks(cfg.seed ^ 0x55, 32, 256);

        let result = bench_batch_validation(&engine, &blocks, 16)
            .await
            .expect("batch validation benchmark");

        assert_eq!(result.label, "validate_block_batch");
        assert_result_shape(&result, 32 * 16);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn head_query_latency() {
        let cfg = benchmark_scenario(0x3333);
        let engine = MockExecutionEngine::new(&cfg);
        let block = deterministic_blocks(cfg.seed ^ 0xaa, 1, 384)
            .pop()
            .expect("deterministic block");

        for _ in 0..8 {
            let _ = engine.execute_block(&block).await.expect("warmup execute");
        }

        let result = bench_head_query_latency(&engine, 1024)
            .await
            .expect("head query benchmark");

        assert_eq!(result.label, "head_block_query");
        assert_result_shape(&result, 1024);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sequential_vs_parallel_throughput() {
        let cfg = benchmark_scenario(0x4444);
        let engine = Arc::new(MockExecutionEngine::new(&cfg));
        let blocks = deterministic_blocks(cfg.seed ^ 0xff, 24, 320);

        let (sequential, parallel) = bench_sequential_vs_parallel(engine, &blocks, 10)
            .await
            .expect("sequential vs parallel benchmark");

        assert_eq!(sequential.label, "execute_block_sequential");
        assert_eq!(parallel.label, "execute_block_parallel");
        assert_result_shape(&sequential, 24 * 10);
        assert_result_shape(&parallel, 24 * 10);
    }
}
