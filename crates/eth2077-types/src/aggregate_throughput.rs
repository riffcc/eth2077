use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

const L1_LANES: [ThroughputLane; 3] = [
    ThroughputLane::L1Execution,
    ThroughputLane::L1DataAvailability,
    ThroughputLane::L1Consensus,
];

const L2_LANES: [ThroughputLane; 3] = [
    ThroughputLane::L2Rollup,
    ThroughputLane::L2Validium,
    ThroughputLane::L2Optimistic,
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ThroughputLane {
    L1Execution,
    L1DataAvailability,
    L1Consensus,
    L2Rollup,
    L2Validium,
    L2Optimistic,
    CrossLane,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AccountingMethod {
    PeakTPS,
    SustainedTPS,
    WeightedAverage,
    BottleneckBound,
    TheoreticalMax,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LaneBenchmark {
    pub lane: ThroughputLane,
    pub measured_tps: f64,
    pub conditions: String,
    pub confidence: f64,
    pub reproducible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregateThroughputConfig {
    pub method: AccountingMethod,
    pub lanes: Vec<ThroughputLane>,
    pub l2_compression_ratio: f64,
    pub include_caveats: bool,
    pub min_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThroughputValidationError {
    EmptyLanes,
    DuplicateLane,
    CompressionRatioInvalid { value: f64 },
    ConfidenceTooLow { value: f64 },
    NoBenchmarkData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregateThroughputStats {
    pub l1_tps: f64,
    pub l2_tps: f64,
    pub aggregate_tps: f64,
    pub bottleneck_lane: String,
    pub accounting_method: String,
    pub caveats: Vec<String>,
    pub confidence_score: f64,
}

#[derive(Debug, Clone, Copy)]
struct LaneStats {
    mean: f64,
    min: f64,
    max: f64,
}

pub fn default_aggregate_throughput_config() -> AggregateThroughputConfig {
    AggregateThroughputConfig {
        method: AccountingMethod::SustainedTPS,
        lanes: vec![
            ThroughputLane::L1Execution,
            ThroughputLane::L1DataAvailability,
            ThroughputLane::L1Consensus,
            ThroughputLane::L2Rollup,
            ThroughputLane::L2Optimistic,
        ],
        l2_compression_ratio: 8.0,
        include_caveats: true,
        min_confidence: 0.70,
    }
}

pub fn validate_aggregate_config(
    config: &AggregateThroughputConfig,
) -> Result<(), Vec<ThroughputValidationError>> {
    let mut errors = Vec::new();

    if config.lanes.is_empty() {
        errors.push(ThroughputValidationError::EmptyLanes);
    }

    let mut seen = HashSet::new();
    for lane in &config.lanes {
        if !seen.insert(*lane) {
            errors.push(ThroughputValidationError::DuplicateLane);
        }
    }

    if !config.l2_compression_ratio.is_finite() || config.l2_compression_ratio <= 0.0 {
        errors.push(ThroughputValidationError::CompressionRatioInvalid {
            value: config.l2_compression_ratio,
        });
    }

    if !config.min_confidence.is_finite() || !(0.0..=1.0).contains(&config.min_confidence) {
        errors.push(ThroughputValidationError::ConfidenceTooLow {
            value: config.min_confidence,
        });
    }

    let has_l1_or_l2 = config
        .lanes
        .iter()
        .any(|lane| is_l1_lane(*lane) || is_l2_lane(*lane));
    if !has_l1_or_l2 {
        errors.push(ThroughputValidationError::NoBenchmarkData);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_aggregate_stats(
    benchmarks: &[LaneBenchmark],
    config: &AggregateThroughputConfig,
) -> AggregateThroughputStats {
    let filtered: Vec<LaneBenchmark> = filtered_benchmarks(benchmarks, config);
    let accounting_method = format!("{:?}", config.method);

    if filtered.is_empty() {
        let caveats = if config.include_caveats {
            vec![
                "No benchmark data passed lane and confidence filters; aggregate TPS set to 0."
                    .to_string(),
            ]
        } else {
            Vec::new()
        };

        return AggregateThroughputStats {
            l1_tps: 0.0,
            l2_tps: 0.0,
            aggregate_tps: 0.0,
            bottleneck_lane: "NoBenchmarkData".to_string(),
            accounting_method,
            caveats,
            confidence_score: 0.0,
        };
    }

    let l1_stats = family_stats(&filtered, &L1_LANES);
    let l2_stats = family_stats(&filtered, &L2_LANES);
    let compression_ratio = sanitize_ratio(config.l2_compression_ratio);

    let (l1_tps, l2_tps) = match config.method {
        AccountingMethod::PeakTPS => (
            family_metric_max(&l1_stats),
            family_metric_max(&l2_stats) * compression_ratio,
        ),
        AccountingMethod::SustainedTPS => (
            family_metric_min_mean(&l1_stats),
            family_metric_avg_mean(&l2_stats) * compression_ratio * 0.90,
        ),
        AccountingMethod::WeightedAverage => (
            weighted_family_tps(&filtered, is_l1_lane),
            weighted_family_tps(&filtered, is_l2_lane) * compression_ratio,
        ),
        AccountingMethod::BottleneckBound => (
            family_metric_min(&l1_stats),
            family_metric_min(&l2_stats) * compression_ratio,
        ),
        AccountingMethod::TheoreticalMax => (
            family_metric_max(&l1_stats),
            family_metric_max(&l2_stats) * compression_ratio * 1.10,
        ),
    };

    let mut aggregate_tps = l1_tps + l2_tps;
    let mut bottleneck_lane = compute_bottleneck(&filtered).0;
    let cross_cap = cross_lane_cap(&filtered);
    if let Some(cap) = cross_cap {
        if aggregate_tps > cap {
            aggregate_tps = cap;
            bottleneck_lane = format!("{:?}", ThroughputLane::CrossLane);
        }
    }

    AggregateThroughputStats {
        l1_tps,
        l2_tps,
        aggregate_tps,
        bottleneck_lane,
        accounting_method,
        caveats: if config.include_caveats {
            generate_caveats(&filtered)
        } else {
            Vec::new()
        },
        confidence_score: confidence_score(&filtered),
    }
}

pub fn compare_accounting_methods(
    benchmarks: &[LaneBenchmark],
    config: &AggregateThroughputConfig,
) -> Vec<(String, AggregateThroughputStats)> {
    all_accounting_methods()
        .into_iter()
        .map(|method| {
            let mut variant = config.clone();
            variant.method = method;
            (
                format!("{method:?}"),
                compute_aggregate_stats(benchmarks, &variant),
            )
        })
        .collect()
}

pub fn generate_caveats(benchmarks: &[LaneBenchmark]) -> Vec<String> {
    if benchmarks.is_empty() {
        return vec![
            "No benchmark data provided; aggregate throughput claims are not reproducible."
                .to_string(),
        ];
    }

    let mut caveats = Vec::new();
    let low_confidence_count = benchmarks
        .iter()
        .filter(|benchmark| benchmark.confidence.is_finite() && benchmark.confidence < 0.70)
        .count();
    if low_confidence_count > 0 {
        caveats.push(format!(
            "{low_confidence_count} benchmark(s) have confidence below 0.70."
        ));
    }

    let non_reproducible_count = benchmarks
        .iter()
        .filter(|benchmark| !benchmark.reproducible)
        .count();
    if non_reproducible_count > 0 {
        caveats.push(format!(
            "{non_reproducible_count} benchmark(s) are marked non-reproducible."
        ));
    }

    for lane in all_lanes() {
        let mut min_tps = f64::INFINITY;
        let mut max_tps: f64 = 0.0;
        let mut seen = false;

        for benchmark in benchmarks {
            if benchmark.lane != lane
                || !benchmark.measured_tps.is_finite()
                || benchmark.measured_tps < 0.0
            {
                continue;
            }
            seen = true;
            min_tps = min_tps.min(benchmark.measured_tps);
            max_tps = max_tps.max(benchmark.measured_tps);
        }

        if seen && min_tps > 0.0 && (max_tps / min_tps) >= 2.0 {
            caveats.push(format!(
                "{lane:?} measurements vary by >=2x ({min_tps:.2} to {max_tps:.2} TPS)."
            ));
        }
    }

    for required_lane in L1_LANES {
        if !benchmarks
            .iter()
            .any(|benchmark| benchmark.lane == required_lane)
        {
            caveats.push(format!(
                "Missing {required_lane:?} benchmark; aggregate L1 accounting is incomplete."
            ));
        }
    }

    if !benchmarks
        .iter()
        .any(|benchmark| benchmark.lane == ThroughputLane::L2Rollup)
    {
        caveats.push("No L2Rollup benchmark supplied; L2 scaling may be understated.".to_string());
    }

    if benchmarks
        .iter()
        .any(|benchmark| benchmark.lane == ThroughputLane::L2Validium)
    {
        caveats.push(
            "L2Validium figures may rely on off-chain data availability assumptions.".to_string(),
        );
    }

    if benchmarks
        .iter()
        .any(|benchmark| benchmark.lane == ThroughputLane::L2Optimistic)
    {
        caveats.push(
            "L2Optimistic throughput may not include dispute-window finality delays.".to_string(),
        );
    }

    caveats
}

pub fn compute_bottleneck(benchmarks: &[LaneBenchmark]) -> (String, f64) {
    let mut best_lane: Option<ThroughputLane> = None;
    let mut best_tps = f64::INFINITY;

    for benchmark in benchmarks {
        if !benchmark.measured_tps.is_finite() || benchmark.measured_tps < 0.0 {
            continue;
        }

        let tps = benchmark.measured_tps;
        if tps < best_tps {
            best_tps = tps;
            best_lane = Some(benchmark.lane);
        } else if (tps - best_tps).abs() <= f64::EPSILON {
            let current = lane_discriminant(benchmark.lane);
            let existing = best_lane.map(lane_discriminant).unwrap_or(u8::MAX);
            if current < existing {
                best_lane = Some(benchmark.lane);
            }
        }
    }

    match best_lane {
        Some(lane) => (format!("{lane:?}"), best_tps),
        None => ("NoBenchmarkData".to_string(), 0.0),
    }
}

pub fn compute_throughput_commitment(benchmarks: &[LaneBenchmark]) -> [u8; 32] {
    let mut benchmark_hashes: Vec<[u8; 32]> = benchmarks.iter().map(benchmark_digest).collect();
    benchmark_hashes.sort_unstable();

    let mut hasher = Sha256::new();
    hasher.update(b"eth2077-aggregate-throughput-v1");
    hasher.update((benchmark_hashes.len() as u64).to_be_bytes());

    for benchmark_hash in benchmark_hashes {
        hasher.update(benchmark_hash);
    }

    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn filtered_benchmarks(
    benchmarks: &[LaneBenchmark],
    config: &AggregateThroughputConfig,
) -> Vec<LaneBenchmark> {
    let min_confidence = sanitize_confidence_floor(config.min_confidence);

    benchmarks
        .iter()
        .filter(|benchmark| {
            config.lanes.contains(&benchmark.lane)
                && benchmark.measured_tps.is_finite()
                && benchmark.measured_tps >= 0.0
                && benchmark.confidence.is_finite()
                && benchmark.confidence >= min_confidence
        })
        .cloned()
        .collect()
}

fn family_stats(benchmarks: &[LaneBenchmark], lanes: &[ThroughputLane]) -> Vec<LaneStats> {
    lanes
        .iter()
        .filter_map(|lane| lane_stats(benchmarks, *lane))
        .collect()
}

fn lane_stats(benchmarks: &[LaneBenchmark], lane: ThroughputLane) -> Option<LaneStats> {
    let lane_benchmarks: Vec<&LaneBenchmark> = benchmarks
        .iter()
        .filter(|benchmark| benchmark.lane == lane)
        .collect();

    if lane_benchmarks.is_empty() {
        return None;
    }

    let mut sum: f64 = 0.0;
    let mut min: f64 = f64::INFINITY;
    let mut max: f64 = 0.0;

    for benchmark in lane_benchmarks.iter().copied() {
        let tps = benchmark.measured_tps.max(0.0);
        sum += tps;
        min = min.min(tps);
        max = max.max(tps);
    }

    let mean = sum / lane_benchmarks.len() as f64;

    Some(LaneStats { mean, min, max })
}

fn family_metric_max(stats: &[LaneStats]) -> f64 {
    stats
        .iter()
        .map(|lane| lane.max)
        .max_by(f64::total_cmp)
        .unwrap_or(0.0)
}

fn family_metric_min(stats: &[LaneStats]) -> f64 {
    stats
        .iter()
        .map(|lane| lane.min)
        .min_by(f64::total_cmp)
        .unwrap_or(0.0)
}

fn family_metric_min_mean(stats: &[LaneStats]) -> f64 {
    stats
        .iter()
        .map(|lane| lane.mean)
        .min_by(f64::total_cmp)
        .unwrap_or(0.0)
}

fn family_metric_avg_mean(stats: &[LaneStats]) -> f64 {
    if stats.is_empty() {
        return 0.0;
    }

    stats.iter().map(|lane| lane.mean).sum::<f64>() / stats.len() as f64
}

fn weighted_family_tps(
    benchmarks: &[LaneBenchmark],
    lane_filter: fn(ThroughputLane) -> bool,
) -> f64 {
    let mut weighted = 0.0;
    let mut confidence_sum = 0.0;
    let mut sum = 0.0;
    let mut count = 0usize;

    for benchmark in benchmarks {
        if !lane_filter(benchmark.lane) {
            continue;
        }

        let tps = benchmark.measured_tps.max(0.0);
        let confidence = benchmark.confidence.clamp(0.0, 1.0);
        weighted += tps * confidence;
        confidence_sum += confidence;
        sum += tps;
        count += 1;
    }

    if confidence_sum > 0.0 {
        weighted / confidence_sum
    } else if count > 0 {
        sum / count as f64
    } else {
        0.0
    }
}

fn cross_lane_cap(benchmarks: &[LaneBenchmark]) -> Option<f64> {
    let cross_stats = lane_stats(benchmarks, ThroughputLane::CrossLane)?;
    Some(cross_stats.min)
}

fn confidence_score(benchmarks: &[LaneBenchmark]) -> f64 {
    if benchmarks.is_empty() {
        return 0.0;
    }

    let avg_confidence = benchmarks
        .iter()
        .map(|benchmark| benchmark.confidence.clamp(0.0, 1.0))
        .sum::<f64>()
        / benchmarks.len() as f64;
    let reproducible_fraction = benchmarks
        .iter()
        .filter(|benchmark| benchmark.reproducible)
        .count() as f64
        / benchmarks.len() as f64;

    (avg_confidence * 0.80 + reproducible_fraction * 0.20).clamp(0.0, 1.0)
}

fn benchmark_digest(benchmark: &LaneBenchmark) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([lane_discriminant(benchmark.lane)]);
    hasher.update(benchmark.measured_tps.to_bits().to_be_bytes());
    hasher.update((benchmark.conditions.len() as u64).to_be_bytes());
    hasher.update(benchmark.conditions.as_bytes());
    hasher.update(benchmark.confidence.to_bits().to_be_bytes());
    hasher.update([u8::from(benchmark.reproducible)]);

    let digest = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

fn all_accounting_methods() -> [AccountingMethod; 5] {
    [
        AccountingMethod::PeakTPS,
        AccountingMethod::SustainedTPS,
        AccountingMethod::WeightedAverage,
        AccountingMethod::BottleneckBound,
        AccountingMethod::TheoreticalMax,
    ]
}

fn all_lanes() -> [ThroughputLane; 7] {
    [
        ThroughputLane::L1Execution,
        ThroughputLane::L1DataAvailability,
        ThroughputLane::L1Consensus,
        ThroughputLane::L2Rollup,
        ThroughputLane::L2Validium,
        ThroughputLane::L2Optimistic,
        ThroughputLane::CrossLane,
    ]
}

fn lane_discriminant(lane: ThroughputLane) -> u8 {
    match lane {
        ThroughputLane::L1Execution => 0,
        ThroughputLane::L1DataAvailability => 1,
        ThroughputLane::L1Consensus => 2,
        ThroughputLane::L2Rollup => 3,
        ThroughputLane::L2Validium => 4,
        ThroughputLane::L2Optimistic => 5,
        ThroughputLane::CrossLane => 6,
    }
}

fn is_l1_lane(lane: ThroughputLane) -> bool {
    matches!(
        lane,
        ThroughputLane::L1Execution
            | ThroughputLane::L1DataAvailability
            | ThroughputLane::L1Consensus
    )
}

fn is_l2_lane(lane: ThroughputLane) -> bool {
    matches!(
        lane,
        ThroughputLane::L2Rollup | ThroughputLane::L2Validium | ThroughputLane::L2Optimistic
    )
}

fn sanitize_ratio(ratio: f64) -> f64 {
    if ratio.is_finite() && ratio > 0.0 {
        ratio
    } else {
        1.0
    }
}

fn sanitize_confidence_floor(confidence: f64) -> f64 {
    if confidence.is_finite() {
        confidence.max(0.0)
    } else {
        0.0
    }
}
