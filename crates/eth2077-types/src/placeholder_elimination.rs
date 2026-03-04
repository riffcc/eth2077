use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Represents the lifecycle state of a placeholder artifact while it is being
/// removed from the ETH2077 codebase.
///
/// The progression is intentionally directional:
/// `Identified -> Analyzed -> InProgress -> Replaced -> Verified -> Certified`.
///
/// Downstream tooling can use this enum to compute elimination velocity,
/// prioritize work queues, and enforce promotion gates before production release.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PlaceholderStatus {
    /// The placeholder has been discovered and cataloged, but no analysis has
    /// been attached yet.
    Identified,
    /// The placeholder has a clear scope, ownership signal, and replacement
    /// strategy prepared.
    Analyzed,
    /// Active implementation work is in progress to remove the placeholder.
    InProgress,
    /// A production implementation has replaced the placeholder path.
    Replaced,
    /// Replacement behavior has passed verification gates configured for the
    /// owning module or rollout track.
    Verified,
    /// The replacement has reached final acceptance and can be treated as
    /// governance-grade quality.
    Certified,
}

/// Describes how the placeholder was eliminated.
///
/// Method-level attribution helps teams compare delivery throughput and risk by
/// migration strategy rather than by only raw counts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EliminationMethod {
    /// Replaced by directly porting a previously validated implementation.
    DirectPort,
    /// Replaced by a new design and fresh implementation effort.
    Rewrite,
    /// Replaced by inserting an adapter around stable internals while external
    /// interfaces are migrated.
    WrapperShim,
    /// Replaced through a foreign-function interface to proven external code.
    FfiBinding,
    /// Replaced via generated code from an auditable source schema.
    CodeGen,
    /// Replaced through manual proof-oriented construction and review.
    ManualProof,
}

/// Verification gates that can be attached to a placeholder replacement entry.
///
/// Gates are intentionally orthogonal; each gate captures confidence through a
/// different failure-detection channel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VerificationGate {
    /// Conventional deterministic unit tests for local behavior.
    UnitTests,
    /// Cross-component tests validating integration-level semantics.
    IntegrationTests,
    /// Property-based tests validating invariants across broad input spaces.
    PropertyTests,
    /// Formal proofs over correctness, safety, or liveness obligations.
    FormalProof,
    /// Fuzz campaign signal validating parser and state-machine resilience.
    FuzzSuite,
    /// Human review sign-off by maintainers or designated domain experts.
    PeerReview,
}

/// Captures how complete the formal argument is for a replacement.
///
/// This level is used for policy checks (`min_proof_completeness`) and for
/// aggregate reporting (`avg_proof_completeness`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProofCompleteness {
    /// No formal reasoning artifacts are attached.
    None_,
    /// Some formal notes exist but major obligations remain open.
    Partial,
    /// Core obligations are addressed; edge obligations may remain open.
    Substantial,
    /// Most obligations are discharged and final closure is near.
    NearComplete,
    /// Complete argument package is provided and reviewed.
    Complete,
    /// Complete argument package with machine-checked validation.
    MachineChecked,
}

/// One tracked placeholder elimination item.
///
/// Each entry corresponds to a specific function-level placeholder, including
/// its lifecycle status, migration method, verification coverage, and auxiliary
/// metadata used by external policy engines.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaceholderEntry {
    /// Stable entry identifier.
    pub id: String,
    /// Rust module path that previously contained the placeholder.
    pub module_path: String,
    /// Function symbol being replaced.
    pub function_name: String,
    /// Current lifecycle status for the replacement effort.
    pub status: PlaceholderStatus,
    /// Strategy used to eliminate the placeholder.
    pub method: EliminationMethod,
    /// Gates passed by the replacement.
    pub verification_gates: Vec<VerificationGate>,
    /// Degree of proof completion for the replacement.
    pub proof_completeness: ProofCompleteness,
    /// Approximate lines of code replaced by production implementation.
    pub lines_of_code: usize,
    /// Arbitrary metadata for provenance, ownership, or rollout context.
    pub metadata: HashMap<String, String>,
}

