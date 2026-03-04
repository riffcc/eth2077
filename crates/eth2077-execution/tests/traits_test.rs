use std::future::Future;
use std::pin::pin;
use std::sync::Mutex;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use eth2077_execution::traits::{ExecutionEngine, ExecutionError, ExecutionResult};

#[derive(Default)]
struct MockExecutionEngine {
    head_block_number: Mutex<u64>,
}

impl ExecutionEngine for MockExecutionEngine {
    async fn execute_block(&self, block: &[u8]) -> Result<ExecutionResult, ExecutionError> {
        if block.is_empty() {
            return Err(ExecutionError::InvalidBlock);
        }

        let mut head = self
            .head_block_number
            .lock()
            .expect("mock state lock poisoned");
        *head += 1;

        Ok(ExecutionResult {
            state_root: [block[0]; 32],
            receipts_root: [block.len() as u8; 32],
            gas_used: block.len() as u64 * 21_000,
        })
    }

    async fn validate_block(&self, block: &[u8]) -> Result<bool, ExecutionError> {
        if block.first() == Some(&0xff) {
            return Err(ExecutionError::StateUnavailable);
        }
        Ok(!block.is_empty())
    }

    async fn get_head_block_number(&self) -> Result<u64, ExecutionError> {
        let head = self
            .head_block_number
            .lock()
            .expect("mock state lock poisoned");
        Ok(*head)
    }
}

fn no_op_raw_waker() -> RawWaker {
    unsafe fn clone(_: *const ()) -> RawWaker {
        no_op_raw_waker()
    }
    unsafe fn no_op(_: *const ()) {}

    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(std::ptr::null(), &VTABLE)
}

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    let waker = unsafe { Waker::from_raw(no_op_raw_waker()) };
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);

    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(result) => return result,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[test]
fn execute_block_updates_head_and_returns_roots() {
    let engine = MockExecutionEngine::default();

    let result = block_on(engine.execute_block(&[0x11, 0x22, 0x33]))
        .expect("execute_block should succeed for non-empty blocks");
    assert_eq!(result.state_root, [0x11; 32]);
    assert_eq!(result.receipts_root, [3; 32]);
    assert_eq!(result.gas_used, 63_000);

    let head = block_on(engine.get_head_block_number()).expect("head number should be available");
    assert_eq!(head, 1);
}

#[test]
fn validate_and_execute_report_expected_errors() {
    let engine = MockExecutionEngine::default();

    let empty_is_valid = block_on(engine.validate_block(&[]))
        .expect("empty block validation should return a boolean");
    assert!(!empty_is_valid);

    let validation_error = block_on(engine.validate_block(&[0xff]))
        .expect_err("state-unavailable marker should return an error");
    assert_eq!(validation_error, ExecutionError::StateUnavailable);

    let execution_error =
        block_on(engine.execute_block(&[])).expect_err("empty block execution should fail");
    assert_eq!(execution_error, ExecutionError::InvalidBlock);
}
