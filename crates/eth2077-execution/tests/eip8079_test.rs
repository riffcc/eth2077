use eth2077_execution::eip8079::{
    compute_anchor_hash, compute_execution_stats, default_rollup_config, mock_execute,
    validate_execute_input, ExecutePrecompileInput, ExecutePrecompileOutput, NativeRollupError,
    RollupAnchor,
};

fn sample_anchor(block_number: u64) -> RollupAnchor {
    RollupAnchor {
        l1_state_root: [0x11; 32],
        message_root: [0x22; 32],
        rolling_hash: [0x33; 32],
        l1_block_number: block_number,
    }
}

fn sample_input(block_number: u64, tx_count: usize) -> ExecutePrecompileInput {
    ExecutePrecompileInput {
        pre_state_root: [0xAA; 32],
        transactions: vec![vec![0x01, 0x02, 0x03]; tx_count],
        anchor: sample_anchor(block_number),
    }
}

#[test]
fn valid_input_passes() {
    let config = default_rollup_config([0x44; 32]);
    let input = sample_input(64, 2);

    assert_eq!(validate_execute_input(&input, &config), Ok(()));
}

#[test]
fn empty_transactions_rejected() {
    let config = default_rollup_config([0x44; 32]);
    let mut input = sample_input(64, 1);
    input.transactions.clear();

    let errors = validate_execute_input(&input, &config).expect_err("empty tx list must fail");
    assert!(errors.contains(&NativeRollupError::EmptyTransactions));
}

#[test]
fn zero_state_root_rejected() {
    let config = default_rollup_config([0x44; 32]);
    let mut input = sample_input(64, 1);
    input.pre_state_root = [0u8; 32];

    let errors = validate_execute_input(&input, &config).expect_err("zero root must fail");
    assert!(errors.contains(&NativeRollupError::InvalidPreStateRoot));
}

#[test]
fn anchor_frequency_violation() {
    let config = default_rollup_config([0x44; 32]);
    let input = sample_input(65, 1);

    let errors =
        validate_execute_input(&input, &config).expect_err("non-divisible block must fail");

    assert!(
        errors
            .iter()
            .any(|err| matches!(err, NativeRollupError::AnchorFrequencyViolation { .. }))
    );
}

#[test]
fn mock_execute_succeeds() {
    let config = default_rollup_config([0x44; 32]);
    let input = sample_input(64, 3);

    let output = mock_execute(&input, &config).expect("execution should succeed");

    assert_eq!(output.gas_used, 63_000);
    assert!(output.success);
    assert_ne!(output.post_state_root, [0u8; 32]);
}

#[test]
fn gas_limit_exceeded() {
    let mut config = default_rollup_config([0x44; 32]);
    config.max_gas_per_execution = 50_000;
    let input = sample_input(64, 3);

    let err = mock_execute(&input, &config).expect_err("gas limit should be exceeded");
    assert_eq!(
        err,
        NativeRollupError::ExceedsGasLimit {
            used: 63_000,
            limit: 50_000,
        }
    );
}

#[test]
fn anchor_hash_deterministic() {
    let anchor = sample_anchor(96);

    let first = compute_anchor_hash(&anchor);
    let second = compute_anchor_hash(&anchor);

    assert_eq!(first, second);
}

#[test]
fn execution_stats_correct() {
    let first_input = sample_input(64, 2);
    let second_input = sample_input(96, 1);

    let first_output = ExecutePrecompileOutput {
        post_state_root: [0x01; 32],
        gas_used: 42_000,
        success: true,
    };
    let second_output = ExecutePrecompileOutput {
        post_state_root: [0x02; 32],
        gas_used: 21_000,
        success: true,
    };

    let stats = compute_execution_stats(&[
        (first_input, first_output),
        (second_input, second_output),
    ]);

    assert_eq!(stats.rollup_id, [0u8; 32]);
    assert_eq!(stats.executions_count, 2);
    assert_eq!(stats.total_gas_used, 63_000);
    assert_eq!(stats.total_transactions, 3);
    assert!((stats.avg_gas_per_execution - 31_500.0).abs() < f64::EPSILON);
}
