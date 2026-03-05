use eth2077_types::ci_artifacts::{
    compute_ci_artifacts_commitment, compute_ci_artifacts_stats, default_ci_artifacts_config,
    validate_ci_artifacts_config, ArtifactKind, CiArtifact, GateStatus, ReportFormat,
    RetentionPolicy,
};
use std::collections::HashMap;

fn artifact(
    id: &str,
    kind: ArtifactKind,
    size_bytes: u64,
    status: Option<&str>,
    coverage: Option<&str>,
) -> CiArtifact {
    let mut metadata = HashMap::new();
    if let Some(s) = status {
        metadata.insert("gate_status".to_string(), s.to_string());
    }
    if let Some(c) = coverage {
        metadata.insert("coverage_pct".to_string(), c.to_string());
    }
    CiArtifact {
        id: id.to_string(),
        kind,
        pipeline_run: "run-42".to_string(),
        git_sha: "abc123".to_string(),
        format: ReportFormat::Json,
        size_bytes,
        retention: RetentionPolicy::ShortTerm,
        metadata,
    }
}

#[test]
fn enum_variants_are_constructible() {
    let kinds = [
        ArtifactKind::ProofReport,
        ArtifactKind::BenchmarkGate,
        ArtifactKind::CoverageMap,
        ArtifactKind::BuildAttestation,
        ArtifactKind::RegressionDiff,
        ArtifactKind::FuzzCorpus,
    ];
    let statuses = [
        GateStatus::Passed,
        GateStatus::Failed,
        GateStatus::Flaky,
        GateStatus::Skipped,
        GateStatus::TimedOut,
        GateStatus::ManualOverride,
    ];
    let formats = [
        ReportFormat::Json,
        ReportFormat::Html,
        ReportFormat::Markdown,
        ReportFormat::Csv,
        ReportFormat::Binary,
        ReportFormat::Protobuf,
    ];
    let retention = [
        RetentionPolicy::Ephemeral,
        RetentionPolicy::ShortTerm,
        RetentionPolicy::LongTerm,
        RetentionPolicy::Permanent,
        RetentionPolicy::ArchiveAfter,
        RetentionPolicy::PurgeOnPass,
    ];
    assert_eq!(kinds.len(), 6);
    assert_eq!(statuses.len(), 6);
    assert_eq!(formats.len(), 6);
    assert_eq!(retention.len(), 6);
}

#[test]
fn default_config_is_valid() {
    let config = default_ci_artifacts_config();
    assert_eq!(validate_ci_artifacts_config(&config), Ok(()));
}

#[test]
fn validation_rejects_bad_fields() {
    let mut config = default_ci_artifacts_config();
    config.max_artifact_size_mb = 0;
    config.retention_days = 0;
    config.required_gates = vec!["proof".to_string(), " proof ".to_string(), " ".to_string()];
    config.coverage_threshold_pct = 101.0;
    config.fuzz_iterations = 0;
    config.metadata.insert("  ".to_string(), "x".to_string());
    let errors = validate_ci_artifacts_config(&config).unwrap_err();

    assert!(errors.iter().any(|e| e.field == "max_artifact_size_mb"));
    assert!(errors.iter().any(|e| e.field == "retention_days"));
    assert!(errors.iter().any(|e| e.field == "required_gates"));
    assert!(errors.iter().any(|e| e.field == "coverage_threshold_pct"));
    assert!(errors.iter().any(|e| e.field == "fuzz_iterations"));
    assert!(errors.iter().any(|e| e.field == "metadata"));
}

#[test]
fn stats_are_computed_from_metadata() {
    let artifacts = vec![
        artifact("a", ArtifactKind::BenchmarkGate, 100, Some("passed"), None),
        artifact("b", ArtifactKind::BenchmarkGate, 300, Some("failed"), None),
        artifact("c", ArtifactKind::BenchmarkGate, 500, Some("flaky"), None),
        artifact(
            "d",
            ArtifactKind::BuildAttestation,
            700,
            Some("manual_override"),
            None,
        ),
        artifact("e", ArtifactKind::CoverageMap, 900, None, Some("92.5")),
        artifact("f", ArtifactKind::CoverageMap, 1100, None, Some("nan")),
    ];
    let stats = compute_ci_artifacts_stats(&artifacts);
    assert_eq!(stats.total_artifacts, 6);
    assert_eq!(stats.passed_gates, 2);
    assert_eq!(stats.failed_gates, 1);
    assert!((stats.flaky_rate - 0.25).abs() < 1e-9);
    assert!((stats.avg_size_bytes - 600.0).abs() < 1e-9);
    assert!((stats.coverage_pct - 92.5).abs() < 1e-9);
}

#[test]
fn commitment_is_deterministic_and_sensitive() {
    let mut left = default_ci_artifacts_config();
    left.required_gates = vec!["coverage".to_string(), "proof".to_string()];
    left.metadata.clear();
    left.metadata.insert("b".to_string(), "2".to_string());
    left.metadata.insert("a".to_string(), "1".to_string());

    let mut right = default_ci_artifacts_config();
    right.required_gates = vec!["proof".to_string(), "coverage".to_string()];
    right.metadata.clear();
    right.metadata.insert("a".to_string(), "1".to_string());
    right.metadata.insert("b".to_string(), "2".to_string());

    let commit_left = compute_ci_artifacts_commitment(&left);
    assert_eq!(commit_left, compute_ci_artifacts_commitment(&left));
    assert_eq!(commit_left, compute_ci_artifacts_commitment(&right));
    right.fuzz_iterations += 1;
    assert_ne!(commit_left, compute_ci_artifacts_commitment(&right));
}
