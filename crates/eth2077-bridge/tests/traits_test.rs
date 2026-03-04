use std::future::Future;
use std::pin::pin;
use std::sync::Mutex;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use eth2077_bridge::engine_api::{Bytes32, ExecutionPayloadV3, ForkchoiceStateV1, HexBytes};
use eth2077_bridge::traits::{BridgeError, BridgeService, ForkchoiceStatus, PayloadStatus};

#[derive(Default)]
struct MockBridgeState {
    last_payload: Option<ExecutionPayloadV3>,
    last_forkchoice: Option<ForkchoiceStateV1>,
}

#[derive(Default)]
struct MockBridgeService {
    state: Mutex<MockBridgeState>,
}

impl BridgeService for MockBridgeService {
    async fn submit_payload(
        &self,
        payload: ExecutionPayloadV3,
    ) -> Result<PayloadStatus, BridgeError> {
        if payload.block_hash == Bytes32::zero() {
            return Ok(PayloadStatus::Syncing);
        }
        if payload.transactions.is_empty() {
            return Ok(PayloadStatus::Invalid("payload has no transactions".to_string()));
        }

        let mut state = self.state.lock().expect("mock state lock poisoned");
        state.last_payload = Some(payload);
        Ok(PayloadStatus::Valid)
    }

    async fn update_forkchoice(
        &self,
        state: ForkchoiceStateV1,
    ) -> Result<ForkchoiceStatus, BridgeError> {
        if state.head_block_hash == Bytes32::zero() {
            return Err(BridgeError::NotReady);
        }

        let is_reorg = state.head_block_hash != state.safe_block_hash;
        let mut mock_state = self.state.lock().expect("mock state lock poisoned");
        mock_state.last_forkchoice = Some(state);

        if is_reorg {
            Ok(ForkchoiceStatus::Reorged)
        } else {
            Ok(ForkchoiceStatus::Success)
        }
    }
}

fn hash(byte: u8) -> Bytes32 {
    Bytes32(format!("0x{}", format!("{byte:02x}").repeat(32)))
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
fn submit_payload_returns_syncing_invalid_and_valid_statuses() {
    let service = MockBridgeService::default();

    let syncing = block_on(service.submit_payload(ExecutionPayloadV3::default()))
        .expect("zero hash payload should be accepted as syncing");
    assert_eq!(syncing, PayloadStatus::Syncing);

    let mut invalid_payload = ExecutionPayloadV3::default();
    invalid_payload.block_hash = hash(0x11);
    let invalid = block_on(service.submit_payload(invalid_payload))
        .expect("non-zero hash payload should return a payload status");
    assert!(matches!(
        invalid,
        PayloadStatus::Invalid(ref message) if message == "payload has no transactions"
    ));

    let mut valid_payload = ExecutionPayloadV3::default();
    valid_payload.block_hash = hash(0x22);
    valid_payload.transactions = vec![HexBytes("0x02aa".to_string())];
    let valid = block_on(service.submit_payload(valid_payload.clone()))
        .expect("valid payload should be accepted");
    assert_eq!(valid, PayloadStatus::Valid);

    let state = service.state.lock().expect("mock state lock poisoned");
    assert_eq!(state.last_payload.as_ref(), Some(&valid_payload));
}

#[test]
fn update_forkchoice_handles_not_ready_reorg_and_success() {
    let service = MockBridgeService::default();

    let not_ready = block_on(service.update_forkchoice(ForkchoiceStateV1::default()))
        .expect_err("zero head block hash should be rejected");
    assert_eq!(not_ready, BridgeError::NotReady);

    let reorg_state = ForkchoiceStateV1 {
        head_block_hash: hash(0x33),
        safe_block_hash: hash(0x44),
        finalized_block_hash: hash(0x44),
    };
    let reorged = block_on(service.update_forkchoice(reorg_state))
        .expect("forkchoice update should return a status");
    assert_eq!(reorged, ForkchoiceStatus::Reorged);

    let success_state = ForkchoiceStateV1 {
        head_block_hash: hash(0x55),
        safe_block_hash: hash(0x55),
        finalized_block_hash: hash(0x44),
    };
    let success = block_on(service.update_forkchoice(success_state.clone()))
        .expect("forkchoice update should return a status");
    assert_eq!(success, ForkchoiceStatus::Success);

    let state = service.state.lock().expect("mock state lock poisoned");
    assert_eq!(state.last_forkchoice.as_ref(), Some(&success_state));
}
