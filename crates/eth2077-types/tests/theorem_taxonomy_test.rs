use std::collections::HashMap;

use eth2077_types::theorem_taxonomy::{
    compute_theorem_taxonomy_commitment, compute_theorem_taxonomy_stats,
    default_theorem_taxonomy_config, validate_theorem_taxonomy_config, AcceptanceCriteria,
    ProofStatus, TheoremEntry, TheoremFamily, TheoremTaxonomyConfig, TheoremTier,
};

fn sample_theorems() -> Vec<TheoremEntry> {
    vec![
        TheoremEntry {
            id: "ETH2077-THM-SAFETY-001".to_string(),
            family: TheoremFamily::SafetyInvariant,
            tier: TheoremTier::Tier1Critical,
            name: "State transition safety".to_string(),
            statement: "Valid blocks preserve global safety invariants.".to_string(),
            proof_status: ProofStatus::Proven,
            acceptance: vec![AcceptanceCriteria::MachineChecked],
            dependencies: vec![],
            metadata: HashMap::new(),
        },
        TheoremEntry {
            id: "ETH2077-THM-LIVE-001".to_string(),
            family: TheoremFamily::LivenessGuarantee,
            tier: TheoremTier::Tier2Important,
            name: "Eventual finality".to_string(),
            statement: "Under partial synchrony, finality is eventual.".to_string(),
            proof_status: ProofStatus::Admitted,
            acceptance: vec![AcceptanceCriteria::PeerReviewed, AcceptanceCriteria::FuzzVerified],
            dependencies: vec!["ETH2077-THM-SAFETY-001".to_string()],
            metadata: HashMap::new(),
        },
        TheoremEntry {
            id: "ETH2077-THM-PERF-001".to_string(),
            family: TheoremFamily::PerformanceBound,
            tier: TheoremTier::Tier3Advisory,
            name: "Throughput lower bound".to_string(),
            statement: "Sustained TPS remains above configured threshold.".to_string(),
            proof_status: ProofStatus::Conjectured,
            acceptance: vec![AcceptanceCriteria::BenchmarkBound],
            dependencies: vec![
                "ETH2077-THM-SAFETY-001".to_string(),
                "ETH2077-THM-LIVE-001".to_string(),
            ],
            metadata: HashMap::new(),
        },
    ]
}

#[test]
fn default_config_is_valid() {
    let config = default_theorem_taxonomy_config();
    assert_eq!(validate_theorem_taxonomy_config(&config), Ok(()));
    assert_eq!(config.namespace_prefix, "ETH2077-THM");
}

#[test]
fn invalid_config_returns_multiple_validation_errors() {
    let mut metadata = HashMap::new();
    metadata.insert("".to_string(), "value".to_string());
    metadata.insert("owner".to_string(), "   ".to_string());

    let config = TheoremTaxonomyConfig {
        require_tier1_proven: true,
        max_admitted_tier1: 1,
        max_conjectured_tier2: 20_001,
        auto_promote_on_proof: true,
        review_period_days: 0,
        namespace_prefix: "bad prefix!".to_string(),
        metadata,
    };

    let errors = validate_theorem_taxonomy_config(&config)
        .expect_err("invalid config should return validation errors");

    assert!(errors.iter().any(|e| e.field == "namespace_prefix"));
    assert!(errors.iter().any(|e| e.field == "review_period_days"));
    assert!(errors.iter().any(|e| e.field == "max_admitted_tier1"));
    assert!(errors.iter().any(|e| e.field == "max_conjectured_tier2"));
    assert!(errors.iter().any(|e| e.field == "metadata" || e.field == "metadata.owner"));
}

#[test]
fn theorem_taxonomy_stats_are_computed_correctly() {
    let stats = compute_theorem_taxonomy_stats(&sample_theorems());

    assert_eq!(stats.total_theorems, 3);
    assert_eq!(stats.admitted_count, 1);
    assert!((stats.proven_pct - 33.3333).abs() < 0.01);
    assert!((stats.avg_dependencies - 1.0).abs() < f64::EPSILON);
    assert_eq!(stats.by_tier.get(&TheoremTier::Tier1Critical), Some(&1));
    assert_eq!(stats.by_tier.get(&TheoremTier::Tier2Important), Some(&1));
    assert_eq!(stats.by_tier.get(&TheoremTier::Tier3Advisory), Some(&1));
    assert!((stats.coverage_by_family[&TheoremFamily::SafetyInvariant] - 33.3333).abs() < 0.01);
}

#[test]
fn taxonomy_commitment_is_stable_and_field_sensitive() {
    let mut config_a = default_theorem_taxonomy_config();
    config_a
        .metadata
        .insert("zeta".to_string(), "last".to_string());
    config_a
        .metadata
        .insert("alpha".to_string(), "first".to_string());

    let mut config_b = default_theorem_taxonomy_config();
    config_b
        .metadata
        .insert("alpha".to_string(), "first".to_string());
    config_b
        .metadata
        .insert("zeta".to_string(), "last".to_string());

    let commitment_a = compute_theorem_taxonomy_commitment(&config_a);
    let commitment_b = compute_theorem_taxonomy_commitment(&config_b);
    assert_eq!(commitment_a, commitment_b);

    config_b.review_period_days = 60;
    let commitment_c = compute_theorem_taxonomy_commitment(&config_b);
    assert_ne!(commitment_a, commitment_c);
    assert_eq!(commitment_a.len(), 64);
}
