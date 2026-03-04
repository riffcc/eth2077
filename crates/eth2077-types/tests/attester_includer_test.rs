use eth2077_types::attester_includer::{
    compare_separation_models, compute_ais_commitment, compute_ais_stats, default_ais_config,
    estimate_censorship_resistance, estimate_mev_difficulty, validate_ais_config, AISConfig,
    AISValidationError, CensorshipResistance, SeparationModel,
};

fn sample_key(byte: u8) -> [u8; 32] {
    [byte; 32]
}

#[test]
fn default_config_matches_expected_values() {
    let config = default_ais_config();

    assert_eq!(config.separation_model, SeparationModel::HardSeparation);
    assert_eq!(
        config.censorship_resistance,
        CensorshipResistance::FocilStyle
    );
    assert_eq!(config.attester_count, 128);
    assert_eq!(config.includer_count, 16);
    assert_eq!(config.inclusion_delay_slots, 1);
    assert_eq!(config.max_inclusion_list_size, 256);
    assert!((config.attester_reward_share - 0.7).abs() < 1e-9);
    assert!((config.includer_reward_share - 0.3).abs() < 1e-9);
}

#[test]
fn default_config_validates() {
    let config = default_ais_config();
    assert_eq!(validate_ais_config(&config), Ok(()));
}

#[test]
fn validation_rejects_zero_roles_and_reward_mismatch() {
    let config = AISConfig {
        separation_model: SeparationModel::HardSeparation,
        censorship_resistance: CensorshipResistance::FocilStyle,
        attester_count: 0,
        includer_count: 0,
        inclusion_delay_slots: 1,
        max_inclusion_list_size: 256,
        attester_reward_share: 0.8,
        includer_reward_share: 0.3,
    };

    let errors = validate_ais_config(&config).expect_err("expected validation errors");
    assert!(errors.contains(&AISValidationError::ZeroAttesters));
    assert!(errors.contains(&AISValidationError::ZeroIncluders));
    assert!(errors
        .iter()
        .any(|error| matches!(error, AISValidationError::RewardShareMismatch { .. })));
}

#[test]
fn validation_rejects_incompatible_model_resistance_pair() {
    let config = AISConfig {
        separation_model: SeparationModel::CurrentUnified,
        censorship_resistance: CensorshipResistance::FocilStyle,
        ..default_ais_config()
    };

    let errors = validate_ais_config(&config).expect_err("expected validation errors");
    assert!(errors.iter().any(|error| matches!(
        error,
        AISValidationError::IncompatibleModel { model, resistance }
            if model == "CurrentUnified" && resistance == "FocilStyle"
    )));
}

#[test]
fn validation_rejects_excess_delay_and_large_inclusion_list() {
    let config = AISConfig {
        separation_model: SeparationModel::HardSeparation,
        censorship_resistance: CensorshipResistance::None,
        inclusion_delay_slots: 20,
        max_inclusion_list_size: 2_048,
        ..default_ais_config()
    };

    let errors = validate_ais_config(&config).expect_err("expected validation errors");
    assert!(errors.iter().any(|error| matches!(
        error,
        AISValidationError::DelayTooHigh {
            slots: 20,
            max_slots: 8
        }
    )));
    assert!(errors.iter().any(|error| matches!(
        error,
        AISValidationError::InclusionListTooLarge { size, max } if *size == 2_048 && *max == 1_024
    )));
}

#[test]
fn compute_stats_outputs_reasonable_ranges() {
    let stats = compute_ais_stats(&default_ais_config());

    assert!((0.0..=1.0).contains(&stats.censorship_resistance_score));
    assert!((0.0..=1.0).contains(&stats.mev_extraction_difficulty));
    assert!(stats.attestation_overhead_ratio > 0.0);
    assert!(stats.inclusion_latency_slots >= 0.0);
    assert!((0.0..=1.0).contains(&stats.centralization_risk));
    assert_eq!(stats.model_comparison.len(), 5);
}

#[test]
fn stronger_censorship_mechanism_improves_score() {
    let weak = AISConfig {
        separation_model: SeparationModel::HardSeparation,
        censorship_resistance: CensorshipResistance::None,
        ..default_ais_config()
    };
    let strong = AISConfig {
        censorship_resistance: CensorshipResistance::ThresholdEncryption,
        ..weak.clone()
    };

    let weak_score = estimate_censorship_resistance(&weak);
    let strong_score = estimate_censorship_resistance(&strong);
    assert!(strong_score > weak_score);
}

#[test]
fn hard_separation_raises_mev_difficulty_vs_unified() {
    let unified = AISConfig {
        separation_model: SeparationModel::CurrentUnified,
        censorship_resistance: CensorshipResistance::ForcedInclusion,
        ..default_ais_config()
    };

    let separated = AISConfig {
        separation_model: SeparationModel::HardSeparation,
        ..unified.clone()
    };

    let unified_score = estimate_mev_difficulty(&unified);
    let separated_score = estimate_mev_difficulty(&separated);
    assert!(separated_score > unified_score);
}

#[test]
fn model_comparison_contains_all_models_and_is_sorted() {
    let comparison = compare_separation_models(&default_ais_config());

    let names: Vec<&str> = comparison.iter().map(|(name, _)| name.as_str()).collect();
    for expected in [
        "CurrentUnified",
        "SoftSeparation",
        "HardSeparation",
        "CommitteeIncluder",
        "AuctionedIncluder",
    ] {
        assert!(names.contains(&expected));
    }

    for pair in comparison.windows(2) {
        assert!(pair[0].1 >= pair[1].1);
    }
}

#[test]
fn commitment_is_order_independent_within_roles_and_role_sensitive() {
    let attesters_a = vec![sample_key(1), sample_key(3), sample_key(2)];
    let includers_a = vec![sample_key(9), sample_key(8)];

    let attesters_b = vec![sample_key(2), sample_key(1), sample_key(3)];
    let includers_b = vec![sample_key(8), sample_key(9)];

    let commitment_a = compute_ais_commitment(&attesters_a, &includers_a);
    let commitment_b = compute_ais_commitment(&attesters_b, &includers_b);
    assert_eq!(commitment_a, commitment_b);

    let swapped_roles = compute_ais_commitment(&includers_a, &attesters_a);
    assert_ne!(commitment_a, swapped_roles);
}
