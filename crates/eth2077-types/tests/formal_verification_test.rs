use eth2077_types::formal_verification::{
    compare_proving_systems, compute_verification_commitment, compute_verification_stats,
    default_formal_verification_config, estimate_aggregation_savings, estimate_pq_security,
    validate_verification_config, FormalVerificationConfig, ProvingSystem, SignatureScheme,
    VerificationStrategy, VerificationValidationError,
};

#[test]
fn default_config_matches_expected_values() {
    let config = default_formal_verification_config();
    assert_eq!(config.proving_system, ProvingSystem::WHIR);
    assert_eq!(config.signature_scheme, SignatureScheme::XMSS);
    assert_eq!(config.strategy, VerificationStrategy::RecursiveComposition);
    assert_eq!(config.security_bits, 124);
    assert_eq!(config.max_aggregation_depth, 16);
    assert_eq!(config.field_size_bits, 255);
    assert!(config.post_quantum);
    assert_eq!(config.max_proof_size_bytes, 256_000);
}

#[test]
fn default_config_validates() {
    let config = default_formal_verification_config();
    assert_eq!(validate_verification_config(&config), Ok(()));
}

#[test]
fn invalid_config_collects_multiple_errors() {
    let config = FormalVerificationConfig {
        proving_system: ProvingSystem::Groth16,
        signature_scheme: SignatureScheme::ECDSA,
        strategy: VerificationStrategy::RecursiveComposition,
        security_bits: 80,
        max_aggregation_depth: 0,
        field_size_bits: 64,
        post_quantum: true,
        max_proof_size_bytes: 5_000_000,
    };

    let errors = validate_verification_config(&config).unwrap_err();
    assert!(
        errors.contains(&VerificationValidationError::InsufficientSecurity {
            bits: 80,
            minimum: 124,
        })
    );
    assert!(errors.contains(&VerificationValidationError::AggregationDepthZero));
    assert!(errors.contains(&VerificationValidationError::FieldSizeTooSmall { bits: 64 }));
    assert!(
        errors.contains(&VerificationValidationError::ProofSizeTooLarge {
            size: 5_000_000,
            max: 4_194_304,
        })
    );
    assert!(errors.contains(&VerificationValidationError::IncompatibleStrategy));
}

#[test]
fn aggregation_savings_increases_with_depth() {
    let shallow = estimate_aggregation_savings(2, 64_000);
    let medium = estimate_aggregation_savings(4, 64_000);
    let deep = estimate_aggregation_savings(16, 64_000);

    assert_eq!(estimate_aggregation_savings(1, 64_000), 1.0);
    assert!(shallow > 1.0);
    assert!(medium > shallow);
    assert!(deep > medium);
}

#[test]
fn pq_security_estimation_matches_scheme_characteristics() {
    assert_eq!(estimate_pq_security(SignatureScheme::XMSS, 124), 124);
    assert_eq!(estimate_pq_security(SignatureScheme::SPHINCS, 128), 120);
    assert_eq!(estimate_pq_security(SignatureScheme::Dilithium, 128), 115);
    assert_eq!(estimate_pq_security(SignatureScheme::BLS, 128), 64);
    assert_eq!(estimate_pq_security(SignatureScheme::ECDSA, 128), 64);
}

#[test]
fn aggregated_proof_strategy_reduces_size_and_time() {
    let mut direct = default_formal_verification_config();
    direct.strategy = VerificationStrategy::DirectVerify;
    let direct_stats = compute_verification_stats(&direct);

    let mut aggregated = direct.clone();
    aggregated.strategy = VerificationStrategy::AggregatedProof;
    let aggregated_stats = compute_verification_stats(&aggregated);

    assert!(aggregated_stats.aggregation_savings_factor > 1.0);
    assert!(aggregated_stats.proof_size_bytes < direct_stats.proof_size_bytes);
    assert!(aggregated_stats.verification_time_ms < direct_stats.verification_time_ms);
}

#[test]
fn compare_proving_systems_returns_all_variants() {
    let config = default_formal_verification_config();
    let compared = compare_proving_systems(&config);

    assert_eq!(compared.len(), 6);
    assert_eq!(compared[0].0, "WHIR");
    assert_eq!(compared[1].0, "SuperSpartan");
    assert_eq!(compared[2].0, "Groth16");
    assert_eq!(compared[3].0, "Plonk");
    assert_eq!(compared[4].0, "Halo2");
    assert_eq!(compared[5].0, "STARKs");
    assert!(compared.iter().all(|(_, stats)| stats.proof_size_bytes > 0));
}

#[test]
fn post_quantum_ready_flag_requires_pq_signature() {
    let mut config = default_formal_verification_config();
    config.signature_scheme = SignatureScheme::BLS;

    let stats = compute_verification_stats(&config);
    assert!(!stats.post_quantum_ready);
    assert_eq!(stats.security_level_bits, 62);
}

#[test]
fn verification_commitment_is_deterministic_and_sensitive_to_changes() {
    let config = default_formal_verification_config();
    let proof_hashes_a = [[1u8; 32], [2u8; 32], [3u8; 32]];
    let proof_hashes_b = [[1u8; 32], [2u8; 32], [4u8; 32]];

    let hash_a1 = compute_verification_commitment(&config, &proof_hashes_a);
    let hash_a2 = compute_verification_commitment(&config, &proof_hashes_a);
    let hash_b = compute_verification_commitment(&config, &proof_hashes_b);

    let mut changed_config = config.clone();
    changed_config.strategy = VerificationStrategy::BatchVerification;
    let hash_c = compute_verification_commitment(&changed_config, &proof_hashes_a);

    assert_eq!(hash_a1, hash_a2);
    assert_ne!(hash_a1, hash_b);
    assert_ne!(hash_a1, hash_c);
}
