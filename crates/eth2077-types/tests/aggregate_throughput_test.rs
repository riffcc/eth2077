use eth2077_types::aggregate_throughput::{
    compare_accounting_methods, compute_aggregate_stats, compute_bottleneck,
    compute_throughput_commitment, default_aggregate_throughput_config, generate_caveats,
    validate_aggregate_config, AccountingMethod, AggregateThroughputConfig, LaneBenchmark,
    ThroughputLane, ThroughputValidationError,
};

fn benchmark(
    lane: ThroughputLane,
    measured_tps: f64,
    confidence: f64,
    reproducible: bool,
    conditions: &str,
) -> LaneBenchmark {
    LaneBenchmark {
        lane,
        measured_tps,
        conditions: conditions.to_string(),
        confidence,
        reproducible,
    }
}

fn sample_benchmarks() -> Vec<LaneBenchmark> {
    vec![
        benchmark(
            ThroughputLane::L1Execution,
            120.0,
            0.90,
            true,
            "l1 execution",
        ),
        benchmark(
            ThroughputLane::L1DataAvailability,
            80.0,
            0.90,
            true,
            "l1 da",
        ),
        benchmark(
            ThroughputLane::L1Consensus,
            100.0,
            0.95,
            true,
            "l1 consensus",
        ),
        benchmark(ThroughputLane::L2Rollup, 1_000.0, 0.90, true, "rollup"),
        benchmark(ThroughputLane::L2Validium, 900.0, 0.85, true, "validium"),
        benchmark(
            ThroughputLane::L2Optimistic,
            700.0,
            0.80,
            true,
            "optimistic",
        ),
    ]
}

#[test]
fn default_config_is_valid() {
    let config = default_aggregate_throughput_config();
    assert_eq!(validate_aggregate_config(&config), Ok(()));
}

#[test]
fn validation_rejects_empty_lanes() {
    let mut config = default_aggregate_throughput_config();
    config.lanes.clear();

    let errors = validate_aggregate_config(&config).unwrap_err();
    assert!(errors.contains(&ThroughputValidationError::EmptyLanes));
}

#[test]
fn validation_rejects_duplicates_bad_ratio_and_bad_confidence() {
    let config = AggregateThroughputConfig {
        method: AccountingMethod::PeakTPS,
        lanes: vec![ThroughputLane::L1Execution, ThroughputLane::L1Execution],
        l2_compression_ratio: 0.0,
        include_caveats: true,
        min_confidence: 1.1,
    };

    let errors = validate_aggregate_config(&config).unwrap_err();
    assert!(errors.contains(&ThroughputValidationError::DuplicateLane));
    assert!(errors.contains(&ThroughputValidationError::CompressionRatioInvalid { value: 0.0 }));
    assert!(errors.contains(&ThroughputValidationError::ConfidenceTooLow { value: 1.1 }));
}

#[test]
fn validation_rejects_cross_lane_only_configuration() {
    let config = AggregateThroughputConfig {
        method: AccountingMethod::PeakTPS,
        lanes: vec![ThroughputLane::CrossLane],
        l2_compression_ratio: 2.0,
        include_caveats: true,
        min_confidence: 0.5,
    };

    let errors = validate_aggregate_config(&config).unwrap_err();
    assert!(errors.contains(&ThroughputValidationError::NoBenchmarkData));
}

#[test]
fn bottleneck_finds_lowest_lane() {
    let benchmarks = sample_benchmarks();
    let (lane, tps) = compute_bottleneck(&benchmarks);
    assert_eq!(lane, "L1DataAvailability");
    assert!((tps - 80.0).abs() < 1e-9);
}

#[test]
fn bottleneck_bound_method_combines_l1_and_l2() {
    let benchmarks = sample_benchmarks();
    let config = AggregateThroughputConfig {
        method: AccountingMethod::BottleneckBound,
        lanes: vec![
            ThroughputLane::L1Execution,
            ThroughputLane::L1DataAvailability,
            ThroughputLane::L1Consensus,
            ThroughputLane::L2Rollup,
            ThroughputLane::L2Validium,
            ThroughputLane::L2Optimistic,
        ],
        l2_compression_ratio: 2.0,
        include_caveats: false,
        min_confidence: 0.5,
    };

    let stats = compute_aggregate_stats(&benchmarks, &config);
    assert!((stats.l1_tps - 80.0).abs() < 1e-9);
    assert!((stats.l2_tps - 1_400.0).abs() < 1e-9);
    assert!((stats.aggregate_tps - 1_480.0).abs() < 1e-9);
    assert_eq!(stats.bottleneck_lane, "L1DataAvailability");
    assert_eq!(stats.accounting_method, "BottleneckBound");
    assert!(stats.caveats.is_empty());
    assert!(stats.confidence_score > 0.9);
}

