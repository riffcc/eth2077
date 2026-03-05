use eth2077_types::eip8141::{
    compute_frame_tx_hash, extract_sponsorship_target, validate_frame_transaction, Frame,
    FramePhase, FrameTransaction, FrameTxError, MAX_TOTAL_GAS,
};

fn address(value: u8) -> [u8; 20] {
    [value; 20]
}

fn frame(phase: FramePhase, gas_limit: u64, target: [u8; 20]) -> Frame {
    Frame {
        phase,
        target,
        calldata: vec![0xAB, 0xCD],
        gas_limit,
        value: 1_000,
    }
}

fn tx_with_frames(frames: Vec<Frame>) -> FrameTransaction {
    FrameTransaction {
        sender: address(0x11),
        nonce: 7,
        max_fee_per_gas: 100,
        max_priority_fee_per_gas: 2,
        frames,
        signature: vec![0xAA, 0xBB],
    }
}

#[test]
fn valid_frame_tx_passes() {
    let tx = tx_with_frames(vec![
        frame(FramePhase::Validation, 21_000, address(0x01)),
        frame(FramePhase::Execution, 100_000, address(0x02)),
    ]);

    assert_eq!(validate_frame_transaction(&tx), Ok(()));
}

#[test]
fn empty_frames_rejected() {
    let tx = tx_with_frames(Vec::new());
    let errors = validate_frame_transaction(&tx).unwrap_err();

    assert!(errors.contains(&FrameTxError::EmptyFrames));
}

#[test]
fn missing_required_phase_rejected() {
    let tx = tx_with_frames(vec![frame(FramePhase::Execution, 50_000, address(0x02))]);
    let errors = validate_frame_transaction(&tx).unwrap_err();

    assert!(errors.contains(&FrameTxError::MissingPhase {
        phase: FramePhase::Validation,
    }));
}

#[test]
fn duplicate_phase_detected() {
    let tx = tx_with_frames(vec![
        frame(FramePhase::Validation, 21_000, address(0x01)),
        frame(FramePhase::Execution, 50_000, address(0x02)),
        frame(FramePhase::Execution, 60_000, address(0x03)),
    ]);
    let errors = validate_frame_transaction(&tx).unwrap_err();

    assert!(errors.contains(&FrameTxError::DuplicatePhase {
        phase: FramePhase::Execution,
    }));
}

#[test]
fn wrong_phase_order_detected() {
    let tx = tx_with_frames(vec![
        frame(FramePhase::Execution, 80_000, address(0x02)),
        frame(FramePhase::Validation, 30_000, address(0x01)),
    ]);
    let errors = validate_frame_transaction(&tx).unwrap_err();

    assert!(errors
        .iter()
        .any(|error| matches!(error, FrameTxError::InvalidPhaseOrder { .. })));
}

#[test]
fn excessive_gas_rejected() {
    let tx = tx_with_frames(vec![
        frame(FramePhase::Validation, MAX_TOTAL_GAS, address(0x01)),
        frame(FramePhase::Execution, 1, address(0x02)),
    ]);
    let errors = validate_frame_transaction(&tx).unwrap_err();

    assert!(errors
        .iter()
        .any(|error| matches!(error, FrameTxError::ExcessiveTotalGas { .. })));
}

#[test]
fn tx_hash_deterministic() {
    let tx = tx_with_frames(vec![
        frame(FramePhase::Validation, 21_000, address(0x01)),
        frame(FramePhase::Execution, 100_000, address(0x02)),
    ]);

    let first = compute_frame_tx_hash(&tx);
    let second = compute_frame_tx_hash(&tx);

    assert_eq!(first, second);
}

#[test]
fn sponsorship_target_extraction() {
    let sponsor = address(0x99);
    let tx = tx_with_frames(vec![
        frame(FramePhase::Validation, 21_000, address(0x01)),
        frame(FramePhase::Sponsorship, 40_000, sponsor),
        frame(FramePhase::Execution, 100_000, address(0x02)),
    ]);

    assert_eq!(extract_sponsorship_target(&tx), Some(sponsor));
}
