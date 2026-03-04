//! Runtime integration types for migrated Citadel modules in ETH2077.
//!
//! This module defines the core data model used to wire migrated modules into
//! a running ETH2077 node. The goal is explicit traceability:
//!
//! 1. Each runtime module records where it is mounted in the runtime.
//! 2. Each module references proof traces that tie implementation artifacts to
//!    theorem IDs, lemmas, audits, and coverage reports.
//! 3. The runtime configuration is validated before activation so integration
//!    policy cannot silently degrade security constraints.
//! 4. Aggregate statistics make it cheap to understand migration status.
//! 5. A deterministic commitment hash provides tamper-evident configuration
//!    snapshots suitable for CI logs, governance payloads, or release metadata.
//!
//! The structures are designed to stay serialization-friendly and ergonomic for
//! both CLI workflows and control-plane APIs.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Domain separator used by [`compute_runtime_integration_commitment`].
///
/// A domain tag prevents accidental cross-protocol hash collisions where
/// unrelated modules might serialize similarly.
const RUNTIME_INTEGRATION_HASH_DOMAIN: &[u8] = b"ETH2077-RUNTIME-INTEGRATION-V1";

/// Upper bound used to reject clearly unrealistic shim-module limits.
///
/// This constant is intentionally conservative and focused on preventing
/// accidental misconfiguration (for example an unbounded migration plan).
const MAX_REASONABLE_SHIM_MODULES: usize = 1_024;

/// Integration lifecycle stage for a migrated module.
///
/// The phases form a practical progression used by orchestration dashboards and
/// release controls:
///
/// - Early phases indicate registration and static setup.
/// - Middle phases indicate wiring and evidence generation.
/// - Late phases indicate production activation or retirement.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum IntegrationPhase {
    /// Module is known to the runtime registry but not yet configured.
    Registered,
    /// Module parameters are set and basic wiring prerequisites are satisfied.
    Configured,
    /// Module endpoints are connected into the selected runtime slot.
    Wired,
    /// Module passed integration and compatibility testing.
    Tested,
    /// Module is active in runtime decision paths.
    Activated,
    /// Module is retained for reference but no longer part of live operation.
    Deprecated,
}

/// Kind of traceable proof evidence attached to a module.
///
/// Each variant captures a different assurance mechanism linking the migrated
/// implementation to formal or empirical evidence.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProofTraceKind {
    /// Direct theorem identifier link proving an expected property.
    TheoremLink,
    /// Supporting lemma reference used to justify a theorem step.
    LemmaReference,
    /// Invariant check produced by static or runtime verification.
    InvariantCheck,
    /// Benchmark output proving target performance constraints.
    BenchmarkEvidence,
    /// External or internal audit report linked to code artifacts.
    AuditReport,
    /// Test coverage artifact that supports integration confidence.
    TestCoverage,
}

/// Runtime subsystem where a migrated module is mounted.
///
/// Slots represent major execution domains in a validator/client runtime.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RuntimeSlot {
    /// Consensus-path logic and fork-choice/state agreement hooks.
    Consensus,
    /// Transaction and state transition execution pipeline.
    Execution,
    /// Peer-to-peer and protocol networking stack.
    Networking,
    /// State, history, or blob persistence subsystem.
    Storage,
    /// Admission and ordering queue for pending transactions.
    Mempool,
    /// Validator duties and associated safety logic.
    Validator,
}

/// Compatibility posture of the migrated module.
///
/// Compatibility feeds release policy and rollout automation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CompatibilityLevel {
    /// Feature parity and behavior are fully aligned with target runtime.
    Full,
    /// Most behavior is native, with small non-critical divergences.
    Partial,
    /// Operates through a compatibility shim layer.
    Shim,
    /// Behavior is reproduced by an emulator or translation path.
    Emulated,
    /// Module functions but with known degradation or feature loss.
    Degraded,
    /// Cannot satisfy the target runtime contract.
    Incompatible,
}

