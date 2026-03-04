use eth2077_types::eip_portability::{
    compute_portability_commitment, compute_portability_stats, default_portability_config,
    estimate_total_effort, prioritize_eips, score_portability, validate_assessments,
    ClientArchitecture, PortabilityAssessment, PortabilityError, PortabilityRisk,
};

fn assessment(
    eip_number: u64,
    complexity_score: f64,
    dependency_count: usize,
    risks: Vec<PortabilityRisk>,
    estimated_dev_days: f64,
    requires_consensus_changes: bool,
    requires_networking_changes: bool,
    test_coverage_available: f64,
) -> PortabilityAssessment {
    PortabilityAssessment {
        eip_number,
        target_client: ClientArchitecture::Eth2077Citadel,
        complexity_score,
        dependency_count,
        risks,
        estimated_dev_days,
        requires_consensus_changes,
        requires_networking_changes,
        test_coverage_available,
    }
}

#[test]
fn default_config_matches_spec() {
    let config = default_portability_config();
    assert_eq!(config.target_client, ClientArchitecture::Eth2077Citadel);
    assert!((config.max_acceptable_complexity - 0.7).abs() < 1e-12);
    assert!((config.min_test_coverage - 0.6).abs() < 1e-12);
    assert!((config.weight_complexity - (1.0 / 3.0)).abs() < 1e-12);
    assert!((config.weight_risk - (1.0 / 3.0)).abs() < 1e-12);
    assert!((config.weight_dependencies - (1.0 / 3.0)).abs() < 1e-12);
}

#[test]
fn validate_accepts_valid_assessments() {
    let config = default_portability_config();
    let assessments = vec![
        assessment(
            1559,
            0.45,
            3,
            vec![PortabilityRisk::PerformanceSensitive],
            6.0,
            false,
            true,
            0.8,
        ),
        assessment(
            4844,
            0.65,
            6,
            vec![PortabilityRisk::TightCoupling],
            12.0,
            true,
            true,
            0.7,
        ),
    ];

    assert_eq!(validate_assessments(&assessments, &config), Ok(()));
}

#[test]
fn validate_rejects_empty_assessments() {
    let config = default_portability_config();
    let errors = validate_assessments(&[], &config).unwrap_err();
    assert_eq!(errors, vec![PortabilityError::EmptyAssessments]);
}

#[test]
fn validate_reports_all_error_classes() {
    let config = default_portability_config();
    let assessments = vec![
        assessment(7000, 1.2, 1, vec![], 2.0, false, false, 0.4),
        assessment(
            7000,
            0.5,
            1,
            vec![PortabilityRisk::None],
            2.0,
            false,
            false,
            0.3,
        ),
    ];

    let errors = validate_assessments(&assessments, &config).unwrap_err();

    assert!(errors.contains(&PortabilityError::InvalidComplexity {
        eip: 7000,
        score: "1.200".to_owned(),
    }));
    assert!(errors.contains(&PortabilityError::DuplicateEip { eip: 7000 }));
    assert!(errors.contains(&PortabilityError::MissingRiskAnalysis { eip: 7000 }));
    assert!(
        errors.contains(&PortabilityError::InsufficientTestCoverage {
            eip: 7000,
            coverage: "0.400".to_owned(),
            required: "0.600".to_owned(),
        })
    );
}

#[test]
fn score_prefers_lower_complexity_and_risk() {
    let config = default_portability_config();
    let easy = assessment(
        1,
        0.2,
        1,
        vec![PortabilityRisk::None],
        4.0,
        false,
        false,
        0.95,
    );
    let hard = assessment(
        2,
        0.85,
        9,
        vec![PortabilityRisk::ConsensusDeviation],
        12.0,
        true,
        true,
        0.95,
    );

    assert!(score_portability(&easy, &config) > score_portability(&hard, &config));
}

