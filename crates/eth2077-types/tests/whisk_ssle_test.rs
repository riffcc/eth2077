use eth2077_types::whisk_ssle::{
    compare_election_modes, compute_anonymity_set, compute_whisk_commitment, compute_whisk_stats,
    default_whisk_config, estimate_dos_resistance, estimate_proof_overhead, validate_whisk_config,
    LeaderElectionMode, WhiskConfig, WhiskValidationError,
};

#[test]
fn default_config_matches_requested_spec_values() {
    let config = default_whisk_config();

    assert_eq!(config.mode, LeaderElectionMode::WhiskSSLE);
    assert_eq!(config.candidate_trackers_count, 16_384);
    assert_eq!(config.proposer_trackers_count, 32);
    assert_eq!(config.validators_per_shuffle, 128);
    assert_eq!(config.epochs_per_shuffling_phase, 256);
    assert_eq!(config.proposer_selection_gap, 2);
    assert_eq!(config.curdleproofs_n_blinders, 4);
}

#[test]
fn default_config_validates_successfully() {
    let config = default_whisk_config();
    assert_eq!(validate_whisk_config(&config), Ok(()));
}

#[test]
fn validation_reports_multiple_constraint_errors() {
    let config = WhiskConfig {
        mode: LeaderElectionMode::WhiskSSLE,
        candidate_trackers_count: 0,
        proposer_trackers_count: 1,
        validators_per_shuffle: 0,
        epochs_per_shuffling_phase: 1,
        proposer_selection_gap: 2,
        curdleproofs_n_blinders: 0,
    };

    let errors = validate_whisk_config(&config).unwrap_err();
    assert!(errors.contains(&WhiskValidationError::ZeroCandidateTrackers));
    assert!(
        errors.contains(&WhiskValidationError::ProposerExceedsCandidates {
            proposers: 1,
            candidates: 0,
        })
    );
    assert!(errors.contains(&WhiskValidationError::ZeroValidatorsPerShuffle));
    assert!(
        errors.contains(&WhiskValidationError::SelectionGapTooLarge {
            gap: 2,
            shuffling_epochs: 1,
        })
    );
    assert!(errors
        .iter()
        .any(|error| matches!(error, WhiskValidationError::IncompatibleMode { .. })));
}

#[test]
fn deterministic_mode_rejects_whisk_specific_params() {
    let config = WhiskConfig {
        mode: LeaderElectionMode::Deterministic,
        candidate_trackers_count: 128,
        proposer_trackers_count: 8,
        validators_per_shuffle: 32,
        epochs_per_shuffling_phase: 64,
        proposer_selection_gap: 1,
        curdleproofs_n_blinders: 2,
    };

    let errors = validate_whisk_config(&config).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| matches!(error, WhiskValidationError::IncompatibleMode { .. })));
}

#[test]
fn anonymity_set_increases_with_stronger_modes() {
    let base = default_whisk_config();

    let deterministic = compute_anonymity_set(&WhiskConfig {
        mode: LeaderElectionMode::Deterministic,
        ..base.clone()
    });
    let partial = compute_anonymity_set(&WhiskConfig {
        mode: LeaderElectionMode::PartialWhisk,
        ..base.clone()
    });
    let whisk = compute_anonymity_set(&WhiskConfig {
        mode: LeaderElectionMode::WhiskSSLE,
        ..base.clone()
    });
    let committee = compute_anonymity_set(&WhiskConfig {
        mode: LeaderElectionMode::CommitteeWhisk,
        ..base
    });

    assert!(deterministic < partial);
    assert!(partial < whisk);
    assert!(whisk < committee);
}

#[test]
fn dos_resistance_is_baseline_for_deterministic_and_higher_for_whisk() {
    let base = default_whisk_config();
    let deterministic_score = estimate_dos_resistance(&WhiskConfig {
        mode: LeaderElectionMode::Deterministic,
        proposer_selection_gap: 0,
        curdleproofs_n_blinders: 0,
        ..base.clone()
    });
    let whisk_score = estimate_dos_resistance(&base);

    assert_eq!(deterministic_score, 1.0);
    assert!(whisk_score > deterministic_score);
}

#[test]
fn proof_overhead_is_zero_for_deterministic_and_nonzero_for_whisk() {
    let base = default_whisk_config();
    let deterministic_proof = estimate_proof_overhead(&WhiskConfig {
        mode: LeaderElectionMode::Deterministic,
        proposer_selection_gap: 0,
        curdleproofs_n_blinders: 0,
        ..base.clone()
    });
    let whisk_proof = estimate_proof_overhead(&base);

    assert_eq!(deterministic_proof, 0);
    assert!(whisk_proof > 0);
}

#[test]
fn mode_comparison_lists_all_modes() {
    let config = default_whisk_config();
    let comparison = compare_election_modes(&config);

    assert_eq!(comparison.len(), 4);
    assert_eq!(comparison[0].0, "Deterministic");
    assert_eq!(comparison[1].0, "WhiskSSLE");
    assert_eq!(comparison[2].0, "PartialWhisk");
    assert_eq!(comparison[3].0, "CommitteeWhisk");
}

#[test]
fn commitment_is_stable_independent_of_input_order() {
    let a = [1u8; 32];
    let b = [2u8; 32];
    let c = [3u8; 32];

    let first = compute_whisk_commitment(&[a, b, c]);
    let second = compute_whisk_commitment(&[c, a, b]);

    assert_eq!(first, second);
}

#[test]
fn stats_fields_are_coherent() {
    let config = default_whisk_config();
    let stats = compute_whisk_stats(&config);

    assert!(stats.dos_resistance_improvement > 1.0);
    assert!(stats.shuffling_overhead_per_epoch > 0.0);
    assert!(stats.proof_size_bytes > 0);
    assert!(stats.selection_entropy_bits > 0.0);
    assert!(stats.latency_overhead_ms > 0.0);
    assert_eq!(
        stats.validator_anonymity_set,
        compute_anonymity_set(&config)
    );
    assert_eq!(stats.mode_comparison.len(), 4);
}
