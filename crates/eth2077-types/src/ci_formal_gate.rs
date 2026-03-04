//! CI formal gate types and evaluators for ETH2077.
//!
//! This module models CI policy for formal verification debt in critical paths.
//! It tracks placeholder-proof debt (`axiom`, `sorry`, etc.), validates policy
//! config, computes aggregate debt statistics, and emits CI-friendly verdicts.
//!
//! Core behavior:
//! - Tier-aware counting for critical-path debt.
//! - Exemption-aware counting by debt ID.
//! - Strict handling for release-candidate branches.
//! - Deterministic commitment hashing for policy attestation.
//!
//! Design goals:
//! - Explicit serializable domain types.
//! - Deterministic, side-effect-free computation.
//! - Complete validation feedback in a single pass.
//! - Stable ordering in hashes and blocked-module output.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Gate execution mode controlling how debt thresholds affect CI outcomes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GateMode {
    /// Track only; no build-impacting signal.
    Advisory,
    /// Warn when thresholds are exceeded.
    Warning,
    /// Block when thresholds are exceeded.
    Blocking,
    /// Zero-tolerance mode for any non-exempt debt.
    Strict,
    /// Release-candidate mode; supports strict branch enforcement.
    ReleaseCandidate,
    /// Temporary exemption mode during incidents.
    Emergency,
}

/// Placeholder debt classification emitted by proof parsers and scanners.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlaceholderDebtKind {
    /// Unproven assumption admitted as an axiom.
    Axiom,
    /// Lean-style placeholder proof marker.
    Sorry,
    /// Generic admission marker.
    Admit,
    /// Stubbed proof body.
    Stub,
    /// TODO-style proof placeholder.
    TodoProof,
    /// Intentionally deferred lemma.
    DeferredLemma,
}

/// Critical-path tier for each debt record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CriticalPathTier {
    /// Safety-critical path.
    Tier1Safety,
    /// Liveness-critical path.
    Tier2Liveness,
    /// Performance-critical path.
    Tier3Performance,
    /// Convenience path.
    Tier4Convenience,
    /// Optional feature path.
    Tier5Optional,
    /// Deprecated compatibility path.
    Tier6Deprecated,
}

/// Final gate outcome consumed by CI and release tooling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GateVerdict {
    /// Fully compliant with policy.
    Pass,
    /// Non-zero debt exists but policy limits are respected.
    ConditionalPass,
    /// Non-blocking failure.
    SoftFail,
    /// Blocking failure.
    HardFail,
    /// Config invalid; trusted evaluation not possible.
    Error,
    /// Gate intentionally exempted by mode.
    Exempted,
}

/// One placeholder-proof debt record used by gate computation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaceholderDebt {
    /// Stable debt ID used by exemptions and dashboards.
    pub id: String,
    /// Placeholder kind.
    pub kind: PlaceholderDebtKind,
    /// Module path where debt resides.
    pub module_path: String,
    /// Critical-path tier.
    pub tier: CriticalPathTier,
    /// Commit where debt was introduced.
    pub introduced_commit: String,
    /// Debt age in whole days.
    pub age_days: u64,
    /// Extension metadata (owner, ticket, provenance, notes).
    pub metadata: HashMap<String, String>,
}

/// CI formal gate policy configuration.
///
/// Threshold semantics:
/// - `max_tier1_debt`: budget for tier-1 safety debt.
/// - `max_tier2_debt`: budget for tier-2 liveness debt.
/// - `max_total_debt`: aggregate budget across all tiers.
///
/// Release strictness:
/// - if `mode == ReleaseCandidate` and `strict_on_release_branch == true`,
///   any non-exempt debt yields `HardFail`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CiFormalGateConfig {
    pub mode: GateMode,
    pub max_tier1_debt: usize,
    pub max_tier2_debt: usize,
    pub max_total_debt: usize,
    pub strict_on_release_branch: bool,
    pub exemption_list: Vec<String>,
    pub metadata: HashMap<String, String>,
}

/// Validation error emitted by config checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CiFormalGateValidationError {
    pub field: String,
    pub reason: String,
}

