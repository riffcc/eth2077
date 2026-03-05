use eth2077_types::strawmap_port::{
    compute_stats, is_phase_a_complete, BenchmarkResult, PortingPhase, PortingTask, StrawmapDomain,
    StrawmapItem, StrawmapPortConfig, VerificationGate,
};

fn sample_items() -> Vec<StrawmapItem> {
    vec![
        StrawmapItem {
            id: "exec-1".into(),
            strawmap_ref: "S-001".into(),
            title: "Engine API".into(),
            domain: StrawmapDomain::Execution,
            phase: PortingPhase::Verified,
            priority: 1,
            estimated_effort_days: 10.0,
            assigned_to: Some("alice".into()),
            dependencies: vec!["cons-1".into()],
            notes: "ready".into(),
        },
        StrawmapItem {
            id: "cons-1".into(),
            strawmap_ref: "S-002".into(),
            title: "Fork choice".into(),
            domain: StrawmapDomain::Consensus,
            phase: PortingPhase::Benchmarked,
            priority: 2,
            estimated_effort_days: 20.0,
            assigned_to: Some("bob".into()),
            dependencies: vec![],
            notes: "bench done".into(),
        },
        StrawmapItem {
            id: "da-1".into(),
            strawmap_ref: "S-003".into(),
            title: "Blob relay".into(),
            domain: StrawmapDomain::DataAvailability,
            phase: PortingPhase::InProgress,
            priority: 3,
            estimated_effort_days: 15.0,
            assigned_to: None,
            dependencies: vec!["exec-1".into()],
            notes: "wip".into(),
        },
    ]
}

fn sample_tasks() -> Vec<PortingTask> {
    vec![
        PortingTask {
            item_id: "exec-1".into(),
            eth2077_module: "execution::engine".into(),
            verification_gate: VerificationGate::FormalProof,
            benchmark_result: BenchmarkResult::Passed,
            proof_artifact: Some("proofs/exec-1.json".into()),
            benchmark_throughput: Some(12_500.0),
            benchmark_latency_ms: Some(76.0),
            started_at_unix: Some(1_700_000_000),
            completed_at_unix: Some(1_700_000_900),
        },
        PortingTask {
            item_id: "cons-1".into(),
            eth2077_module: "consensus::fork_choice".into(),
            verification_gate: VerificationGate::PropertyTest,
            benchmark_result: BenchmarkResult::Marginal,
            proof_artifact: None,
            benchmark_throughput: Some(9_500.0),
            benchmark_latency_ms: Some(101.0),
            started_at_unix: Some(1_700_100_000),
            completed_at_unix: None,
        },
    ]
}

#[test]
fn defaults_match_phase_a_contract() {
    let config = StrawmapPortConfig::default();
    assert_eq!(
        config.required_verification_gates,
        vec![
            VerificationGate::FormalProof,
            VerificationGate::PropertyTest
        ]
    );
    assert_eq!(config.min_benchmark_throughput, 10_000.0);
    assert_eq!(config.max_benchmark_latency_ms, 100.0);
    assert_eq!(config.max_parallel_ports, 5);
    assert!(config.require_all_domains);
    assert!(!config.auto_integrate_on_pass);
    assert!(config.validate().is_empty());
}

#[test]
fn validation_reports_bad_inputs() {
    let mut config = StrawmapPortConfig::default();
    config.required_verification_gates = vec![
        VerificationGate::FormalProof,
        VerificationGate::FormalProof,
        VerificationGate::NotRequired,
    ];
    config.min_benchmark_throughput = 0.0;
    config.max_benchmark_latency_ms = -1.0;
    config.max_parallel_ports = 0;
    config.phase_a_deadline_unix = Some(0);

    let errors = config.validate();
    assert!(errors
        .iter()
        .any(|e| e.field == "required_verification_gates"));
    assert!(errors.iter().any(|e| e.field == "min_benchmark_throughput"));
    assert!(errors.iter().any(|e| e.field == "max_benchmark_latency_ms"));
    assert!(errors.iter().any(|e| e.field == "max_parallel_ports"));
    assert!(errors.iter().any(|e| e.field == "phase_a_deadline_unix"));
}

#[test]
fn stats_include_counts_progress_and_commitment() {
    let items = sample_items();
    let tasks = sample_tasks();
    let stats = compute_stats(&items, &tasks);
    let stats_repeat = compute_stats(&items, &tasks);

    assert_eq!(stats.total_items, 3);
    assert_eq!(stats.verified_count, 2);
    assert_eq!(stats.benchmarked_count, 1);
    assert_eq!(stats.integrated_count, 0);
    assert!((stats.avg_effort_days - 15.0).abs() < f64::EPSILON);
    assert!((stats.completion_pct - (2.0 / 3.0 * 100.0)).abs() < 1e-9);
    assert!(stats
        .items_by_domain
        .iter()
        .any(|(d, c)| *d == StrawmapDomain::Networking && *c == 0));
    assert!(stats
        .items_by_phase
        .iter()
        .any(|(p, c)| *p == PortingPhase::Verified && *c == 1));
    assert_eq!(stats.commitment, stats_repeat.commitment);

    let mut changed_tasks = tasks.clone();
    changed_tasks[0].benchmark_result = BenchmarkResult::Failed;
    let changed = compute_stats(&items, &changed_tasks).commitment;
    assert_ne!(stats.commitment, changed);
}

#[test]
fn phase_a_completion_requires_verified_floor() {
    let items = sample_items();
    assert!(!is_phase_a_complete(&items));

    let upgraded: Vec<StrawmapItem> = items
        .into_iter()
        .map(|mut item| {
            item.phase = PortingPhase::Verified;
            item
        })
        .collect();
    assert!(is_phase_a_complete(&upgraded));
}
