use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use crate::traits::{OobBackend, OobVerifier};
use crate::types::{CommitmentEnvelope, FinalityProof, FinalityStatus, OobError};

#[derive(Debug)]
pub struct MockState {
    pub commitments: Vec<CommitmentEnvelope>,
    pub finalized_block_hashes: HashSet<[u8; 32]>,
    pub poll_counts: HashMap<[u8; 32], usize>,
    pub finality_delay: usize,
    pub latest_finalized: Option<FinalityProof>,
}

#[derive(Debug, Clone)]
pub struct MockOobBackend {
    state: Arc<RwLock<MockState>>,
}

impl MockOobBackend {
    pub fn new(finality_delay: usize) -> Self {
        let state = MockState {
            commitments: Vec::new(),
            finalized_block_hashes: HashSet::new(),
            poll_counts: HashMap::new(),
            finality_delay,
            latest_finalized: None,
        };

        Self {
            state: Arc::new(RwLock::new(state)),
        }
    }
}

impl OobBackend for MockOobBackend {
    async fn submit_commitment(&self, commitment: CommitmentEnvelope) -> Result<(), OobError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| OobError::BackendUnavailable)?;
        state.commitments.push(commitment);
        Ok(())
    }

    async fn poll_finality(&self, block_hash: [u8; 32]) -> Result<FinalityStatus, OobError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| OobError::BackendUnavailable)?;

        let commitment = state
            .commitments
            .iter()
            .rev()
            .find(|c| c.block_hash == block_hash)
            .cloned()
            .ok_or(OobError::InvalidCommitment)?;

        let poll_count = state.poll_counts.entry(block_hash).or_insert(0);
        *poll_count += 1;

        if *poll_count < state.finality_delay {
            return Ok(FinalityStatus::Pending);
        }

        let proof = FinalityProof {
            commitment,
            proof_data: vec![0x4D, 0x4F, 0x43, 0x4B],
            finalized_at: state.finality_delay as u64,
        };

        state.finalized_block_hashes.insert(block_hash);
        state.latest_finalized = Some(proof.clone());

        Ok(FinalityStatus::Finalized(proof))
    }

    async fn get_latest_finalized(&self) -> Result<Option<FinalityProof>, OobError> {
        let state = self
            .state
            .read()
            .map_err(|_| OobError::BackendUnavailable)?;
        Ok(state.latest_finalized.clone())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MockOobVerifier;

impl OobVerifier for MockOobVerifier {
    fn verify_commitment(&self, commitment: &CommitmentEnvelope) -> Result<(), OobError> {
        if commitment.signature.iter().all(|&b| b == 0) {
            return Err(OobError::VerificationFailed);
        }
        Ok(())
    }
}
