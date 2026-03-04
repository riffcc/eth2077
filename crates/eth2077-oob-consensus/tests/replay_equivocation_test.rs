use eth2077_oob_consensus::mock::MockOobBackend;
use eth2077_oob_consensus::traits::OobBackend;
use eth2077_oob_consensus::types::{CommitmentEnvelope, FinalityProof, FinalityStatus, OobError};

fn sample_commitment(
    block_hash: [u8; 32],
    state_root: [u8; 32],
    proposer_index: u64,
) -> CommitmentEnvelope {
    CommitmentEnvelope {
        block_hash,
        block_number: 77,
        state_root,
        timestamp: 1_700_001_000,
        signature: vec![0xAB; 64],
        proposer_index,
    }
}

async fn finalized_after_polls(
    backend: &MockOobBackend,
    block_hash: [u8; 32],
    polls: usize,
) -> FinalityProof {
    let mut finalized = None;
    for _ in 0..polls {
        let status = backend
            .poll_finality(block_hash)
            .await
            .expect("poll should succeed");
        if let FinalityStatus::Finalized(proof) = status {
            finalized = Some(proof);
        }
    }

    finalized.expect("block should be finalized by the expected poll count")
}

async fn polls_until_finalized(backend: &MockOobBackend, block_hash: [u8; 32]) -> usize {
    let mut polls = 0usize;
    loop {
        polls += 1;
        match backend
            .poll_finality(block_hash)
            .await
            .expect("poll should succeed")
        {
            FinalityStatus::Pending => {}
            FinalityStatus::Finalized(_) => return polls,
            FinalityStatus::Failed(err) => panic!("unexpected failed status: {err:?}"),
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn deterministic_replay_produces_same_finality() {
    let finality_delay = 3usize;
    let backend_a = MockOobBackend::new(finality_delay);
    let backend_b = MockOobBackend::new(finality_delay);

    let commitment = sample_commitment([0x41; 32], [0x51; 32], 9);

    backend_a
        .submit_commitment(commitment.clone())
        .await
        .expect("submit should succeed");
    backend_b
        .submit_commitment(commitment.clone())
        .await
        .expect("submit should succeed");

    let proof_a = finalized_after_polls(&backend_a, commitment.block_hash, finality_delay).await;
    let proof_b = finalized_after_polls(&backend_b, commitment.block_hash, finality_delay).await;

    assert_eq!(proof_a, proof_b);
}

#[tokio::test(flavor = "current_thread")]
async fn equivocation_detected_different_state_root() {
    let backend = MockOobBackend::new(1);
    let block_hash = [0x61; 32];

    let first = sample_commitment(block_hash, [0xA1; 32], 3);
    let second = sample_commitment(block_hash, [0xB2; 32], 3);

    backend
        .submit_commitment(first.clone())
        .await
        .expect("first submit should succeed");
    backend
        .submit_commitment(second.clone())
        .await
        .expect("second submit should succeed");

    let status = backend
        .poll_finality(block_hash)
        .await
        .expect("poll should succeed");

    let proof = match status {
        FinalityStatus::Finalized(proof) => proof,
        other => panic!("expected finalized status, got: {other:?}"),
    };

    assert_eq!(proof.commitment.block_hash, block_hash);
    assert_eq!(proof.commitment.state_root, second.state_root);
    assert_ne!(proof.commitment.state_root, first.state_root);
}

#[tokio::test(flavor = "current_thread")]
async fn equivocation_detected_different_proposer() {
    let backend = MockOobBackend::new(1);
    let block_hash = [0x71; 32];
    let state_root = [0xC1; 32];

    let first = sample_commitment(block_hash, state_root, 1);
    let second = sample_commitment(block_hash, state_root, 2);

    backend
        .submit_commitment(first.clone())
        .await
        .expect("first submit should succeed");
    backend
        .submit_commitment(second.clone())
        .await
        .expect("second submit should succeed");

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
    assert_eq!(proof.commitment.proposer_index, second.proposer_index);
    assert_ne!(proof.commitment.proposer_index, first.proposer_index);
}

#[tokio::test(flavor = "current_thread")]
async fn replay_with_different_finality_delay() {
    let fast_backend = MockOobBackend::new(2);
    let slow_backend = MockOobBackend::new(5);

    let commitment = sample_commitment([0x81; 32], [0x82; 32], 4);

    fast_backend
        .submit_commitment(commitment.clone())
        .await
        .expect("fast submit should succeed");
    slow_backend
        .submit_commitment(commitment.clone())
        .await
        .expect("slow submit should succeed");

    let fast_polls = polls_until_finalized(&fast_backend, commitment.block_hash).await;
    let slow_polls = polls_until_finalized(&slow_backend, commitment.block_hash).await;

    assert_eq!(fast_polls, 2);
    assert_eq!(slow_polls, 5);
    assert!(fast_polls < slow_polls);
}

#[tokio::test(flavor = "current_thread")]
async fn concurrent_submit_and_poll() {
    let backend = MockOobBackend::new(2);
    let total_blocks = 8u8;

    let mut submit_tasks = Vec::new();
    let mut poll_tasks = Vec::new();

    for id in 0..total_blocks {
        let submit_backend = backend.clone();
        let poll_backend = backend.clone();
        let block_hash = [id; 32];
        let commitment = sample_commitment(block_hash, [id.wrapping_add(1); 32], id as u64);

        submit_tasks.push(tokio::spawn(async move {
            submit_backend
                .submit_commitment(commitment)
                .await
                .expect("submit should succeed");
        }));

        poll_tasks.push(tokio::spawn(async move {
            loop {
                match poll_backend.poll_finality(block_hash).await {
                    Ok(FinalityStatus::Pending) => tokio::task::yield_now().await,
                    Ok(FinalityStatus::Finalized(proof)) => return proof,
                    Ok(FinalityStatus::Failed(err)) => {
                        panic!("unexpected failed status: {err:?}")
                    }
                    Err(OobError::InvalidCommitment) => tokio::task::yield_now().await,
                    Err(err) => panic!("unexpected poll error: {err:?}"),
                }
            }
        }));
    }

    for task in submit_tasks {
        task.await.expect("submit task should not panic");
    }

    let mut finalized_hashes = Vec::new();
    for task in poll_tasks {
        let proof = task.await.expect("poll task should not panic");
        finalized_hashes.push(proof.commitment.block_hash);
    }

    assert_eq!(finalized_hashes.len(), total_blocks as usize);
    for id in 0..total_blocks {
        assert!(finalized_hashes.contains(&[id; 32]));
    }

    let latest = backend
        .get_latest_finalized()
        .await
        .expect("latest query should succeed");
    assert!(latest.is_some());
}
