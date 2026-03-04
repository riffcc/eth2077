//! Citadel migration planning primitives for ETH2077.
//!
//! This module defines strongly typed planning data for moving from placeholder
//! implementations to production-grade Citadel consensus modules. The focus is
//! operational planning instead of execution details: tracking phase placement,
//! risk posture, rollback strategy, dependency pressure, and quantitative plan
//! metrics.
//!
//! The API is intentionally compact:
//! - `MigrationStep` models one concrete migration task.
//! - `MigrationPlanConfig` models program-level policy and guardrails.
//! - `validate_migration_plan_config` enforces baseline sanity checks.
//! - `compute_migration_plan_stats` aggregates execution-readiness metrics.
//! - `compute_migration_plan_commitment` yields a deterministic SHA-256 digest
//!   that can be logged, signed, or referenced by external governance systems.
//!
//! Design notes:
//! - Configuration commitment generation is deterministic and resilient to
//!   `HashMap` insertion order by sorting metadata pairs before hashing.
//! - Plan statistics are deliberately phase-aware and risk-aware so planning
//!   dashboards can express both progress and exposure in one view.
//! - Validation emits field-scoped, human-readable reasons to support UI and CI
//!   surfaces without introducing additional error-type complexity.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// High-level lifecycle stage for one migration step.
///
/// The phase order is linear and expected to progress from early discovery to
/// production deployment. Even if some teams work iteratively, expressing steps
/// through this sequence provides a common planning language for governance,
/// risk review, and release coordination.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MigrationPhase {
    /// Scope identification and inventory of placeholders, assumptions, and
    /// target behavior for a Citadel module.
    Discovery,
    /// Feasibility analysis, architecture tradeoff review, and compatibility
    /// evaluation before code-level changes begin.
    Analysis,
    /// Core implementation work that replaces placeholder behavior with
    /// production-grade logic.
    Implementation,
    /// Functional, integration, and adversarial testing against migration
    /// acceptance criteria.
    Testing,
    /// Pre-production deployment in an environment representative of production
    /// workload and fault conditions.
    Staging,
    /// Production rollout with governance approval and on-call support.
    Production,
}

/// Coarse-grained risk classification used for policy and reporting.
///
/// This enum intentionally separates high-risk-but-movable items from fully
/// blocking items to support nuanced scheduling and escalation behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskLevel {
    /// Risk impact is effectively zero for planning purposes.
    Negligible,
    /// Limited impact; likely recoverable without major process disruption.
    Low,
    /// Meaningful impact; requires explicit mitigation planning.
    Medium,
    /// Elevated impact; typically requires additional controls and reviews.
    High,
    /// Severe impact with potential systemic consequences.
    Critical,
    /// Step cannot safely proceed without resolving blockers first.
    Blocking,
}

/// Rollback posture selected for a migration step.
///
/// Different modules can adopt different rollback mechanics depending on blast
/// radius, deployment topology, and feature isolation boundaries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RollbackStrategy {
    /// Rollback is triggered by automated health checks and predefined policy.
    Automatic,
    /// Rollback requires human approval from designated operators.
    ManualApproval,
    /// Rollback is implemented by disabling guarded features.
    FeatureFlag,
    /// Rollback is implemented by reverting traffic to a parallel environment.
    BlueGreen,
    /// Rollback is achieved by reducing rollout exposure through canary control.
    Canary,
    /// Rollback is not available; step is expected to be irreversible.
    NoRollback,
}

/// Dependency semantics for migration coordination.
///
/// This enum exists to classify dependency edges used by external schedulers,
/// dashboards, or policy engines. The current `MigrationStep` model stores only
/// dependency identifiers and does not yet encode kind directly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DependencyKind {
    /// Missing dependency blocks execution.
    HardBlock,
    /// Missing dependency introduces risk but execution may continue.
    SoftBlock,
    /// Dependency is informational and non-gating.
    Advisory,
    /// Dependency is only required for tests.
    TestOnly,
    /// Dependency is only required during build/compile flow.
    BuildTime,
    /// Dependency is required in runtime path.
    Runtime,
}