#[test]
fn prioritize_sorts_easiest_first() {
    let config = default_portability_config();
    let mut assessments = vec![
        assessment(
            2,
            0.8,
            8,
            vec![PortabilityRisk::ConsensusDeviation],
            10.0,
            true,
            true,
            0.8,
        ),
        assessment(
            1,
            0.3,
            2,
            vec![PortabilityRisk::None],
            4.0,
            false,
            false,
            0.9,
        ),
        assessment(
            3,
            0.55,
            4,
            vec![PortabilityRisk::PerformanceSensitive],
            7.0,
            false,
            true,
            0.8,
        ),
    ];

    prioritize_eips(&mut assessments, &config);
    let ordered: Vec<u64> = assessments.iter().map(|a| a.eip_number).collect();
    assert_eq!(ordered, vec![1, 3, 2]);
}

#[test]
fn effort_estimate_applies_risk_multiplier() {
    let low = assessment(
        1,
        0.2,
        0,
        vec![PortabilityRisk::None],
        10.0,
        false,
        false,
        0.9,
    );
    let high = assessment(
        2,
        0.8,
        4,
        vec![
            PortabilityRisk::ConsensusDeviation,
            PortabilityRisk::UndefinedBehavior,
        ],
        10.0,
        true,
        true,
        0.9,
    );

    let low_effort = estimate_total_effort(&[low]);
    let high_effort = estimate_total_effort(&[high]);

    assert!((low_effort - 10.0).abs() < 1e-12);
    assert!(high_effort > low_effort);
}

#[test]
fn stats_computation_tracks_distribution_and_counts() {
    let assessments = vec![
        assessment(
            1111,
            0.3,
            2,
            vec![PortabilityRisk::PerformanceSensitive],
            5.0,
            false,
            false,
            0.8,
        ),
        assessment(
            2222,
            0.7,
            6,
            vec![PortabilityRisk::ConsensusDeviation],
            9.0,
            true,
            true,
            0.7,
        ),
    ];

    let stats = compute_portability_stats(&assessments);
    assert_eq!(stats.total_eips_assessed, 2);
    assert!((stats.avg_complexity - 0.5).abs() < 1e-12);
    assert!((stats.total_dev_days - 14.0).abs() < 1e-12);
    assert_eq!(stats.high_risk_count, 1);
    assert_eq!(stats.consensus_change_count, 1);
    assert_eq!(
        stats.risk_distribution,
        vec![
            ("ConsensusDeviation".to_owned(), 1),
            ("PerformanceSensitive".to_owned(), 1),
        ]
    );
    assert!((0.0..=1.0).contains(&stats.client_readiness_score));
}

#[test]
fn stats_for_empty_input_are_zeroed() {
    let stats = compute_portability_stats(&[]);
    assert_eq!(stats.total_eips_assessed, 0);
    assert_eq!(stats.avg_complexity, 0.0);
    assert_eq!(stats.total_dev_days, 0.0);
    assert_eq!(stats.high_risk_count, 0);
    assert_eq!(stats.consensus_change_count, 0);
    assert_eq!(stats.risk_distribution, Vec::<(String, usize)>::new());
    assert_eq!(stats.client_readiness_score, 0.0);
}

#[test]
fn commitment_is_deterministic_and_order_independent() {
    let first = assessment(
        1000,
        0.4,
        2,
        vec![
            PortabilityRisk::TightCoupling,
            PortabilityRisk::PerformanceSensitive,
        ],
        8.0,
        false,
        true,
        0.8,
    );
    let second = assessment(
        2000,
        0.2,
        1,
        vec![PortabilityRisk::None],
        4.0,
        false,
        false,
        0.9,
    );

    let a = compute_portability_commitment(&[first.clone(), second.clone()]);
    let b = compute_portability_commitment(&[second.clone(), first.clone()]);
    let c = compute_portability_commitment(&[first, second]);

    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn commitment_changes_when_assessment_changes() {
    let baseline = assessment(
        9999,
        0.4,
        3,
        vec![PortabilityRisk::TightCoupling],
        7.0,
        false,
        false,
        0.8,
    );
    let mut changed = baseline.clone();
    changed.estimated_dev_days = 8.0;

    let baseline_commitment = compute_portability_commitment(&[baseline]);
    let changed_commitment = compute_portability_commitment(&[changed]);

    assert_ne!(baseline_commitment, changed_commitment);
}
