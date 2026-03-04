use eth2077_types::sre_runbooks::{
    compute_stats, EscalationPolicy, GameDayExercise, GameDayPhase, IncidentSeverity, Runbook,
    RunbookCategory, RunbookStep, SreRunbooksConfig,
};

fn sample_runbook(last_tested_unix: Option<u64>) -> Runbook {
    Runbook {
        id: "rb-node-recovery-01".to_string(),
        title: "Recover execution node".to_string(),
        category: RunbookCategory::NodeRecovery,
        severity_applies: vec![IncidentSeverity::Sev1Critical, IncidentSeverity::Sev2Major],
        steps: vec![
            RunbookStep {
                order: 1,
                title: "Check process status".to_string(),
                command: Some("systemctl status geth".to_string()),
                expected_outcome: "Process state is known".to_string(),
                timeout_seconds: 60,
                rollback_command: None,
            },
            RunbookStep {
                order: 2,
                title: "Restart node".to_string(),
                command: Some("systemctl restart geth".to_string()),
                expected_outcome: "Node rejoins peer mesh".to_string(),
                timeout_seconds: 180,
                rollback_command: Some("systemctl stop geth".to_string()),
            },
        ],
        estimated_duration_minutes: 15,
        last_tested_unix,
        owner: "sre@eth2077.local".to_string(),
        version: 3,
    }
}

fn sample_exercise(
    id: &str,
    detect: Option<u64>,
    mitigate: Option<u64>,
    success: bool,
) -> GameDayExercise {
    GameDayExercise {
        id: id.to_string(),
        scenario_name: "regional partition".to_string(),
        phase: GameDayPhase::PostMortem,
        injected_faults: vec!["partition-eu-west".to_string()],
        participating_nodes: 12,
        started_at_unix: 1_700_000_000,
        ended_at_unix: Some(1_700_000_600),
        detection_time_seconds: detect,
        mitigation_time_seconds: mitigate,
        success,
    }
}

#[test]
fn default_config_matches_requested_values() {
    let config = SreRunbooksConfig::default();
    assert_eq!(config.max_detection_time_seconds, 300);
    assert_eq!(config.max_mitigation_time_seconds, 1_800);
    assert_eq!(config.min_runbook_test_interval_days, 30);
    assert_eq!(config.escalation_policy, EscalationPolicy::PagerDuty);
    assert_eq!(config.game_day_frequency_days, 14);
    assert_eq!(config.required_game_day_scenarios, 5);
    assert!(config.auto_rollback_enabled);
    assert!(config.validate().is_empty());
}

#[test]
fn validation_catches_multiple_invalid_fields() {
    let mut config = SreRunbooksConfig::default();
    config.max_detection_time_seconds = 0;
    config.max_mitigation_time_seconds = 0;
    config.game_day_frequency_days = 0;
    config.required_game_day_scenarios = 0;

    let errors = config.validate();
    assert!(errors
        .iter()
        .any(|e| e.field == "max_detection_time_seconds"));
    assert!(errors
        .iter()
        .any(|e| e.field == "max_mitigation_time_seconds"));
    assert!(errors.iter().any(|e| e.field == "game_day_frequency_days"));
    assert!(errors
        .iter()
        .any(|e| e.field == "required_game_day_scenarios"));
}

#[test]
fn runbook_staleness_detection_behaves_as_expected() {
    let now = 1_800_000_000;
    let fresh = sample_runbook(Some(now - 5 * 86_400));
    let stale = sample_runbook(Some(now - 45 * 86_400));
    let never_tested = sample_runbook(None);

    assert!(!fresh.is_stale(now, 30));
    assert!(stale.is_stale(now, 30));
    assert!(never_tested.is_stale(now, 30));
}

#[test]
fn stats_compute_averages_success_rate_and_stale_count() {
    let now = 1_800_000_000;
    let runbooks = vec![
        sample_runbook(Some(now - 10 * 86_400)),
        sample_runbook(Some(now - 45 * 86_400)),
        sample_runbook(None),
    ];

    let exercises = vec![
        sample_exercise("gd-1", Some(120), Some(900), true),
        sample_exercise("gd-2", Some(180), Some(1_200), false),
        sample_exercise("gd-3", None, None, true),
    ];

    let stats = compute_stats(&runbooks, &exercises, now);
    assert_eq!(stats.total_runbooks, 3);
    assert_eq!(stats.stale_runbooks, 2);
    assert_eq!(stats.game_days_run, 3);
    assert!((stats.avg_detection_time_s - 150.0).abs() < f64::EPSILON);
    assert!((stats.avg_mitigation_time_s - 1_050.0).abs() < f64::EPSILON);
    assert!((stats.game_day_success_rate - (2.0 / 3.0)).abs() < 1e-12);
}

#[test]
fn stats_commitment_is_deterministic_and_input_sensitive() {
    let now = 1_800_000_000;
    let runbooks = vec![sample_runbook(Some(now - 10 * 86_400))];
    let exercises = vec![sample_exercise("gd-1", Some(120), Some(900), true)];

    let a = compute_stats(&runbooks, &exercises, now);
    let b = compute_stats(&runbooks, &exercises, now);
    assert_eq!(a.commitment, b.commitment);

    let changed_exercises = vec![sample_exercise("gd-1", Some(121), Some(900), true)];
    let c = compute_stats(&runbooks, &changed_exercises, now);
    assert_ne!(a.commitment, c.commitment);
}
