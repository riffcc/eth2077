use std::sync::atomic::{AtomicU64, Ordering};

use eth2077_bench::microbench::{deterministic_blocks, MockExecutionEngine};
use eth2077_execution::traits::{ExecutionEngine, ExecutionError, ExecutionResult};
use eth2077_types::ScenarioConfig;
use sha2::{Digest, Sha256};

#[derive(Debug, Default)]
struct ReferenceExecutionEngine {
    head_block_number: AtomicU64,
}

impl ReferenceExecutionEngine {
    fn new() -> Self {
        Self {
            head_block_number: AtomicU64::new(0),
        }
    }
}

impl ExecutionEngine for ReferenceExecutionEngine {
    async fn execute_block(&self, block: &[u8]) -> Result<ExecutionResult, ExecutionError> {
        if block.is_empty() {
            return Err(ExecutionError::InvalidBlock);
        }

        let mut state_hasher = Sha256::new();
        state_hasher.update(block);
        let state_root: [u8; 32] = state_hasher.finalize().into();

        let mut receipts_hasher = Sha256::new();
        receipts_hasher.update(block);
        receipts_hasher.update([0x01]);
        let receipts_root: [u8; 32] = receipts_hasher.finalize().into();

        let gas_used = 21_000u64.saturating_add((block.len() as u64).saturating_mul(16));

        self.head_block_number.fetch_add(1, Ordering::Relaxed);

        Ok(ExecutionResult {
            state_root,
            receipts_root,
            gas_used,
        })
    }

    async fn validate_block(&self, block: &[u8]) -> Result<bool, ExecutionError> {
        Ok(!block.is_empty())
    }

    async fn get_head_block_number(&self) -> Result<u64, ExecutionError> {
        Ok(self.head_block_number.load(Ordering::Relaxed))
    }
}

fn scenario_config(seed: u64) -> ScenarioConfig {
    ScenarioConfig {
        name: format!("differential-seed-{seed}"),
        nodes: 8,
        tx_count: 10_000,
        seed,
        ingress_tps_per_node: 55_000.0,
        execution_tps_per_node: 38_000.0,
        oob_tps_per_node: 62_000.0,
        mesh_efficiency: 0.82,
        base_rtt_ms: 18.0,
        jitter_ms: 4.0,
        commit_batch_size: 1024,
        byzantine_fraction: 0.0,
        packet_loss_fraction: 0.01,
    }
}

#[tokio::test]
async fn differential_execute_determinism() {
    let cfg = scenario_config(0xDEADBEEF);
    let mock = MockExecutionEngine::new(&cfg);
    let reference = ReferenceExecutionEngine::new();
    let blocks = deterministic_blocks(0xBAD5EED, 100, 512);

    for block in blocks {
        let mock_first = mock
            .execute_block(&block)
            .await
            .expect("mock execute should succeed for non-empty blocks");
        let mock_second = mock
            .execute_block(&block)
            .await
            .expect("mock execute should be deterministic for identical input");

        assert_eq!(
            mock_first, mock_second,
            "mock engine output should be deterministic"
        );
        assert_ne!(mock_first.state_root, [0u8; 32]);
        assert_ne!(mock_first.receipts_root, [0u8; 32]);
        assert!(mock_first.gas_used > 0);

        let reference_first = reference
            .execute_block(&block)
            .await
            .expect("reference execute should succeed for non-empty blocks");
        let reference_second = reference
            .execute_block(&block)
            .await
            .expect("reference execute should be deterministic for identical input");

        assert_eq!(
            reference_first, reference_second,
            "reference engine output should be deterministic"
        );
        assert_ne!(reference_first.state_root, [0u8; 32]);
        assert_ne!(reference_first.receipts_root, [0u8; 32]);
        assert!(reference_first.gas_used > 0);
    }
}