/// Aggregate formal-gate statistics for one evaluation window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CiFormalGateStats {
    /// Total non-exempt debt count.
    pub total_debt: usize,
    /// Non-exempt tier-1 count.
    pub tier1_count: usize,
    /// Non-exempt tier-2 count.
    pub tier2_count: usize,
    /// Non-exempt tier-3 count.
    pub tier3_count: usize,
    /// Mode-aware verdict.
    pub verdict: GateVerdict,
    /// Oldest non-exempt debt in days.
    pub oldest_debt_days: u64,
    /// Sorted unique module paths that explain soft/hard failures.
    pub blocked_modules: Vec<String>,
}

/// Returns ETH2077 default formal gate policy.
///
/// The default profile is practical but strict where it matters:
/// - blocking mode,
/// - zero tier-1 budget,
/// - bounded tier-2/total budgets,
/// - strict release-candidate enforcement enabled,
/// - no exemptions by default.
pub fn default_ci_formal_gate_config() -> CiFormalGateConfig {
    let mut metadata = HashMap::new();
    metadata.insert("policy_profile".to_string(), "eth2077-formal-gate-v1".to_string());
    metadata.insert("owner".to_string(), "eth2077-core".to_string());

    CiFormalGateConfig {
        mode: GateMode::Blocking,
        max_tier1_debt: 0,
        max_tier2_debt: 2,
        max_total_debt: 6,
        strict_on_release_branch: true,
        exemption_list: Vec::new(),
        metadata,
    }
}