#[test]
fn cross_lane_caps_aggregate_when_present() {
    let mut benchmarks = sample_benchmarks();
    benchmarks.push(benchmark(
        ThroughputLane::CrossLane,
        1_000.0,
        0.95,
        true,
        "bridge limiter",
    ));

    let config = AggregateThroughputConfig {
        method: AccountingMethod::PeakTPS,
        lanes: vec![
            ThroughputLane::L1Execution,
            ThroughputLane::L1DataAvailability,
            ThroughputLane::L1Consensus,
            ThroughputLane::L2Rollup,
            ThroughputLane::L2Validium,
            ThroughputLane::L2Optimistic,
            ThroughputLane::CrossLane,
        ],
        l2_compression_ratio: 2.0,
        include_caveats: false,
        min_confidence: 0.5,
    };

    let stats = compute_aggregate_stats(&benchmarks, &config);
    assert!((stats.l1_tps - 120.0).abs() < 1e-9);
    assert!((stats.l2_tps - 2_000.0).abs() < 1e-9);
    assert!((stats.aggregate_tps - 1_000.0).abs() < 1e-9);
    assert_eq!(stats.bottleneck_lane, "CrossLane");
}

#[test]
fn compare_methods_returns_all_accounting_variants() {
    let benchmarks = sample_benchmarks();
    let config = default_aggregate_throughput_config();
    let methods = compare_accounting_methods(&benchmarks, &config);

    assert_eq!(methods.len(), 5);
    assert!(methods.iter().any(|(name, _)| name == "PeakTPS"));
    assert!(methods.iter().any(|(name, _)| name == "SustainedTPS"));
    assert!(methods.iter().any(|(name, _)| name == "WeightedAverage"));
    assert!(methods.iter().any(|(name, _)| name == "BottleneckBound"));
    assert!(methods.iter().any(|(name, _)| name == "TheoreticalMax"));
}

#[test]
fn generate_caveats_reports_common_limitations() {
    let benchmarks = vec![
        benchmark(ThroughputLane::L1Execution, 100.0, 0.6, false, "single run"),
        benchmark(ThroughputLane::L1Execution, 260.0, 0.6, false, "burst mode"),
        benchmark(ThroughputLane::L2Validium, 2_000.0, 0.95, true, "validium"),
    ];

    let caveats = generate_caveats(&benchmarks);
    assert!(caveats
        .iter()
        .any(|entry| entry.contains("confidence below 0.70")));
    assert!(caveats
        .iter()
        .any(|entry| entry.contains("non-reproducible")));
    assert!(caveats
        .iter()
        .any(|entry| entry.contains("L1Execution measurements vary")));
    assert!(caveats
        .iter()
        .any(|entry| entry.contains("Missing L1DataAvailability")));
    assert!(caveats
        .iter()
        .any(|entry| entry.contains("Missing L1Consensus")));
    assert!(caveats
        .iter()
        .any(|entry| entry.contains("No L2Rollup benchmark")));
    assert!(caveats.iter().any(|entry| entry.contains("L2Validium")));
}

#[test]
fn no_matching_data_produces_zeroed_stats() {
    let benchmarks = sample_benchmarks();
    let config = AggregateThroughputConfig {
        method: AccountingMethod::WeightedAverage,
        lanes: vec![ThroughputLane::L1Execution, ThroughputLane::L2Rollup],
        l2_compression_ratio: 3.0,
        include_caveats: true,
        min_confidence: 0.99,
    };

    let stats = compute_aggregate_stats(&benchmarks, &config);
    assert_eq!(stats.aggregate_tps, 0.0);
    assert_eq!(stats.bottleneck_lane, "NoBenchmarkData");
    assert_eq!(stats.confidence_score, 0.0);
    assert!(stats
        .caveats
        .iter()
        .any(|entry| entry.contains("No benchmark data passed lane and confidence filters")));
}

#[test]
fn commitment_is_deterministic_order_invariant_and_sensitive() {
    let a = vec![
        benchmark(ThroughputLane::L1Execution, 100.0, 0.9, true, "a"),
        benchmark(ThroughputLane::L2Rollup, 500.0, 0.8, true, "b"),
        benchmark(ThroughputLane::L1Consensus, 90.0, 0.95, true, "c"),
    ];

    let b = vec![
        benchmark(ThroughputLane::L1Consensus, 90.0, 0.95, true, "c"),
        benchmark(ThroughputLane::L1Execution, 100.0, 0.9, true, "a"),
        benchmark(ThroughputLane::L2Rollup, 500.0, 0.8, true, "b"),
    ];

    let c = vec![
        benchmark(ThroughputLane::L1Execution, 100.0, 0.9, true, "a"),
        benchmark(ThroughputLane::L2Rollup, 501.0, 0.8, true, "b"),
        benchmark(ThroughputLane::L1Consensus, 90.0, 0.95, true, "c"),
    ];

    let hash_a = compute_throughput_commitment(&a);
    let hash_b = compute_throughput_commitment(&b);
    let hash_c = compute_throughput_commitment(&c);

    assert_eq!(hash_a, hash_b);
    assert_ne!(hash_a, hash_c);
}
