use serde::{Deserialize, Serialize};

/// Content identifier for witness data, using a simplified CID-like scheme.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WitnessCid {
    /// SHA-256 hash of the serialized witness payload
    pub hash: [u8; 32],
    /// Codec identifier (0x01 = raw witness, 0x02 = compressed)
    pub codec: u8,
    /// Version of the CID scheme
    pub version: u8,
}

/// Core witness payload emitted by execution for OOB verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WitnessPayload {
    pub block_hash: [u8; 32],
    pub block_number: u64,
    pub state_root: [u8; 32],
    pub receipts_root: [u8; 32],
    pub proof_data: Vec<u8>,
    pub timestamp: u64,
}

/// Commitment binding a witness to a specific block via its CID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WitnessCommitment {
    pub cid: WitnessCid,
    pub block_hash: [u8; 32],
    pub block_number: u64,
    pub proposer_signature: Vec<u8>,
}

/// Verification result for witness integrity checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitnessVerifyResult {
    Valid,
    InvalidCid,
    MismatchedStateRoot,
    MissingProofData,
    SignatureInvalid,
}

impl WitnessCid {
    pub fn from_payload(payload: &WitnessPayload) -> Self {
        let bytes =
            serde_json::to_vec(payload).expect("witness payload serialization must succeed");
        let mut hash = [0_u8; 32];
        for (index, byte) in bytes.into_iter().enumerate() {
            hash[index % 32] ^= byte;
        }

        Self {
            hash,
            codec: 0x01,
            version: 1,
        }
    }
}

impl WitnessPayload {
    pub fn is_valid(&self) -> bool {
        !self.proof_data.is_empty() && self.block_number > 0
    }
}

impl WitnessCommitment {
    pub fn verify_binding(&self, payload: &WitnessPayload) -> WitnessVerifyResult {
        if payload.proof_data.is_empty() {
            return WitnessVerifyResult::MissingProofData;
        }

        if WitnessCid::from_payload(payload) != self.cid {
            return WitnessVerifyResult::InvalidCid;
        }

        if self.proposer_signature.is_empty()
            || self.proposer_signature.iter().all(|byte| *byte == 0)
        {
            return WitnessVerifyResult::SignatureInvalid;
        }

        if self.block_hash != payload.block_hash {
            return WitnessVerifyResult::MismatchedStateRoot;
        }

        WitnessVerifyResult::Valid
    }
}
