use eth2077_types::placeholder_elimination::{
    compute_placeholder_elimination_commitment, compute_placeholder_elimination_stats,
    default_placeholder_elimination_config, validate_placeholder_elimination_config,
    EliminationMethod, PlaceholderEliminationConfig, PlaceholderEntry, PlaceholderStatus,
    ProofCompleteness, VerificationGate,
};
use std::collections::HashMap;

#[test]
fn default_config_is_valid_and_has_expected_shape() {
    let config = default_placeholder_elimination_config();

    assert_eq!(config.max_placeholder_age_days, 30);
    assert_eq!(config.min_proof_completeness, ProofCompleteness::Substantial);
    assert!(!config.auto_certify);
    assert_eq!(config.coverage_threshold_pct, 95.0);
    assert!(config.fuzz_iterations > 0);
    assert!(!config.required_gates.is_empty());
    assert!(validate_placeholder_elimination_config(&config).is_ok());
}

#[test]
fn validation_collects_multiple_policy_errors() {
    let config = PlaceholderEliminationConfig {
        max_placeholder_age_days: 0,
        required_gates: vec![VerificationGate::FuzzSuite, VerificationGate::FuzzSuite],
        min_proof_completeness: ProofCompleteness::MachineChecked,
        auto_certify: true,
        coverage_threshold_pct: 140.0,
        fuzz_iterations: 0,
        metadata: HashMap::from([(" ".to_string(), " ".to_string())]),
    };

    let errors = validate_placeholder_elimination_config(&config)
        .expect_err("invalid placeholder elimination config should fail");

    assert!(errors.iter().any(|e| e.field == "max_placeholder_age_days"));
    assert!(errors.iter().any(|e| e.field == "required_gates"));
    assert!(errors.iter().any(|e| e.field == "coverage_threshold_pct"));
    assert!(errors.iter().any(|e| e.field == "fuzz_iterations"));
    assert!(errors.iter().any(|e| e.field == "min_proof_completeness"));
    assert!(errors.iter().any(|e| e.field == "metadata"));
}

#[test]
fn stats_for_empty_input_are_zeroed() {
    let stats = compute_placeholder_elimination_stats(&[]);

    assert_eq!(stats.total_placeholders, 0);
    assert_eq!(stats.by_status.len(), 6);
    assert_eq!(stats.elimination_rate_pct, 0.0);
    assert_eq!(stats.avg_proof_completeness, 0.0);
    assert_eq!(stats.total_loc_replaced, 0);
    assert_eq!(stats.gates_passed_pct, 0.0);
}

#[test]
fn stats_aggregate_status_loc_proof_and_gate_coverage() {
    let entries = vec![
        PlaceholderEntry {
            id: "ph-1".to_string(),
            module_path: "eth2077::exec".to_string(),
            function_name: "apply_block".to_string(),
            status: PlaceholderStatus::Identified,
            method: EliminationMethod::Rewrite,
            verification_gates: vec![],
            proof_completeness: ProofCompleteness::None_,
            lines_of_code: 80,
            metadata: HashMap::new(),
        },
        PlaceholderEntry {
            id: "ph-2".to_string(),
            module_path: "eth2077::consensus".to_string(),
            function_name: "finalize_round".to_string(),
            status: PlaceholderStatus::Replaced,
            method: EliminationMethod::DirectPort,
            verification_gates: vec![
                VerificationGate::UnitTests,
                VerificationGate::IntegrationTests,
                VerificationGate::PropertyTests,
            ],
            proof_completeness: ProofCompleteness::Complete,
            lines_of_code: 120,
            metadata: HashMap::new(),
        },
        PlaceholderEntry {
            id: "ph-3".to_string(),
            module_path: "eth2077::net".to_string(),
            function_name: "validate_frame".to_string(),
            status: PlaceholderStatus::Certified,
            method: EliminationMethod::ManualProof,
            verification_gates: vec![
                VerificationGate::UnitTests,
                VerificationGate::IntegrationTests,
                VerificationGate::PropertyTests,
                VerificationGate::FormalProof,
                VerificationGate::FuzzSuite,
                VerificationGate::PeerReview,
            ],
            proof_completeness: ProofCompleteness::MachineChecked,
            lines_of_code: 40,
            metadata: HashMap::new(),
        },
    ];

    let stats = compute_placeholder_elimination_stats(&entries);

    assert_eq!(stats.total_placeholders, 3);
    assert_eq!(stats.by_status.get(&PlaceholderStatus::Identified), Some(&1));
    assert_eq!(stats.by_status.get(&PlaceholderStatus::Replaced), Some(&1));
    assert_eq!(stats.by_status.get(&PlaceholderStatus::Certified), Some(&1));
    assert!((stats.elimination_rate_pct - 66.666_666).abs() < 0.001);
    assert!((stats.avg_proof_completeness - 3.0).abs() < f64::EPSILON);
    assert_eq!(stats.total_loc_replaced, 160);
    assert!((stats.gates_passed_pct - 50.0).abs() < f64::EPSILON);
}

#[test]
fn commitment_is_deterministic_and_sensitive_to_changes() {
    let base = default_placeholder_elimination_config();
    let hash_a = compute_placeholder_elimination_commitment(&base);
    let hash_b = compute_placeholder_elimination_commitment(&base);
    assert_eq!(hash_a, hash_b);
    assert_eq!(hash_a.len(), 64);

    let mut changed = base.clone();
    changed.auto_certify = true;
    let hash_c = compute_placeholder_elimination_commitment(&changed);
    assert_ne!(hash_a, hash_c);
}

#[test]
fn all_enum_variants_roundtrip_through_json() {
    let status_variants = vec![
        PlaceholderStatus::Identified,
        PlaceholderStatus::Analyzed,
        PlaceholderStatus::InProgress,
        PlaceholderStatus::Replaced,
        PlaceholderStatus::Verified,
        PlaceholderStatus::Certified,
    ];
    for value in status_variants {
        let encoded = serde_json::to_string(&value).unwrap();
        let decoded: PlaceholderStatus = serde_json::from_str(&encoded).unwrap();
        assert_eq!(value, decoded);
    }

    let method_variants = vec![
        EliminationMethod::DirectPort,
        EliminationMethod::Rewrite,
        EliminationMethod::WrapperShim,
        EliminationMethod::FfiBinding,
        EliminationMethod::CodeGen,
        EliminationMethod::ManualProof,
    ];
    for value in method_variants {
        let encoded = serde_json::to_string(&value).unwrap();
        let decoded: EliminationMethod = serde_json::from_str(&encoded).unwrap();
        assert_eq!(value, decoded);
    }

    let gate_variants = vec![
        VerificationGate::UnitTests,
        VerificationGate::IntegrationTests,
        VerificationGate::PropertyTests,
        VerificationGate::FormalProof,
        VerificationGate::FuzzSuite,
        VerificationGate::PeerReview,
    ];
    for value in gate_variants {
        let encoded = serde_json::to_string(&value).unwrap();
        let decoded: VerificationGate = serde_json::from_str(&encoded).unwrap();
        assert_eq!(value, decoded);
    }

    let proof_variants = vec![
        ProofCompleteness::None_,
        ProofCompleteness::Partial,
        ProofCompleteness::Substantial,
        ProofCompleteness::NearComplete,
        ProofCompleteness::Complete,
        ProofCompleteness::MachineChecked,
    ];
    for value in proof_variants {
        let encoded = serde_json::to_string(&value).unwrap();
        let decoded: ProofCompleteness = serde_json::from_str(&encoded).unwrap();
        assert_eq!(value, decoded);
    }
}
