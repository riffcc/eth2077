use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// The theorem taxonomy module defines a structured language for managing ETH2077
/// formal verification claims.
///
/// It groups statements by semantic family, maps them into operational release tiers,
/// tracks acceptance evidence, and provides deterministic commitment hashing so review
/// and governance tooling can anchor exact taxonomy configuration snapshots.
///
/// The design goal is to keep policy logic in one place:
/// - `TheoremEntry` captures one proof target and its gate state.
/// - `TheoremTaxonomyConfig` captures policy-level controls for release gating.
/// - `compute_theorem_taxonomy_stats` summarizes current theorem portfolio health.
/// - `compute_theorem_taxonomy_commitment` produces an auditable SHA-256 identity.
///
/// Tier semantics in ETH2077 are aligned to verification gates:
/// - Tier 1: safety-critical and deployment blocking when unsatisfied.
/// - Tier 2: liveness and correctness progress requirements for sustained operation.
/// - Tier 3: performance and optimization assurances that support confidence.
///
/// Defines the semantic family for a theorem statement.
///
/// Families allow ETH2077 to report verification coverage in business-relevant buckets
/// (safety, liveness, throughput, fairness, privacy, and resource use) rather than
/// treating all theorems as a flat list.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TheoremFamily {
    /// Statements that forbid invalid states or bad transitions.
    SafetyInvariant,
    /// Statements that guarantee eventual progress or completion.
    LivenessGuarantee,
    /// Statements that provide explicit latency or throughput bounds.
    PerformanceBound,
    /// Statements that constrain scheduler, proposer, or queue fairness.
    FairnessProperty,
    /// Statements that preserve confidentiality or minimize information leakage.
    PrivacyPreserving,
    /// Statements that cap usage of compute, memory, bandwidth, or storage.
    ResourceBound,
}

/// Defines theorem tier and release gate severity.
///
/// The enum includes both broad importance grades and gate-specific control labels,
/// allowing governance processes to represent both conceptual tiering and policy impact
/// in one normalized field.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TheoremTier {
    /// Tier 1 safety theorem with critical impact.
    Tier1Critical,
    /// Tier 2 liveness theorem with important impact.
    Tier2Important,
    /// Tier 3 performance theorem with lower operational risk.
    Tier3Nice,
    /// Tier 1 theorem whose failure blocks release.
    Tier1Blocking,
    /// Tier 2 theorem tracking a known regression risk.
    Tier2Regression,
    /// Tier 3 theorem with advisory status.
    Tier3Advisory,
}

/// Enumerates acceptable evidence types for theorem acceptance.
///
/// ETH2077 can require one or multiple acceptance criteria for a theorem. This allows
/// rigorous proof workflows (machine-checked proofs) while still capturing operational
/// evidence paths (fuzzing, benchmarking, and audits).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AcceptanceCriteria {
    /// Mechanically checked proof artifact (for example Lean/Coq/Isabelle).
    MachineChecked,
    /// Manual review by qualified peers independent from original author.
    PeerReviewed,
    /// Property-based testing that supports statement confidence.
    PropertyTested,
    /// Fuzzing campaign validating theorem-relevant invariants.
    FuzzVerified,
    /// Quantitative benchmark demonstrating required bounds.
    BenchmarkBound,
    /// Explicit manual security or correctness audit sign-off.
    ManualAudit,
}

/// Tracks proof lifecycle stage for one theorem.
///
/// Status values intentionally include non-final states (`InProgress`, `Conjectured`)
/// and governance outcomes (`Disputed`, `Withdrawn`) so audit tooling can distinguish
/// engineering progress from scientific certainty.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProofStatus {
    /// Proof has been completed to accepted standard.
    Proven,
    /// Proof work is active but incomplete.
    InProgress,
    /// Statement is believed true but not yet proven.
    Conjectured,
    /// Statement is admitted as an assumption.
    Admitted,
    /// Statement validity has been challenged.
    Disputed,
    /// Statement has been retired and is no longer active.
    Withdrawn,
}

