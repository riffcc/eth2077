use crate::types::{CommitmentEnvelope, FinalityProof, FinalityStatus, OobError};

#[allow(async_fn_in_trait)]
pub trait OobBackend: Send + Sync {
    async fn submit_commitment(&self, commitment: CommitmentEnvelope) -> Result<(), OobError>;

    async fn poll_finality(&self, block_hash: [u8; 32]) -> Result<FinalityStatus, OobError>;

    async fn get_latest_finalized(&self) -> Result<Option<FinalityProof>, OobError>;
}

pub trait OobVerifier: Send + Sync {
    fn verify_commitment(&self, commitment: &CommitmentEnvelope) -> Result<(), OobError>;
}
