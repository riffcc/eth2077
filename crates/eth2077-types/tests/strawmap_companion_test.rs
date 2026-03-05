use eth2077_types::strawmap_companion::{
    compute_stats, CompanionMirror, CompanionSection, DocumentSection, ExtractionStatus, GateType,
    StrawmapCompanionConfig, UpgradeComplexity, UpgradeItem,
};
fn bytes32(seed: u8) -> [u8; 32] {
    [seed; 32]
}
fn mk_section(section: DocumentSection, title: &str, seed: u8) -> CompanionSection {
    CompanionSection {
        section,
        title: title.to_string(),
        content_hash: bytes32(seed),
        item_count: 2,
        last_synced_unix: 1_700_000_000,
        source_url: "https://example.com/strawmap-companion".to_string(),
    }
}
fn mk_mirror() -> CompanionMirror {
    CompanionMirror {
        id: "mirror-1".to_string(),
        source_url: "https://example.com/strawmap-companion".to_string(),
        mirrored_at_unix: 1_700_000_000,
        sections: vec![
            mk_section(DocumentSection::ExecutionUpgrades, "Execution", 0x11),
            mk_section(DocumentSection::Overview, "Overview", 0x22),
        ],
        total_items_extracted: 3,
        document_hash: bytes32(0xAB),
        version: 2,
    }
}
fn mk_item(
    id: &str,
    status: ExtractionStatus,
    complexity: UpgradeComplexity,
    gates_required: Vec<GateType>,
    gates_passed: Vec<GateType>,
) -> UpgradeItem {
    UpgradeItem {
        id: id.to_string(),
        section: DocumentSection::ExecutionUpgrades,
        title: format!("Item {id}"),
        description: format!("Description for {id}"),
        complexity,
        status,
        linked_eips: vec![7000, 7001, 7000],
        gates_required,
        gates_passed,
        assigned_to: Some("eth2077-dev".to_string()),
        eth2077_ticket_id: Some(format!("ETH2077-{id}")),
    }
}
#[test]
fn defaults_match_requested_policy() {
    let cfg = StrawmapCompanionConfig::default();
    assert_eq!(cfg.sync_interval_hours, 24);
    assert!(cfg.auto_extract);
    assert_eq!(cfg.min_review_count, 2);
    assert_eq!(
        cfg.complexity_threshold_for_proof,
        UpgradeComplexity::Complex
    );
    assert_eq!(cfg.max_open_items, 50);
    assert_eq!(
        cfg.required_gates,
        vec![
            GateType::ProofGate,
            GateType::BenchGate,
            GateType::ReviewGate
        ]
    );
}
#[test]
fn validation_reports_multiple_issues() {
    let cfg = StrawmapCompanionConfig {
        source_url: "ftp://bad-url".to_string(),
        sync_interval_hours: 0,
        auto_extract: true,
        required_gates: vec![GateType::ReviewGate, GateType::ReviewGate],
        min_review_count: 1,
        complexity_threshold_for_proof: UpgradeComplexity::Trivial,
        max_open_items: 0,
    };
    let errors = cfg.validate();
    for field in [
        "source_url",
        "sync_interval_hours",
        "required_gates",
        "min_review_count",
        "max_open_items",
    ] {
        assert!(errors.iter().any(|error| error.field == field));
    }
}
#[test]
fn stats_compute_expected_counts_rates_and_commitment() {
    let mirror = mk_mirror();
    let items = vec![
        mk_item(
            "A",
            ExtractionStatus::Assigned,
            UpgradeComplexity::Complex,
            vec![
                GateType::ProofGate,
                GateType::BenchGate,
                GateType::ReviewGate,
            ],
            vec![GateType::ProofGate, GateType::ReviewGate],
        ),
        mk_item(
            "B",
            ExtractionStatus::Completed,
            UpgradeComplexity::Simple,
            vec![GateType::ReviewGate],
            vec![GateType::ReviewGate, GateType::IntegrationGate],
        ),
        mk_item(
            "C",
            ExtractionStatus::Completed,
            UpgradeComplexity::Epic,
            vec![],
            vec![],
        ),
    ];
    let stats = compute_stats(&mirror, &items);
    assert_eq!(stats.total_sections, 2);
    assert_eq!(stats.total_items, 3);
    assert_eq!(
        stats
            .items_by_status
            .iter()
            .find(|(status, _)| *status == ExtractionStatus::Completed)
            .map(|(_, count)| *count),
        Some(2)
    );
    assert_eq!(
        stats
            .items_by_complexity
            .iter()
            .find(|(complexity, _)| *complexity == UpgradeComplexity::Complex)
            .map(|(_, count)| *count),
        Some(1)
    );
    assert!((stats.gates_completion_rate - 0.75).abs() < 1e-12);
    assert!((stats.avg_gates_per_item - (4.0 / 3.0)).abs() < 1e-12);
    assert!((stats.completion_pct - (200.0 / 3.0)).abs() < 1e-9);
    assert_ne!(stats.commitment, [0u8; 32]);
    let mut reordered_items = items.clone();
    reordered_items.reverse();
    let mut reordered_mirror = mirror.clone();
    reordered_mirror.sections.reverse();
    let reordered_stats = compute_stats(&reordered_mirror, &reordered_items);
    assert_eq!(stats.commitment, reordered_stats.commitment);
}
#[test]
fn needs_sync_respects_interval_and_source_drift() {
    let mut cfg = StrawmapCompanionConfig::default();
    cfg.source_url = "https://example.com/strawmap-companion".to_string();
    let mirror = mk_mirror();
    assert!(!cfg.needs_sync(&mirror, mirror.mirrored_at_unix + (23 * 3600)));
    assert!(cfg.needs_sync(&mirror, mirror.mirrored_at_unix + (24 * 3600)));
    assert!(!cfg.needs_sync(&mirror, mirror.mirrored_at_unix - 30));
    let mut changed_source = cfg.clone();
    changed_source.source_url = "https://example.com/strawmap-companion-v2".to_string();
    assert!(changed_source.needs_sync(&mirror, mirror.mirrored_at_unix + 60));
}
