use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitmentEnvelope {
    pub block_hash: [u8; 32],
    pub block_number: u64,
    pub state_root: [u8; 32],
    pub timestamp: u64,
    pub signature: Vec<u8>,
    pub proposer_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalityProof {
    pub commitment: CommitmentEnvelope,
    pub proof_data: Vec<u8>,
    pub finalized_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OobError {
    NetworkError,
    InvalidCommitment,
    VerificationFailed,
    Timeout,
    BackendUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinalityStatus {
    Pending,
    Finalized(FinalityProof),
    Failed(OobError),
}
