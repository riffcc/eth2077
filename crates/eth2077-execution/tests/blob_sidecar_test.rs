use eth2077_execution::blob_sidecar::{
    compute_bundle_stats, mock_versioned_hash, validate_blob_bundle_structure,
    validate_versioned_hashes, BlobBundle, BlobBundleStats, BlobSidecar, BlobValidationError,
    BLOB_SIZE, MAX_BLOBS_PER_BLOCK,
};

fn make_sidecar(index: u64, seed: u8) -> BlobSidecar {
    let blob = vec![seed; BLOB_SIZE];
    let kzg_commitment = (0..48)
        .map(|offset| seed.wrapping_add(offset as u8))
        .collect::<Vec<_>>();
    let kzg_proof = (0..48)
        .map(|offset| seed.wrapping_add(offset as u8).wrapping_add(1))
        .collect::<Vec<_>>();

    BlobSidecar {
        index,
        blob,
        kzg_commitment,
        kzg_proof,
    }
}

fn make_bundle(blob_count: usize) -> BlobBundle {
    let blobs = (0..blob_count)
        .map(|i| make_sidecar(i as u64, i as u8 + 10))
        .collect::<Vec<_>>();
    let versioned_hashes = blobs
        .iter()
        .map(|sidecar| mock_versioned_hash(&sidecar.kzg_commitment))
        .collect::<Vec<_>>();

    BlobBundle {
        blobs,
        versioned_hashes,
    }
}

#[test]
fn test_valid_single_blob() {
    let bundle = make_bundle(1);
    let result = validate_blob_bundle_structure(&bundle);
    assert_eq!(result, Ok(()));
}

#[test]
fn test_valid_max_blobs() {
    let bundle = make_bundle(MAX_BLOBS_PER_BLOCK);
    let result = validate_blob_bundle_structure(&bundle);
    assert_eq!(result, Ok(()));
}

#[test]
fn test_too_many_blobs() {
    let bundle = make_bundle(MAX_BLOBS_PER_BLOCK + 1);
    let result = validate_blob_bundle_structure(&bundle);

    assert_eq!(
        result,
        Err(BlobValidationError::TooManyBlobs {
            count: MAX_BLOBS_PER_BLOCK + 1,
            max: MAX_BLOBS_PER_BLOCK,
        })
    );
}

#[test]
fn test_empty_bundle() {
    let bundle = BlobBundle {
        blobs: Vec::new(),
        versioned_hashes: Vec::new(),
    };

    let result = validate_blob_bundle_structure(&bundle);
    assert_eq!(result, Err(BlobValidationError::EmptyBlobSidecar));
}

#[test]
fn test_wrong_blob_size() {
    let mut sidecar = make_sidecar(0, 1);
    sidecar.blob = vec![0u8; BLOB_SIZE - 1];

    let versioned_hashes = vec![mock_versioned_hash(&sidecar.kzg_commitment)];
    let bundle = BlobBundle {
        blobs: vec![sidecar],
        versioned_hashes,
    };

    let result = validate_blob_bundle_structure(&bundle);
    assert_eq!(
        result,
        Err(BlobValidationError::BlobSizeMismatch {
            index: 0,
            actual: BLOB_SIZE - 1,
            expected: BLOB_SIZE,
        })
    );
}

#[test]
fn test_versioned_hash_validation_pass() {
    let bundle = make_bundle(3);
    let result = validate_versioned_hashes(&bundle);
    assert_eq!(result, Ok(()));
}

#[test]
fn test_versioned_hash_mismatch() {
    let mut bundle = make_bundle(1);
    bundle.versioned_hashes[0][31] ^= 0xaa;

    let result = validate_versioned_hashes(&bundle);
    assert!(matches!(
        result,
        Err(BlobValidationError::VersionedHashMismatch { index: 0, .. })
    ));
}

#[test]
fn test_bundle_stats_computation() {
    let bundle = make_bundle(2);
    let stats: BlobBundleStats = compute_bundle_stats(&bundle);

    assert_eq!(stats.blob_count, 2);
    assert_eq!(stats.total_blob_bytes, 2 * BLOB_SIZE);
    assert_eq!(stats.total_commitment_bytes, 2 * 48);
    assert_eq!(stats.versioned_hash_version, 0x01);
}
