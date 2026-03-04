use eth2077_oob_consensus::mock::{MockOobBackend, MockOobVerifier};
use eth2077_oob_consensus::traits::{OobBackend, OobVerifier};
use eth2077_oob_consensus::types::{CommitmentEnvelope, FinalityStatus, OobError};

fn sample_commitment(block_hash: [u8; 32], signature: Vec<u8>) -> CommitmentEnvelope {
    CommitmentEnvelope {
        block_hash,
        block_number: 42,
        state_root: [0x22; 32],
        timestamp: 1_700_000_123,
        signature,
        proposer_index: 7,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn submit_poll_cycle_honors_delay() {
    let backend = MockOobBackend::new(2);
    let commitment = sample_commitment([0x11; 32], vec![0xAB; 64]);

    backend
        .submit_commitment(commitment.clone())
        .await
        .expect("submit should succeed");

    let first_poll = backend
        .poll_finality(commitment.block_hash)
        .await
        .expect("first poll should succeed");
    assert!(matches!(first_poll, FinalityStatus::Pending));

    let second_poll = backend
        .poll_finality(commitment.block_hash)
        .await
        .expect("second poll should succeed");
    let finalized = match second_poll {
        FinalityStatus::Finalized(proof) => proof,
        other => panic!("expected finalized status, got: {other:?}"),
    };

    assert_eq!(finalized.commitment.block_hash, commitment.block_hash);
}

#[tokio::test(flavor = "current_thread")]
async fn latest_finalized_none_then_some_after_finalization() {
    let backend = MockOobBackend::new(1);
    let commitment = sample_commitment([0x22; 32], vec![0xCD; 64]);

    let initial = backend
        .get_latest_finalized()
        .await
        .expect("query should succeed");
    assert_eq!(initial, None);

    backend
        .submit_commitment(commitment.clone())
        .await
        .expect("submit should succeed");

    let poll = backend
        .poll_finality(commitment.block_hash)
        .await
        .expect("poll should succeed");
    let finalized = match poll {
        FinalityStatus::Finalized(proof) => proof,
        other => panic!("expected finalized status, got: {other:?}"),
    };

    let latest = backend
        .get_latest_finalized()
        .await
        .expect("query should succeed");
    assert_eq!(latest, Some(finalized));
}

#[tokio::test(flavor = "current_thread")]
async fn verifier_rejects_all_zero_signature() {
    let verifier = MockOobVerifier;
    let bad_commitment = sample_commitment([0x33; 32], vec![0; 64]);

    let result = verifier.verify_commitment(&bad_commitment);
    assert_eq!(result, Err(OobError::VerificationFailed));

    let good_commitment = sample_commitment([0x34; 32], vec![0x01; 64]);
    assert!(verifier.verify_commitment(&good_commitment).is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn backend_supports_access_from_cloned_handles() {
    let backend = MockOobBackend::new(1);
    let submit_handle = backend.clone();
    let poll_handle = backend.clone();

    let commitment = sample_commitment([0x44; 32], vec![0xEF; 64]);
    let block_hash = commitment.block_hash;

    let submit_task = tokio::spawn(async move {
        submit_handle
            .submit_commitment(commitment)
            .await
            .expect("submit should succeed");
    });

    let poll_task = tokio::spawn(async move {
        loop {
            match poll_handle.poll_finality(block_hash).await {
                Ok(status) => return status,
                Err(OobError::InvalidCommitment) => tokio::task::yield_now().await,
                Err(err) => panic!("unexpected poll error: {err:?}"),
            }
        }
    });

    submit_task.await.expect("submit task should not panic");
    let status = poll_task.await.expect("poll task should not panic");

    assert!(matches!(status, FinalityStatus::Finalized(_)));
}