/// Trace entry linking an artifact to theorem-oriented evidence.
///
/// A trace is intentionally small and pointer-like; the heavy artifact remains
/// in object storage or repository paths while this structure carries stable
/// references.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProofTrace {
    /// Canonical theorem or proof registry identifier.
    pub theorem_id: String,
    /// Classification of the trace evidence.
    pub kind: ProofTraceKind,
    /// Path or URI to the artifact that contains evidence.
    pub artifact_path: String,
    /// Indicates if this trace has completed verification checks.
    pub verified: bool,
}

/// Runtime-mounted module record for migrated Citadel components.
///
/// This record is the primary integration unit used by dashboards, validators,
/// and release automation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeModule {
    /// Stable integration identifier for this module entry.
    pub id: String,
    /// Human-readable or canonical module name.
    pub module_name: String,
    /// Current integration phase.
    pub phase: IntegrationPhase,
    /// Runtime subsystem where the module is connected.
    pub slot: RuntimeSlot,
    /// Compatibility classification for rollout policy decisions.
    pub compatibility: CompatibilityLevel,
    /// Proof traces that connect this module to formal or empirical evidence.
    pub proof_traces: Vec<ProofTrace>,
    /// IDs of modules that must be present before activation.
    pub dependencies: Vec<String>,
    /// Additional metadata for operators and automation.
    pub metadata: HashMap<String, String>,
}

/// Policy controls for runtime integration and activation.
///
/// This configuration is expected to be validated with
/// [`validate_runtime_integration_config`] before use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeIntegrationConfig {
    /// Maximum number of modules allowed to run in `Shim` compatibility mode.
    pub max_shim_modules: usize,
    /// Whether every module is required to carry at least one proof trace.
    pub require_proof_traces: bool,
    /// Minimum accepted compatibility level for integration workflows.
    pub min_compatibility: CompatibilityLevel,
    /// Whether tested modules may be auto-promoted to `Activated`.
    pub auto_activate: bool,
    /// Required minimum test coverage percentage in `[0.0, 100.0]`.
    pub test_coverage_min_pct: f64,
    /// Whether integration orchestration should rollback on failure.
    pub rollback_on_failure: bool,
    /// Additional integration metadata used in policy and reporting.
    pub metadata: HashMap<String, String>,
}

/// Validation issue produced when checking a runtime integration config.
///
/// The `field` identifies the offending field, while `reason` provides a
/// machine-readable explanation suitable for UI or API propagation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeIntegrationValidationError {
    /// Name of the field that failed validation.
    pub field: String,
    /// Reason describing why the value is invalid.
    pub reason: String,
}

/// Aggregate statistics summarizing current runtime integration state.
///
/// The metrics are intentionally compact and dashboard-friendly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeIntegrationStats {
    /// Total module entries included in the computation.
    pub total_modules: usize,
    /// Module counts grouped by integration phase.
    pub by_phase: HashMap<IntegrationPhase, usize>,
    /// Percentage of modules that contain at least one proof trace.
    pub traced_pct: f64,
    /// Average number of proof traces per module.
    pub avg_proof_traces: f64,
    /// Compatibility score expressed on a `0.0..=100.0` scale.
    pub compatibility_score: f64,
    /// Number of modules currently in the `Activated` phase.
    pub activated_count: usize,
}

/// Constructs the default runtime integration policy.
///
/// Defaults are designed to be strict enough for production-like workflows:
///
/// - Proof traces are required.
/// - Shim-module usage is bounded.
/// - Coverage requirements are high but achievable.
/// - Rollback is enabled on failure.
pub fn default_runtime_integration_config() -> RuntimeIntegrationConfig {
    RuntimeIntegrationConfig {
        max_shim_modules: 8,
        require_proof_traces: true,
        min_compatibility: CompatibilityLevel::Shim,
        auto_activate: false,
        test_coverage_min_pct: 90.0,
        rollback_on_failure: true,
        metadata: HashMap::new(),
    }
}

