use serde::{Deserialize, Serialize};

/// Fingerprint of a set of witness CIDs using XOR accumulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SporeFingerprint {
    /// XOR of all CID hashes in the set
    pub xor_accumulator: [u8; 32],
    /// Number of elements in the set
    pub count: usize,
}

impl SporeFingerprint {
    /// Creates an empty SPORE fingerprint.
    pub fn new() -> Self {
        Self {
            xor_accumulator: [0u8; 32],
            count: 0,
        }
    }

    /// Inserts a CID hash into the fingerprint via XOR accumulation.
    pub fn insert(&mut self, cid_hash: &[u8; 32]) {
        for (acc, byte) in self.xor_accumulator.iter_mut().zip(cid_hash.iter()) {
            *acc ^= *byte;
        }
        self.count += 1;
    }

    /// Removes a CID hash from the fingerprint (XOR is self-inverse).
    pub fn remove(&mut self, cid_hash: &[u8; 32]) {
        for (acc, byte) in self.xor_accumulator.iter_mut().zip(cid_hash.iter()) {
            *acc ^= *byte;
        }
        self.count = self.count.saturating_sub(1);
    }

    /// Returns true when both fingerprints represent the same set summary.
    pub fn is_synced_with(&self, other: &SporeFingerprint) -> bool {
        self == other
    }

    /// Returns the XOR fingerprint of the symmetric difference.
    pub fn difference(&self, other: &SporeFingerprint) -> [u8; 32] {
        let mut diff = [0u8; 32];
        for ((out, lhs), rhs) in diff
            .iter_mut()
            .zip(self.xor_accumulator.iter())
            .zip(other.xor_accumulator.iter())
        {
            *out = *lhs ^ *rhs;
        }
        diff
    }
}

impl Default for SporeFingerprint {
    fn default() -> Self {
        Self::new()
    }
}

/// A diff-sync request from a peer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SporeSyncRequest {
    pub local_fingerprint: SporeFingerprint,
    pub peer_id: [u8; 32],
    pub requested_range: (u64, u64), // (start_block, end_block)
}

/// A diff-sync response containing missing witness CIDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SporeSyncResponse {
    pub missing_cid_hashes: Vec<[u8; 32]>,
    pub peer_fingerprint: SporeFingerprint,
}

/// Error types for SPORE sync operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SporeSyncError {
    EmptySet,
    PeerUnreachable,
    FingerprintMismatch,
    RangeOutOfBounds,
}
