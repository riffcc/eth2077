use eth2077_types::eip8025::{
    check_client_diversity, compute_attestation_stats, compute_proof_commitment,
    default_proof_config, validate_proof_attestation, ClientImplementation, ExecutionProof,
    ProofAttestation, ProofConsensusConfig, ProofValidationError,
};

fn bytes32(value: u8) -> [u8; 32] {
    [value; 32]
}

fn make_proof(
    client: ClientImplementation,
    block_hash: [u8; 32],
    state_root_after: [u8; 32],
    proof_data: Vec<u8>,
) -> ExecutionProof {
    ExecutionProof {
        block_number: 1_000,
        block_hash,
        state_root_before: bytes32(0x10),
        state_root_after,
        proof_data,
        prover_client: client,
    }
}

fn sample_attestation(proofs: Vec<ExecutionProof>) -> ProofAttestation {
    ProofAttestation {
        block_number: 1_000,
        block_hash: bytes32(0xAA),
        proofs,
        attester_id: bytes32(0x42),
    }
}

#[test]
fn valid_attestation_passes() {
    let root = bytes32(0xBB);
    let attestation = sample_attestation(vec![
        make_proof(
            ClientImplementation::Geth,
            bytes32(0xAA),
            root,
            vec![1, 2, 3],
        ),
        make_proof(
            ClientImplementation::Nethermind,
            bytes32(0xAA),
            root,
            vec![4, 5, 6],
        ),
        make_proof(
            ClientImplementation::Besu,
            bytes32(0xAA),
            root,
            vec![7, 8, 9],
        ),
    ]);

    assert_eq!(
        validate_proof_attestation(&attestation, &default_proof_config()),
        Ok(())
    );
}

#[test]
fn empty_proofs_rejected() {
    let attestation = sample_attestation(Vec::new());

    let errors = validate_proof_attestation(&attestation, &default_proof_config()).unwrap_err();
    assert!(errors.contains(&ProofValidationError::EmptyProofs));
}

#[test]
fn insufficient_diversity_rejected() {
    let root = bytes32(0xBB);
    let attestation = sample_attestation(vec![
        make_proof(ClientImplementation::Geth, bytes32(0xAA), root, vec![1]),
        make_proof(
            ClientImplementation::Nethermind,
            bytes32(0xAA),
            root,
            vec![2],
        ),
    ]);

    let errors = validate_proof_attestation(&attestation, &default_proof_config()).unwrap_err();
    assert!(
        errors.contains(&ProofValidationError::InsufficientClientDiversity {
            unique_clients: 2,
            required: 3,
        })
    );
}

#[test]
fn duplicate_client_detected() {
    let root = bytes32(0xBB);
    let config = ProofConsensusConfig {
        required_client_diversity: 2,
        total_known_clients: 5,
    };
    let attestation = sample_attestation(vec![
        make_proof(ClientImplementation::Geth, bytes32(0xAA), root, vec![1]),
        make_proof(ClientImplementation::Geth, bytes32(0xAA), root, vec![2]),
        make_proof(
            ClientImplementation::Nethermind,
            bytes32(0xAA),
            root,
            vec![3],
        ),
    ]);

    let errors = validate_proof_attestation(&attestation, &config).unwrap_err();
    assert!(errors.contains(&ProofValidationError::DuplicateClient {
        client: ClientImplementation::Geth,
    }));
}

#[test]
fn block_hash_mismatch_detected() {
    let root = bytes32(0xBB);
    let attestation = sample_attestation(vec![
        make_proof(ClientImplementation::Geth, bytes32(0xAA), root, vec![1]),
        make_proof(
            ClientImplementation::Nethermind,
            bytes32(0xAB),
            root,
            vec![2],
        ),
        make_proof(ClientImplementation::Besu, bytes32(0xAA), root, vec![3]),
    ]);

    let errors = validate_proof_attestation(&attestation, &default_proof_config()).unwrap_err();
    assert!(errors.contains(&ProofValidationError::BlockHashMismatch { proof_index: 1 }));
}

#[test]
fn state_root_inconsistency_detected() {
    let attestation = sample_attestation(vec![
        make_proof(
            ClientImplementation::Geth,
            bytes32(0xAA),
            bytes32(0xBB),
            vec![1],
        ),
        make_proof(
            ClientImplementation::Nethermind,
            bytes32(0xAA),
            bytes32(0xCC),
            vec![2],
        ),
        make_proof(
            ClientImplementation::Besu,
            bytes32(0xAA),
            bytes32(0xBB),
            vec![3],
        ),
    ]);

    let errors = validate_proof_attestation(&attestation, &default_proof_config()).unwrap_err();
    assert!(errors.contains(&ProofValidationError::StateRootMismatch { proof_index: 1 }));
}

#[test]
fn proof_commitment_deterministic() {
    let proof = make_proof(
        ClientImplementation::Erigon,
        bytes32(0xAA),
        bytes32(0xBB),
        vec![1, 2, 3, 4],
    );

    let first = compute_proof_commitment(&proof);
    let second = compute_proof_commitment(&proof);
    assert_eq!(first, second);
}

#[test]
fn stats_computation_correct() {
    let root = bytes32(0xBB);
    let attestation = sample_attestation(vec![
        make_proof(ClientImplementation::Geth, bytes32(0xAA), root, vec![1, 2]),
        make_proof(
            ClientImplementation::Nethermind,
            bytes32(0xAA),
            root,
            vec![3, 4, 5],
        ),
        make_proof(ClientImplementation::Besu, bytes32(0xAA), root, vec![6]),
    ]);
    let config = default_proof_config();

    let stats = compute_attestation_stats(&attestation, &config);
    assert_eq!(stats.block_number, 1_000);
    assert_eq!(stats.unique_clients, 3);
    assert_eq!(stats.total_proof_bytes, 6);
    assert!(stats.meets_diversity_threshold);
    assert!(stats.state_roots_consistent);
    assert!(check_client_diversity(
        &attestation.proofs,
        config.required_client_diversity
    ));
}