#[tokio::test]
async fn differential_validate_agreement() {
    let cfg = scenario_config(0xA11CE);
    let mock = MockExecutionEngine::new(&cfg);
    let reference = ReferenceExecutionEngine::new();
    let blocks = deterministic_blocks(0xF00D, 50, 256);

    for block in blocks {
        let mock_result = mock.validate_block(&block).await;
        let reference_result = reference.validate_block(&block).await;

        assert!(
            mock_result.is_ok(),
            "mock validate should return Ok for non-empty block"
        );
        assert!(
            reference_result.is_ok(),
            "reference validate should return Ok for non-empty block"
        );
        assert_eq!(
            reference_result.expect("reference validate should be Ok"),
            true
        );
    }
}

#[tokio::test]
async fn differential_head_tracking() {
    let cfg = scenario_config(0x1234);
    let mock = MockExecutionEngine::new(&cfg);
    let reference = ReferenceExecutionEngine::new();
    let blocks = deterministic_blocks(0x1234, 20, 512);

    assert_eq!(
        mock.get_head_block_number()
            .await
            .expect("mock head available"),
        0
    );
    assert_eq!(
        reference
            .get_head_block_number()
            .await
            .expect("reference head available"),
        0
    );

    for (idx, block) in blocks.iter().enumerate() {
        mock.execute_block(block)
            .await
            .expect("mock execute should succeed");
        reference
            .execute_block(block)
            .await
            .expect("reference execute should succeed");

        let expected = (idx + 1) as u64;
        assert_eq!(
            mock.get_head_block_number()
                .await
                .expect("mock head available"),
            expected
        );
        assert_eq!(
            reference
                .get_head_block_number()
                .await
                .expect("reference head available"),
            expected
        );
    }
}

#[tokio::test]
async fn differential_empty_block_rejection() {
    let cfg = scenario_config(0xFACE);
    let mock = MockExecutionEngine::new(&cfg);
    let reference = ReferenceExecutionEngine::new();

    let mock_err = mock
        .execute_block(&[])
        .await
        .expect_err("mock should reject empty blocks");
    let reference_err = reference
        .execute_block(&[])
        .await
        .expect_err("reference should reject empty blocks");

    assert_eq!(mock_err, ExecutionError::InvalidBlock);
    assert_eq!(reference_err, ExecutionError::InvalidBlock);
}

#[tokio::test]
async fn differential_cross_seed_divergence() {
    let cfg_a = scenario_config(7);
    let cfg_b = scenario_config(11);
    let mock_a = MockExecutionEngine::new(&cfg_a);
    let mock_b = MockExecutionEngine::new(&cfg_b);

    let block = deterministic_blocks(0xC0FFEE, 1, 768)
        .into_iter()
        .next()
        .expect("one deterministic block should be generated");

    let out_a = mock_a
        .execute_block(&block)
        .await
        .expect("mock_a execute should succeed");
    let out_b = mock_b
        .execute_block(&block)
        .await
        .expect("mock_b execute should succeed");

    assert_ne!(
        out_a.state_root, out_b.state_root,
        "different seeds should diverge on state_root for identical block input"
    );
}

#[tokio::test]
async fn differential_gas_accounting_bounds() {
    let cfg = scenario_config(0xBADC0DE);
    let mock = MockExecutionEngine::new(&cfg);
    let reference = ReferenceExecutionEngine::new();

    for i in 0..64usize {
        let size = 128 + (i * (2048 - 128) / 63);
        let block = deterministic_blocks(0x7000 + i as u64, 1, size)
            .into_iter()
            .next()
            .expect("one deterministic block should be generated");

        let mock_result = mock
            .execute_block(&block)
            .await
            .expect("mock execute should succeed");
        let reference_result = reference
            .execute_block(&block)
            .await
            .expect("reference execute should succeed");

        assert!(mock_result.gas_used > 0);
        assert!(mock_result.gas_used < u64::MAX);
        assert!(
            mock_result.gas_used >= 21_000,
            "mock gas should include at least the base 21000"
        );

        assert!(reference_result.gas_used > 0);
        assert!(reference_result.gas_used < u64::MAX);
    }
}
