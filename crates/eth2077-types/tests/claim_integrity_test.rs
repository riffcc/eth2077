use std::time::{SystemTime, UNIX_EPOCH};

use eth2077_types::claim_integrity::*;

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn sample_artifact(
    kind: ArtifactKind,
    signer: &str,
    payload_seed: u8,
    scheme: SignatureScheme,
    gate_status: GateStatus,
) -> SignedArtifact {
    SignedArtifact {
        kind,
        payload_hash: [payload_seed; 32],
        signature_scheme: scheme,
        signer_id: signer.to_owned(),
        timestamp_unix: now_unix(),
        metadata: format!("artifact-{payload_seed}"),
        gate_status,
    }
}

#[test]
fn default_config_is_valid() {
    let config = default_claim_integrity_config();
    assert_eq!(validate_claim_config(&config), Ok(()));
    assert!(config.min_signers >= 1);
    assert!(!config.milestones.is_empty());
    assert!(!config.allowed_schemes.is_empty());
}

#[test]
fn validation_rejects_empty_milestones() {
    let mut config = default_claim_integrity_config();
    config.milestones.clear();

    let errors = validate_claim_config(&config).unwrap_err();
    assert!(errors.contains(&ClaimValidationError::EmptyMilestones));
}

#[test]
fn validation_rejects_no_signers() {
    let mut config = default_claim_integrity_config();
    config.min_signers = 0;

    let errors = validate_claim_config(&config).unwrap_err();
    assert!(errors.contains(&ClaimValidationError::NoSigners));
}

#[test]
fn gate_evaluation_with_full_coverage() {
    let gate = MilestoneGate {
        milestone_name: "50k".to_owned(),
        required_artifacts: vec![
            ArtifactKind::BenchmarkResult,
            ArtifactKind::FormalProof,
            ArtifactKind::TestVectorPass,
        ],
        achieved_artifacts: vec![
            sample_artifact(
                ArtifactKind::BenchmarkResult,
                "alice",
                1,
                SignatureScheme::Ed25519,
                GateStatus::InteropPassed,
            ),
            sample_artifact(
                ArtifactKind::FormalProof,
                "bob",
                2,
                SignatureScheme::Ed25519,
                GateStatus::InteropPassed,
            ),
            sample_artifact(
                ArtifactKind::TestVectorPass,
                "carol",
                3,
                SignatureScheme::Ed25519,
                GateStatus::InteropPassed,
            ),
        ],
        target_tps: 50_000.0,
        measured_tps: Some(52_000.0),
        gate_status: GateStatus::Draft,
    };

    let status = evaluate_milestone_gate(&gate, &[SignatureScheme::Ed25519], 2);
    assert_eq!(status, GateStatus::InteropPassed);
}

#[test]
fn gate_evaluation_with_partial_coverage() {
    let gate = MilestoneGate {
        milestone_name: "100k".to_owned(),
        required_artifacts: vec![
            ArtifactKind::BenchmarkResult,
            ArtifactKind::FormalProof,
            ArtifactKind::TestVectorPass,
        ],
        achieved_artifacts: vec![
            sample_artifact(
                ArtifactKind::BenchmarkResult,
                "alice",
                11,
                SignatureScheme::Secp256k1,
                GateStatus::Draft,
            ),
            sample_artifact(
                ArtifactKind::FormalProof,
                "bob",
                12,
                SignatureScheme::Secp256k1,
                GateStatus::Draft,
            ),
        ],
        target_tps: 100_000.0,
        measured_tps: Some(75_000.0),
        gate_status: GateStatus::Draft,
    };

    let status = evaluate_milestone_gate(&gate, &[SignatureScheme::Secp256k1], 2);
    assert_eq!(status, GateStatus::Partial);
}

#[test]
fn stats_computation_with_mixed_gates() {
    let pass_gate = MilestoneGate {
        milestone_name: "pass".to_owned(),
        required_artifacts: vec![ArtifactKind::BenchmarkResult, ArtifactKind::FormalProof],
        achieved_artifacts: vec![
            sample_artifact(
                ArtifactKind::BenchmarkResult,
                "alice",
                21,
                SignatureScheme::Ed25519,
                GateStatus::InteropPassed,
            ),
            sample_artifact(
                ArtifactKind::FormalProof,
                "bob",
                22,
                SignatureScheme::Ed25519,
                GateStatus::InteropPassed,
            ),
        ],
        target_tps: 80_000.0,
        measured_tps: Some(81_000.0),
        gate_status: GateStatus::InteropPassed,
    };
    let partial_gate = MilestoneGate {
        milestone_name: "partial".to_owned(),
        required_artifacts: vec![ArtifactKind::BenchmarkResult, ArtifactKind::FormalProof],
        achieved_artifacts: vec![sample_artifact(
            ArtifactKind::BenchmarkResult,
            "alice",
            23,
            SignatureScheme::Ed25519,
            GateStatus::Draft,
        )],
        target_tps: 90_000.0,
        measured_tps: Some(70_000.0),
        gate_status: GateStatus::Partial,
    };
    let not_started_gate = MilestoneGate {
        milestone_name: "none".to_owned(),
        required_artifacts: vec![ArtifactKind::BenchmarkResult],
        achieved_artifacts: vec![],
        target_tps: 120_000.0,
        measured_tps: None,
        gate_status: GateStatus::NotStarted,
    };

    let config = ClaimIntegrityConfig {
        milestones: vec![pass_gate, partial_gate, not_started_gate],
        require_all_gates: true,
        min_signers: 2,
        allowed_schemes: vec![SignatureScheme::Ed25519],
    };

    let stats = compute_claim_integrity_stats(&config);
    assert_eq!(stats.total_milestones, 3);
    assert_eq!(stats.gates_passed, 1);
    assert_eq!(stats.gates_failed, 2);
    assert!((stats.coverage_ratio - (1.0 / 3.0)).abs() < 1e-9);
    assert_eq!(stats.strongest_gate, "pass");
    assert_eq!(stats.weakest_gate, "none");
    assert_eq!(stats.overall_status, GateStatus::NotStarted);
}

