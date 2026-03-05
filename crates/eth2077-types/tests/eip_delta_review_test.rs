use eth2077_types::eip_delta_review::{
    classify_impact, compute_implementation_coverage, compute_review_commitment,
    compute_review_stats, default_review_config, prioritize_entries, validate_delta_review,
    ChangeCategory, DeltaEntry, DeltaReviewConfig, DeltaReviewError, ImpactLevel,
    ImplementationStatus,
};

fn entry(
    eip: u64,
    pr: u64,
    category: ChangeCategory,
    impact: ImpactLevel,
    status: ImplementationStatus,
    breaking: bool,
    description: &str,
    sections: &[&str],
) -> DeltaEntry {
    DeltaEntry {
        eip_number: eip,
        pr_number: pr,
        category,
        impact,
        description: description.to_string(),
        affected_sections: sections.iter().map(|section| section.to_string()).collect(),
        implementation_status: status,
        breaking,
    }
}

#[test]
fn default_review_config_has_expected_defaults() {
    let config = default_review_config();
    assert_eq!(
        config,
        DeltaReviewConfig {
            auto_classify_threshold: 0.8,
            require_test_vectors: true,
            max_entries_per_review: 100,
            track_implementation: true,
        }
    );
}

#[test]
fn validate_delta_review_collects_duplicate_missing_invalid_and_limit_errors() {
    let entries = vec![
        entry(
            0,
            10900,
            ChangeCategory::Normative,
            ImpactLevel::High,
            ImplementationStatus::NotStarted,
            true,
            "",
            &["consensus"],
        ),
        entry(
            0,
            10900,
            ChangeCategory::SecurityCritical,
            ImpactLevel::Critical,
            ImplementationStatus::InProgress,
            true,
            "changes transition edge case",
            &["execution"],
        ),
    ];
    let config = DeltaReviewConfig {
        auto_classify_threshold: 0.8,
        require_test_vectors: true,
        max_entries_per_review: 1,
        track_implementation: true,
    };

    let errors = validate_delta_review(&entries, &config).unwrap_err();
    assert!(errors.contains(&DeltaReviewError::TooManyEntries { count: 2, max: 1 }));
    assert!(errors.contains(&DeltaReviewError::MissingDescription { index: 0 }));
    assert!(errors.contains(&DeltaReviewError::DuplicateEntry { eip: 0, pr: 10900 }));
    assert!(
        errors
            .iter()
            .filter(|error| matches!(error, DeltaReviewError::InvalidEipNumber { eip: 0 }))
            .count()
            >= 1
    );
}

#[test]
fn validate_delta_review_accepts_valid_input() {
    let entries = vec![
        entry(
            7702,
            10900,
            ChangeCategory::Normative,
            ImpactLevel::High,
            ImplementationStatus::InProgress,
            true,
            "clarifies nonce handling on delegation",
            &["state transition", "execution"],
        ),
        entry(
            7702,
            10901,
            ChangeCategory::TestVector,
            ImpactLevel::Medium,
            ImplementationStatus::NeedsReview,
            false,
            "adds vectors for transaction replacement",
            &["test vectors"],
        ),
    ];
    let config = default_review_config();

    assert_eq!(validate_delta_review(&entries, &config), Ok(()));
}

#[test]
fn compute_review_stats_reports_expected_counts() {
    let entries = vec![
        entry(
            1,
            100,
            ChangeCategory::Normative,
            ImpactLevel::High,
            ImplementationStatus::Implemented,
            true,
            "normative update",
            &["execution"],
        ),
        entry(
            2,
            101,
            ChangeCategory::SecurityCritical,
            ImpactLevel::Critical,
            ImplementationStatus::InProgress,
            true,
            "security fix",
            &["consensus"],
        ),
        entry(
            3,
            102,
            ChangeCategory::Editorial,
            ImpactLevel::None,
            ImplementationStatus::NotStarted,
            false,
            "typo fix",
            &["wording"],
        ),
        entry(
            4,
            103,
            ChangeCategory::GasSchedule,
            ImpactLevel::Medium,
            ImplementationStatus::NotApplicable,
            false,
            "gas tuning",
            &["fees"],
        ),
    ];

    let stats = compute_review_stats(&entries);
    assert_eq!(stats.total_entries, 4);
    assert_eq!(stats.normative_count, 3);
    assert_eq!(stats.breaking_count, 2);
    assert_eq!(stats.security_critical_count, 1);
    assert!((stats.implementation_coverage - (2.0 / 3.0)).abs() < 1e-9);
    assert!((stats.avg_impact_score - 2.25).abs() < 1e-9);
    assert!(stats
        .category_distribution
        .contains(&(String::from("Normative"), 1)));
    assert!(stats
        .category_distribution
        .contains(&(String::from("SecurityCritical"), 1)));
    assert!(stats
        .impact_distribution
        .contains(&(String::from("Critical"), 1)));
    assert!(stats
        .impact_distribution
        .contains(&(String::from("None"), 1)));
}

