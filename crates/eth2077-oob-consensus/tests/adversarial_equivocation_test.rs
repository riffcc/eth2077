use eth2077_oob_consensus::mock::{MockOobBackend, MockOobVerifier};
use eth2077_oob_consensus::traits::{OobBackend, OobVerifier};
use eth2077_oob_consensus::types::{CommitmentEnvelope, FinalityStatus, OobError};

fn adversarial_commitment(
    block_hash: [u8; 32],
    state_root: [u8; 32],
    proposer: u64,
    sig: Vec<u8>,
) -> CommitmentEnvelope {
    CommitmentEnvelope {
        block_hash,
        block_number: 200,
        state_root,
        timestamp: 1_700_002_000,
        signature: sig,
        proposer_index: proposer,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn conflicting_state_roots_last_wins() {
    let backend = MockOobBackend::new(1);
    let block_hash = [0xAA; 32];
    let first_state_root = [0x10; 32];
    let second_state_root = [0x20; 32];

    let first = adversarial_commitment(block_hash, first_state_root, 1, vec![0xAB; 64]);
    let second = adversarial_commitment(block_hash, second_state_root, 1, vec![0xCD; 64]);

    backend
        .submit_commitment(first)
        .await
        .expect("first submit should succeed");
    backend
        .submit_commitment(second)
        .await
        .expect("second submit should succeed");

    let status = backend
        .poll_finality(block_hash)
        .await
        .expect("poll should succeed");

    match status {
        FinalityStatus::Finalized(proof) => {
            assert_eq!(proof.commitment.state_root, second_state_root);
        }
        other => panic!("expected finalized status, got: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn replay_same_commitment_idempotent() {
    let backend = MockOobBackend::new(1);
    let block_hash = [0xBB; 32];
    let state_root = [0x33; 32];
    let commitment = adversarial_commitment(block_hash, state_root, 2, vec![0xEF; 64]);

    for _ in 0..5 {
        backend
            .submit_commitment(commitment.clone())
            .await
            .expect("replay submit should succeed");
    }

    let status = backend
        .poll_finality(block_hash)
        .await
        .expect("poll should succeed");

    let proof = match status {
        FinalityStatus::Finalized(proof) => proof,
        other => panic!("expected finalized status, got: {other:?}"),
    };

    assert_eq!(proof.commitment.block_hash, block_hash);
    assert_eq!(proof.commitment.state_root, state_root);
    assert!(!proof.proof_data.is_empty());

    let latest = backend
        .get_latest_finalized()
        .await
        .expect("latest query should succeed");
    assert_eq!(latest, Some(proof));
}

#[tokio::test(flavor = "current_thread")]
async fn zero_signature_always_rejected() {
    let verifier = MockOobVerifier;
    let commitment = adversarial_commitment([0xCC; 32], [0x44; 32], 3, vec![0; 64]);

    let result = verifier.verify_commitment(&commitment);
    assert_eq!(result, Err(OobError::VerificationFailed));
}

#[tokio::test(flavor = "current_thread")]
async fn ordering_manipulation_submit_after_finality() {
    let backend = MockOobBackend::new(1);
    let block_hash = [0xDD; 32];
    let first_state_root = [0x55; 32];
    let second_state_root = [0x66; 32];

    let first = adversarial_commitment(block_hash, first_state_root, 4, vec![0x11; 64]);
    let second = adversarial_commitment(block_hash, second_state_root, 4, vec![0x22; 64]);

    backend
        .submit_commitment(first)
        .await
        .expect("first submit should succeed");

    let first_status = backend
        .poll_finality(block_hash)
        .await
        .expect("first poll should succeed");
    match first_status {
        FinalityStatus::Finalized(proof) => {
            assert_eq!(proof.commitment.state_root, first_state_root);
        }
        other => panic!("expected finalized status on first poll, got: {other:?}"),
    }

    backend
        .submit_commitment(second)
        .await
        .expect("second submit should succeed");

    let second_status = backend
        .poll_finality(block_hash)
        .await
        .expect("second poll should succeed");
    match second_status {
        FinalityStatus::Finalized(proof) => {
            assert_eq!(proof.commitment.state_root, second_state_root);
        }
        other => panic!("expected finalized status on second poll, got: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn mixed_valid_invalid_signatures() {
    let verifier = MockOobVerifier;
    let block_hash = [0xEE; 32];
    let state_root = [0x77; 32];

    let valid_ff = adversarial_commitment(block_hash, state_root, 5, vec![0xFF; 64]);
    let invalid_zero = adversarial_commitment(block_hash, state_root, 5, vec![0; 64]);
    let valid_one = adversarial_commitment(block_hash, state_root, 5, vec![0x01; 64]);

    assert_eq!(verifier.verify_commitment(&valid_ff), Ok(()));
    assert_eq!(
        verifier.verify_commitment(&invalid_zero),
        Err(OobError::VerificationFailed)
    );
    assert_eq!(verifier.verify_commitment(&valid_one), Ok(()));
}
