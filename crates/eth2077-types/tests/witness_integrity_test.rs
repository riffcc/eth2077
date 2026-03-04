use eth2077_types::spore_sync::SporeFingerprint;
use eth2077_types::witness::{
    WitnessCid, WitnessCommitment, WitnessPayload, WitnessVerifyResult,
};

fn bytes32(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn valid_payload() -> WitnessPayload {
    WitnessPayload {
        block_hash: bytes32(0x11),
        block_number: 42,
        state_root: bytes32(0x22),
        receipts_root: bytes32(0x33),
        proof_data: vec![1, 2, 3, 4],
        timestamp: 1_700_000_000,
    }
}

#[test]
fn witness_cid_deterministic() {
    let payload_a = valid_payload();
    let payload_b = valid_payload();

    let cid_a = WitnessCid::from_payload(&payload_a);
    let cid_b = WitnessCid::from_payload(&payload_b);

    assert_eq!(cid_a.hash, cid_b.hash);
    assert_eq!(cid_a.codec, cid_b.codec);
    assert_eq!(cid_a.version, cid_b.version);
}

#[test]
fn witness_payload_validation() {
    let payload_valid = valid_payload();
    assert!(payload_valid.is_valid());

    let payload_empty_proof = WitnessPayload {
        proof_data: vec![],
        ..valid_payload()
    };
    assert!(!payload_empty_proof.is_valid());

    let payload_zero_block = WitnessPayload {
        block_number: 0,
        ..valid_payload()
    };
    assert!(!payload_zero_block.is_valid());
}

#[test]
fn witness_commitment_valid_binding() {
    let payload = valid_payload();
    let cid = WitnessCid::from_payload(&payload);
    let commitment = WitnessCommitment {
        cid,
        block_hash: payload.block_hash,
        block_number: payload.block_number,
        proposer_signature: vec![1; 64],
    };

    let result = commitment.verify_binding(&payload);
    assert!(matches!(result, WitnessVerifyResult::Valid));
}

#[test]
fn witness_commitment_invalid_cid() {
    let payload = valid_payload();
    let mut bad_cid = WitnessCid::from_payload(&payload);
    bad_cid.hash[0] ^= 0xFF;

    let commitment = WitnessCommitment {
        cid: bad_cid,
        block_hash: payload.block_hash,
        block_number: payload.block_number,
        proposer_signature: vec![1; 64],
    };

    let result = commitment.verify_binding(&payload);
    assert!(matches!(result, WitnessVerifyResult::InvalidCid));
}

#[test]
fn witness_commitment_zero_signature() {
    let payload = valid_payload();
    let cid = WitnessCid::from_payload(&payload);
    let commitment = WitnessCommitment {
        cid,
        block_hash: payload.block_hash,
        block_number: payload.block_number,
        proposer_signature: vec![0; 64],
    };

    let result = commitment.verify_binding(&payload);
    assert!(matches!(result, WitnessVerifyResult::SignatureInvalid));
}

#[test]
fn spore_fingerprint_insert_remove_inverse() {
    let hash_a = bytes32(0xA1);
    let hash_b = bytes32(0xB2);

    let mut fingerprint = SporeFingerprint::new();
    fingerprint.insert(&hash_a);
    fingerprint.insert(&hash_b);

    fingerprint.remove(&hash_a);
    let mut only_b = SporeFingerprint::new();
    only_b.insert(&hash_b);
    assert_eq!(fingerprint.count, only_b.count);
    assert_eq!(fingerprint.xor_accumulator, only_b.xor_accumulator);

    fingerprint.remove(&hash_b);
    let empty = SporeFingerprint::new();
    assert_eq!(fingerprint.count, empty.count);
    assert_eq!(fingerprint.xor_accumulator, empty.xor_accumulator);
}

#[test]
fn spore_sync_detection() {
    let hash_a = bytes32(0x41);
    let hash_b = bytes32(0x42);
    let extra_hash = bytes32(0x99);

    let mut left = SporeFingerprint::new();
    let mut right = SporeFingerprint::new();

    left.insert(&hash_a);
    left.insert(&hash_b);
    right.insert(&hash_a);
    right.insert(&hash_b);
    assert!(left.is_synced_with(&right));

    right.insert(&extra_hash);
    assert!(!left.is_synced_with(&right));
    assert_eq!(left.difference(&right), extra_hash);
}

#[test]
fn spore_empty_set() {
    let fingerprint = SporeFingerprint::new();
    assert_eq!(fingerprint.count, 0);
    assert_eq!(fingerprint.xor_accumulator, [0u8; 32]);
}