/// Validates runtime integration policy settings.
///
/// Validation aims to catch dangerous or self-contradictory settings before
/// orchestration starts.
///
/// Checks performed:
///
/// - `max_shim_modules` must not exceed a conservative upper bound.
/// - `test_coverage_min_pct` must be finite and in `[0.0, 100.0]`.
/// - `auto_activate` cannot be used with very weak compatibility floors.
/// - `require_proof_traces` conflicts with `Incompatible` minimum acceptance.
/// - `min_compatibility = Shim` requires at least one shim slot.
/// - Metadata keys and values must not be blank after trimming.
pub fn validate_runtime_integration_config(
    config: &RuntimeIntegrationConfig,
) -> Result<(), Vec<RuntimeIntegrationValidationError>> {
    let mut errors = Vec::new();

    if config.max_shim_modules > MAX_REASONABLE_SHIM_MODULES {
        errors.push(RuntimeIntegrationValidationError {
            field: "max_shim_modules".to_string(),
            reason: format!(
                "must be <= {MAX_REASONABLE_SHIM_MODULES}, got {}",
                config.max_shim_modules
            ),
        });
    }

    if !config.test_coverage_min_pct.is_finite()
        || config.test_coverage_min_pct < 0.0
        || config.test_coverage_min_pct > 100.0
    {
        errors.push(RuntimeIntegrationValidationError {
            field: "test_coverage_min_pct".to_string(),
            reason: "must be finite and within 0.0..=100.0".to_string(),
        });
    }

    if config.auto_activate
        && matches!(
            config.min_compatibility,
            CompatibilityLevel::Degraded | CompatibilityLevel::Incompatible
        )
    {
        errors.push(RuntimeIntegrationValidationError {
            field: "auto_activate".to_string(),
            reason: "cannot auto-activate when minimum compatibility is Degraded/Incompatible"
                .to_string(),
        });
    }

    if config.require_proof_traces
        && matches!(config.min_compatibility, CompatibilityLevel::Incompatible)
    {
        errors.push(RuntimeIntegrationValidationError {
            field: "min_compatibility".to_string(),
            reason: "Incompatible minimum conflicts with required proof traces".to_string(),
        });
    }

    if matches!(config.min_compatibility, CompatibilityLevel::Shim) && config.max_shim_modules == 0
    {
        errors.push(RuntimeIntegrationValidationError {
            field: "max_shim_modules".to_string(),
            reason: "must be > 0 when minimum compatibility is Shim".to_string(),
        });
    }

    for (key, value) in &config.metadata {
        if key.trim().is_empty() {
            errors.push(RuntimeIntegrationValidationError {
                field: "metadata".to_string(),
                reason: "metadata keys must not be blank".to_string(),
            });
            break;
        }
        if value.trim().is_empty() {
            errors.push(RuntimeIntegrationValidationError {
                field: format!("metadata.{key}"),
                reason: "metadata values must not be blank".to_string(),
            });
            break;
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Computes summary metrics across runtime module integration entries.
///
/// Metric definitions:
///
/// - `total_modules`: number of modules in the input slice.
/// - `by_phase`: phase-wise counts for quick rollout visibility.
/// - `traced_pct`: share of modules with at least one attached proof trace.
/// - `avg_proof_traces`: mean trace count per module.
/// - `compatibility_score`: weighted average compatibility mapped to `0..100`.
/// - `activated_count`: modules currently marked as activated.
///
/// Empty input returns zero-valued metrics and an empty `by_phase` map.
pub fn compute_runtime_integration_stats(modules: &[RuntimeModule]) -> RuntimeIntegrationStats {
    let total_modules = modules.len();
    let mut by_phase: HashMap<IntegrationPhase, usize> = HashMap::new();
    let mut traced_modules = 0usize;
    let mut total_proof_traces = 0usize;
    let mut compatibility_acc = 0.0f64;
    let mut activated_count = 0usize;

    for module in modules {
        by_phase
            .entry(module.phase)
            .and_modify(|count| *count += 1)
            .or_insert(1);

        if !module.proof_traces.is_empty() {
            traced_modules += 1;
        }
        total_proof_traces += module.proof_traces.len();
        compatibility_acc += compatibility_weight(module.compatibility);

        if module.phase == IntegrationPhase::Activated {
            activated_count += 1;
        }
    }

    let traced_pct = if total_modules == 0 {
        0.0
    } else {
        (traced_modules as f64 / total_modules as f64) * 100.0
    };

    let avg_proof_traces = if total_modules == 0 {
        0.0
    } else {
        total_proof_traces as f64 / total_modules as f64
    };

    let compatibility_score = if total_modules == 0 {
        0.0
    } else {
        (compatibility_acc / total_modules as f64) * 100.0
    };

    RuntimeIntegrationStats {
        total_modules,
        by_phase,
        traced_pct,
        avg_proof_traces,
        compatibility_score,
        activated_count,
    }
}

/// Computes a deterministic SHA-256 commitment for integration configuration.
///
/// The resulting string is lowercase hexadecimal and can be logged or stored as
/// a stable digest of rollout policy.
///
/// Serialization notes:
///
/// - Primitive fields are encoded in fixed order.
/// - Enum values are encoded via explicit discriminants.
/// - Metadata is hashed in sorted key/value order for determinism.
pub fn compute_runtime_integration_commitment(config: &RuntimeIntegrationConfig) -> String {
    let mut hasher = Sha256::new();

    hasher.update(RUNTIME_INTEGRATION_HASH_DOMAIN);
    hasher.update((config.max_shim_modules as u64).to_be_bytes());
    hasher.update([bool_to_u8(config.require_proof_traces)]);
    hasher.update([compatibility_discriminant(config.min_compatibility)]);
    hasher.update([bool_to_u8(config.auto_activate)]);
    hasher.update(config.test_coverage_min_pct.to_bits().to_be_bytes());
    hasher.update([bool_to_u8(config.rollback_on_failure)]);

    let mut metadata_entries: Vec<(&String, &String)> = config.metadata.iter().collect();
    metadata_entries
        .sort_unstable_by(|left, right| left.0.cmp(right.0).then_with(|| left.1.cmp(right.1)));

    hasher.update((metadata_entries.len() as u64).to_be_bytes());
    for (key, value) in metadata_entries {
        hasher.update((key.len() as u64).to_be_bytes());
        hasher.update(key.as_bytes());
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }

    digest_to_hex(&hasher.finalize())
}

/// Converts a compatibility level to a normalized scoring weight.
///
/// Weights are chosen for coarse operational meaning rather than strict
/// statistical modeling.
fn compatibility_weight(level: CompatibilityLevel) -> f64 {
    match level {
        CompatibilityLevel::Full => 1.00,
        CompatibilityLevel::Partial => 0.85,
        CompatibilityLevel::Shim => 0.70,
        CompatibilityLevel::Emulated => 0.50,
        CompatibilityLevel::Degraded => 0.25,
        CompatibilityLevel::Incompatible => 0.00,
    }
}

/// Encodes compatibility enum variants as stable byte discriminants.
fn compatibility_discriminant(level: CompatibilityLevel) -> u8 {
    match level {
        CompatibilityLevel::Full => 0,
        CompatibilityLevel::Partial => 1,
        CompatibilityLevel::Shim => 2,
        CompatibilityLevel::Emulated => 3,
        CompatibilityLevel::Degraded => 4,
        CompatibilityLevel::Incompatible => 5,
    }
}

/// Maps a boolean flag to a single-byte representation for hashing.
fn bool_to_u8(value: bool) -> u8 {
    if value {
        1
    } else {
        0
    }
}

/// Converts a SHA-256 digest into lowercase hexadecimal.
fn digest_to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(hex_nibble_to_char(byte >> 4));
        output.push(hex_nibble_to_char(byte & 0x0f));
    }
    output
}

/// Encodes a half-byte into a lowercase hexadecimal character.
fn hex_nibble_to_char(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => '0',
    }
}
