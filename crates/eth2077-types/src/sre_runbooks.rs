//! SRE runbook and game-day primitives for ETH2077 Ethereum node operations.
//!
//! Design notes:
//! - All primary records derive `Serialize` and `Deserialize` for persistence and signing.
//! - `Runbook::is_stale` is intentionally simple and deterministic around unix timestamps.
//! - `SreRunbooksConfig::validate` returns structured field-level errors instead of failing fast.
//! - `compute_stats` aggregates runbook freshness and game-day performance across a fleet.
//! - `SreRunbooksStats::commitment` binds raw inputs and derived outputs with SHA-256.
//!
//! Operational intent:
//! - Keep runbook ownership and lifecycle metadata directly on the runbook object.
//! - Represent game-day exercises as immutable event summaries suitable for audits.
//! - Keep validation logic local to config to avoid cross-module policy drift.
//! - Make stale detection reusable for dashboards and policy automation.
//! - Preserve deterministic stats commitment for attestation pipelines.
//!
//! Scope:
//! - This module does not schedule runbooks or game days.
//! - This module does not execute runbook commands.
//! - This module does not store encrypted secrets.
//! - This module does not integrate directly with incident vendors.
//! - This module does not enforce organization-specific workflows.
//!
//! Expected integration points:
//! - Incident coordinators can select runbooks by severity and category.
//! - Policy engines can reject invalid configs via `validate`.
//! - Dashboards can derive freshness and success rates from `compute_stats`.
//! - Integrity layers can compare `commitment` values across replicas.
//! - Operators can mark runbooks stale using `is_stale` and refresh cadence rules.
//!
//! Conventions:
//! - Unix timestamps are seconds.
//! - Durations are explicit units (`_seconds`, `_minutes`, `_days`).
//! - Success rates are normalized to `[0.0, 1.0]`.
//! - Missing timings are excluded from averages.
//! - Empty populations produce zero-valued averages and rates.
//!
//! Commitment coverage:
//! - Domain separator string.
//! - `now_unix` passed into stats computation.
//! - Full runbook input slice.
//! - Full exercise input slice.
//! - Derived scalar outputs in `SreRunbooksStats`.
//! - Stable JSON serialization for commitment payload.
//!
//! Validation policy goals:
//! - Reject impossible zero thresholds.
//! - Reject unrealistic operational cadences.
//! - Catch contradictory detection/mitigation expectations.
//! - Flag manual escalation with ultra-aggressive detection SLOs.
//! - Flag disabled rollback with unbounded mitigation windows.
//!
//! Staleness semantics:
//! - Never-tested runbooks are stale.
//! - Future test timestamps are treated as non-stale (clock skew tolerance).
//! - A runbook becomes stale only when age is strictly greater than max age.
//! - `max_age_days` uses saturating arithmetic.
//! - Comparison is deterministic for all `u64` values.
//!
//! Metrics semantics:
//! - `total_runbooks` is `runbooks.len()`.
//! - `stale_runbooks` uses default config's test interval (30 days).
//! - `game_days_run` is `exercises.len()`.
//! - `avg_detection_time_s` averages non-zero, bounded observed values.
//! - `avg_mitigation_time_s` averages non-zero, bounded observed values.
//! - `game_day_success_rate` is successes divided by all exercises.
//! - `commitment` binds all of the above.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SECONDS_PER_DAY: u64 = 86_400;
const STATS_COMMITMENT_DOMAIN_BYTES: &[u8] = b"ETH2077::SRE_RUNBOOKS_STATS::V1";
const STATS_COMMITMENT_DOMAIN_STR: &str = "ETH2077::SRE_RUNBOOKS_STATS::V1";
const MAX_REASONABLE_DETECTION_SECONDS: u64 = 7_200;
const MAX_REASONABLE_MITIGATION_SECONDS: u64 = 172_800;
const MAX_REASONABLE_TEST_INTERVAL_DAYS: u64 = 365;
const MAX_REASONABLE_GAME_DAY_FREQUENCY_DAYS: u64 = 365;
const MAX_REASONABLE_GAME_DAY_SCENARIOS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentSeverity {
    Sev1Critical,
    Sev2Major,
    Sev3Minor,
    Sev4Info,
}

