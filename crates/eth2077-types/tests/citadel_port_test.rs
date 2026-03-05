use eth2077_types::citadel_port::{
    compute_citadel_port_commitment, compute_citadel_port_stats, default_citadel_port_config,
    validate_citadel_port, CitadelModule, CitadelPortValidationError, PlaceholderEntry,
    PlaceholderKind,
};

#[test]
fn default_config_is_valid() {
    let config = default_citadel_port_config();
    assert!(validate_citadel_port(&config).is_ok());
}

#[test]
fn validation_rejects_empty_migrations() {
    let mut config = default_citadel_port_config();
    config.migrations.clear();

    let errors =
        validate_citadel_port(&config).expect_err("empty migrations should fail validation");
    assert!(errors.contains(&CitadelPortValidationError::EmptyMigrations));
}

#[test]
fn validation_rejects_too_many_placeholders() {
    let mut config = default_citadel_port_config();
    config.max_allowed_placeholders = 0;

    let errors = validate_citadel_port(&config)
        .expect_err("placeholder count above max should fail validation");
    assert!(errors.contains(&CitadelPortValidationError::TooManyPlaceholders { count: 4, max: 0 }));
}

#[test]
fn validation_rejects_high_severity() {
    let mut config = default_citadel_port_config();
    config.max_severity_allowed = 3;
    config.migrations[0].placeholders.push(PlaceholderEntry {
        location: "citadel/vote_accumulator.rs:999".to_string(),
        kind: PlaceholderKind::HardcodedValue,
        description: "temporary constant for liveness scoring".to_string(),
        blocking_proofs: vec!["fork_choice_liveness".to_string()],
        severity: 5,
    });

    let errors = validate_citadel_port(&config)
        .expect_err("severity above max allowed should fail validation");
    assert!(errors.iter().any(|error| {
        matches!(
            error,
            CitadelPortValidationError::HighSeverityPlaceholder { location, severity }
            if location == "citadel/vote_accumulator.rs:999" && *severity == 5
        )
    }));
}

#[test]
fn stats_count_placeholders() {
    let config = default_citadel_port_config();
    let stats = compute_citadel_port_stats(&config);

    assert_eq!(stats.total_modules, 4);
    assert_eq!(stats.modules_complete, 1);
    assert_eq!(stats.total_placeholders, 4);
    assert_eq!(stats.critical_placeholders, 2);
}

#[test]
fn stats_progress_bounded() {
    let config = default_citadel_port_config();
    let stats = compute_citadel_port_stats(&config);

    assert!((0.0..=1.0).contains(&stats.migration_progress));
}

#[test]
fn commitment_is_deterministic() {
    let config = default_citadel_port_config();

    let first = compute_citadel_port_commitment(&config);
    let second = compute_citadel_port_commitment(&config);
    assert_eq!(first, second);
}

#[test]
fn commitment_is_config_sensitive() {
    let config = default_citadel_port_config();
    let original = compute_citadel_port_commitment(&config);

    let mut modified = config.clone();
    modified.target_module = CitadelModule::ForkChoice;
    let changed = compute_citadel_port_commitment(&modified);

    assert_ne!(original, changed);
}