/// One migration task for a specific Citadel module.
///
/// The `id` is globally unique within a plan and used as the reference target
/// for entries in `dependencies`. Dependencies are modeled as identifier links
/// so planners can run simple graph-level checks without tightly coupling this
/// crate to graph-specific data structures.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MigrationStep {
    /// Unique step identifier (for example: `citadel-fork-choice-v2`).
    pub id: String,
    /// Consensus module name associated with this step.
    pub module_name: String,
    /// Lifecycle stage currently assigned to this step.
    pub phase: MigrationPhase,
    /// Current assessed risk level for execution of this step.
    pub risk: RiskLevel,
    /// Chosen rollback posture for this step.
    pub rollback: RollbackStrategy,
    /// Identifier list of prerequisite steps.
    pub dependencies: Vec<String>,
    /// Estimated implementation and validation effort in hours.
    pub estimated_hours: f64,
    /// Free-form metadata for governance IDs, owners, links, or tickets.
    pub metadata: HashMap<String, String>,
}

/// Plan-level policy and execution constraints.
///
/// This structure encodes governance defaults shared across all migration
/// steps, including acceptable risk posture, rollout control, and minimum test
/// confidence expectations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MigrationPlanConfig {
    /// Maximum number of migration steps allowed to execute in parallel.
    pub max_parallel_migrations: usize,
    /// Highest risk level accepted by default without escalation.
    pub risk_tolerance: RiskLevel,
    /// Whether every step must define a practical rollback path.
    pub require_rollback: bool,
    /// Minimum time budget for staging soak, in hours.
    pub staging_duration_hours: f64,
    /// Minimum required test coverage percentage for rollout readiness.
    pub test_coverage_min_pct: f64,
    /// Whether explicit approval is required before production promotion.
    pub approval_required: bool,
    /// Free-form metadata for approvers, versioning, or external references.
    pub metadata: HashMap<String, String>,
}

/// Field-scoped validation error emitted by configuration checks.
///
/// Validation is intentionally non-failing-by-default: all discovered issues are
/// returned at once as a vector so CI and UIs can present a complete report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MigrationPlanValidationError {
    /// Logical field name that failed validation.
    pub field: String,
    /// Human-readable explanation of the failure.
    pub reason: String,
}

/// Aggregated planning metrics over a list of migration steps.
///
/// The statistics provide a compact operational snapshot suitable for dashboards
/// and governance review artifacts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MigrationPlanStats {
    /// Total number of migration steps analyzed.
    pub total_steps: usize,
    /// Count of steps grouped by phase label.
    pub by_phase: HashMap<String, usize>,
    /// Mean numeric risk score across all steps.
    pub avg_risk_score: f64,
    /// Sum of non-negative estimated hours across all steps.
    pub total_estimated_hours: f64,
    /// Number of steps currently blocked by risk or unresolved dependencies.
    pub blocked_count: usize,
    /// Approximate plan completion percentage in range `[0, 100]`.
    pub completion_pct: f64,
}

/// Returns a conservative default migration policy.
///
/// Defaults are biased toward safety:
/// - moderate parallelism,
/// - rollback required,
/// - explicit approval required,
/// - strong minimum test coverage.
///
/// Callers are expected to adapt metadata and strictness to their operating
/// environment.
pub fn default_migration_plan_config() -> MigrationPlanConfig {
    let mut metadata = HashMap::new();
    metadata.insert("program".to_string(), "ETH2077-Citadel".to_string());
    metadata.insert("owner".to_string(), "consensus-core".to_string());
    metadata.insert("mode".to_string(), "safety-first".to_string());

    MigrationPlanConfig {
        max_parallel_migrations: 2,
        risk_tolerance: RiskLevel::Medium,
        require_rollback: true,
        staging_duration_hours: 72.0,
        test_coverage_min_pct: 90.0,
        approval_required: true,
        metadata,
    }
}

