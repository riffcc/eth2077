//! Strawmap companion document mirror, extraction, checklist gating, and stats commitment types.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
const MAX_SYNC_INTERVAL_HOURS: u64 = 24 * 31;
const MAX_MIN_REVIEW_COUNT: usize = 64;
const MAX_OPEN_ITEMS: usize = 100_000;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentSection {
    Overview,
    ExecutionUpgrades,
    ConsensusChanges,
    DataLayer,
    Cryptography,
    Networking,
    Governance,
}
impl DocumentSection {
    const fn rank(self) -> u8 {
        match self {
            Self::Overview => 0,
            Self::ExecutionUpgrades => 1,
            Self::ConsensusChanges => 2,
            Self::DataLayer => 3,
            Self::Cryptography => 4,
            Self::Networking => 5,
            Self::Governance => 6,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtractionStatus {
    Identified,
    Extracted,
    Validated,
    Assigned,
    Completed,
}
impl ExtractionStatus {
    const ORDERED: [Self; 5] = [
        Self::Identified,
        Self::Extracted,
        Self::Validated,
        Self::Assigned,
        Self::Completed,
    ];
    const fn rank(self) -> u8 {
        match self {
            Self::Identified => 0,
            Self::Extracted => 1,
            Self::Validated => 2,
            Self::Assigned => 3,
            Self::Completed => 4,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpgradeComplexity {
    Trivial,
    Simple,
    Moderate,
    Complex,
    Epic,
}
impl UpgradeComplexity {
    const ORDERED: [Self; 5] = [
        Self::Trivial,
        Self::Simple,
        Self::Moderate,
        Self::Complex,
        Self::Epic,
    ];
    const fn rank(self) -> u8 {
        match self {
            Self::Trivial => 0,
            Self::Simple => 1,
            Self::Moderate => 2,
            Self::Complex => 3,
            Self::Epic => 4,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateType {
    ProofGate,
    BenchGate,
    ReviewGate,
    IntegrationGate,
}
impl GateType {
    const ORDERED: [Self; 4] = [
        Self::ProofGate,
        Self::BenchGate,
        Self::ReviewGate,
        Self::IntegrationGate,
    ];
    const fn rank(self) -> u8 {
        match self {
            Self::ProofGate => 0,
            Self::BenchGate => 1,
            Self::ReviewGate => 2,
            Self::IntegrationGate => 3,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanionSection {
    pub section: DocumentSection,
    pub title: String,
    pub content_hash: [u8; 32],
    pub item_count: usize,
    pub last_synced_unix: u64,
    pub source_url: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpgradeItem {
    pub id: String,
    pub section: DocumentSection,
    pub title: String,
    pub description: String,
    pub complexity: UpgradeComplexity,
    pub status: ExtractionStatus,
    pub linked_eips: Vec<u64>,
    pub gates_required: Vec<GateType>,
    pub gates_passed: Vec<GateType>,
    pub assigned_to: Option<String>,
    pub eth2077_ticket_id: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanionMirror {
    pub id: String,
    pub source_url: String,
    pub mirrored_at_unix: u64,
    pub sections: Vec<CompanionSection>,
    pub total_items_extracted: usize,
    pub document_hash: [u8; 32],
    pub version: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrawmapCompanionConfig {
    pub source_url: String,
    pub sync_interval_hours: u64,
    pub auto_extract: bool,
    pub required_gates: Vec<GateType>,
    pub min_review_count: usize,
    pub complexity_threshold_for_proof: UpgradeComplexity,
    pub max_open_items: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrawmapCompanionValidationError {
    pub field: String,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrawmapCompanionStats {
    pub total_sections: usize,
    pub total_items: usize,
    pub items_by_status: Vec<(ExtractionStatus, usize)>,
    pub items_by_complexity: Vec<(UpgradeComplexity, usize)>,
    pub gates_completion_rate: f64,
    pub avg_gates_per_item: f64,
    pub completion_pct: f64,
    pub commitment: [u8; 32],
}
impl Default for StrawmapCompanionConfig {
    fn default() -> Self {
        Self {
            source_url: String::new(),
            sync_interval_hours: 24,
            auto_extract: true,
            required_gates: vec![
                GateType::ProofGate,
                GateType::BenchGate,
                GateType::ReviewGate,
            ],
            min_review_count: 2,
            complexity_threshold_for_proof: UpgradeComplexity::Complex,
            max_open_items: 50,
        }
    }
}
impl StrawmapCompanionConfig {
    pub fn validate(&self) -> Vec<StrawmapCompanionValidationError> {
        let mut errors = Vec::new();
        let source = self.source_url.trim();
        if source.is_empty() {
            push_validation_error(&mut errors, "source_url", "must not be empty");
        } else if !source.starts_with("http://") && !source.starts_with("https://") {
            push_validation_error(
                &mut errors,
                "source_url",
                "must start with http:// or https://",
            );
        }
        if self.sync_interval_hours == 0 {
            push_validation_error(
                &mut errors,
                "sync_interval_hours",
                "must be greater than zero",
            );
        }
        if self.sync_interval_hours > MAX_SYNC_INTERVAL_HOURS {
            push_validation_error(
                &mut errors,
                "sync_interval_hours",
                "must not exceed 744 hours (31 days)",
            );
        }
        if self.required_gates.is_empty() {
            push_validation_error(
                &mut errors,
                "required_gates",
                "must include at least one gate",
            );
        }
        if has_duplicate_gate_types(&self.required_gates) {
            push_validation_error(
                &mut errors,
                "required_gates",
                "must not contain duplicate entries",
            );
        }
        if self.min_review_count == 0 {
            push_validation_error(&mut errors, "min_review_count", "must be greater than zero");
        }
        if self.min_review_count > MAX_MIN_REVIEW_COUNT {
            push_validation_error(
                &mut errors,
                "min_review_count",
                "is too high; expected <= 64",
            );
        }
        if self.required_gates.contains(&GateType::ReviewGate) && self.min_review_count < 2 {
            push_validation_error(
                &mut errors,
                "min_review_count",
                "must be at least 2 when ReviewGate is required",
            );
        }
        if self.max_open_items == 0 {
            push_validation_error(&mut errors, "max_open_items", "must be greater than zero");
        }
        if self.max_open_items > MAX_OPEN_ITEMS {
            push_validation_error(
                &mut errors,
                "max_open_items",
                "is too high; expected <= 100000",
            );
        }
        if self.max_open_items > 0 && self.max_open_items < self.min_review_count {
            push_validation_error(
                &mut errors,
                "max_open_items",
                "must be at least min_review_count",
            );
        }
        if self.required_gates.contains(&GateType::ProofGate)
            && self.complexity_threshold_for_proof == UpgradeComplexity::Trivial
        {
            push_validation_error(
                &mut errors,
                "complexity_threshold_for_proof",
                "must be at least Simple when ProofGate is required",
            );
        }
        errors
    }
    pub fn needs_sync(&self, mirror: &CompanionMirror, now_unix: u64) -> bool {
        if self.source_url.trim() != mirror.source_url.trim() {
            return true;
        }
        if self.sync_interval_hours == 0 {
            return true;
        }
        if mirror.sections.is_empty() && mirror.total_items_extracted == 0 {
            return true;
        }
        if is_zero_32(&mirror.document_hash) {
            return true;
        }
        if now_unix <= mirror.mirrored_at_unix {
            return false;
        }
        let elapsed = now_unix - mirror.mirrored_at_unix;
        let interval = self.sync_interval_hours.saturating_mul(3600);
        elapsed >= interval
    }
}
pub fn compute_stats(mirror: &CompanionMirror, items: &[UpgradeItem]) -> StrawmapCompanionStats {
    let total_sections = mirror.sections.len();
    let total_items = items.len();
    let mut status_counts = [0usize; 5];
    let mut complexity_counts = [0usize; 5];
    let mut required_gates_total = 0usize;
    let mut passed_required_total = 0usize;
    let mut completed_items = 0usize;
    for item in items {
        status_counts[item.status.rank() as usize] += 1;
        complexity_counts[item.complexity.rank() as usize] += 1;
        if item.status == ExtractionStatus::Completed {
            completed_items += 1;
        }
        let required = dedup_and_sort_gates(&item.gates_required);
        let passed = dedup_and_sort_gates(&item.gates_passed);
        required_gates_total += required.len();
        for gate in required {
            if passed.contains(&gate) {
                passed_required_total += 1;
            }
        }
    }
    let items_by_status: Vec<(ExtractionStatus, usize)> = ExtractionStatus::ORDERED
        .iter()
        .enumerate()
        .map(|(idx, status)| (*status, status_counts[idx]))
        .collect();
    let items_by_complexity: Vec<(UpgradeComplexity, usize)> = UpgradeComplexity::ORDERED
        .iter()
        .enumerate()
        .map(|(idx, complexity)| (*complexity, complexity_counts[idx]))
        .collect();
    let gates_completion_rate = ratio_or_zero(passed_required_total, required_gates_total);
    let avg_gates_per_item = ratio_or_zero(required_gates_total, total_items);
    let completion_pct = if total_items == 0 {
        0.0
    } else {
        100.0 * completed_items as f64 / total_items as f64
    };
    let base_stats = StrawmapCompanionStats {
        total_sections,
        total_items,
        items_by_status,
        items_by_complexity,
        gates_completion_rate,
        avg_gates_per_item,
        completion_pct,
        commitment: [0u8; 32],
    };
    let commitment = compute_commitment_payload(mirror, items, &base_stats);
    StrawmapCompanionStats {
        commitment,
        ..base_stats
    }
}
#[derive(Debug, Clone, Serialize)]
struct SectionCommitmentView {
    section: DocumentSection,
    title: String,
    content_hash: [u8; 32],
    item_count: usize,
    last_synced_unix: u64,
    source_url: String,
}
#[derive(Debug, Clone, Serialize)]
struct ItemCommitmentView {
    id: String,
    section: DocumentSection,
    title: String,
    description: String,
    complexity: UpgradeComplexity,
    status: ExtractionStatus,
    linked_eips: Vec<u64>,
    gates_required: Vec<GateType>,
    gates_passed: Vec<GateType>,
    assigned_to: Option<String>,
    eth2077_ticket_id: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
struct MirrorCommitmentView {
    id: String,
    source_url: String,
    mirrored_at_unix: u64,
    sections: Vec<SectionCommitmentView>,
    total_items_extracted: usize,
    document_hash: [u8; 32],
    version: u32,
}
#[derive(Debug, Clone, Serialize)]
struct StatsCommitmentView {
    total_sections: usize,
    total_items: usize,
    items_by_status: Vec<(ExtractionStatus, usize)>,
    items_by_complexity: Vec<(UpgradeComplexity, usize)>,
    gates_completion_rate: f64,
    avg_gates_per_item: f64,
    completion_pct: f64,
}
#[derive(Debug, Clone, Serialize)]
struct CommitmentPayload {
    mirror: MirrorCommitmentView,
    items: Vec<ItemCommitmentView>,
    stats: StatsCommitmentView,
}
fn compute_commitment_payload(
    mirror: &CompanionMirror,
    items: &[UpgradeItem],
    stats: &StrawmapCompanionStats,
) -> [u8; 32] {
    let payload = CommitmentPayload {
        mirror: canonicalize_mirror(mirror),
        items: canonicalize_items(items),
        stats: canonicalize_stats(stats),
    };
    let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(payload_bytes);
    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}
fn canonicalize_mirror(mirror: &CompanionMirror) -> MirrorCommitmentView {
    let mut sections: Vec<SectionCommitmentView> = mirror
        .sections
        .iter()
        .map(|section| SectionCommitmentView {
            section: section.section,
            title: section.title.clone(),
            content_hash: section.content_hash,
            item_count: section.item_count,
            last_synced_unix: section.last_synced_unix,
            source_url: section.source_url.clone(),
        })
        .collect();
    sections.sort_by(|left, right| {
        left.section
            .rank()
            .cmp(&right.section.rank())
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.source_url.cmp(&right.source_url))
            .then_with(|| left.content_hash.cmp(&right.content_hash))
            .then_with(|| left.item_count.cmp(&right.item_count))
            .then_with(|| left.last_synced_unix.cmp(&right.last_synced_unix))
    });
    MirrorCommitmentView {
        id: mirror.id.clone(),
        source_url: mirror.source_url.clone(),
        mirrored_at_unix: mirror.mirrored_at_unix,
        sections,
        total_items_extracted: mirror.total_items_extracted,
        document_hash: mirror.document_hash,
        version: mirror.version,
    }
}
fn canonicalize_items(items: &[UpgradeItem]) -> Vec<ItemCommitmentView> {
    let mut out: Vec<ItemCommitmentView> = items
        .iter()
        .map(|item| ItemCommitmentView {
            id: item.id.clone(),
            section: item.section,
            title: item.title.clone(),
            description: item.description.clone(),
            complexity: item.complexity,
            status: item.status,
            linked_eips: sorted_unique_u64(&item.linked_eips),
            gates_required: dedup_and_sort_gates(&item.gates_required),
            gates_passed: dedup_and_sort_gates(&item.gates_passed),
            assigned_to: item.assigned_to.clone(),
            eth2077_ticket_id: item.eth2077_ticket_id.clone(),
        })
        .collect();
    out.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.section.rank().cmp(&right.section.rank()))
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.description.cmp(&right.description))
            .then_with(|| left.complexity.rank().cmp(&right.complexity.rank()))
            .then_with(|| left.status.rank().cmp(&right.status.rank()))
    });
    out
}
fn canonicalize_stats(stats: &StrawmapCompanionStats) -> StatsCommitmentView {
    let mut status = stats.items_by_status.clone();
    status.sort_by(|left, right| left.0.rank().cmp(&right.0.rank()));
    let mut complexity = stats.items_by_complexity.clone();
    complexity.sort_by(|left, right| left.0.rank().cmp(&right.0.rank()));
    StatsCommitmentView {
        total_sections: stats.total_sections,
        total_items: stats.total_items,
        items_by_status: status,
        items_by_complexity: complexity,
        gates_completion_rate: stats.gates_completion_rate,
        avg_gates_per_item: stats.avg_gates_per_item,
        completion_pct: stats.completion_pct,
    }
}
fn push_validation_error(
    errors: &mut Vec<StrawmapCompanionValidationError>,
    field: &str,
    message: &str,
) {
    errors.push(StrawmapCompanionValidationError {
        field: field.to_string(),
        message: message.to_string(),
    });
}
fn has_duplicate_gate_types(gates: &[GateType]) -> bool {
    let mut seen = [false; 4];
    for gate in gates {
        let index = gate.rank() as usize;
        if seen[index] {
            return true;
        }
        seen[index] = true;
    }
    false
}
fn dedup_and_sort_gates(gates: &[GateType]) -> Vec<GateType> {
    let mut flags = [false; 4];
    for gate in gates {
        flags[gate.rank() as usize] = true;
    }
    GateType::ORDERED
        .iter()
        .copied()
        .filter(|gate| flags[gate.rank() as usize])
        .collect()
}
fn sorted_unique_u64(values: &[u64]) -> Vec<u64> {
    let mut out = values.to_vec();
    out.sort_unstable();
    out.dedup();
    out
}
fn ratio_or_zero(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}
fn is_zero_32(bytes: &[u8; 32]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}