/// Validates `CiFormalGateConfig` and reports all discovered issues.
///
/// Rules:
/// - `max_total_debt >= max_tier1_debt`.
/// - `max_total_debt >= max_tier2_debt`.
/// - strict and release-candidate modes require zero budgets.
/// - strict and release-candidate modes disallow exemptions.
/// - release-candidate mode requires `strict_on_release_branch = true`.
/// - emergency mode requires `strict_on_release_branch = false`.
/// - exemption entries must be non-empty and unique after trimming.
/// - metadata keys/values must not be empty after trimming.
pub fn validate_ci_formal_gate_config(
    config: &CiFormalGateConfig,
) -> Result<(), Vec<CiFormalGateValidationError>> {
    let mut errors = Vec::new();

    if config.max_total_debt < config.max_tier1_debt {
        push_validation_error(&mut errors, "max_total_debt", "must be greater than or equal to max_tier1_debt");
    }
    if config.max_total_debt < config.max_tier2_debt {
        push_validation_error(&mut errors, "max_total_debt", "must be greater than or equal to max_tier2_debt");
    }

    if matches!(config.mode, GateMode::Strict | GateMode::ReleaseCandidate) {
        if config.max_tier1_debt != 0 {
            push_validation_error(&mut errors, "max_tier1_debt", "must be zero in strict or release-candidate mode");
        }
        if config.max_tier2_debt != 0 {
            push_validation_error(&mut errors, "max_tier2_debt", "must be zero in strict or release-candidate mode");
        }
        if config.max_total_debt != 0 {
            push_validation_error(&mut errors, "max_total_debt", "must be zero in strict or release-candidate mode");
        }
        if !config.exemption_list.is_empty() {
            push_validation_error(&mut errors, "exemption_list", "must be empty in strict or release-candidate mode");
        }
    }

    if matches!(config.mode, GateMode::ReleaseCandidate) && !config.strict_on_release_branch {
        push_validation_error(&mut errors, "strict_on_release_branch", "must be true when mode is ReleaseCandidate");
    }
    if matches!(config.mode, GateMode::Emergency) && config.strict_on_release_branch {
        push_validation_error(&mut errors, "strict_on_release_branch", "must be false in Emergency mode");
    }

    let mut seen_exemptions: HashMap<String, usize> = HashMap::new();
    for exemption in &config.exemption_list {
        let normalized = exemption.trim();
        if normalized.is_empty() {
            push_validation_error(&mut errors, "exemption_list", "entries must not be empty or whitespace");
            continue;
        }
        let seen = seen_exemptions.entry(normalized.to_string()).or_insert(0);
        *seen += 1;
        if *seen > 1 {
            push_validation_error(&mut errors, "exemption_list", &format!("duplicate exemption id `{normalized}`"));
        }
    }

    for (key, value) in &config.metadata {
        if key.trim().is_empty() {
            push_validation_error(&mut errors, "metadata", "metadata keys must not be empty or whitespace");
        }
        if value.trim().is_empty() {
            push_validation_error(&mut errors, "metadata", &format!("metadata value for key `{key}` must not be empty"));
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Computes aggregate counts and verdict for formal-gate debt.
///
/// Notes:
/// - Exempt debt IDs are filtered before counting.
/// - Counters are returned even when config is invalid.
/// - Invalid config forces `GateVerdict::Error`.
/// - `blocked_modules` is only populated for `SoftFail` and `HardFail`.
pub fn compute_ci_formal_gate_stats(
    debts: &[PlaceholderDebt],
    config: &CiFormalGateConfig,
) -> CiFormalGateStats {
    let exemptions = build_exemption_lookup(&config.exemption_list);

    let mut total_debt = 0usize;
    let mut tier1_count = 0usize;
    let mut tier2_count = 0usize;
    let mut tier3_count = 0usize;
    let mut oldest_debt_days = 0u64;

    let mut all_modules = Vec::new();
    let mut tier1_modules = Vec::new();
    let mut tier2_modules = Vec::new();

    for debt in debts {
        if is_exempt(&debt.id, &exemptions) {
            continue;
        }

        total_debt += 1;
        oldest_debt_days = oldest_debt_days.max(debt.age_days);
        push_unique(&mut all_modules, debt.module_path.clone());

        match debt.tier {
            CriticalPathTier::Tier1Safety => {
                tier1_count += 1;
                push_unique(&mut tier1_modules, debt.module_path.clone());
            }
            CriticalPathTier::Tier2Liveness => {
                tier2_count += 1;
                push_unique(&mut tier2_modules, debt.module_path.clone());
            }
            CriticalPathTier::Tier3Performance => {
                tier3_count += 1;
            }
            CriticalPathTier::Tier4Convenience
            | CriticalPathTier::Tier5Optional
            | CriticalPathTier::Tier6Deprecated => {}
        }
    }

    let config_invalid = validate_ci_formal_gate_config(config).is_err();
    let verdict = compute_verdict(config, total_debt, tier1_count, tier2_count, config_invalid);

    let blocked_modules = compute_blocked_modules(
        config,
        &verdict,
        total_debt,
        tier1_count,
        tier2_count,
        &all_modules,
        &tier1_modules,
        &tier2_modules,
    );

    CiFormalGateStats { total_debt, tier1_count, tier2_count, tier3_count, verdict, oldest_debt_days, blocked_modules }
}

/// Computes deterministic SHA-256 commitment for a formal gate config.
///
/// Canonicalization strategy:
/// - fixed field order,
/// - normalized/sorted/deduplicated exemption IDs,
/// - metadata sorted by key.
///
/// Output is lowercase hex with 64 characters.
pub fn compute_ci_formal_gate_commitment(config: &CiFormalGateConfig) -> String {
    let mut hasher = Sha256::new();

    hasher.update(b"ci_formal_gate_config:v1\n");
    hasher.update(format!("mode={}\n", gate_mode_label(&config.mode)).as_bytes());
    hasher.update(format!("max_tier1_debt={}\n", config.max_tier1_debt).as_bytes());
    hasher.update(format!("max_tier2_debt={}\n", config.max_tier2_debt).as_bytes());
    hasher.update(format!("max_total_debt={}\n", config.max_total_debt).as_bytes());
    hasher.update(format!("strict_on_release_branch={}\n", if config.strict_on_release_branch { "true" } else { "false" }).as_bytes());

    let mut exemptions = normalized_exemptions(&config.exemption_list);
    exemptions.sort();
    exemptions.dedup();
    for exemption in &exemptions {
        hasher.update(format!("exemption={exemption}\n").as_bytes());
    }

    let mut keys = config.metadata.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    for key in &keys {
        if let Some(value) = config.metadata.get(key) {
            hasher.update(format!("metadata:{key}={value}\n").as_bytes());
        }
    }

    format!("{:x}", hasher.finalize())
}

fn push_validation_error(errors: &mut Vec<CiFormalGateValidationError>, field: &str, reason: &str) {
    errors.push(CiFormalGateValidationError { field: field.to_string(), reason: reason.to_string() });
}

fn build_exemption_lookup(exemptions: &[String]) -> HashMap<String, bool> {
    let mut lookup = HashMap::new();
    for id in exemptions {
        let normalized = id.trim();
        if !normalized.is_empty() {
            lookup.insert(normalized.to_string(), true);
        }
    }
    lookup
}

fn normalized_exemptions(exemptions: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for id in exemptions {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            normalized.push(trimmed.to_string());
        }
    }
    normalized
}

fn is_exempt(id: &str, exemptions: &HashMap<String, bool>) -> bool {
    exemptions.contains_key(id.trim())
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|v| v == &value) {
        values.push(value);
    }
}

fn compute_verdict(
    config: &CiFormalGateConfig,
    total_debt: usize,
    tier1_count: usize,
    tier2_count: usize,
    config_invalid: bool,
) -> GateVerdict {
    if config_invalid {
        return GateVerdict::Error;
    }
    if matches!(config.mode, GateMode::Emergency) {
        return GateVerdict::Exempted;
    }
    if total_debt == 0 {
        return GateVerdict::Pass;
    }

    let strict_enforced = matches!(config.mode, GateMode::Strict)
        || (matches!(config.mode, GateMode::ReleaseCandidate) && config.strict_on_release_branch);
    if strict_enforced {
        return GateVerdict::HardFail;
    }

    let exceeds = tier1_count > config.max_tier1_debt
        || tier2_count > config.max_tier2_debt
        || total_debt > config.max_total_debt;

    match config.mode {
        GateMode::Advisory => GateVerdict::ConditionalPass,
        GateMode::Warning => if exceeds { GateVerdict::SoftFail } else { GateVerdict::ConditionalPass },
        GateMode::Blocking | GateMode::ReleaseCandidate => if exceeds { GateVerdict::HardFail } else { GateVerdict::ConditionalPass },
        GateMode::Strict => GateVerdict::HardFail,
        GateMode::Emergency => GateVerdict::Exempted,
    }
}

fn compute_blocked_modules(
    config: &CiFormalGateConfig,
    verdict: &GateVerdict,
    total_debt: usize,
    tier1_count: usize,
    tier2_count: usize,
    all_modules: &[String],
    tier1_modules: &[String],
    tier2_modules: &[String],
) -> Vec<String> {
    if !matches!(verdict, GateVerdict::SoftFail | GateVerdict::HardFail) {
        return Vec::new();
    }

    let strict_enforced = matches!(config.mode, GateMode::Strict)
        || (matches!(config.mode, GateMode::ReleaseCandidate) && config.strict_on_release_branch);
    if strict_enforced {
        let mut modules = all_modules.to_vec();
        modules.sort();
        modules.dedup();
        return modules;
    }

    let mut blocked = Vec::new();
    if tier1_count > config.max_tier1_debt {
        for module in tier1_modules {
            push_unique(&mut blocked, module.clone());
        }
    }
    if tier2_count > config.max_tier2_debt {
        for module in tier2_modules {
            push_unique(&mut blocked, module.clone());
        }
    }
    if total_debt > config.max_total_debt {
        for module in all_modules {
            push_unique(&mut blocked, module.clone());
        }
    }

    blocked.sort();
    blocked
}

fn gate_mode_label(mode: &GateMode) -> &'static str {
    match mode {
        GateMode::Advisory => "advisory",
        GateMode::Warning => "warning",
        GateMode::Blocking => "blocking",
        GateMode::Strict => "strict",
        GateMode::ReleaseCandidate => "release_candidate",
        GateMode::Emergency => "emergency",
    }
}