/// A single theorem record in the ETH2077 taxonomy.
///
/// `TheoremEntry` is intended to be serializable and repository-friendly. Every field
/// is explicit to support deterministic analysis in CI and external reporting tools.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TheoremEntry {
    /// Stable theorem identifier.
    ///
    /// IDs are expected to include a namespace prefix and a unique suffix, such as
    /// `ETH2077-THM-SAFETY-001`.
    pub id: String,
    /// Semantic family of the theorem.
    pub family: TheoremFamily,
    /// Tier and gate severity for release policy.
    pub tier: TheoremTier,
    /// Human-readable theorem title.
    pub name: String,
    /// Formal or semi-formal theorem statement.
    pub statement: String,
    /// Current lifecycle state of theorem proof.
    pub proof_status: ProofStatus,
    /// Accepted evidence categories backing this theorem.
    pub acceptance: Vec<AcceptanceCriteria>,
    /// Upstream theorem IDs that this theorem depends on.
    pub dependencies: Vec<String>,
    /// Free-form metadata for pipeline IDs, owners, links, or tags.
    pub metadata: HashMap<String, String>,
}

/// Configuration controlling theorem taxonomy gate policy.
///
/// This config captures governance rules used by tooling to decide whether theorem
/// status is acceptable for shipping or whether additional proof work is mandatory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TheoremTaxonomyConfig {
    /// If `true`, Tier 1 theorems are expected to end in `ProofStatus::Proven` except
    /// for the admitted budget controlled by `max_admitted_tier1`.
    pub require_tier1_proven: bool,
    /// Maximum number of Tier 1 theorems allowed in `Admitted` status.
    pub max_admitted_tier1: usize,
    /// Maximum number of Tier 2 theorems allowed in `Conjectured` status.
    pub max_conjectured_tier2: usize,
    /// Enables automatic promotion workflows when proof status becomes `Proven`.
    pub auto_promote_on_proof: bool,
    /// Expected review cadence in days for taxonomy maintenance.
    pub review_period_days: u64,
    /// Prefix used to namespace theorem IDs.
    pub namespace_prefix: String,
    /// Free-form metadata for ownership, policy versioning, and governance links.
    pub metadata: HashMap<String, String>,
}

/// Validation error emitted when a taxonomy configuration is malformed.
///
/// Keeping this as a struct (instead of an enum) makes error reporting straightforward
/// for frontends and APIs that display field-level diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TheoremTaxonomyValidationError {
    /// Name of the invalid field.
    pub field: String,
    /// Human-readable explanation of why the field failed validation.
    pub reason: String,
}

/// Aggregate portfolio statistics for theorem taxonomy state.
///
/// These metrics are intended for dashboards and CI guardrails. Values are derived from
/// theorem entries and do not mutate source data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TheoremTaxonomyStats {
    /// Total theorem entries observed.
    pub total_theorems: usize,
    /// Count of theorem entries by tier.
    pub by_tier: HashMap<TheoremTier, usize>,
    /// Percentage of theorems currently in `ProofStatus::Proven`.
    pub proven_pct: f64,
    /// Total number of theorems in `ProofStatus::Admitted`.
    pub admitted_count: usize,
    /// Mean dependency count across all theorem entries.
    pub avg_dependencies: f64,
    /// Family coverage percentage by theorem family.
    pub coverage_by_family: HashMap<TheoremFamily, f64>,
}

/// Creates default theorem taxonomy policy values for ETH2077.
///
/// Defaults are intentionally conservative:
/// - Tier 1 proofs are required.
/// - No Tier 1 admissions are allowed by default.
/// - Small conjecture budget is allowed for Tier 2.
/// - Review cadence is monthly.
/// - Namespace prefix is stable and explicit.
pub fn default_theorem_taxonomy_config() -> TheoremTaxonomyConfig {
    let mut metadata = HashMap::new();
    metadata.insert("taxonomy_version".to_string(), "1".to_string());
    metadata.insert(
        "owner".to_string(),
        "eth2077-formal-verification".to_string(),
    );

    TheoremTaxonomyConfig {
        require_tier1_proven: true,
        max_admitted_tier1: 0,
        max_conjectured_tier2: 2,
        auto_promote_on_proof: true,
        review_period_days: 30,
        namespace_prefix: "ETH2077-THM".to_string(),
        metadata,
    }
}

