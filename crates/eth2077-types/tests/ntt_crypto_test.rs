use eth2077_types::ntt_crypto::{
    compare_strategies, compute_ntt_commitment, compute_ntt_stats, compute_pq_readiness,
    default_ntt_config, estimate_precompile_gas, scheme_ntt_degree, validate_ntt_config, NttConfig,
    NttScheme, NttValidationError, PrecompileStrategy,
};

#[test]
fn default_config_matches_expected_values() {
    let config = default_ntt_config();
    assert_eq!(config.target_scheme, NttScheme::Falcon512);
    assert_eq!(config.precompile_strategy, PrecompileStrategy::GenericNtt);
    assert_eq!(config.ntt_degree, 512);
    assert_eq!(config.modulus_bits, 12);
    assert_eq!(config.batch_size, 1);
    assert_eq!(config.target_gas_per_verify, 50_000);
    assert_eq!(config.current_evm_gas, 600_000);
}

#[test]
fn default_config_validates() {
    let config = default_ntt_config();
    assert_eq!(validate_ntt_config(&config), Ok(()));
}

#[test]
fn invalid_config_collects_multiple_errors() {
    let config = NttConfig {
        target_scheme: NttScheme::Falcon512,
        precompile_strategy: PrecompileStrategy::GenericNtt,
        ntt_degree: 300,
        modulus_bits: 8,
        batch_size: 0,
        target_gas_per_verify: 0,
        current_evm_gas: 600_000,
    };

    let errors = validate_ntt_config(&config).unwrap_err();
    assert!(errors.contains(&NttValidationError::InvalidDegree { degree: 300 }));
    assert!(errors.contains(&NttValidationError::ModulusTooSmall { bits: 8, min: 10 }));
    assert!(errors.contains(&NttValidationError::GasBudgetZero));
    assert!(errors.contains(&NttValidationError::BatchSizeZero));
    assert!(
        errors.contains(&NttValidationError::IncompatibleSchemeAndDegree {
            scheme: "Falcon512".to_string(),
            degree: 300,
        })
    );
}

#[test]
fn degree_lookup_returns_canonical_values() {
    assert_eq!(scheme_ntt_degree(NttScheme::Falcon512), 512);
    assert_eq!(scheme_ntt_degree(NttScheme::Falcon1024), 1024);
    assert_eq!(scheme_ntt_degree(NttScheme::Dilithium3), 256);
    assert_eq!(scheme_ntt_degree(NttScheme::Kyber1024), 256);
}

#[test]
fn gas_estimation_reflects_strategy_efficiency() {
    let mut config = default_ntt_config();
    config.precompile_strategy = PrecompileStrategy::NoPrecompile;
    let no_precompile = estimate_precompile_gas(&config);

    config.precompile_strategy = PrecompileStrategy::GenericNtt;
    let generic = estimate_precompile_gas(&config);

    config.precompile_strategy = PrecompileStrategy::HardwareAccelerated;
    let hw = estimate_precompile_gas(&config);

    assert!(no_precompile > generic);
    assert!(generic > hw);
}

#[test]
fn compare_strategies_returns_all_variants() {
    let config = default_ntt_config();
    let compared = compare_strategies(&config);

    assert_eq!(compared.len(), 5);
    assert_eq!(compared[0].0, "NoPrecompile");
    assert_eq!(compared[1].0, "GenericNtt");
    assert_eq!(compared[2].0, "SchemeSpecific");
    assert_eq!(compared[3].0, "BatchedNtt");
    assert_eq!(compared[4].0, "HardwareAccelerated");
}

#[test]
fn pq_readiness_scales_with_scheme_coverage() {
    let narrow = compute_pq_readiness(&[NttScheme::Falcon512]);
    let broad = compute_pq_readiness(&[
        NttScheme::Falcon512,
        NttScheme::Falcon1024,
        NttScheme::Dilithium2,
        NttScheme::Dilithium3,
        NttScheme::Dilithium5,
        NttScheme::Kyber512,
        NttScheme::Kyber768,
        NttScheme::Kyber1024,
    ]);

    assert!(narrow > 0.0);
    assert!(narrow < broad);
    assert_eq!(broad, 1.0);
}

#[test]
fn computed_stats_include_strategy_and_scheme_breakdowns() {
    let config = default_ntt_config();
    let stats = compute_ntt_stats(&config);

    assert!(stats.gas_reduction_factor > 1.0);
    assert!(stats.estimated_precompile_gas > 0);
    assert!(stats.ntt_operations_per_verify > 0);
    assert_eq!(stats.strategy_comparison.len(), 5);
    assert_eq!(stats.scheme_comparison.len(), 8);
    assert!(stats
        .scheme_comparison
        .iter()
        .any(|(scheme, _)| scheme == "Falcon512"));
}

#[test]
fn ntt_commitment_is_deterministic_and_sensitive_to_inputs() {
    let config = default_ntt_config();
    let params_a = [1u8, 2, 3, 4];
    let params_b = [1u8, 2, 3, 5];

    let hash_a1 = compute_ntt_commitment(&config, &params_a);
    let hash_a2 = compute_ntt_commitment(&config, &params_a);
    let hash_b = compute_ntt_commitment(&config, &params_b);

    assert_eq!(hash_a1, hash_a2);
    assert_ne!(hash_a1, hash_b);
}
