use eth2077_types::post_quantum::*;

fn base_entry(component: CryptoComponent) -> PqMigrationEntry {
    PqMigrationEntry {
        component,
        current_algorithm: "baseline".to_string(),
        target_algorithm: PqAlgorithm::HybridClassicalPQ,
        phase: MigrationPhase::Prototype,
        signature_size_bytes: 800,
        verification_time_us: 120.0,
        security_bits: 192,
    }
}

#[test]
fn default_config_is_valid() {
    let config = default_post_quantum_config();
    assert!(!config.entries.is_empty());
    assert_eq!(validate_pq_config(&config), Ok(()));
}

#[test]
fn validation_rejects_empty_entries() {
    let mut config = default_post_quantum_config();
    config.entries.clear();

    let errors = validate_pq_config(&config).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(errors.contains(&PqValidationError::EmptyEntries));
}

#[test]
fn validation_rejects_duplicate_components() {
    let mut config = default_post_quantum_config();
    let duplicate = PqMigrationEntry {
        component: CryptoComponent::ValidatorSignatures,
        current_algorithm: "BLS12-381".to_string(),
        target_algorithm: PqAlgorithm::MLDSA65,
        phase: MigrationPhase::Prototype,
        signature_size_bytes: 3309,
        verification_time_us: 190.0,
        security_bits: 192,
    };
    config.entries.push(duplicate);

    let errors = validate_pq_config(&config).unwrap_err();
    assert!(errors.contains(&PqValidationError::DuplicateComponent));
}

#[test]
fn stats_computation_with_mixed_phases() {
    let config = PostQuantumConfig {
        entries: vec![
            PqMigrationEntry {
                phase: MigrationPhase::MainnetDefault,
                security_bits: 192,
                ..base_entry(CryptoComponent::ValidatorSignatures)
            },
            PqMigrationEntry {
                phase: MigrationPhase::MainnetOptIn,
                security_bits: 192,
                ..base_entry(CryptoComponent::TransactionSignatures)
            },
            PqMigrationEntry {
                phase: MigrationPhase::TestnetDeploy,
                security_bits: 128,
                ..base_entry(CryptoComponent::AttestationAggregation)
            },
            PqMigrationEntry {
                phase: MigrationPhase::Prototype,
                security_bits: 96,
                ..base_entry(CryptoComponent::BeaconBlockSigning)
            },
        ],
        target_phase: MigrationPhase::MainnetOptIn,
        max_signature_overhead_pct: 2000.0,
        max_verification_overhead_pct: 1000.0,
        require_hybrid: false,
    };

    let stats = compute_pq_stats(&config);
    assert_eq!(stats.components_assessed, 4);
    assert_eq!(stats.on_track, 2);
    assert_eq!(stats.behind_schedule, 2);
    assert!(stats.avg_signature_overhead_pct > 0.0);
    assert!(stats.avg_verification_overhead_pct > 0.0);
    assert_eq!(stats.weakest_component, "BeaconBlockSigning");
    assert!(stats.migration_readiness >= 0.0 && stats.migration_readiness <= 1.0);
}

#[test]
fn compare_algorithms_returns_multiple_options() {
    let compared = compare_algorithms(CryptoComponent::ValidatorSignatures);
    assert!(compared.len() >= 4);
    assert_eq!(compared[0].0, "BLS12-381");
    assert!(compared
        .iter()
        .any(|(name, _, _)| name == "HybridClassicalPQ"));
    assert!(compared.iter().any(|(name, _, _)| name == "MLDSA65"));
    assert!(compared
        .iter()
        .all(|(_, size, verify)| *size > 0 && *verify > 0.0));
}

#[test]
fn signature_overhead_calculation() {
    let overhead = estimate_signature_overhead(64, 320);
    assert!((overhead - 400.0).abs() < 1e-9);

    let reduced = estimate_signature_overhead(128, 64);
    assert!((reduced + 50.0).abs() < 1e-9);

    let degenerate = estimate_signature_overhead(0, 1024);
    assert_eq!(degenerate, 0.0);
}

#[test]
fn commitment_is_deterministic() {
    let config = default_post_quantum_config();
    let commitment_a = compute_pq_commitment(&config);
    let commitment_b = compute_pq_commitment(&config);
    assert_eq!(commitment_a, commitment_b);

    let mut tweaked = config.clone();
    tweaked.entries[0].verification_time_us += 1.0;
    let commitment_c = compute_pq_commitment(&tweaked);
    assert_ne!(commitment_a, commitment_c);
}