/// Policy configuration for placeholder elimination.
///
/// This config is intended to be hashed into commitments for attestable policy
/// drift detection and applied uniformly by CI gating.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaceholderEliminationConfig {
    /// Maximum age of an unresolved placeholder before escalation is required.
    pub max_placeholder_age_days: u64,
    /// Verification gates required to declare success for an entry.
    pub required_gates: Vec<VerificationGate>,
    /// Minimum formal proof level required by policy.
    pub min_proof_completeness: ProofCompleteness,
    /// Enables automatic promotion to `Certified` when gates and proof levels
    /// satisfy policy constraints.
    pub auto_certify: bool,
    /// Coverage threshold expressed in percent (`0.0..=100.0`).
    pub coverage_threshold_pct: f64,
    /// Minimum fuzz iterations required when `FuzzSuite` is in required gates.
    pub fuzz_iterations: u64,
    /// Additional policy metadata included in commitment hashing.
    pub metadata: HashMap<String, String>,
}

/// Validation error emitted when a configuration field violates policy.
///
/// Errors are aggregated so callers can display all policy violations in a
/// single pass instead of failing on first issue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaceholderEliminationValidationError {
    /// Field name that failed validation.
    pub field: String,
    /// Human-readable reason for the violation.
    pub reason: String,
}

/// Aggregate metrics describing placeholder elimination progress.
///
/// Values are designed for dashboarding and release-readiness checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaceholderEliminationStats {
    /// Total placeholders tracked in the input slice.
    pub total_placeholders: usize,
    /// Counts by placeholder status. All statuses are always present.
    pub by_status: HashMap<PlaceholderStatus, usize>,
    /// Percentage of placeholders at `Replaced` or beyond.
    pub elimination_rate_pct: f64,
    /// Average proof completeness score on a `0.0..=5.0` scale.
    pub avg_proof_completeness: f64,
    /// Total lines replaced for placeholders at `Replaced` or beyond.
    pub total_loc_replaced: usize,
    /// Percentage of verification gates satisfied across all placeholders.
    pub gates_passed_pct: f64,
}

const TOTAL_VERIFICATION_GATES: usize = 6;

/// Builds a baseline placeholder elimination configuration.
///
/// The defaults are intentionally strict enough for production migration while
/// still practical for continuous integration:
/// - bounded placeholder age,
/// - explicit multi-gate verification,
/// - substantial minimum proof quality,
/// - high coverage targets,
/// - non-zero fuzz campaign budget.
pub fn default_placeholder_elimination_config() -> PlaceholderEliminationConfig {
    let mut metadata = HashMap::new();
    metadata.insert(
        "policy_profile".to_string(),
        "eth2077-placeholder-elimination-v1".to_string(),
    );
    metadata.insert("owner".to_string(), "eth2077-core".to_string());

    PlaceholderEliminationConfig {
        max_placeholder_age_days: 30,
        required_gates: vec![
            VerificationGate::UnitTests,
            VerificationGate::IntegrationTests,
            VerificationGate::PropertyTests,
            VerificationGate::FuzzSuite,
            VerificationGate::PeerReview,
        ],
        min_proof_completeness: ProofCompleteness::Substantial,
        auto_certify: false,
        coverage_threshold_pct: 95.0,
        fuzz_iterations: 100_000,
        metadata,
    }
}