/// Validates migration planning policy for consistency and operational safety.
///
/// Validation criteria are intentionally practical rather than exhaustive:
/// - structural bounds (`max_parallel_migrations > 0`),
/// - percentage bounds (`test_coverage_min_pct` in `[0, 100]`),
/// - duration sanity (`staging_duration_hours >= 0`),
/// - guardrail coupling (for risky modes, require rollback/approval).
///
/// Returns `Ok(())` if no issues are found; otherwise returns all failures.
pub fn validate_migration_plan_config(
    config: &MigrationPlanConfig,
) -> Result<(), Vec<MigrationPlanValidationError>> {
    let mut errors = Vec::new();

    if config.max_parallel_migrations == 0 {
        errors.push(MigrationPlanValidationError {
            field: "max_parallel_migrations".to_string(),
            reason: "must be greater than zero".to_string(),
        });
    }

    if config.staging_duration_hours < 0.0 {
        errors.push(MigrationPlanValidationError {
            field: "staging_duration_hours".to_string(),
            reason: "must be non-negative".to_string(),
        });
    }

    if config.test_coverage_min_pct < 0.0 || config.test_coverage_min_pct > 100.0 {
        errors.push(MigrationPlanValidationError {
            field: "test_coverage_min_pct".to_string(),
            reason: "must be within [0, 100]".to_string(),
        });
    }

    if config.require_rollback
        && matches!(
            config.risk_tolerance,
            RiskLevel::Critical | RiskLevel::Blocking
        )
        && config.staging_duration_hours < 24.0
    {
        errors.push(MigrationPlanValidationError {
            field: "staging_duration_hours".to_string(),
            reason: "critical/blocking tolerance requires at least 24h staging".to_string(),
        });
    }

    if !config.require_rollback
        && matches!(
            config.risk_tolerance,
            RiskLevel::High | RiskLevel::Critical | RiskLevel::Blocking
        )
    {
        errors.push(MigrationPlanValidationError {
            field: "require_rollback".to_string(),
            reason: "cannot be false when risk tolerance is high or above".to_string(),
        });
    }

    if !config.approval_required
        && matches!(
            config.risk_tolerance,
            RiskLevel::Critical | RiskLevel::Blocking
        )
    {
        errors.push(MigrationPlanValidationError {
            field: "approval_required".to_string(),
            reason: "cannot be false when risk tolerance is critical or blocking".to_string(),
        });
    }

    if config.max_parallel_migrations > 8 {
        errors.push(MigrationPlanValidationError {
            field: "max_parallel_migrations".to_string(),
            reason: "values above 8 are considered unsafe for consensus migration".to_string(),
        });
    }

    if config.metadata.keys().any(|key| key.trim().is_empty()) {
        errors.push(MigrationPlanValidationError {
            field: "metadata".to_string(),
            reason: "metadata keys must not be empty".to_string(),
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Computes high-level migration statistics for a step list.
///
/// The computation includes:
/// - phase distribution,
/// - average risk score,
/// - total effort estimate,
/// - blocked step count,
/// - coarse completion percentage.
///
/// Completion is a weighted phase average where later phases represent greater
/// progress toward production readiness.
pub fn compute_migration_plan_stats(steps: &[MigrationStep]) -> MigrationPlanStats {
    let total_steps = steps.len();

    let mut by_phase = HashMap::new();
    let mut total_risk_score = 0.0;
    let mut total_estimated_hours = 0.0;
    let mut blocked_count = 0_usize;
    let mut completion_total = 0.0;

    let mut ids = HashMap::new();
    for step in steps {
        ids.insert(step.id.as_str(), true);
    }

    for step in steps {
        let label = phase_label(&step.phase).to_string();
        let count = by_phase.entry(label).or_insert(0);
        *count += 1;

        total_risk_score += risk_score(&step.risk);
        total_estimated_hours += step.estimated_hours.max(0.0);
        completion_total += phase_completion_weight(&step.phase);

        let unresolved_dependency = step
            .dependencies
            .iter()
            .any(|dep| !ids.contains_key(dep.as_str()));
        let blocked_by_risk = step.risk == RiskLevel::Blocking;

        if unresolved_dependency || blocked_by_risk {
            blocked_count += 1;
        }
    }

    let avg_risk_score = if total_steps == 0 {
        0.0
    } else {
        total_risk_score / total_steps as f64
    };

    let completion_pct = if total_steps == 0 {
        0.0
    } else {
        (completion_total / total_steps as f64) * 100.0
    };

    MigrationPlanStats {
        total_steps,
        by_phase,
        avg_risk_score,
        total_estimated_hours,
        blocked_count,
        completion_pct,
    }
}

/// Computes a deterministic SHA-256 commitment for plan configuration.
///
/// The commitment is suitable for audit trails, governance snapshots, and
/// reproducibility checks. Metadata is hashed in sorted key order to avoid
/// nondeterminism from `HashMap` iteration order.
pub fn compute_migration_plan_commitment(config: &MigrationPlanConfig) -> String {
    let mut hasher = Sha256::new();

    hasher.update(format!(
        "max_parallel_migrations={}|risk_tolerance={}|require_rollback={}|staging_duration_hours={:.6}|test_coverage_min_pct={:.6}|approval_required={}|",
        config.max_parallel_migrations,
        risk_label(&config.risk_tolerance),
        config.require_rollback,
        config.staging_duration_hours,
        config.test_coverage_min_pct,
        config.approval_required,
    ));

    let mut metadata_pairs: Vec<(&String, &String)> = config.metadata.iter().collect();
    metadata_pairs.sort_by(|(ka, va), (kb, vb)| ka.cmp(kb).then(va.cmp(vb)));

    for (key, value) in metadata_pairs {
        hasher.update(key.as_bytes());
        hasher.update(b"=");
        hasher.update(value.as_bytes());
        hasher.update(b";");
    }

    let digest = hasher.finalize();
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push_str(&format!("{:02x}", byte));
    }
    output
}

/// Returns a canonical label for a migration phase.
fn phase_label(phase: &MigrationPhase) -> &'static str {
    match phase {
        MigrationPhase::Discovery => "Discovery",
        MigrationPhase::Analysis => "Analysis",
        MigrationPhase::Implementation => "Implementation",
        MigrationPhase::Testing => "Testing",
        MigrationPhase::Staging => "Staging",
        MigrationPhase::Production => "Production",
    }
}

/// Converts a risk level to a stable scoring scale for aggregation.
///
/// The chosen scale is linear and intentionally simple so downstream reporting
/// can reason about changes without hidden weighting rules.
fn risk_score(risk: &RiskLevel) -> f64 {
    match risk {
        RiskLevel::Negligible => 0.0,
        RiskLevel::Low => 1.0,
        RiskLevel::Medium => 2.0,
        RiskLevel::High => 3.0,
        RiskLevel::Critical => 4.0,
        RiskLevel::Blocking => 5.0,
    }
}

/// Returns progress weight for each phase in the migration lifecycle.
fn phase_completion_weight(phase: &MigrationPhase) -> f64 {
    match phase {
        MigrationPhase::Discovery => 0.10,
        MigrationPhase::Analysis => 0.25,
        MigrationPhase::Implementation => 0.50,
        MigrationPhase::Testing => 0.70,
        MigrationPhase::Staging => 0.90,
        MigrationPhase::Production => 1.00,
    }
}

/// Returns canonical risk label used by commitment hashing.
fn risk_label(risk: &RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Negligible => "Negligible",
        RiskLevel::Low => "Low",
        RiskLevel::Medium => "Medium",
        RiskLevel::High => "High",
        RiskLevel::Critical => "Critical",
        RiskLevel::Blocking => "Blocking",
    }
}
