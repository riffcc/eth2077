use eth2077_types::go_nogo_gate::{
    compute_stats, ChecklistCategory, ChecklistItem, GateDecision, GateVerdict, GoNogoGateConfig,
    LaunchChecklist, RiskLevel, SignoffStatus,
};

fn item(
    id: &str,
    category: ChecklistCategory,
    status: SignoffStatus,
    assignee: &str,
    risk: RiskLevel,
) -> ChecklistItem {
    ChecklistItem {
        id: id.to_string(),
        category,
        description: format!("{id} description"),
        status,
        assignee: assignee.to_string(),
        risk_if_skipped: risk,
        evidence_url: Some(format!("https://example.com/{id}")),
        signed_off_at_unix: Some(1_700_000_000),
        notes: None,
    }
}

fn gate(name: &str, verdict: GateVerdict) -> GateDecision {
    GateDecision {
        gate_name: name.to_string(),
        verdict,
        decided_at_unix: 1_700_000_100,
        decided_by: "release-bot".to_string(),
        blocking_items: Vec::new(),
        conditions: Vec::new(),
        next_review_unix: None,
    }
}

fn ready_checklist() -> LaunchChecklist {
    LaunchChecklist {
        id: "lc-mainnet-001".to_string(),
        name: "Mainnet Launch".to_string(),
        target_network: "mainnet".to_string(),
        items: vec![
            item(
                "sec-1",
                ChecklistCategory::Security,
                SignoffStatus::Approved,
                "alice",
                RiskLevel::Critical,
            ),
            item(
                "perf-1",
                ChecklistCategory::Performance,
                SignoffStatus::Approved,
                "bob",
                RiskLevel::High,
            ),
            item(
                "cons-1",
                ChecklistCategory::Consensus,
                SignoffStatus::Approved,
                "carol",
                RiskLevel::Medium,
            ),
            item(
                "infra-1",
                ChecklistCategory::Infrastructure,
                SignoffStatus::Approved,
                "dave",
                RiskLevel::Low,
            ),
            item(
                "test-1",
                ChecklistCategory::Testing,
                SignoffStatus::Approved,
                "erin",
                RiskLevel::Low,
            ),
            item(
                "gov-1",
                ChecklistCategory::Governance,
                SignoffStatus::Pending,
                "frank",
                RiskLevel::Negligible,
            ),
        ],
        gates: vec![
            gate("security-gate", GateVerdict::Go),
            gate("perf-gate", GateVerdict::Go),
            gate("consensus-gate", GateVerdict::Go),
            gate("infra-gate", GateVerdict::Go),
            gate("test-gate", GateVerdict::Go),
            gate("ops-gate", GateVerdict::ConditionalGo),
        ],
        created_at_unix: 1_700_000_000,
        target_launch_unix: Some(1_700_086_400),
        final_verdict: Some(GateVerdict::Go),
    }
}

#[test]
fn default_config_matches_expected_eth2077_values() {
    let config = GoNogoGateConfig::default();
    assert_eq!(config.required_go_gates, 5);
    assert_eq!(config.max_conditional_gates, 2);
    assert_eq!(config.min_signoff_count, 3);
    assert_eq!(config.max_critical_risks_open, 0);
    assert_eq!(config.review_cadence_hours, 24);
    assert!(config.auto_nogo_on_security_fail);
    assert_eq!(
        config.required_categories,
        vec![
            ChecklistCategory::Security,
            ChecklistCategory::Performance,
            ChecklistCategory::Consensus,
            ChecklistCategory::Infrastructure,
            ChecklistCategory::Testing,
        ]
    );
}

#[test]
fn validation_returns_multiple_config_errors() {
    let config = GoNogoGateConfig {
        required_go_gates: 0,
        max_conditional_gates: 2,
        required_categories: vec![
            ChecklistCategory::Performance,
            ChecklistCategory::Performance,
        ],
        min_signoff_count: 0,
        max_critical_risks_open: 0,
        review_cadence_hours: 0,
        auto_nogo_on_security_fail: true,
    };

    let errors = config.validate();
    assert!(errors.iter().any(|e| e.field == "required_go_gates"));
    assert!(errors.iter().any(|e| e.field == "max_conditional_gates"));
    assert!(errors.iter().any(|e| e.field == "required_categories"));
    assert!(errors.iter().any(|e| e.field == "min_signoff_count"));
    assert!(errors.iter().any(|e| e.field == "review_cadence_hours"));
}

#[test]
fn compute_stats_counts_fields_and_commitment_correctly() {
    let mut checklist = ready_checklist();
    checklist.items.push(item(
        "sec-2",
        ChecklistCategory::Security,
        SignoffStatus::Rejected,
        "alice",
        RiskLevel::Critical,
    ));
    checklist.gates.push(gate("final-gate", GateVerdict::NoGo));

    let stats = compute_stats(&checklist);
    assert_eq!(stats.total_items, 7);
    assert_eq!(stats.approved_items, 5);
    assert_eq!(stats.rejected_items, 1);
    assert_eq!(stats.pending_items, 1);
    assert_eq!(stats.go_gates, 5);
    assert_eq!(stats.nogo_gates, 1);
    assert_eq!(stats.critical_risks_open, 1);
    assert!((stats.approval_rate - (5.0 / 7.0)).abs() < 1e-12);

    let same = compute_stats(&checklist);
    assert_eq!(stats.commitment, same.commitment);

    checklist.items[0].notes = Some("updated evidence note".to_string());
    let changed = compute_stats(&checklist);
    assert_ne!(stats.commitment, changed.commitment);
}

#[test]
fn launch_ready_happy_path_and_blocking_paths() {
    let config = GoNogoGateConfig::default();
    let checklist = ready_checklist();
    assert!(checklist.is_launch_ready(&config));

    let mut security_rejected = ready_checklist();
    security_rejected.items[0].status = SignoffStatus::Rejected;
    assert!(!security_rejected.is_launch_ready(&config));

    let mut insufficient_go = ready_checklist();
    insufficient_go.gates[0].verdict = GateVerdict::ConditionalGo;
    assert!(!insufficient_go.is_launch_ready(&config));
}