#[test]
fn commitment_is_order_invariant_and_deterministic() {
    let a = sample_artifact(
        ArtifactKind::BenchmarkResult,
        "alice",
        31,
        SignatureScheme::BLS12_381,
        GateStatus::Draft,
    );
    let b = sample_artifact(
        ArtifactKind::FormalProof,
        "bob",
        32,
        SignatureScheme::BLS12_381,
        GateStatus::Partial,
    );
    let c = sample_artifact(
        ArtifactKind::CrossClientReplay,
        "carol",
        33,
        SignatureScheme::BLS12_381,
        GateStatus::SpecComplete,
    );

    let commit_abc_1 = compute_claim_commitment(&[a.clone(), b.clone(), c.clone()]);
    let commit_cba = compute_claim_commitment(&[c, b, a]);
    let commit_abc_2 = compute_claim_commitment(&[
        sample_artifact(
            ArtifactKind::BenchmarkResult,
            "alice",
            31,
            SignatureScheme::BLS12_381,
            GateStatus::Draft,
        ),
        sample_artifact(
            ArtifactKind::FormalProof,
            "bob",
            32,
            SignatureScheme::BLS12_381,
            GateStatus::Partial,
        ),
        sample_artifact(
            ArtifactKind::CrossClientReplay,
            "carol",
            33,
            SignatureScheme::BLS12_381,
            GateStatus::SpecComplete,
        ),
    ]);

    assert_eq!(commit_abc_1, commit_cba);
    assert_eq!(commit_abc_1, commit_abc_2);
}

#[test]
fn commitment_is_sensitive_to_payload_changes() {
    let original = sample_artifact(
        ArtifactKind::BenchmarkResult,
        "alice",
        41,
        SignatureScheme::Ed25519,
        GateStatus::Draft,
    );
    let changed = sample_artifact(
        ArtifactKind::BenchmarkResult,
        "alice",
        42,
        SignatureScheme::Ed25519,
        GateStatus::Draft,
    );

    let commitment_a = compute_claim_commitment(&[original]);
    let commitment_b = compute_claim_commitment(&[changed]);

    assert_ne!(commitment_a, commitment_b);
}

#[test]
fn validation_rejects_disallowed_signature_scheme() {
    let gate = MilestoneGate {
        milestone_name: "disallowed-scheme".to_owned(),
        required_artifacts: vec![ArtifactKind::BenchmarkResult],
        achieved_artifacts: vec![sample_artifact(
            ArtifactKind::BenchmarkResult,
            "alice",
            51,
            SignatureScheme::Multisig,
            GateStatus::SpecComplete,
        )],
        target_tps: 1.0,
        measured_tps: Some(1.0),
        gate_status: GateStatus::SpecComplete,
    };

    let config = ClaimIntegrityConfig {
        milestones: vec![gate],
        require_all_gates: true,
        min_signers: 1,
        allowed_schemes: vec![SignatureScheme::Ed25519],
    };

    let errors = validate_claim_config(&config).unwrap_err();
    assert!(errors.contains(&ClaimValidationError::InvalidSignatureScheme));
}

#[test]
fn validation_rejects_duplicate_artifact_entries() {
    let artifact = sample_artifact(
        ArtifactKind::BenchmarkResult,
        "alice",
        61,
        SignatureScheme::Ed25519,
        GateStatus::Draft,
    );
    let gate = MilestoneGate {
        milestone_name: "duplicates".to_owned(),
        required_artifacts: vec![ArtifactKind::BenchmarkResult],
        achieved_artifacts: vec![artifact.clone(), artifact],
        target_tps: 1.0,
        measured_tps: Some(1.0),
        gate_status: GateStatus::Draft,
    };
    let config = ClaimIntegrityConfig {
        milestones: vec![gate],
        require_all_gates: true,
        min_signers: 1,
        allowed_schemes: vec![SignatureScheme::Ed25519],
    };

    let errors = validate_claim_config(&config).unwrap_err();
    assert!(errors.contains(&ClaimValidationError::DuplicateArtifact));
}
