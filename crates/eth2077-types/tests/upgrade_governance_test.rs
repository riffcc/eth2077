use eth2077_types::upgrade_governance::{
    compute_stats, ForkStrategy, GovernanceModel, RehearsalOutcome, RehearsalRun,
    UpgradeGovernanceConfig, UpgradePhase, UpgradeProposal,
};

fn mk_proposal(id: &str, phase: UpgradePhase, signal: f64) -> UpgradeProposal {
    UpgradeProposal {
        id: id.to_string(),
        eip_numbers: vec![7000, 7001],
        title: format!("Upgrade {id}"),
        champion: "eth2077-core".to_string(),
        phase,
        governance_model: GovernanceModel::CoreDevConsensus,
        signal_threshold_pct: 66.7,
        current_signal_pct: signal,
        fork_strategy: ForkStrategy::EpochBoundary,
        activation_epoch: Some(2048),
        created_at_unix: 1_700_000_000,
        approved_at_unix: Some(1_700_010_000),
    }
}

fn mk_rehearsal(id: &str, proposal_id: &str, outcome: RehearsalOutcome) -> RehearsalRun {
    RehearsalRun {
        id: id.to_string(),
        proposal_id: proposal_id.to_string(),
        testnet_name: "eth2077-rehearsal-a".to_string(),
        node_count: 12,
        client_diversity: vec![
            ("lighthouse".to_string(), 40.0),
            ("teku".to_string(), 35.0),
            ("prysm".to_string(), 25.0),
        ],
        started_at_unix: 1_700_100_000,
        ended_at_unix: Some(1_700_100_300),
        outcome,
        blocks_post_fork: 1200,
        finality_disruption_slots: 8,
        rollback_triggered: false,
    }
}

#[test]
fn default_config_matches_spec_and_is_valid() {
    let config = UpgradeGovernanceConfig::default();
    assert_eq!(config.signal_threshold_pct, 66.7);
    assert_eq!(config.min_rehearsal_nodes, 8);
    assert_eq!(config.required_client_count, 3);
    assert_eq!(config.min_blocks_post_fork, 1000);
    assert_eq!(config.max_finality_disruption_slots, 32);
    assert_eq!(config.cooldown_between_upgrades_days, 90);
    assert!(config.emergency_rollback_enabled);
    assert!(config.validate().is_empty());
}

#[test]
fn validation_reports_multiple_numeric_bound_errors() {
    let mut config = UpgradeGovernanceConfig::default();
    config.signal_threshold_pct = 120.0;
    config.min_rehearsal_nodes = 1;
    config.required_client_count = 2;
    config.min_blocks_post_fork = 0;
    config.max_finality_disruption_slots = 10;
    config.cooldown_between_upgrades_days = 0;

    let errors = config.validate();
    for field in [
        "signal_threshold_pct",
        "required_client_count",
        "min_blocks_post_fork",
        "max_finality_disruption_slots",
        "cooldown_between_upgrades_days",
    ] {
        assert!(errors.iter().any(|error| error.field == field));
    }
}

#[test]
fn stats_compute_expected_values_and_stable_commitment() {
    let proposals = vec![
        mk_proposal("u2", UpgradePhase::Accepted, 75.0),
        mk_proposal("u1", UpgradePhase::Discussion, 60.0),
        mk_proposal("u3", UpgradePhase::PostMortem, 90.0),
    ];
    let mut run_a = mk_rehearsal("r2", "u2", RehearsalOutcome::Success);
    run_a.started_at_unix = 1_700_100_000;
    run_a.ended_at_unix = Some(1_700_100_100);
    let mut run_b = mk_rehearsal("r1", "u3", RehearsalOutcome::Failure);
    run_b.started_at_unix = 1_700_110_000;
    run_b.ended_at_unix = Some(1_700_110_300);
    run_b.client_diversity.reverse();
    let rehearsals = vec![run_a.clone(), run_b.clone()];

    let stats = compute_stats(&proposals, &rehearsals);
    assert_eq!(stats.total_proposals, 3);
    assert_eq!(stats.accepted_proposals, 2);
    assert_eq!(stats.rehearsals_run, 2);
    assert!((stats.rehearsal_success_rate - 0.5).abs() < 1e-12);
    assert!((stats.avg_signal_pct - 75.0).abs() < 1e-12);
    assert!((stats.avg_rehearsal_duration_s - 200.0).abs() < 1e-12);

    let proposals_reordered = vec![
        proposals[2].clone(),
        proposals[0].clone(),
        proposals[1].clone(),
    ];
    let rehearsals_reordered = vec![run_b, run_a];
    let reordered_stats = compute_stats(&proposals_reordered, &rehearsals_reordered);
    assert_eq!(stats, reordered_stats);
}

#[test]
fn activation_readiness_requires_successful_qualifying_rehearsal() {
    let config = UpgradeGovernanceConfig::default();
    let proposal = mk_proposal("u-activation", UpgradePhase::Rehearsal, 72.0);
    let rehearsals = vec![
        mk_rehearsal("r-pass", "u-activation", RehearsalOutcome::Success),
        mk_rehearsal("r-other", "different-proposal", RehearsalOutcome::Success),
    ];
    assert!(proposal.is_ready_for_activation(&config, &rehearsals));
}

#[test]
fn activation_readiness_rejects_failed_constraints() {
    let mut config = UpgradeGovernanceConfig::default();
    config.emergency_rollback_enabled = false;

    let mut proposal = mk_proposal("u-fail", UpgradePhase::Accepted, 64.0);
    proposal.signal_threshold_pct = 66.7;

    let mut rehearsal = mk_rehearsal("r-fail", "u-fail", RehearsalOutcome::Success);
    rehearsal.rollback_triggered = true;

    assert!(!proposal.is_ready_for_activation(&config, &[rehearsal]));
}

#[test]
fn serde_roundtrip_preserves_commitment_and_data() {
    let stats = compute_stats(
        &[mk_proposal("u-json", UpgradePhase::Accepted, 81.0)],
        &[mk_rehearsal("r-json", "u-json", RehearsalOutcome::Success)],
    );
    let encoded = serde_json::to_string(&stats).expect("stats should serialize");
    let decoded = serde_json::from_str(&encoded).expect("stats should deserialize");
    assert_eq!(stats, decoded);
}