#[test]
fn overall_phase_reflects_weakest_component() {
    let config = PostQuantumConfig {
        entries: vec![
            PqMigrationEntry {
                phase: MigrationPhase::MainnetDefault,
                ..base_entry(CryptoComponent::ValidatorSignatures)
            },
            PqMigrationEntry {
                phase: MigrationPhase::Research,
                ..base_entry(CryptoComponent::DepositContract)
            },
            PqMigrationEntry {
                phase: MigrationPhase::MainnetOptIn,
                ..base_entry(CryptoComponent::WithdrawalCredentials)
            },
        ],
        target_phase: MigrationPhase::Prototype,
        max_signature_overhead_pct: 10_000.0,
        max_verification_overhead_pct: 10_000.0,
        require_hybrid: true,
    };

    let stats = compute_pq_stats(&config);
    assert_eq!(stats.overall_phase, MigrationPhase::Research);
}

#[test]
fn validation_rejects_insufficient_security_bits() {
    let config = PostQuantumConfig {
        entries: vec![
            PqMigrationEntry {
                security_bits: 96,
                ..base_entry(CryptoComponent::ValidatorSignatures)
            },
            PqMigrationEntry {
                ..base_entry(CryptoComponent::TransactionSignatures)
            },
        ],
        target_phase: MigrationPhase::Prototype,
        max_signature_overhead_pct: 50_000.0,
        max_verification_overhead_pct: 50_000.0,
        require_hybrid: false,
    };

    let errors = validate_pq_config(&config).unwrap_err();
    assert!(errors.iter().any(|error| matches!(
        error,
        PqValidationError::InsufficientSecurityBits {
            component,
            bits,
            min
        } if component == "ValidatorSignatures" && *bits == 96 && *min == 128
    )));
}

#[test]
fn validation_rejects_overhead_exceeds_limits() {
    let config = PostQuantumConfig {
        entries: vec![
            PqMigrationEntry {
                component: CryptoComponent::DepositContract,
                current_algorithm: "ECDSA-secp256k1".to_string(),
                target_algorithm: PqAlgorithm::SPHINCSSHA2_128f,
                phase: MigrationPhase::Research,
                signature_size_bytes: 17_088,
                verification_time_us: 900.0,
                security_bits: 128,
            },
            PqMigrationEntry {
                ..base_entry(CryptoComponent::ValidatorSignatures)
            },
        ],
        target_phase: MigrationPhase::Prototype,
        max_signature_overhead_pct: 250.0,
        max_verification_overhead_pct: 200.0,
        require_hybrid: false,
    };

    let errors = validate_pq_config(&config).unwrap_err();
    assert!(errors.iter().any(|error| matches!(
        error,
        PqValidationError::OverheadExceedsLimit { component, .. } if component == "DepositContract"
    )));
}

#[test]
fn commitment_is_stable_under_entry_reordering() {
    let mut config_a = default_post_quantum_config();
    let mut config_b = config_a.clone();
    config_b.entries.reverse();

    let commitment_a = compute_pq_commitment(&config_a);
    let commitment_b = compute_pq_commitment(&config_b);

    assert_eq!(commitment_a, commitment_b);

    config_a.entries[0].current_algorithm = "altered".to_string();
    let commitment_c = compute_pq_commitment(&config_a);
    assert_ne!(commitment_a, commitment_c);
}

#[test]
fn stats_empty_config_returns_zeroed_metrics() {
    let config = PostQuantumConfig {
        entries: vec![],
        target_phase: MigrationPhase::MainnetDefault,
        max_signature_overhead_pct: 1000.0,
        max_verification_overhead_pct: 500.0,
        require_hybrid: true,
    };

    let stats = compute_pq_stats(&config);
    assert_eq!(stats.components_assessed, 0);
    assert_eq!(stats.on_track, 0);
    assert_eq!(stats.behind_schedule, 0);
    assert_eq!(stats.avg_signature_overhead_pct, 0.0);
    assert_eq!(stats.avg_verification_overhead_pct, 0.0);
    assert_eq!(stats.migration_readiness, 0.0);
    assert_eq!(stats.overall_phase, MigrationPhase::Research);
}
