use eth2077_oob_consensus::mock::MockOobBackend;
use eth2077_oob_consensus::traits::OobBackend;
use eth2077_oob_consensus::types::{CommitmentEnvelope, FinalityStatus, OobError};

fn make_commitment(block_hash: [u8; 32], proposer: u64) -> CommitmentEnvelope {
    CommitmentEnvelope {
        block_hash,
        block_number: 100,
        state_root: [0xDD; 32],
        timestamp: 1_700_000_000,
        signature: vec![0xAB; 64],
        proposer_index: proposer,
    }
}

async fn poll_until_finalized(
    backend: &MockOobBackend,
    block_hash: [u8; 32],
) -> (usize, CommitmentEnvelope) {
    let mut polls = 0usize;
    loop {
        polls += 1;
        match backend
            .poll_finality(block_hash)
            .await
            .expect("poll should succeed")
        {
            FinalityStatus::Pending => {}
            FinalityStatus::Finalized(proof) => return (polls, proof.commitment),
            FinalityStatus::Failed(err) => panic!("unexpected failed status: {err:?}"),
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn partition_then_merge_finalizes() {
    let backend_a = MockOobBackend::new(2);
    let backend_b = MockOobBackend::new(2);
    let commitment = make_commitment([0x41; 32], 7);

    backend_a
        .submit_commitment(commitment.clone())
        .await
        .expect("submit on backend A should succeed");
    backend_b
        .submit_commitment(commitment.clone())
        .await
        .expect("submit on backend B should succeed");

    let mut proof_a = None;
    let mut proof_b = None;

    for _ in 0..2 {
        match backend_a
            .poll_finality(commitment.block_hash)
            .await
            .expect("poll on backend A should succeed")
        {
            FinalityStatus::Pending => {}
            FinalityStatus::Finalized(proof) => proof_a = Some(proof),
            FinalityStatus::Failed(err) => panic!("unexpected failed status on backend A: {err:?}"),
        }

        match backend_b
            .poll_finality(commitment.block_hash)
            .await
            .expect("poll on backend B should succeed")
        {
            FinalityStatus::Pending => {}
            FinalityStatus::Finalized(proof) => proof_b = Some(proof),
            FinalityStatus::Failed(err) => panic!("unexpected failed status on backend B: {err:?}"),
        }
    }

    let proof_a = proof_a.expect("backend A should finalize after 2 polls");
    let proof_b = proof_b.expect("backend B should finalize after 2 polls");
    assert_eq!(proof_a, proof_b);
}

#[tokio::test(flavor = "current_thread")]
async fn churn_rapid_proposer_rotation() {
    let backend = MockOobBackend::new(1);

    for i in 0u8..20 {
        backend
            .submit_commitment(make_commitment([i; 32], i as u64))
            .await
            .expect("submit should succeed");
    }

    let mut finalized = 0usize;
    for i in 0u8..20 {
        let (_, finalized_commitment) = poll_until_finalized(&backend, [i; 32]).await;
        assert_eq!(finalized_commitment.block_hash, [i; 32]);
        finalized += 1;
    }

    assert_eq!(finalized, 20);
    let latest = backend
        .get_latest_finalized()
        .await
        .expect("latest query should succeed");
    assert!(latest.is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn partition_stale_commitment_rejected() {
    let backend = MockOobBackend::new(3);

    let result = backend.poll_finality([0xFF; 32]).await;
    assert_eq!(result, Err(OobError::InvalidCommitment));
}

#[tokio::test(flavor = "current_thread")]
async fn asymmetric_partition_different_views() {
    let fast_backend = MockOobBackend::new(1);
    let slow_backend = MockOobBackend::new(5);
    let commitment = make_commitment([0x99; 32], 12);

    fast_backend
        .submit_commitment(commitment.clone())
        .await
        .expect("fast submit should succeed");
    slow_backend
        .submit_commitment(commitment.clone())
        .await
        .expect("slow submit should succeed");

    let (fast_polls, fast_commitment) =
        poll_until_finalized(&fast_backend, commitment.block_hash).await;
    let (slow_polls, slow_commitment) =
        poll_until_finalized(&slow_backend, commitment.block_hash).await;

    assert_eq!(fast_polls, 1);
    assert_eq!(slow_polls, 5);
    assert_eq!(fast_commitment, slow_commitment);
}