/// Validates a placeholder elimination configuration.
///
/// Validation rules enforce structural correctness and policy coherence:
/// - age limits must be meaningful,
/// - required gates must be non-empty and non-duplicated,
/// - coverage thresholds must be finite percentages,
/// - fuzz requirements must include non-zero campaign budget,
/// - proof requirements must align with formal-proof expectations,
/// - auto-certification must satisfy stronger governance constraints,
/// - metadata keys and values must be non-empty.
pub fn validate_placeholder_elimination_config(
    config: &PlaceholderEliminationConfig,
) -> Result<(), Vec<PlaceholderEliminationValidationError>> {
    let mut errors = Vec::new();

    if config.max_placeholder_age_days == 0 {
        push_validation_error(
            &mut errors,
            "max_placeholder_age_days",
            "must be greater than zero",
        );
    }

    if config.required_gates.is_empty() {
        push_validation_error(
            &mut errors,
            "required_gates",
            "must contain at least one verification gate",
        );
    }

    if let Some(duplicate) = find_duplicate_gate(&config.required_gates) {
        push_validation_error(
            &mut errors,
            "required_gates",
            &format!(
                "contains duplicate gate `{}`; each gate must appear at most once",
                gate_label(&duplicate)
            ),
        );
    }

    if !(config.coverage_threshold_pct.is_finite()
        && config.coverage_threshold_pct >= 0.0
        && config.coverage_threshold_pct <= 100.0)
    {
        push_validation_error(
            &mut errors,
            "coverage_threshold_pct",
            "must be a finite percentage in the inclusive range 0..=100",
        );
    }

    if contains_gate(&config.required_gates, &VerificationGate::FuzzSuite)
        && config.fuzz_iterations == 0
    {
        push_validation_error(
            &mut errors,
            "fuzz_iterations",
            "must be greater than zero when FuzzSuite is required",
        );
    }

    if config.min_proof_completeness == ProofCompleteness::MachineChecked
        && !contains_gate(&config.required_gates, &VerificationGate::FormalProof)
    {
        push_validation_error(
            &mut errors,
            "required_gates",
            "must include FormalProof when min_proof_completeness is MachineChecked",
        );
    }

    if contains_gate(&config.required_gates, &VerificationGate::FormalProof)
        && proof_completeness_rank(&config.min_proof_completeness)
            < proof_completeness_rank(&ProofCompleteness::Substantial)
    {
        push_validation_error(
            &mut errors,
            "min_proof_completeness",
            "must be at least Substantial when FormalProof is required",
        );
    }

    if config.auto_certify {
        if proof_completeness_rank(&config.min_proof_completeness)
            < proof_completeness_rank(&ProofCompleteness::Complete)
        {
            push_validation_error(
                &mut errors,
                "min_proof_completeness",
                "must be Complete or MachineChecked when auto_certify is enabled",
            );
        }

        if !contains_gate(&config.required_gates, &VerificationGate::PeerReview) {
            push_validation_error(
                &mut errors,
                "required_gates",
                "must include PeerReview when auto_certify is enabled",
            );
        }
    }

    for (key, value) in &config.metadata {
        if key.trim().is_empty() {
            push_validation_error(
                &mut errors,
                "metadata",
                "metadata keys must be non-empty after trimming whitespace",
            );
            break;
        }

        if value.trim().is_empty() {
            push_validation_error(
                &mut errors,
                "metadata",
                &format!(
                    "metadata value for key `{}` must be non-empty after trimming whitespace",
                    key
                ),
            );
            break;
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Computes aggregate placeholder elimination statistics over a set of entries.
///
/// Metric semantics:
/// - `elimination_rate_pct`: entries in `Replaced`, `Verified`, or `Certified`.
/// - `avg_proof_completeness`: arithmetic mean of `0..=5` proof levels.
/// - `total_loc_replaced`: LOC summed only for eliminated entries.
/// - `gates_passed_pct`: unique passed gates vs. max possible gates.
pub fn compute_placeholder_elimination_stats(
    entries: &[PlaceholderEntry],
) -> PlaceholderEliminationStats {
    let mut by_status = HashMap::new();
    for status in all_placeholder_statuses() {
        by_status.insert(status, 0);
    }

    let total_placeholders = entries.len();
    if total_placeholders == 0 {
        return PlaceholderEliminationStats {
            total_placeholders: 0,
            by_status,
            elimination_rate_pct: 0.0,
            avg_proof_completeness: 0.0,
            total_loc_replaced: 0,
            gates_passed_pct: 0.0,
        };
    }

    let mut eliminated_count = 0usize;
    let mut proof_score_sum = 0.0f64;
    let mut total_loc_replaced = 0usize;
    let mut total_unique_gates = 0usize;

    for entry in entries {
        *by_status.entry(entry.status.clone()).or_insert(0) += 1;

        if status_is_eliminated(&entry.status) {
            eliminated_count += 1;
            total_loc_replaced = total_loc_replaced.saturating_add(entry.lines_of_code);
        }

        proof_score_sum += proof_completeness_score(&entry.proof_completeness);
        total_unique_gates += unique_gate_count(&entry.verification_gates);
    }

    let elimination_rate_pct = (eliminated_count as f64 * 100.0) / total_placeholders as f64;
    let avg_proof_completeness = proof_score_sum / total_placeholders as f64;

    let max_possible_gates = total_placeholders * TOTAL_VERIFICATION_GATES;
    let gates_passed_pct = if max_possible_gates == 0 {
        0.0
    } else {
        (total_unique_gates as f64 * 100.0) / max_possible_gates as f64
    };

    PlaceholderEliminationStats {
        total_placeholders,
        by_status,
        elimination_rate_pct,
        avg_proof_completeness,
        total_loc_replaced,
        gates_passed_pct,
    }
}

/// Computes a deterministic SHA-256 commitment of elimination policy config.
///
/// Commitment encoding includes:
/// - fixed domain separator,
/// - scalar config fields,
/// - ordered required gate sequence,
/// - sorted metadata key/value pairs.
///
/// The returned value is lower-case hexadecimal (`64` chars).
pub fn compute_placeholder_elimination_commitment(config: &PlaceholderEliminationConfig) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"eth2077-placeholder-elimination-v1");

    hasher.update(config.max_placeholder_age_days.to_be_bytes());

    hasher.update((config.required_gates.len() as u64).to_be_bytes());
    for gate in &config.required_gates {
        hasher.update([verification_gate_discriminant(gate)]);
    }

    hasher.update([proof_completeness_discriminant(
        &config.min_proof_completeness,
    )]);
    hasher.update([u8::from(config.auto_certify)]);
    hasher.update(config.coverage_threshold_pct.to_bits().to_be_bytes());
    hasher.update(config.fuzz_iterations.to_be_bytes());

    let mut metadata_keys: Vec<&String> = config.metadata.keys().collect();
    metadata_keys.sort();

    hasher.update((metadata_keys.len() as u64).to_be_bytes());
    for key in metadata_keys {
        let value = config
            .metadata
            .get(key)
            .expect("sorted metadata key exists");
        hasher.update((key.len() as u64).to_be_bytes());
        hasher.update(key.as_bytes());
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }

    let digest = hasher.finalize();
    to_lower_hex(&digest)
}

fn all_placeholder_statuses() -> [PlaceholderStatus; 6] {
    [
        PlaceholderStatus::Identified,
        PlaceholderStatus::Analyzed,
        PlaceholderStatus::InProgress,
        PlaceholderStatus::Replaced,
        PlaceholderStatus::Verified,
        PlaceholderStatus::Certified,
    ]
}

fn status_is_eliminated(status: &PlaceholderStatus) -> bool {
    matches!(
        status,
        PlaceholderStatus::Replaced | PlaceholderStatus::Verified | PlaceholderStatus::Certified
    )
}

fn proof_completeness_score(level: &ProofCompleteness) -> f64 {
    proof_completeness_rank(level) as f64
}

fn proof_completeness_rank(level: &ProofCompleteness) -> u8 {
    match level {
        ProofCompleteness::None_ => 0,
        ProofCompleteness::Partial => 1,
        ProofCompleteness::Substantial => 2,
        ProofCompleteness::NearComplete => 3,
        ProofCompleteness::Complete => 4,
        ProofCompleteness::MachineChecked => 5,
    }
}

fn verification_gate_discriminant(gate: &VerificationGate) -> u8 {
    match gate {
        VerificationGate::UnitTests => 1,
        VerificationGate::IntegrationTests => 2,
        VerificationGate::PropertyTests => 3,
        VerificationGate::FormalProof => 4,
        VerificationGate::FuzzSuite => 5,
        VerificationGate::PeerReview => 6,
    }
}

fn proof_completeness_discriminant(level: &ProofCompleteness) -> u8 {
    match level {
        ProofCompleteness::None_ => 1,
        ProofCompleteness::Partial => 2,
        ProofCompleteness::Substantial => 3,
        ProofCompleteness::NearComplete => 4,
        ProofCompleteness::Complete => 5,
        ProofCompleteness::MachineChecked => 6,
    }
}

fn contains_gate(gates: &[VerificationGate], target: &VerificationGate) -> bool {
    gates.iter().any(|gate| gate == target)
}

fn find_duplicate_gate(gates: &[VerificationGate]) -> Option<VerificationGate> {
    for (index, gate) in gates.iter().enumerate() {
        for other in gates.iter().skip(index + 1) {
            if gate == other {
                return Some(gate.clone());
            }
        }
    }
    None
}

fn unique_gate_count(gates: &[VerificationGate]) -> usize {
    let mut unique = Vec::new();

    for gate in gates {
        if !unique.iter().any(|seen: &VerificationGate| seen == gate) {
            unique.push(gate.clone());
        }
    }

    unique.len()
}

fn gate_label(gate: &VerificationGate) -> &'static str {
    match gate {
        VerificationGate::UnitTests => "UnitTests",
        VerificationGate::IntegrationTests => "IntegrationTests",
        VerificationGate::PropertyTests => "PropertyTests",
        VerificationGate::FormalProof => "FormalProof",
        VerificationGate::FuzzSuite => "FuzzSuite",
        VerificationGate::PeerReview => "PeerReview",
    }
}

fn push_validation_error(
    errors: &mut Vec<PlaceholderEliminationValidationError>,
    field: &str,
    reason: &str,
) {
    errors.push(PlaceholderEliminationValidationError {
        field: field.to_string(),
        reason: reason.to_string(),
    });
}

fn to_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(HEX[(byte >> 4) as usize]));
        out.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    out
}
