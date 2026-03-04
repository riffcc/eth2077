use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_ARTIFACT_AGE_HOURS: u64 = 24 * 30;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArtifactKind {
    BenchmarkResult,
    FormalProof,
    TestVectorPass,
    HiveConformance,
    ShadowForkResult,
    CrossClientReplay,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GateStatus {
    NotStarted,
    Draft,
    Partial,
    SpecComplete,
    InteropPassed,
    ProductionReady,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SignatureScheme {
    Ed25519,
    Secp256k1,
    BLS12_381,
    Multisig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedArtifact {
    pub kind: ArtifactKind,
    pub payload_hash: [u8; 32],
    pub signature_scheme: SignatureScheme,
    pub signer_id: String,
    pub timestamp_unix: u64,
    pub metadata: String,
    pub gate_status: GateStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MilestoneGate {
    pub milestone_name: String,
    pub required_artifacts: Vec<ArtifactKind>,
    pub achieved_artifacts: Vec<SignedArtifact>,
    pub target_tps: f64,
    pub measured_tps: Option<f64>,
    pub gate_status: GateStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaimIntegrityConfig {
    pub milestones: Vec<MilestoneGate>,
    pub require_all_gates: bool,
    pub min_signers: usize,
    pub allowed_schemes: Vec<SignatureScheme>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClaimValidationError {
    EmptyMilestones,
    NoSigners,
    DuplicateArtifact,
    InvalidSignatureScheme,
    MissingRequiredArtifact {
        kind: ArtifactKind,
        milestone: String,
    },
    StaleArtifact {
        age_hours: u64,
        max_hours: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaimIntegrityStats {
    pub total_milestones: usize,
    pub gates_passed: usize,
    pub gates_failed: usize,
    pub coverage_ratio: f64,
    pub strongest_gate: String,
    pub weakest_gate: String,
    pub overall_status: GateStatus,
}

pub fn default_claim_integrity_config() -> ClaimIntegrityConfig {
    ClaimIntegrityConfig {
        milestones: vec![
            MilestoneGate {
                milestone_name: "TPS 50k qualification".to_owned(),
                required_artifacts: vec![
                    ArtifactKind::BenchmarkResult,
                    ArtifactKind::FormalProof,
                    ArtifactKind::TestVectorPass,
                ],
                achieved_artifacts: Vec::new(),
                target_tps: 50_000.0,
                measured_tps: None,
                gate_status: GateStatus::NotStarted,
            },
            MilestoneGate {
                milestone_name: "TPS 100k interop".to_owned(),
                required_artifacts: vec![
                    ArtifactKind::BenchmarkResult,
                    ArtifactKind::HiveConformance,
                    ArtifactKind::CrossClientReplay,
                    ArtifactKind::ShadowForkResult,
                ],
                achieved_artifacts: Vec::new(),
                target_tps: 100_000.0,
                measured_tps: None,
                gate_status: GateStatus::NotStarted,
            },
        ],
        require_all_gates: true,
        min_signers: 2,
        allowed_schemes: vec![
            SignatureScheme::Ed25519,
            SignatureScheme::Secp256k1,
            SignatureScheme::BLS12_381,
            SignatureScheme::Multisig,
        ],
    }
}

pub fn validate_claim_config(
    config: &ClaimIntegrityConfig,
) -> Result<(), Vec<ClaimValidationError>> {
    let mut errors = Vec::new();

    if config.milestones.is_empty() {
        errors.push(ClaimValidationError::EmptyMilestones);
    }
    if config.min_signers == 0 {
        errors.push(ClaimValidationError::NoSigners);
    }

    let allowed: HashSet<SignatureScheme> = config.allowed_schemes.iter().copied().collect();
    let now_unix = current_unix_timestamp();

    for gate in &config.milestones {
        let mut seen = HashSet::new();
        let required: HashSet<ArtifactKind> = gate.required_artifacts.iter().copied().collect();
        let achieved_kinds: HashSet<ArtifactKind> = gate
            .achieved_artifacts
            .iter()
            .map(|artifact| artifact.kind)
            .collect();

        for artifact in &gate.achieved_artifacts {
            if !allowed.contains(&artifact.signature_scheme) {
                errors.push(ClaimValidationError::InvalidSignatureScheme);
            }

            let identity = (
                artifact.kind,
                artifact.payload_hash,
                artifact.signer_id.clone(),
            );
            if !seen.insert(identity) {
                errors.push(ClaimValidationError::DuplicateArtifact);
            }

            if now_unix > artifact.timestamp_unix {
                let age_hours = (now_unix - artifact.timestamp_unix) / 3600;
                if age_hours > MAX_ARTIFACT_AGE_HOURS {
                    errors.push(ClaimValidationError::StaleArtifact {
                        age_hours,
                        max_hours: MAX_ARTIFACT_AGE_HOURS,
                    });
                }
            }
        }

        if gate.gate_status >= GateStatus::SpecComplete {
            for kind in &required {
                if !achieved_kinds.contains(kind) {
                    errors.push(ClaimValidationError::MissingRequiredArtifact {
                        kind: *kind,
                        milestone: gate.milestone_name.clone(),
                    });
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_claim_integrity_stats(config: &ClaimIntegrityConfig) -> ClaimIntegrityStats {
    let statuses: Vec<GateStatus> = config
        .milestones
        .iter()
        .map(|gate| evaluate_milestone_gate(gate, &config.allowed_schemes, config.min_signers))
        .collect();

    let total_milestones = statuses.len();
    let gates_passed = statuses
        .iter()
        .filter(|status| **status >= GateStatus::InteropPassed)
        .count();
    let gates_failed = total_milestones.saturating_sub(gates_passed);
    let coverage_ratio = if total_milestones == 0 {
        0.0
    } else {
        gates_passed as f64 / total_milestones as f64
    };

    let mut strongest_gate = String::new();
    let mut weakest_gate = String::new();
    let mut strongest_status = GateStatus::NotStarted;
    let mut weakest_status = GateStatus::ProductionReady;

    for (idx, status) in statuses.iter().enumerate() {
        if idx == 0 || *status > strongest_status {
            strongest_status = *status;
            strongest_gate = config.milestones[idx].milestone_name.clone();
        }
        if idx == 0 || *status < weakest_status {
            weakest_status = *status;
            weakest_gate = config.milestones[idx].milestone_name.clone();
        }
    }

    let overall_status = if statuses.is_empty() {
        GateStatus::NotStarted
    } else if config.require_all_gates {
        statuses
            .iter()
            .copied()
            .min()
            .unwrap_or(GateStatus::NotStarted)
    } else {
        statuses
            .iter()
            .copied()
            .max()
            .unwrap_or(GateStatus::NotStarted)
    };

    ClaimIntegrityStats {
        total_milestones,
        gates_passed,
        gates_failed,
        coverage_ratio,
        strongest_gate,
        weakest_gate,
        overall_status,
    }
}

pub fn evaluate_milestone_gate(
    gate: &MilestoneGate,
    allowed_schemes: &[SignatureScheme],
    min_signers: usize,
) -> GateStatus {
    let allowed: HashSet<SignatureScheme> = allowed_schemes.iter().copied().collect();
    let required: HashSet<ArtifactKind> = gate.required_artifacts.iter().copied().collect();

    let achieved_allowed: Vec<&SignedArtifact> = gate
        .achieved_artifacts
        .iter()
        .filter(|artifact| allowed.contains(&artifact.signature_scheme))
        .collect();

    if achieved_allowed.is_empty() {
        return GateStatus::NotStarted;
    }

    let mut covered_required = HashSet::new();
    let mut signers = HashSet::new();
    for artifact in &achieved_allowed {
        signers.insert(artifact.signer_id.as_str());
        if required.contains(&artifact.kind) {
            covered_required.insert(artifact.kind);
        }
    }

    let coverage = if required.is_empty() {
        1.0
    } else {
        covered_required.len() as f64 / required.len() as f64
    };

    if coverage == 0.0 {
        return GateStatus::Draft;
    }
    if coverage < 1.0 {
        return GateStatus::Partial;
    }

    if signers.len() < min_signers {
        return GateStatus::SpecComplete;
    }

    let meets_tps = gate.measured_tps.unwrap_or(0.0) >= gate.target_tps;
    if !meets_tps {
        return GateStatus::SpecComplete;
    }

    let all_production_ready = achieved_allowed
        .iter()
        .filter(|artifact| required.contains(&artifact.kind))
        .all(|artifact| artifact.gate_status >= GateStatus::ProductionReady);
    if all_production_ready && signers.len() >= min_signers.saturating_mul(2) {
        GateStatus::ProductionReady
    } else {
        GateStatus::InteropPassed
    }
}

pub fn compute_claim_commitment(artifacts: &[SignedArtifact]) -> [u8; 32] {
    let mut artifact_hashes = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        let mut artifact_hasher = Sha256::new();
        artifact_hasher.update([artifact_kind_discriminant(artifact.kind)]);
        artifact_hasher.update(artifact.payload_hash);
        artifact_hasher.update([signature_scheme_discriminant(artifact.signature_scheme)]);
        artifact_hasher.update((artifact.signer_id.len() as u64).to_be_bytes());
        artifact_hasher.update(artifact.signer_id.as_bytes());
        artifact_hasher.update(artifact.timestamp_unix.to_be_bytes());
        artifact_hasher.update((artifact.metadata.len() as u64).to_be_bytes());
        artifact_hasher.update(artifact.metadata.as_bytes());
        artifact_hasher.update([gate_status_discriminant(artifact.gate_status)]);

        let digest = artifact_hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&digest);
        artifact_hashes.push(hash);
    }

    artifact_hashes.sort_unstable();

    let mut commitment_hasher = Sha256::new();
    commitment_hasher.update(b"eth2077-claim-integrity-v1");
    commitment_hasher.update((artifact_hashes.len() as u64).to_be_bytes());
    for hash in &artifact_hashes {
        commitment_hasher.update(hash);
    }

    let digest = commitment_hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn artifact_kind_discriminant(kind: ArtifactKind) -> u8 {
    match kind {
        ArtifactKind::BenchmarkResult => 0,
        ArtifactKind::FormalProof => 1,
        ArtifactKind::TestVectorPass => 2,
        ArtifactKind::HiveConformance => 3,
        ArtifactKind::ShadowForkResult => 4,
        ArtifactKind::CrossClientReplay => 5,
    }
}

fn signature_scheme_discriminant(scheme: SignatureScheme) -> u8 {
    match scheme {
        SignatureScheme::Ed25519 => 0,
        SignatureScheme::Secp256k1 => 1,
        SignatureScheme::BLS12_381 => 2,
        SignatureScheme::Multisig => 3,
    }
}

fn gate_status_discriminant(status: GateStatus) -> u8 {
    match status {
        GateStatus::NotStarted => 0,
        GateStatus::Draft => 1,
        GateStatus::Partial => 2,
        GateStatus::SpecComplete => 3,
        GateStatus::InteropPassed => 4,
        GateStatus::ProductionReady => 5,
    }
}
