use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ClientImplementation {
    Geth,
    Nethermind,
    Besu,
    Erigon,
    Reth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionProof {
    pub block_number: u64,
    pub block_hash: [u8; 32],
    pub state_root_before: [u8; 32],
    pub state_root_after: [u8; 32],
    pub proof_data: Vec<u8>,
    pub prover_client: ClientImplementation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProofAttestation {
    pub block_number: u64,
    pub block_hash: [u8; 32],
    pub proofs: Vec<ExecutionProof>,
    pub attester_id: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProofValidationError {
    InsufficientClientDiversity {
        unique_clients: usize,
        required: usize,
    },
    BlockHashMismatch {
        proof_index: usize,
    },
    EmptyProofs,
    DuplicateClient {
        client: ClientImplementation,
    },
    StateRootMismatch {
        proof_index: usize,
    },
    ProofDataEmpty {
        proof_index: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProofConsensusConfig {
    pub required_client_diversity: usize, // e.g., 3 out of 5
    pub total_known_clients: usize,       // e.g., 5
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProofAttestationStats {
    pub block_number: u64,
    pub unique_clients: usize,
    pub total_proof_bytes: usize,
    pub meets_diversity_threshold: bool,
    pub state_roots_consistent: bool,
}

pub fn default_proof_config() -> ProofConsensusConfig {
    ProofConsensusConfig {
        required_client_diversity: 3,
        total_known_clients: 5,
    }
}

fn client_discriminant(client: ClientImplementation) -> u8 {
    match client {
        ClientImplementation::Geth => 0,
        ClientImplementation::Nethermind => 1,
        ClientImplementation::Besu => 2,
        ClientImplementation::Erigon => 3,
        ClientImplementation::Reth => 4,
    }
}

pub fn validate_proof_attestation(
    attestation: &ProofAttestation,
    config: &ProofConsensusConfig,
) -> Result<(), Vec<ProofValidationError>> {
    let mut errors = Vec::new();
    let mut unique_clients = HashSet::new();

    if attestation.proofs.is_empty() {
        errors.push(ProofValidationError::EmptyProofs);
    }

    let expected_state_root_after = attestation
        .proofs
        .first()
        .map(|proof| proof.state_root_after);

    for (proof_index, proof) in attestation.proofs.iter().enumerate() {
        if proof.block_hash != attestation.block_hash {
            errors.push(ProofValidationError::BlockHashMismatch { proof_index });
        }

        if !unique_clients.insert(proof.prover_client) {
            errors.push(ProofValidationError::DuplicateClient {
                client: proof.prover_client,
            });
        }

        if proof.proof_data.is_empty() {
            errors.push(ProofValidationError::ProofDataEmpty { proof_index });
        }

        if let Some(expected) = expected_state_root_after {
            if proof.state_root_after != expected {
                errors.push(ProofValidationError::StateRootMismatch { proof_index });
            }
        }
    }

    if unique_clients.len() < config.required_client_diversity {
        errors.push(ProofValidationError::InsufficientClientDiversity {
            unique_clients: unique_clients.len(),
            required: config.required_client_diversity,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_attestation_stats(
    attestation: &ProofAttestation,
    config: &ProofConsensusConfig,
) -> ProofAttestationStats {
    let mut unique_clients = HashSet::new();
    let mut total_proof_bytes = 0usize;

    for proof in &attestation.proofs {
        unique_clients.insert(proof.prover_client);
        total_proof_bytes = total_proof_bytes.saturating_add(proof.proof_data.len());
    }

    let state_roots_consistent = attestation.proofs.first().is_none_or(|first| {
        attestation
            .proofs
            .iter()
            .all(|proof| proof.state_root_after == first.state_root_after)
    });

    ProofAttestationStats {
        block_number: attestation.block_number,
        unique_clients: unique_clients.len(),
        total_proof_bytes,
        meets_diversity_threshold: unique_clients.len() >= config.required_client_diversity,
        state_roots_consistent,
    }
}

pub fn compute_proof_commitment(proof: &ExecutionProof) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(proof.block_number.to_be_bytes());
    hasher.update(proof.block_hash);
    hasher.update(proof.state_root_before);
    hasher.update(proof.state_root_after);
    hasher.update(&proof.proof_data);
    hasher.update([client_discriminant(proof.prover_client)]);

    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

pub fn check_client_diversity(proofs: &[ExecutionProof], required: usize) -> bool {
    let mut clients = HashSet::new();
    for proof in proofs {
        clients.insert(proof.prover_client);
    }
    clients.len() >= required
}
