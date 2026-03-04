use std::collections::HashMap;
use std::sync::Mutex;

use eth2077_oob_consensus::traits::{OobBackend, OobVerifier};
use eth2077_oob_consensus::types::{CommitmentEnvelope, FinalityProof, FinalityStatus, OobError};

#[derive(Default)]
struct MockState {
    commitments: HashMap<[u8; 32], CommitmentEnvelope>,
    poll_counts: HashMap<[u8; 32], u8>,
    latest: Option<FinalityProof>,
}

#[derive(Default)]
struct MockOobBackend {
    state: Mutex<MockState>,
}

impl OobBackend for MockOobBackend {
    async fn submit_commitment(&self, commitment: CommitmentEnvelope) -> Result<(), OobError> {
        let mut state = self.state.lock().expect("mock state lock poisoned");
        state.commitments.insert(commitment.block_hash, commitment);
        Ok(())
    }

    async fn poll_finality(&self, block_hash: [u8; 32]) -> Result<FinalityStatus, OobError> {
        let mut state = self.state.lock().expect("mock state lock poisoned");
        let commitment = state
            .commitments
            .get(&block_hash)
            .cloned()
            .ok_or(OobError::InvalidCommitment)?;
        let polls = state.poll_counts.entry(block_hash).or_insert(0);
        *polls += 1;

        if *polls < 2 {
            return Ok(FinalityStatus::Pending);
        }

        let proof = FinalityProof {
            commitment,
            proof_data: vec![0xAA, 0xBB, 0xCC],
            finalized_at: 1_700_000_000,
        };
        state.latest = Some(proof.clone());
        Ok(FinalityStatus::Finalized(proof))
    }

    async fn get_latest_finalized(&self) -> Result<Option<FinalityProof>, OobError> {
        let state = self.state.lock().expect("mock state lock poisoned");
        Ok(state.latest.clone())
    }
}

struct MockOobVerifier;

impl OobVerifier for MockOobVerifier {
    fn verify_commitment(&self, commitment: &CommitmentEnvelope) -> Result<(), OobError> {
        if commitment.signature.iter().all(|b| *b == 0) {
            return Err(OobError::VerificationFailed);
        }
        Ok(())
    }
}

fn sample_commitment() -> CommitmentEnvelope {
    CommitmentEnvelope {
        block_hash: [0x11; 32],
        block_number: 42,
        state_root: [0x22; 32],
        timestamp: 1_700_000_123,
        signature: vec![0xAB; 64],
        proposer_index: 7,
    }
}

#[test]
fn submit_poll_and_verify_cycle() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime should build");

    runtime.block_on(async {
        let backend = MockOobBackend::default();
        let verifier = MockOobVerifier;
        let commitment = sample_commitment();

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

        verifier
            .verify_commitment(&finalized.commitment)
            .expect("verifier should accept commitment");

        let latest = backend
            .get_latest_finalized()
            .await
            .expect("latest finalized query should succeed");
        assert_eq!(latest, Some(finalized));
    });
}
