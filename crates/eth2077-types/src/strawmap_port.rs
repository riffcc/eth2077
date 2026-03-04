use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::BTreeSet;

const COMMITMENT_DOMAIN_SEPARATOR: &[u8] = b"eth2077-strawmap-port-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StrawmapDomain {
    Execution,
    Consensus,
    DataAvailability,
    Networking,
    Cryptography,
}

impl StrawmapDomain {
    pub const ALL: [StrawmapDomain; 5] = [
        StrawmapDomain::Execution,
        StrawmapDomain::Consensus,
        StrawmapDomain::DataAvailability,
        StrawmapDomain::Networking,
        StrawmapDomain::Cryptography,
    ];

    pub const fn index(self) -> usize {
        match self {
            StrawmapDomain::Execution => 0,
            StrawmapDomain::Consensus => 1,
            StrawmapDomain::DataAvailability => 2,
            StrawmapDomain::Networking => 3,
            StrawmapDomain::Cryptography => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PortingPhase {
    Identified,
    Scoped,
    InProgress,
    Ported,
    Verified,
    Benchmarked,
    Integrated,
}

impl PortingPhase {
    pub const ALL: [PortingPhase; 7] = [
        PortingPhase::Identified,
        PortingPhase::Scoped,
        PortingPhase::InProgress,
        PortingPhase::Ported,
        PortingPhase::Verified,
        PortingPhase::Benchmarked,
        PortingPhase::Integrated,
    ];

    pub const fn index(self) -> usize {
        match self {
            PortingPhase::Identified => 0,
            PortingPhase::Scoped => 1,
            PortingPhase::InProgress => 2,
            PortingPhase::Ported => 3,
            PortingPhase::Verified => 4,
            PortingPhase::Benchmarked => 5,
            PortingPhase::Integrated => 6,
        }
    }

    pub const fn rank(self) -> u8 {
        self.index() as u8
    }

    pub const fn is_at_least(self, target: PortingPhase) -> bool {
        self.rank() >= target.rank()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum VerificationGate {
    FormalProof,
    PropertyTest,
    FuzzTest,
    ManualReview,
    NotRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BenchmarkResult {
    Passed,
    Marginal,
    Failed,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrawmapItem {
    pub id: String,
    pub strawmap_ref: String,
    pub title: String,
    pub domain: StrawmapDomain,
    pub phase: PortingPhase,
    pub priority: u8,
    pub estimated_effort_days: f64,
    pub assigned_to: Option<String>,
    pub dependencies: Vec<String>,
    pub notes: String,
}

impl StrawmapItem {
    pub fn is_verified_or_better(&self) -> bool {
        self.phase.is_at_least(PortingPhase::Verified)
    }

    pub fn is_benchmarked_or_better(&self) -> bool {
        self.phase.is_at_least(PortingPhase::Benchmarked)
    }

    pub fn is_integrated(&self) -> bool {
        self.phase.is_at_least(PortingPhase::Integrated)
    }

    fn commitment_record(&self) -> ItemCommitmentRecord {
        let mut dependencies = self.dependencies.clone();
        dependencies.sort();
        ItemCommitmentRecord {
            id: self.id.clone(),
            strawmap_ref: self.strawmap_ref.clone(),
            title: self.title.clone(),
            domain: self.domain,
            phase: self.phase,
            priority: self.priority,
            estimated_effort_days_bits: finite_bits(self.estimated_effort_days),
            assigned_to: self.assigned_to.clone(),
            dependencies,
            notes: self.notes.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortingTask {
    pub item_id: String,
    pub eth2077_module: String,
    pub verification_gate: VerificationGate,
    pub benchmark_result: BenchmarkResult,
    pub proof_artifact: Option<String>,
    pub benchmark_throughput: Option<f64>,
    pub benchmark_latency_ms: Option<f64>,
    pub started_at_unix: Option<u64>,
    pub completed_at_unix: Option<u64>,
}

impl PortingTask {
    fn commitment_record(&self) -> TaskCommitmentRecord {
        TaskCommitmentRecord {
            item_id: self.item_id.clone(),
            eth2077_module: self.eth2077_module.clone(),
            verification_gate: self.verification_gate,
            benchmark_result: self.benchmark_result,
            proof_artifact: self.proof_artifact.clone(),
            benchmark_throughput_bits: self.benchmark_throughput.map(finite_bits),
            benchmark_latency_ms_bits: self.benchmark_latency_ms.map(finite_bits),
            started_at_unix: self.started_at_unix,
            completed_at_unix: self.completed_at_unix,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrawmapPortConfig {
    pub required_verification_gates: Vec<VerificationGate>,
    pub min_benchmark_throughput: f64,
    pub max_benchmark_latency_ms: f64,
    pub max_parallel_ports: usize,
    pub phase_a_deadline_unix: Option<u64>,
    pub require_all_domains: bool,
    pub auto_integrate_on_pass: bool,
}

impl Default for StrawmapPortConfig {
    fn default() -> Self {
        StrawmapPortConfig {
            required_verification_gates: vec![
                VerificationGate::FormalProof,
                VerificationGate::PropertyTest,
            ],
            min_benchmark_throughput: 10_000.0,
            max_benchmark_latency_ms: 100.0,
            max_parallel_ports: 5,
            phase_a_deadline_unix: None,
            require_all_domains: true,
            auto_integrate_on_pass: false,
        }
    }
}

impl StrawmapPortConfig {
    pub fn validate(&self) -> Vec<StrawmapPortValidationError> {
        let mut errors = Vec::new();

        if self.required_verification_gates.is_empty() {
            errors.push(StrawmapPortValidationError::new(
                "required_verification_gates",
                "at least one verification gate is required",
            ));
        } else {
            let mut seen = BTreeSet::new();
            for (index, gate) in self.required_verification_gates.iter().copied().enumerate() {
                if !seen.insert(gate) {
                    errors.push(StrawmapPortValidationError::new(
                        "required_verification_gates",
                        format!(
                            "duplicate verification gate `{:?}` at index {}",
                            gate, index
                        ),
                    ));
                }
            }

            if self
                .required_verification_gates
                .contains(&VerificationGate::NotRequired)
                && self.required_verification_gates.len() > 1
            {
                errors.push(StrawmapPortValidationError::new(
                    "required_verification_gates",
                    "`NotRequired` cannot be mixed with hard verification gates",
                ));
            }
        }

        if !is_finite_positive(self.min_benchmark_throughput) {
            errors.push(StrawmapPortValidationError::new(
                "min_benchmark_throughput",
                "must be finite and greater than zero",
            ));
        }

        if !is_finite_positive(self.max_benchmark_latency_ms) {
            errors.push(StrawmapPortValidationError::new(
                "max_benchmark_latency_ms",
                "must be finite and greater than zero",
            ));
        }

        if self.max_parallel_ports == 0 {
            errors.push(StrawmapPortValidationError::new(
                "max_parallel_ports",
                "must be at least 1",
            ));
        }

        if let Some(deadline) = self.phase_a_deadline_unix {
            if deadline == 0 {
                errors.push(StrawmapPortValidationError::new(
                    "phase_a_deadline_unix",
                    "must be a valid unix timestamp greater than zero",
                ));
            }
        }

        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrawmapPortValidationError {
    pub field: String,
    pub message: String,
}

impl StrawmapPortValidationError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        StrawmapPortValidationError {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrawmapPortStats {
    pub total_items: usize,
    pub items_by_domain: Vec<(StrawmapDomain, usize)>,
    pub items_by_phase: Vec<(PortingPhase, usize)>,
    pub verified_count: usize,
    pub benchmarked_count: usize,
    pub integrated_count: usize,
    pub avg_effort_days: f64,
    pub completion_pct: f64,
    pub commitment: [u8; 32],
}

#[derive(Debug, Clone, Serialize)]
struct ItemCommitmentRecord {
    id: String,
    strawmap_ref: String,
    title: String,
    domain: StrawmapDomain,
    phase: PortingPhase,
    priority: u8,
    estimated_effort_days_bits: u64,
    assigned_to: Option<String>,
    dependencies: Vec<String>,
    notes: String,
}

impl ItemCommitmentRecord {
    fn ordering_key(&self) -> (&str, &str, &str) {
        (&self.id, &self.strawmap_ref, &self.title)
    }
}

#[derive(Debug, Clone, Serialize)]
struct TaskCommitmentRecord {
    item_id: String,
    eth2077_module: String,
    verification_gate: VerificationGate,
    benchmark_result: BenchmarkResult,
    proof_artifact: Option<String>,
    benchmark_throughput_bits: Option<u64>,
    benchmark_latency_ms_bits: Option<u64>,
    started_at_unix: Option<u64>,
    completed_at_unix: Option<u64>,
}

impl TaskCommitmentRecord {
    fn ordering_key(&self) -> (&str, &str) {
        (&self.item_id, &self.eth2077_module)
    }
}

#[derive(Debug, Clone, Serialize)]
struct CountByDomainRecord {
    domain: StrawmapDomain,
    count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct CountByPhaseRecord {
    phase: PortingPhase,
    count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct CommitmentPayload {
    version: u8,
    total_items: usize,
    total_tasks: usize,
    counts_by_domain: Vec<CountByDomainRecord>,
    counts_by_phase: Vec<CountByPhaseRecord>,
    verified_count: usize,
    benchmarked_count: usize,
    integrated_count: usize,
    avg_effort_days_bits: u64,
    completion_pct_bits: u64,
    items: Vec<ItemCommitmentRecord>,
    tasks: Vec<TaskCommitmentRecord>,
}

pub fn compute_stats(items: &[StrawmapItem], tasks: &[PortingTask]) -> StrawmapPortStats {
    let mut domain_counts = [0_usize; StrawmapDomain::ALL.len()];
    let mut phase_counts = [0_usize; PortingPhase::ALL.len()];

    let mut verified_count = 0_usize;
    let mut benchmarked_count = 0_usize;
    let mut integrated_count = 0_usize;

    let mut effort_sum = 0.0_f64;
    let mut effort_samples = 0_usize;

    for item in items {
        domain_counts[item.domain.index()] += 1;
        phase_counts[item.phase.index()] += 1;

        if item.is_verified_or_better() {
            verified_count += 1;
        }
        if item.is_benchmarked_or_better() {
            benchmarked_count += 1;
        }
        if item.is_integrated() {
            integrated_count += 1;
        }

        if item.estimated_effort_days.is_finite() && item.estimated_effort_days >= 0.0 {
            effort_sum += item.estimated_effort_days;
            effort_samples += 1;
        }
    }

    let avg_effort_days = if effort_samples == 0 {
        0.0
    } else {
        effort_sum / effort_samples as f64
    };

    let completion_pct = if items.is_empty() {
        0.0
    } else {
        (verified_count as f64 / items.len() as f64) * 100.0
    };

    let items_by_domain: Vec<(StrawmapDomain, usize)> = StrawmapDomain::ALL
        .iter()
        .copied()
        .map(|domain| (domain, domain_counts[domain.index()]))
        .collect();

    let items_by_phase: Vec<(PortingPhase, usize)> = PortingPhase::ALL
        .iter()
        .copied()
        .map(|phase| (phase, phase_counts[phase.index()]))
        .collect();

    let commitment = compute_commitment(
        items,
        tasks,
        &items_by_domain,
        &items_by_phase,
        verified_count,
        benchmarked_count,
        integrated_count,
        avg_effort_days,
        completion_pct,
    );

    StrawmapPortStats {
        total_items: items.len(),
        items_by_domain,
        items_by_phase,
        verified_count,
        benchmarked_count,
        integrated_count,
        avg_effort_days,
        completion_pct,
        commitment,
    }
}

pub fn is_phase_a_complete(items: &[StrawmapItem]) -> bool {
    items
        .iter()
        .all(|item| item.phase.is_at_least(PortingPhase::Verified))
}

fn compute_commitment(
    items: &[StrawmapItem],
    tasks: &[PortingTask],
    items_by_domain: &[(StrawmapDomain, usize)],
    items_by_phase: &[(PortingPhase, usize)],
    verified_count: usize,
    benchmarked_count: usize,
    integrated_count: usize,
    avg_effort_days: f64,
    completion_pct: f64,
) -> [u8; 32] {
    let mut normalized_items: Vec<ItemCommitmentRecord> =
        items.iter().map(StrawmapItem::commitment_record).collect();
    normalized_items.sort_by(compare_item_records);

    let mut normalized_tasks: Vec<TaskCommitmentRecord> =
        tasks.iter().map(PortingTask::commitment_record).collect();
    normalized_tasks.sort_by(compare_task_records);

    let counts_by_domain = items_by_domain
        .iter()
        .map(|(domain, count)| CountByDomainRecord {
            domain: *domain,
            count: *count,
        })
        .collect();
    let counts_by_phase = items_by_phase
        .iter()
        .map(|(phase, count)| CountByPhaseRecord {
            phase: *phase,
            count: *count,
        })
        .collect();

    let payload = CommitmentPayload {
        version: 1,
        total_items: items.len(),
        total_tasks: tasks.len(),
        counts_by_domain,
        counts_by_phase,
        verified_count,
        benchmarked_count,
        integrated_count,
        avg_effort_days_bits: finite_bits(avg_effort_days),
        completion_pct_bits: finite_bits(completion_pct),
        items: normalized_items,
        tasks: normalized_tasks,
    };

    let serialized = serde_json::to_vec(&payload).unwrap_or_default();

    let mut hasher = Sha256::new();
    hasher.update(COMMITMENT_DOMAIN_SEPARATOR);
    hasher.update((serialized.len() as u64).to_be_bytes());
    hasher.update(serialized);

    let digest = hasher.finalize();
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn compare_item_records(left: &ItemCommitmentRecord, right: &ItemCommitmentRecord) -> Ordering {
    left.ordering_key()
        .cmp(&right.ordering_key())
        .then(left.priority.cmp(&right.priority))
        .then(left.domain.cmp(&right.domain))
        .then(left.phase.cmp(&right.phase))
        .then(
            left.estimated_effort_days_bits
                .cmp(&right.estimated_effort_days_bits),
        )
}

fn compare_task_records(left: &TaskCommitmentRecord, right: &TaskCommitmentRecord) -> Ordering {
    left.ordering_key()
        .cmp(&right.ordering_key())
        .then(left.verification_gate.cmp(&right.verification_gate))
        .then(left.benchmark_result.cmp(&right.benchmark_result))
}

fn is_finite_positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn finite_bits(value: f64) -> u64 {
    if value.is_finite() {
        value.to_bits()
    } else if value.is_sign_negative() {
        f64::NEG_INFINITY.to_bits()
    } else {
        f64::INFINITY.to_bits()
    }
}