impl IncidentSeverity {
    pub fn rank(self) -> u8 {
        match self {
            Self::Sev1Critical => 1,
            Self::Sev2Major => 2,
            Self::Sev3Minor => 3,
            Self::Sev4Info => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunbookCategory {
    NodeRecovery,
    ConsensusFailure,
    NetworkPartition,
    StateCorruption,
    PerformanceDegradation,
    SecurityIncident,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameDayPhase {
    Planning,
    Injection,
    Observation,
    Mitigation,
    PostMortem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscalationPolicy {
    PagerDuty,
    SlackAlert,
    EmailChain,
    ManualEscalation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookStep {
    pub order: usize,
    pub title: String,
    pub command: Option<String>,
    pub expected_outcome: String,
    pub timeout_seconds: u64,
    pub rollback_command: Option<String>,
}

impl RunbookStep {
    pub fn has_command(&self) -> bool {
        self.command
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
    }

    pub fn has_rollback(&self) -> bool {
        self.rollback_command
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
    }

    pub fn timeout_with_floor(&self) -> u64 {
        self.timeout_seconds.max(1)
    }

    pub fn looks_well_formed(&self) -> bool {
        self.order > 0 && !self.title.trim().is_empty() && !self.expected_outcome.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Runbook {
    pub id: String,
    pub title: String,
    pub category: RunbookCategory,
    pub severity_applies: Vec<IncidentSeverity>,
    pub steps: Vec<RunbookStep>,
    pub estimated_duration_minutes: u64,
    pub last_tested_unix: Option<u64>,
    pub owner: String,
    pub version: u32,
}

impl Runbook {
    pub fn is_stale(&self, now_unix: u64, max_age_days: u64) -> bool {
        let max_age_seconds = max_age_days.saturating_mul(SECONDS_PER_DAY);
        match self.last_tested_unix {
            Some(last_tested_unix) => {
                if last_tested_unix > now_unix {
                    return false;
                }
                now_unix.saturating_sub(last_tested_unix) > max_age_seconds
            }
            None => true,
        }
    }

    pub fn has_owner(&self) -> bool {
        !self.owner.trim().is_empty()
    }

    pub fn highest_priority_severity(&self) -> Option<IncidentSeverity> {
        self.severity_applies
            .iter()
            .copied()
            .min_by_key(|value| value.rank())
    }

    pub fn step_timeout_budget_seconds(&self) -> u64 {
        self.steps.iter().map(RunbookStep::timeout_with_floor).sum()
    }

    pub fn all_steps_well_formed(&self) -> bool {
        self.steps.iter().all(RunbookStep::looks_well_formed)
    }

    pub fn has_ordered_steps(&self) -> bool {
        if self.steps.is_empty() {
            return false;
        }
        let mut expected_order = 1usize;
        for step in &self.steps {
            if step.order != expected_order {
                return false;
            }
            expected_order += 1;
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameDayExercise {
    pub id: String,
    pub scenario_name: String,
    pub phase: GameDayPhase,
    pub injected_faults: Vec<String>,
    pub participating_nodes: usize,
    pub started_at_unix: u64,
    pub ended_at_unix: Option<u64>,
    pub detection_time_seconds: Option<u64>,
    pub mitigation_time_seconds: Option<u64>,
    pub success: bool,
}

impl GameDayExercise {
    pub fn is_complete(&self) -> bool {
        self.ended_at_unix.is_some() || self.phase == GameDayPhase::PostMortem
    }

    pub fn duration_seconds(&self) -> Option<u64> {
        let end = self.ended_at_unix?;
        if end < self.started_at_unix {
            return None;
        }
        Some(end - self.started_at_unix)
    }

    pub fn has_timings(&self) -> bool {
        self.detection_time_seconds.is_some() && self.mitigation_time_seconds.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SreRunbooksConfig {
    pub max_detection_time_seconds: u64,
    pub max_mitigation_time_seconds: u64,
    pub min_runbook_test_interval_days: u64,
    pub escalation_policy: EscalationPolicy,
    pub game_day_frequency_days: u64,
    pub required_game_day_scenarios: usize,
    pub auto_rollback_enabled: bool,
}

impl Default for SreRunbooksConfig {
    fn default() -> Self {
        Self {
            max_detection_time_seconds: 300,
            max_mitigation_time_seconds: 1_800,
            min_runbook_test_interval_days: 30,
            escalation_policy: EscalationPolicy::PagerDuty,
            game_day_frequency_days: 14,
            required_game_day_scenarios: 5,
            auto_rollback_enabled: true,
        }
    }
}

impl SreRunbooksConfig {
    pub fn validate(&self) -> Vec<SreRunbooksValidationError> {
        let mut errors = Vec::new();

        if self.max_detection_time_seconds == 0 {
            push_error(
                &mut errors,
                "max_detection_time_seconds",
                "must be greater than zero",
            );
        }
        if self.max_detection_time_seconds > MAX_REASONABLE_DETECTION_SECONDS {
            push_error(
                &mut errors,
                "max_detection_time_seconds",
                "is unrealistically high for incident detection",
            );
        }

        if self.max_mitigation_time_seconds == 0 {
            push_error(
                &mut errors,
                "max_mitigation_time_seconds",
                "must be greater than zero",
            );
        }
        if self.max_mitigation_time_seconds > MAX_REASONABLE_MITIGATION_SECONDS {
            push_error(
                &mut errors,
                "max_mitigation_time_seconds",
                "is unrealistically high for mitigation",
            );
        }

        if self.max_detection_time_seconds > self.max_mitigation_time_seconds {
            push_error(
                &mut errors,
                "max_detection_time_seconds",
                "must not exceed max_mitigation_time_seconds",
            );
        }

        if self.min_runbook_test_interval_days == 0 {
            push_error(
                &mut errors,
                "min_runbook_test_interval_days",
                "must be greater than zero",
            );
        }
        if self.min_runbook_test_interval_days > MAX_REASONABLE_TEST_INTERVAL_DAYS {
            push_error(
                &mut errors,
                "min_runbook_test_interval_days",
                "is too infrequent for production runbook hygiene",
            );
        }

        if self.game_day_frequency_days == 0 {
            push_error(
                &mut errors,
                "game_day_frequency_days",
                "must be greater than zero",
            );
        }
        if self.game_day_frequency_days > MAX_REASONABLE_GAME_DAY_FREQUENCY_DAYS {
            push_error(
                &mut errors,
                "game_day_frequency_days",
                "is too infrequent for rehearsal cadence",
            );
        }

        if self.required_game_day_scenarios == 0 {
            push_error(
                &mut errors,
                "required_game_day_scenarios",
                "must be greater than zero",
            );
        }
        if self.required_game_day_scenarios > MAX_REASONABLE_GAME_DAY_SCENARIOS {
            push_error(
                &mut errors,
                "required_game_day_scenarios",
                "is too high for a single planning cycle",
            );
        }

        if self.escalation_policy == EscalationPolicy::ManualEscalation
            && self.max_detection_time_seconds <= 60
        {
            push_error(
                &mut errors,
                "escalation_policy",
                "manual escalation is incompatible with sub-minute detection goals",
            );
        }

        if !self.auto_rollback_enabled
            && self.max_mitigation_time_seconds > self.max_detection_time_seconds.saturating_mul(20)
        {
            push_error(
                &mut errors,
                "auto_rollback_enabled",
                "disabling auto rollback requires tighter mitigation bounds",
            );
        }

        errors
    }

    pub fn stale_threshold_days(&self) -> u64 {
        self.min_runbook_test_interval_days
    }

    pub fn uses_real_time_escalation(&self) -> bool {
        matches!(
            self.escalation_policy,
            EscalationPolicy::PagerDuty | EscalationPolicy::SlackAlert
        )
    }

    pub fn requires_manual_escalation(&self) -> bool {
        self.escalation_policy == EscalationPolicy::ManualEscalation
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SreRunbooksValidationError {
    pub field: String,
    pub message: String,
}

impl SreRunbooksValidationError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SreRunbooksStats {
    pub total_runbooks: usize,
    pub stale_runbooks: usize,
    pub game_days_run: usize,
    pub avg_detection_time_s: f64,
    pub avg_mitigation_time_s: f64,
    pub game_day_success_rate: f64,
    pub commitment: [u8; 32],
}

impl SreRunbooksStats {
    pub fn fresh_runbooks(&self) -> usize {
        self.total_runbooks.saturating_sub(self.stale_runbooks)
    }

    pub fn success_rate_percent(&self) -> f64 {
        self.game_day_success_rate * 100.0
    }

    pub fn stale_ratio(&self) -> f64 {
        if self.total_runbooks == 0 {
            return 0.0;
        }
        self.stale_runbooks as f64 / self.total_runbooks as f64
    }

    pub fn commitment_hex(&self) -> String {
        bytes_to_lower_hex(&self.commitment)
    }
}

pub fn compute_stats(
    runbooks: &[Runbook],
    exercises: &[GameDayExercise],
    now_unix: u64,
) -> SreRunbooksStats {
    let stale_days = SreRunbooksConfig::default().min_runbook_test_interval_days;

    let total_runbooks = runbooks.len();
    let stale_runbooks = runbooks
        .iter()
        .filter(|runbook| runbook.is_stale(now_unix, stale_days))
        .count();

    let game_days_run = exercises.len();
    let avg_detection_time_s = mean_optional_u64(exercises.iter().map(|exercise| {
        exercise
            .detection_time_seconds
            .filter(|value| *value > 0 && *value <= MAX_REASONABLE_MITIGATION_SECONDS)
    }));
    let avg_mitigation_time_s = mean_optional_u64(exercises.iter().map(|exercise| {
        exercise
            .mitigation_time_seconds
            .filter(|value| *value > 0 && *value <= MAX_REASONABLE_MITIGATION_SECONDS)
    }));

    let successes = exercises.iter().filter(|exercise| exercise.success).count();
    let game_day_success_rate = if game_days_run == 0 {
        0.0
    } else {
        successes as f64 / game_days_run as f64
    };

    let commitment = compute_stats_commitment(
        runbooks,
        exercises,
        now_unix,
        total_runbooks,
        stale_runbooks,
        game_days_run,
        avg_detection_time_s,
        avg_mitigation_time_s,
        game_day_success_rate,
    );

    SreRunbooksStats {
        total_runbooks,
        stale_runbooks,
        game_days_run,
        avg_detection_time_s,
        avg_mitigation_time_s,
        game_day_success_rate,
        commitment,
    }
}

fn push_error(errors: &mut Vec<SreRunbooksValidationError>, field: &str, message: &str) {
    errors.push(SreRunbooksValidationError::new(field, message));
}

fn mean_optional_u64(values: impl Iterator<Item = Option<u64>>) -> f64 {
    let mut count = 0u64;
    let mut total = 0u128;
    for value in values.flatten() {
        count += 1;
        total += u128::from(value);
    }
    if count == 0 {
        0.0
    } else {
        total as f64 / count as f64
    }
}

fn compute_stats_commitment(
    runbooks: &[Runbook],
    exercises: &[GameDayExercise],
    now_unix: u64,
    total_runbooks: usize,
    stale_runbooks: usize,
    game_days_run: usize,
    avg_detection_time_s: f64,
    avg_mitigation_time_s: f64,
    game_day_success_rate: f64,
) -> [u8; 32] {
    #[derive(Serialize)]
    struct CommitmentPayload<'a> {
        domain: &'static str,
        now_unix: u64,
        runbooks: &'a [Runbook],
        exercises: &'a [GameDayExercise],
        total_runbooks: usize,
        stale_runbooks: usize,
        game_days_run: usize,
        avg_detection_time_s: f64,
        avg_mitigation_time_s: f64,
        game_day_success_rate: f64,
    }

    let payload = CommitmentPayload {
        domain: STATS_COMMITMENT_DOMAIN_STR,
        now_unix,
        runbooks,
        exercises,
        total_runbooks,
        stale_runbooks,
        game_days_run,
        avg_detection_time_s,
        avg_mitigation_time_s,
        game_day_success_rate,
    };

    let encoded = serde_json::to_vec(&payload).expect("sre runbooks stats payload must serialize");
    let mut hasher = Sha256::new();
    hasher.update(STATS_COMMITMENT_DOMAIN_BYTES);
    hasher.update(encoded);

    let digest = hasher.finalize();
    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn bytes_to_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
