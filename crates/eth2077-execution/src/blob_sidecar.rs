use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Maximum blobs per block (EIP-4844 Deneb target)
pub const MAX_BLOBS_PER_BLOCK: usize = 6;
/// Blob size in bytes (131072 = 128 KiB)
pub const BLOB_SIZE: usize = 131072;
/// Number of field elements per blob
pub const FIELD_ELEMENTS_PER_BLOB: usize = 4096;

const VERSIONED_HASH_VERSION_KZG: u8 = 0x01;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobValidationError {
    TooManyBlobs {
        count: usize,
        max: usize,
    },
    BlobSizeMismatch {
        index: usize,
        actual: usize,
        expected: usize,
    },
    CommitmentMismatch {
        index: usize,
    },
    ProofVerificationFailed {
        index: usize,
    },
    VersionedHashMismatch {
        index: usize,
        expected: [u8; 32],
        actual: [u8; 32],
    },
    EmptyBlobSidecar,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlobSidecar {
    pub index: u64,
    pub blob: Vec<u8>,
    pub kzg_commitment: Vec<u8>, // 48 bytes
    pub kzg_proof: Vec<u8>,      // 48 bytes
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlobBundle {
    pub blobs: Vec<BlobSidecar>,
    pub versioned_hashes: Vec<[u8; 32]>,
}

/// Validate structural properties of a blob bundle (no cryptographic verification).
/// Checks: blob count limits, blob sizes, index ordering, versioned hash format.
pub fn validate_blob_bundle_structure(bundle: &BlobBundle) -> Result<(), BlobValidationError> {
    if bundle.blobs.is_empty() {
        return Err(BlobValidationError::EmptyBlobSidecar);
    }

    let blob_count = bundle.blobs.len();
    if blob_count > MAX_BLOBS_PER_BLOCK {
        return Err(BlobValidationError::TooManyBlobs {
            count: blob_count,
            max: MAX_BLOBS_PER_BLOCK,
        });
    }

    if bundle.versioned_hashes.len() != blob_count {
        return Err(BlobValidationError::CommitmentMismatch {
            index: bundle.versioned_hashes.len().min(blob_count),
        });
    }

    for (index, sidecar) in bundle.blobs.iter().enumerate() {
        if sidecar.index != index as u64 {
            return Err(BlobValidationError::CommitmentMismatch { index });
        }

        if sidecar.blob.len() != BLOB_SIZE {
            return Err(BlobValidationError::BlobSizeMismatch {
                index,
                actual: sidecar.blob.len(),
                expected: BLOB_SIZE,
            });
        }
    }

    for (index, versioned_hash) in bundle.versioned_hashes.iter().enumerate() {
        if versioned_hash[0] != VERSIONED_HASH_VERSION_KZG {
            let mut expected = *versioned_hash;
            expected[0] = VERSIONED_HASH_VERSION_KZG;
            return Err(BlobValidationError::VersionedHashMismatch {
                index,
                expected,
                actual: *versioned_hash,
            });
        }
    }

    Ok(())
}

/// Compute a mock versioned hash from a KZG commitment (SHA-256 based for testing).
/// Real implementation would use the actual KZG ceremony trusted setup.
/// Format: 0x01 ++ SHA-256(commitment)[1..32]
pub fn mock_versioned_hash(kzg_commitment: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(kzg_commitment);
    let mut out = [0u8; 32];
    out[0] = VERSIONED_HASH_VERSION_KZG;
    out[1..].copy_from_slice(&digest[1..]);
    out
}

/// Validate that versioned hashes match their corresponding commitments.
/// Uses mock_versioned_hash for now (real KZG to be integrated later).
pub fn validate_versioned_hashes(bundle: &BlobBundle) -> Result<(), BlobValidationError> {
    if bundle.versioned_hashes.len() != bundle.blobs.len() {
        return Err(BlobValidationError::CommitmentMismatch {
            index: bundle.versioned_hashes.len().min(bundle.blobs.len()),
        });
    }

    for (index, (sidecar, expected_hash)) in bundle
        .blobs
        .iter()
        .zip(bundle.versioned_hashes.iter())
        .enumerate()
    {
        let actual_hash = mock_versioned_hash(&sidecar.kzg_commitment);
        if actual_hash != *expected_hash {
            return Err(BlobValidationError::VersionedHashMismatch {
                index,
                expected: *expected_hash,
                actual: actual_hash,
            });
        }
    }

    Ok(())
}

/// Summary statistics for a validated blob bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobBundleStats {
    pub blob_count: usize,
    pub total_blob_bytes: usize,
    pub total_commitment_bytes: usize,
    pub versioned_hash_version: u8,
}

pub fn compute_bundle_stats(bundle: &BlobBundle) -> BlobBundleStats {
    BlobBundleStats {
        blob_count: bundle.blobs.len(),
        total_blob_bytes: bundle.blobs.iter().map(|sidecar| sidecar.blob.len()).sum(),
        total_commitment_bytes: bundle
            .blobs
            .iter()
            .map(|sidecar| sidecar.kzg_commitment.len())
            .sum(),
        versioned_hash_version: bundle
            .versioned_hashes
            .first()
            .map(|hash| hash[0])
            .unwrap_or(VERSIONED_HASH_VERSION_KZG),
    }
}