/// Validates theorem taxonomy policy settings.
///
/// Validation rules are designed to prevent ambiguous or unsafe gate policy:
/// - `namespace_prefix` must be non-empty after trimming.
/// - `namespace_prefix` may only contain ASCII alphanumeric, `_`, and `-`.
/// - `review_period_days` must be at least one day and not excessively large.
/// - If Tier 1 proofs are required, admitted budget for Tier 1 must be zero.
/// - Metadata keys must be non-empty and values must be non-empty after trimming.
///
/// The function collects all violations and returns them together.
pub fn validate_theorem_taxonomy_config(
    config: &TheoremTaxonomyConfig,
) -> Result<(), Vec<TheoremTaxonomyValidationError>> {
    let mut errors = Vec::new();

    if config.namespace_prefix.trim().is_empty() {
        errors.push(TheoremTaxonomyValidationError {
            field: "namespace_prefix".to_string(),
            reason: "namespace prefix must not be empty".to_string(),
        });
    }

    if !config
        .namespace_prefix
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        errors.push(TheoremTaxonomyValidationError {
            field: "namespace_prefix".to_string(),
            reason: "namespace prefix must use ASCII letters, digits, '_' or '-'".to_string(),
        });
    }

    if config.review_period_days == 0 {
        errors.push(TheoremTaxonomyValidationError {
            field: "review_period_days".to_string(),
            reason: "review period must be at least one day".to_string(),
        });
    }

    if config.review_period_days > 3650 {
        errors.push(TheoremTaxonomyValidationError {
            field: "review_period_days".to_string(),
            reason: "review period must not exceed 3650 days".to_string(),
        });
    }

    if config.require_tier1_proven && config.max_admitted_tier1 > 0 {
        errors.push(TheoremTaxonomyValidationError {
            field: "max_admitted_tier1".to_string(),
            reason: "must be zero when require_tier1_proven is enabled".to_string(),
        });
    }

    if config.max_conjectured_tier2 > 10_000 {
        errors.push(TheoremTaxonomyValidationError {
            field: "max_conjectured_tier2".to_string(),
            reason: "must be less than or equal to 10000".to_string(),
        });
    }

    for (key, value) in &config.metadata {
        if key.trim().is_empty() {
            errors.push(TheoremTaxonomyValidationError {
                field: "metadata".to_string(),
                reason: "metadata key must not be empty".to_string(),
            });
        }

        if value.trim().is_empty() {
            errors.push(TheoremTaxonomyValidationError {
                field: format!("metadata.{key}"),
                reason: "metadata value must not be empty".to_string(),
            });
        }

        if key.len() > 128 {
            errors.push(TheoremTaxonomyValidationError {
                field: "metadata".to_string(),
                reason: "metadata key length must be <= 128".to_string(),
            });
        }

        if value.len() > 4096 {
            errors.push(TheoremTaxonomyValidationError {
                field: format!("metadata.{key}"),
                reason: "metadata value length must be <= 4096".to_string(),
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Computes aggregate theorem taxonomy statistics.
///
/// The returned structure is designed to be directly serializable for dashboarding.
/// Coverage by family is represented as percentages of total theorem count.
pub fn compute_theorem_taxonomy_stats(theorems: &[TheoremEntry]) -> TheoremTaxonomyStats {
    let mut by_tier = HashMap::new();
    for tier in all_theorem_tiers() {
        by_tier.insert(tier, 0usize);
    }

    let mut family_counts = HashMap::new();
    for family in all_theorem_families() {
        family_counts.insert(family, 0usize);
    }

    let total_theorems = theorems.len();
    let mut proven_count = 0usize;
    let mut admitted_count = 0usize;
    let mut dependency_sum = 0usize;

    for theorem in theorems {
        if let Some(count) = by_tier.get_mut(&theorem.tier) {
            *count += 1;
        }

        if let Some(count) = family_counts.get_mut(&theorem.family) {
            *count += 1;
        }

        if theorem.proof_status == ProofStatus::Proven {
            proven_count += 1;
        }

        if theorem.proof_status == ProofStatus::Admitted {
            admitted_count += 1;
        }

        dependency_sum += theorem.dependencies.len();
    }

    let proven_pct = if total_theorems == 0 {
        0.0
    } else {
        (proven_count as f64 / total_theorems as f64) * 100.0
    };

    let avg_dependencies = if total_theorems == 0 {
        0.0
    } else {
        dependency_sum as f64 / total_theorems as f64
    };

    let mut coverage_by_family = HashMap::new();
    for family in all_theorem_families() {
        let count = family_counts.get(&family).copied().unwrap_or(0);
        let coverage = if total_theorems == 0 {
            0.0
        } else {
            (count as f64 / total_theorems as f64) * 100.0
        };
        coverage_by_family.insert(family, coverage);
    }

    TheoremTaxonomyStats {
        total_theorems,
        by_tier,
        proven_pct,
        admitted_count,
        avg_dependencies,
        coverage_by_family,
    }
}

/// Computes a deterministic SHA-256 commitment over taxonomy configuration.
///
/// Commitment behavior:
/// - Includes every policy field.
/// - Canonicalizes metadata map by sorting `(key, value)` pairs lexicographically.
/// - Uses length-prefixed encoding for variable-length strings.
/// - Returns lowercase hexadecimal digest.
///
/// This commitment is suitable for storing in CI artifacts, release notes, or signed
/// governance records where exact configuration identity must be reproducible.
pub fn compute_theorem_taxonomy_commitment(config: &TheoremTaxonomyConfig) -> String {
    let mut hasher = Sha256::new();

    hasher.update(b"eth2077-theorem-taxonomy-config-v1");
    hasher.update([u8::from(config.require_tier1_proven)]);
    hasher.update((config.max_admitted_tier1 as u64).to_be_bytes());
    hasher.update((config.max_conjectured_tier2 as u64).to_be_bytes());
    hasher.update([u8::from(config.auto_promote_on_proof)]);
    hasher.update(config.review_period_days.to_be_bytes());

    update_len_prefixed_string(&mut hasher, &config.namespace_prefix);

    let mut metadata_entries: Vec<(&String, &String)> = config.metadata.iter().collect();
    metadata_entries.sort_unstable_by(|(key_a, value_a), (key_b, value_b)| {
        key_a.cmp(key_b).then_with(|| value_a.cmp(value_b))
    });

    hasher.update((metadata_entries.len() as u64).to_be_bytes());
    for (key, value) in metadata_entries {
        update_len_prefixed_string(&mut hasher, key);
        update_len_prefixed_string(&mut hasher, value);
    }

    let digest = hasher.finalize();
    encode_hex_lower(&digest)
}

/// Updates a hasher with a big-endian length prefix followed by UTF-8 bytes.
fn update_len_prefixed_string(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

/// Encodes bytes as lowercase hexadecimal.
fn encode_hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(hex_nibble(byte >> 4));
        output.push(hex_nibble(byte & 0x0F));
    }
    output
}

/// Returns ASCII lowercase hex character for 0..=15.
fn hex_nibble(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + (value - 10)) as char,
        _ => '0',
    }
}

/// Returns all theorem tiers in canonical reporting order.
fn all_theorem_tiers() -> [TheoremTier; 6] {
    [
        TheoremTier::Tier1Critical,
        TheoremTier::Tier2Important,
        TheoremTier::Tier3Nice,
        TheoremTier::Tier1Blocking,
        TheoremTier::Tier2Regression,
        TheoremTier::Tier3Advisory,
    ]
}

/// Returns all theorem families in canonical reporting order.
fn all_theorem_families() -> [TheoremFamily; 6] {
    [
        TheoremFamily::SafetyInvariant,
        TheoremFamily::LivenessGuarantee,
        TheoremFamily::PerformanceBound,
        TheoremFamily::FairnessProperty,
        TheoremFamily::PrivacyPreserving,
        TheoremFamily::ResourceBound,
    ]
}
