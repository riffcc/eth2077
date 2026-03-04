use crate::types::{CommitmentEnvelope, FinalityProof};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastPathConfig {
    pub quorum_threshold: usize,
    pub timeout_ms: u64,
    pub optimistic_threshold: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastPathAttestation {
    pub block_hash: [u8; 32],
    pub attester_index: u64,
    pub signature: Vec<u8>,
    pub received_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastPathOutcome {
    FastFinalized(FinalityProof),
    SlowPath,
    Timeout,
    ConflictDetected,
}

#[derive(Debug, Clone)]
pub struct FastPathAccumulator {
    config: FastPathConfig,
    attestations: Vec<FastPathAttestation>,
    round_block_hash: Option<[u8; 32]>,
    round_start_ms: Option<u64>,
    conflict_detected: bool,
}

impl FastPathAccumulator {
    pub fn new(config: FastPathConfig) -> Self {
        Self {
            config,
            attestations: Vec::new(),
            round_block_hash: None,
            round_start_ms: None,
            conflict_detected: false,
        }
    }

    pub fn add_attestation(&mut self, attestation: FastPathAttestation) -> Option<FastPathOutcome> {
        if self.conflict_detected {
            return Some(FastPathOutcome::ConflictDetected);
        }

        if self.round_start_ms.is_none() {
            self.round_start_ms = Some(attestation.received_at_ms);
        }

        if self.is_timed_out(attestation.received_at_ms) {
            return Some(FastPathOutcome::Timeout);
        }

        if let Some(existing) = self
            .attestations
            .iter_mut()
            .find(|a| a.attester_index == attestation.attester_index)
        {
            if existing.block_hash != attestation.block_hash {
                self.conflict_detected = true;
                self.attestations.push(attestation);
                return Some(FastPathOutcome::ConflictDetected);
            }

            *existing = attestation.clone();
        } else {
            if let Some(round_hash) = self.round_block_hash {
                if round_hash != attestation.block_hash {
                    self.conflict_detected = true;
                    self.attestations.push(attestation);
                    return Some(FastPathOutcome::ConflictDetected);
                }
            } else {
                self.round_block_hash = Some(attestation.block_hash);
            }

            self.attestations.push(attestation.clone());
        }

        let agreed_count = self.agreement_count();
        if self.config.optimistic_threshold > 0 && agreed_count >= self.config.optimistic_threshold {
            let block_hash = self.round_block_hash.unwrap_or(attestation.block_hash);
            let proof = self.build_fast_proof(block_hash, attestation.received_at_ms);
            return Some(FastPathOutcome::FastFinalized(proof));
        }

        if self.has_quorum() {
            return Some(FastPathOutcome::SlowPath);
        }

        None
    }

    pub fn has_quorum(&self) -> bool {
        !self.conflict_detected && self.agreement_count() >= self.config.quorum_threshold
    }

    pub fn has_conflict(&self) -> bool {
        self.conflict_detected
    }

    pub fn attestation_count(&self) -> usize {
        self.attestations.len()
    }

    pub fn reset(&mut self) {
        self.attestations.clear();
        self.round_block_hash = None;
        self.round_start_ms = None;
        self.conflict_detected = false;
    }

    fn agreement_count(&self) -> usize {
        let Some(round_hash) = self.round_block_hash else {
            return 0;
        };

        self.attestations
            .iter()
            .filter(|a| a.block_hash == round_hash)
            .count()
    }

    fn is_timed_out(&self, now_ms: u64) -> bool {
        match self.round_start_ms {
            Some(start_ms) => now_ms.saturating_sub(start_ms) > self.config.timeout_ms,
            None => false,
        }
    }

    fn build_fast_proof(&self, block_hash: [u8; 32], finalized_at: u64) -> FinalityProof {
        let mut proof_data = Vec::with_capacity(self.attestations.len() * core::mem::size_of::<u64>());
        let mut first_signature = Vec::new();

        for att in self.attestations.iter().filter(|a| a.block_hash == block_hash) {
            if first_signature.is_empty() {
                first_signature = att.signature.clone();
            }
            proof_data.extend_from_slice(&att.attester_index.to_le_bytes());
        }

        FinalityProof {
            commitment: CommitmentEnvelope {
                block_hash,
                block_number: 0,
                state_root: [0u8; 32],
                timestamp: finalized_at,
                signature: first_signature,
                proposer_index: 0,
            },
            proof_data,
            finalized_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FastPathAccumulator, FastPathAttestation, FastPathConfig, FastPathOutcome};

    fn hash(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn attestation(block_hash: [u8; 32], attester_index: u64, received_at_ms: u64) -> FastPathAttestation {
        FastPathAttestation {
            block_hash,
            attester_index,
            signature: vec![attester_index as u8],
            received_at_ms,
        }
    }

    #[test]
    fn fast_path_reaches_quorum() {
        let cfg = FastPathConfig {
            quorum_threshold: 3,
            timeout_ms: 100,
            optimistic_threshold: 5,
        };
        let mut acc = FastPathAccumulator::new(cfg);
        let block_hash = hash(7);

        assert_eq!(acc.add_attestation(attestation(block_hash, 1, 0)), None);
        assert_eq!(acc.add_attestation(attestation(block_hash, 2, 4)), None);
        assert_eq!(
            acc.add_attestation(attestation(block_hash, 3, 9)),
            Some(FastPathOutcome::SlowPath)
        );
        assert!(acc.has_quorum());
        assert!(!acc.has_conflict());
        assert_eq!(acc.attestation_count(), 3);
    }

    #[test]
    fn fast_path_timeout_falls_back() {
        let cfg = FastPathConfig {
            quorum_threshold: 3,
            timeout_ms: 10,
            optimistic_threshold: 4,
        };
        let mut acc = FastPathAccumulator::new(cfg);
        let block_hash = hash(3);

        assert_eq!(acc.add_attestation(attestation(block_hash, 1, 100)), None);
        assert_eq!(
            acc.add_attestation(attestation(block_hash, 2, 111)),
            Some(FastPathOutcome::Timeout)
        );
        assert!(!acc.has_quorum());
        assert_eq!(acc.attestation_count(), 1);
    }

    #[test]
    fn fast_path_conflict_detected() {
        let cfg = FastPathConfig {
            quorum_threshold: 2,
            timeout_ms: 100,
            optimistic_threshold: 2,
        };
        let mut acc = FastPathAccumulator::new(cfg);

        assert_eq!(acc.add_attestation(attestation(hash(9), 1, 0)), None);
        assert_eq!(
            acc.add_attestation(attestation(hash(10), 2, 1)),
            Some(FastPathOutcome::ConflictDetected)
        );
        assert!(acc.has_conflict());
        assert_eq!(acc.attestation_count(), 2);
    }

    #[test]
    fn fast_path_optimistic_threshold() {
        let cfg = FastPathConfig {
            quorum_threshold: 4,
            timeout_ms: 100,
            optimistic_threshold: 2,
        };
        let mut acc = FastPathAccumulator::new(cfg);
        let block_hash = hash(11);

        assert_eq!(acc.add_attestation(attestation(block_hash, 1, 0)), None);
        let outcome = acc.add_attestation(attestation(block_hash, 2, 1));

        match outcome {
            Some(FastPathOutcome::FastFinalized(proof)) => {
                assert_eq!(proof.commitment.block_hash, block_hash);
                assert_eq!(proof.finalized_at, 1);
                assert!(!proof.proof_data.is_empty());
            }
            other => panic!("expected FastFinalized outcome, got {other:?}"),
        }
    }

    #[test]
    fn fast_path_reset_clears_state() {
        let cfg = FastPathConfig {
            quorum_threshold: 2,
            timeout_ms: 100,
            optimistic_threshold: 3,
        };
        let mut acc = FastPathAccumulator::new(cfg);

        assert_eq!(acc.add_attestation(attestation(hash(1), 1, 0)), None);
        assert_eq!(
            acc.add_attestation(attestation(hash(2), 2, 1)),
            Some(FastPathOutcome::ConflictDetected)
        );
        assert!(acc.has_conflict());
        assert_eq!(acc.attestation_count(), 2);

        acc.reset();

        assert!(!acc.has_conflict());
        assert!(!acc.has_quorum());
        assert_eq!(acc.attestation_count(), 0);
        assert_eq!(acc.add_attestation(attestation(hash(3), 5, 10)), None);
    }
}