#[test]
fn classify_impact_applies_heuristics() {
    let critical = classify_impact(
        ChangeCategory::SecurityCritical,
        false,
        &[String::from("consensus failure handling")],
    );
    assert_eq!(critical, ImpactLevel::Critical);

    let high = classify_impact(
        ChangeCategory::Normative,
        true,
        &[String::from("api behavior")],
    );
    assert_eq!(high, ImpactLevel::High);

    let none = classify_impact(ChangeCategory::Editorial, false, &[String::from("typos")]);
    assert_eq!(none, ImpactLevel::None);
}

#[test]
fn prioritize_entries_orders_by_impact_then_breaking_then_category() {
    let mut entries = vec![
        entry(
            10,
            1,
            ChangeCategory::Editorial,
            ImpactLevel::High,
            ImplementationStatus::NotStarted,
            false,
            "a",
            &[],
        ),
        entry(
            11,
            1,
            ChangeCategory::Normative,
            ImpactLevel::Critical,
            ImplementationStatus::NotStarted,
            false,
            "b",
            &[],
        ),
        entry(
            12,
            1,
            ChangeCategory::SecurityCritical,
            ImpactLevel::Critical,
            ImplementationStatus::NotStarted,
            true,
            "c",
            &[],
        ),
        entry(
            13,
            1,
            ChangeCategory::GasSchedule,
            ImpactLevel::Critical,
            ImplementationStatus::NotStarted,
            false,
            "d",
            &[],
        ),
    ];

    prioritize_entries(&mut entries);

    assert_eq!(entries[0].eip_number, 12);
    assert_eq!(entries[1].eip_number, 11);
    assert_eq!(entries[2].eip_number, 13);
    assert_eq!(entries[3].eip_number, 10);
}

#[test]
fn compute_implementation_coverage_uses_only_normative_categories() {
    let entries = vec![
        entry(
            1,
            1,
            ChangeCategory::Normative,
            ImpactLevel::High,
            ImplementationStatus::Implemented,
            false,
            "done",
            &[],
        ),
        entry(
            2,
            1,
            ChangeCategory::SecurityCritical,
            ImpactLevel::Critical,
            ImplementationStatus::NotStarted,
            true,
            "pending",
            &[],
        ),
        entry(
            3,
            1,
            ChangeCategory::Informational,
            ImpactLevel::Low,
            ImplementationStatus::NotStarted,
            false,
            "ignored for coverage",
            &[],
        ),
    ];

    let coverage = compute_implementation_coverage(&entries);
    assert!((coverage - 0.5).abs() < 1e-9);
}

#[test]
fn compute_review_commitment_is_order_independent() {
    let a = entry(
        1,
        10,
        ChangeCategory::Normative,
        ImpactLevel::Medium,
        ImplementationStatus::InProgress,
        true,
        "update",
        &["execution"],
    );
    let b = entry(
        2,
        11,
        ChangeCategory::TestVector,
        ImpactLevel::Low,
        ImplementationStatus::NeedsReview,
        false,
        "vectors",
        &["tests"],
    );

    let first = compute_review_commitment(&[a.clone(), b.clone()]);
    let second = compute_review_commitment(&[b, a]);
    assert_eq!(first, second);
}

#[test]
fn compute_review_commitment_changes_when_data_changes() {
    let base = entry(
        1,
        10,
        ChangeCategory::Normative,
        ImpactLevel::Medium,
        ImplementationStatus::InProgress,
        true,
        "update",
        &["execution"],
    );
    let mut changed = base.clone();
    changed.description = String::from("update with extra requirement");

    let first = compute_review_commitment(&[base]);
    let second = compute_review_commitment(&[changed]);
    assert_ne!(first, second);
}
