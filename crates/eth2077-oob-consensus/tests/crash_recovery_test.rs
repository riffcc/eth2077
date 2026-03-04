use eth2077_oob_consensus::mock::MockOobBackend;
use eth2077_oob_consensus::traits::OobBackend;
use eth2077_oob_consensus::types::{CommitmentEnvelope, FinalityStatus};

fn recovery_commitment(id: u8) -> CommitmentEnvelope {
    CommitmentEnvelope {
        block_hash: [id; 32],
        block_number: id as u64,
        state_root: [id.wrapping_add(0x10); 32],
        timestamp: 1_700_003_000 + id as u64,
        signature: vec![0xCC; 64],
        proposer_index: 0,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn state_survives_clone_simulated_restart() {
    let backend = MockOobBackend::new(2);
    let commitment = recovery_commitment(1);

    backend
        .submit_commitment(commitment.clone())
        .await
        .expect("submit should succeed");

    let first_poll = backend
        .poll_finality(commitment.block_hash)
        .await
        .expect("first poll should succeed");
    assert!(matches!(first_poll, FinalityStatus::Pending));

    let restarted_backend = backend.clone();
    let second_poll = restarted_backend
        .poll_finality(commitment.block_hash)
        .await
        .expect("second poll should succeed");
    let finalized = match second_poll {
        FinalityStatus::Finalized(proof) => proof,
        other => panic!("expected finalized status after restart, got: {other:?}"),
    };

    let latest = restarted_backend
        .get_latest_finalized()
        .await
        .expect("latest query should succeed");
    assert_eq!(latest, Some(finalized));
}

#[tokio::test(flavor = "current_thread")]
async fn deterministic_state_after_crash() {
    let backend_a = MockOobBackend::new(3);
    let backend_b = MockOobBackend::new(3);
    let commitment = recovery_commitment(2);

    backend_a
        .submit_commitment(commitment.clone())
        .await
        .expect("submit to backend_a should succeed");
    backend_b
        .submit_commitment(commitment.clone())
        .await
        .expect("submit to backend_b should succeed");

    let mut proof_a = None;
    let mut proof_b = None;

    for _ in 0..3 {
        match backend_a
            .poll_finality(commitment.block_hash)
            .await
            .expect("poll on backend_a should succeed")
        {
            FinalityStatus::Pending => {}
            FinalityStatus::Finalized(proof) => proof_a = Some(proof),
            FinalityStatus::Failed(err) => panic!("unexpected failed status on backend_a: {err:?}"),
        }

        match backend_b
            .poll_finality(commitment.block_hash)
            .await
            .expect("poll on backend_b should succeed")
        {
            FinalityStatus::Pending => {}
            FinalityStatus::Finalized(proof) => proof_b = Some(proof),
            FinalityStatus::Failed(err) => panic!("unexpected failed status on backend_b: {err:?}"),
        }
    }

    assert_eq!(
        proof_a.expect("backend_a should finalize"),
        proof_b.expect("backend_b should finalize")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn recovery_preserves_poll_count() {
    let backend = MockOobBackend::new(4);
    let commitment = recovery_commitment(3);

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
    assert!(matches!(second_poll, FinalityStatus::Pending));

    let restarted_backend = backend.clone();

    let third_poll = restarted_backend
        .poll_finality(commitment.block_hash)
        .await
        .expect("third poll should succeed");
    assert!(matches!(third_poll, FinalityStatus::Pending));

    let fourth_poll = restarted_backend
        .poll_finality(commitment.block_hash)
        .await
        .expect("fourth poll should succeed");
    assert!(matches!(fourth_poll, FinalityStatus::Finalized(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn multiple_blocks_survive_restart() {
    let backend = MockOobBackend::new(1);
    let mut block_hashes = Vec::new();

    for id in 0u8..5 {
        let commitment = recovery_commitment(id);
        block_hashes.push(commitment.block_hash);
        backend
            .submit_commitment(commitment)
            .await
            .expect("submit should succeed");
    }

    let restarted_backend = backend.clone();

    for block_hash in block_hashes {
        let status = restarted_backend
            .poll_finality(block_hash)
            .await
            .expect("poll should succeed");
        assert!(matches!(status, FinalityStatus::Finalized(_)));
    }

    let latest = restarted_backend
        .get_latest_finalized()
        .await
        .expect("latest query should succeed");
    assert!(latest.is_some());
}
