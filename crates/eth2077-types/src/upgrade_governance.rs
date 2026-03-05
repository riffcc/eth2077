//! Upgrade governance and hardfork rehearsal planning types for ETH2077.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const MIN_SIGNAL_THRESHOLD_PCT: f64 = 50.0;
const MAX_SIGNAL_THRESHOLD_PCT: f64 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpgradePhase {
    Proposal,
    Discussion,
    Signaling,
    Accepted,
    Rehearsal,
    Activation,
    PostMortem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GovernanceModel {
    CoreDevConsensus,
    TokenVote,
    MultisigApproval,
    TimelockExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RehearsalOutcome {
    Success,
    PartialSuccess,
    Failure,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForkStrategy {
    EpochBoundary,
    BlockHeight,
    TimestampTrigger,
    ManualActivation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpgradeProposal {
    pub id: String,
    pub eip_numbers: Vec<u64>,
    pub title: String,
    pub champion: String,
    pub phase: UpgradePhase,
    pub governance_model: GovernanceModel,
    pub signal_threshold_pct: f64,
    pub current_signal_pct: f64,
    pub fork_strategy: ForkStrategy,
    pub activation_epoch: Option<u64>,
    pub created_at_unix: u64,
    pub approved_at_unix: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RehearsalRun {
    pub id: String,
    pub proposal_id: String,
    pub testnet_name: String,
    pub node_count: usize,
    pub client_diversity: Vec<(String, f64)>,
    pub started_at_unix: u64,
    pub ended_at_unix: Option<u64>,
    pub outcome: RehearsalOutcome,
    pub blocks_post_fork: u64,
    pub finality_disruption_slots: u64,
    pub rollback_triggered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpgradeGovernanceConfig {
    pub governance_model: GovernanceModel,
    pub signal_threshold_pct: f64,
    pub min_rehearsal_nodes: usize,
    pub required_client_count: usize,
    pub min_blocks_post_fork: u64,
    pub max_finality_disruption_slots: u64,
    pub cooldown_between_upgrades_days: u64,
    pub emergency_rollback_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpgradeGovernanceValidationError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpgradeGovernanceStats {
    pub total_proposals: usize,
    pub accepted_proposals: usize,
    pub rehearsals_run: usize,
    pub rehearsal_success_rate: f64,
    pub avg_signal_pct: f64,
    pub avg_rehearsal_duration_s: f64,
    pub commitment: [u8; 32],
}

impl Default for UpgradeGovernanceConfig {
    fn default() -> Self {
        Self {
            governance_model: GovernanceModel::CoreDevConsensus,
            signal_threshold_pct: 66.7,
            min_rehearsal_nodes: 8,
            required_client_count: 3,
            min_blocks_post_fork: 1000,
            max_finality_disruption_slots: 32,
            cooldown_between_upgrades_days: 90,
            emergency_rollback_enabled: true,
        }
    }
}

impl UpgradeGovernanceConfig {
    /// Validates policy bounds and cross-field numeric relationships.
    ///
    /// This routine is intentionally exhaustive: it does not short-circuit and
    /// instead returns every issue discovered in one pass.
    pub fn validate(&self) -> Vec<UpgradeGovernanceValidationError> {
        let mut errors = Vec::new();

        if !self.signal_threshold_pct.is_finite() {
            push_validation_error(
                &mut errors,
                "signal_threshold_pct",
                "must be a finite number",
            );
        } else {
            if self.signal_threshold_pct < MIN_SIGNAL_THRESHOLD_PCT {
                push_validation_error(&mut errors, "signal_threshold_pct", "must be at least 50.0");
            }
            if self.signal_threshold_pct > MAX_SIGNAL_THRESHOLD_PCT {
                push_validation_error(&mut errors, "signal_threshold_pct", "must be at most 100.0");
            }
        }

        if self.min_rehearsal_nodes == 0 {
            push_validation_error(
                &mut errors,
                "min_rehearsal_nodes",
                "must be greater than zero",
            );
        }

        if self.required_client_count == 0 {
            push_validation_error(
                &mut errors,
                "required_client_count",
                "must be greater than zero",
            );
        }

        if self.required_client_count > self.min_rehearsal_nodes && self.min_rehearsal_nodes > 0 {
            push_validation_error(
                &mut errors,
                "required_client_count",
                "must not exceed min_rehearsal_nodes",
            );
        }

        if self.min_blocks_post_fork == 0 {
            push_validation_error(
                &mut errors,
                "min_blocks_post_fork",
                "must be greater than zero",
            );
        }

        if self.max_finality_disruption_slots == 0 {
            push_validation_error(
                &mut errors,
                "max_finality_disruption_slots",
                "must be greater than zero",
            );
        }

        if self.max_finality_disruption_slots > self.min_blocks_post_fork
            && self.min_blocks_post_fork > 0
        {
            push_validation_error(
                &mut errors,
                "max_finality_disruption_slots",
                "must not exceed min_blocks_post_fork",
            );
        }

        if self.cooldown_between_upgrades_days == 0 {
            push_validation_error(
                &mut errors,
                "cooldown_between_upgrades_days",
                "must be greater than zero",
            );
        }

        errors
    }
}

impl UpgradeProposal {
    /// Returns true when the proposal satisfies governance and rehearsal gates.
    ///
    /// Readiness criteria:
    /// - `phase` is accepted or later.
    /// - `approved_at_unix` exists and is not earlier than `created_at_unix`.
    /// - current signaling is at least `max(proposal_threshold, config_threshold)`.
    /// - if strategy is `EpochBoundary`, `activation_epoch` is present and > 0.
    /// - config is valid.
    /// - at least one successful rehearsal for this proposal satisfies config.
    pub fn is_ready_for_activation(
        &self,
        config: &UpgradeGovernanceConfig,
        rehearsals: &[RehearsalRun],
    ) -> bool {
        if self.id.trim().is_empty() {
            return false;
        }

        if !self.phase.is_accepted_or_later() {
            return false;
        }

        let Some(approved_at) = self.approved_at_unix else {
            return false;
        };
        if approved_at < self.created_at_unix {
            return false;
        }

        if !config.validate().is_empty() {
            return false;
        }

        let effective_threshold = self.effective_signal_threshold(config);
        if normalized_pct(self.current_signal_pct) < effective_threshold {
            return false;
        }

        if matches!(self.fork_strategy, ForkStrategy::EpochBoundary)
            && self.activation_epoch.is_none_or(|epoch| epoch == 0)
        {
            return false;
        }

        rehearsals
            .iter()
            .filter(|run| run.proposal_id.trim() == self.id.trim())
            .any(|run| run.qualifies_for_activation(config))
    }

    /// Returns the strongest threshold between proposal-local and policy value.
    fn effective_signal_threshold(&self, config: &UpgradeGovernanceConfig) -> f64 {
        let proposal_threshold = normalized_pct(self.signal_threshold_pct);
        let policy_threshold = normalized_pct(config.signal_threshold_pct);
        proposal_threshold.max(policy_threshold)
    }
}

impl RehearsalRun {
    /// Duration in seconds when run has an end timestamp and non-negative span.
    pub fn duration_s(&self) -> Option<u64> {
        self.ended_at_unix.and_then(|ended| {
            (ended >= self.started_at_unix).then_some(ended - self.started_at_unix)
        })
    }

    /// Number of distinct clients with strictly positive participation share.
    pub fn unique_client_count(&self) -> usize {
        let mut clients = BTreeSet::new();
        for (client, share_pct) in &self.client_diversity {
            let normalized_name = client.trim().to_ascii_lowercase();
            if normalized_name.is_empty() {
                continue;
            }
            if share_pct.is_finite() && *share_pct > 0.0 {
                clients.insert(normalized_name);
            }
        }
        clients.len()
    }

    /// Canonicalized client diversity for deterministic hashing.
    fn normalized_client_diversity(&self) -> Vec<(String, f64)> {
        let mut entries: Vec<(String, f64)> = self
            .client_diversity
            .iter()
            .map(|(client, share_pct)| (client.trim().to_string(), normalized_pct(*share_pct)))
            .filter(|(client, _)| !client.is_empty())
            .collect();

        entries.sort_by(|(client_a, share_a), (client_b, share_b)| {
            client_a
                .to_ascii_lowercase()
                .cmp(&client_b.to_ascii_lowercase())
                .then_with(|| share_a.total_cmp(share_b))
        });
        entries
    }

    /// Returns true when this rehearsal run can be used for activation gating.
    fn qualifies_for_activation(&self, config: &UpgradeGovernanceConfig) -> bool {
        if self.id.trim().is_empty() {
            return false;
        }
        if self.proposal_id.trim().is_empty() {
            return false;
        }
        if self.testnet_name.trim().is_empty() {
            return false;
        }
        if !matches!(self.outcome, RehearsalOutcome::Success) {
            return false;
        }
        if self.duration_s().is_none() {
            return false;
        }
        if self.node_count < config.min_rehearsal_nodes {
            return false;
        }
        if self.unique_client_count() < config.required_client_count {
            return false;
        }
        if self.blocks_post_fork < config.min_blocks_post_fork {
            return false;
        }
        if self.finality_disruption_slots > config.max_finality_disruption_slots {
            return false;
        }
        if self.rollback_triggered && !config.emergency_rollback_enabled {
            return false;
        }
        true
    }
}

/// Computes aggregate proposal/rehearsal statistics and a SHA-256 commitment.
///
/// Commitment inputs are canonicalized to avoid instability caused by:
/// - vector insertion order,
/// - client-diversity ordering,
/// - invalid float values (NaN or infinities).
pub fn compute_stats(
    proposals: &[UpgradeProposal],
    rehearsals: &[RehearsalRun],
) -> UpgradeGovernanceStats {
    let total_proposals = proposals.len();
    let accepted_proposals = proposals
        .iter()
        .filter(|proposal| proposal.phase.is_accepted_or_later())
        .count();

    let rehearsals_run = rehearsals.len();
    let successful_rehearsals = rehearsals
        .iter()
        .filter(|run| matches!(run.outcome, RehearsalOutcome::Success))
        .count();

    let rehearsal_success_rate = ratio(successful_rehearsals, rehearsals_run);
    let avg_signal_pct = average(
        proposals
            .iter()
            .map(|proposal| normalized_pct(proposal.current_signal_pct)),
    );
    let avg_rehearsal_duration_s = average(
        rehearsals
            .iter()
            .filter_map(|run| run.duration_s().map(|s| s as f64)),
    );

    let mut stats = UpgradeGovernanceStats {
        total_proposals,
        accepted_proposals,
        rehearsals_run,
        rehearsal_success_rate,
        avg_signal_pct,
        avg_rehearsal_duration_s,
        commitment: [0u8; 32],
    };

    stats.commitment = compute_stats_commitment(proposals, rehearsals, &stats);
    stats
}

/// Canonical payload used for deterministic hashing.
#[derive(Debug, Clone, Serialize)]
struct StatsCommitmentPayload {
    version: &'static str,
    stats: StatsCommitmentFields,
    proposals: Vec<ProposalCommitmentEntry>,
    rehearsals: Vec<RehearsalCommitmentEntry>,
}

/// Scalar stats subset copied into commitment payload.
#[derive(Debug, Clone, Serialize)]
struct StatsCommitmentFields {
    total_proposals: usize,
    accepted_proposals: usize,
    rehearsals_run: usize,
    rehearsal_success_rate: f64,
    avg_signal_pct: f64,
    avg_rehearsal_duration_s: f64,
}

/// Canonical proposal representation used for commitment generation.
#[derive(Debug, Clone, Serialize)]
struct ProposalCommitmentEntry {
    id: String,
    eip_numbers: Vec<u64>,
    title: String,
    champion: String,
    phase: UpgradePhase,
    governance_model: GovernanceModel,
    signal_threshold_pct: f64,
    current_signal_pct: f64,
    fork_strategy: ForkStrategy,
    activation_epoch: Option<u64>,
    created_at_unix: u64,
    approved_at_unix: Option<u64>,
}

/// Canonical rehearsal representation used for commitment generation.
#[derive(Debug, Clone, Serialize)]
struct RehearsalCommitmentEntry {
    id: String,
    proposal_id: String,
    testnet_name: String,
    node_count: usize,
    client_diversity: Vec<(String, f64)>,
    started_at_unix: u64,
    ended_at_unix: Option<u64>,
    outcome: RehearsalOutcome,
    blocks_post_fork: u64,
    finality_disruption_slots: u64,
    rollback_triggered: bool,
}

/// Computes SHA-256 commitment from canonicalized stats, proposals, and runs.
fn compute_stats_commitment(
    proposals: &[UpgradeProposal],
    rehearsals: &[RehearsalRun],
    stats: &UpgradeGovernanceStats,
) -> [u8; 32] {
    let mut canonical_proposals = proposals
        .iter()
        .map(|proposal| ProposalCommitmentEntry {
            id: proposal.id.trim().to_string(),
            eip_numbers: {
                let mut eips = proposal.eip_numbers.clone();
                eips.sort_unstable();
                eips.dedup();
                eips
            },
            title: proposal.title.trim().to_string(),
            champion: proposal.champion.trim().to_string(),
            phase: proposal.phase,
            governance_model: proposal.governance_model,
            signal_threshold_pct: normalized_pct(proposal.signal_threshold_pct),
            current_signal_pct: normalized_pct(proposal.current_signal_pct),
            fork_strategy: proposal.fork_strategy,
            activation_epoch: proposal.activation_epoch,
            created_at_unix: proposal.created_at_unix,
            approved_at_unix: proposal.approved_at_unix,
        })
        .collect::<Vec<_>>();

    canonical_proposals.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.created_at_unix.cmp(&right.created_at_unix))
            .then_with(|| left.title.cmp(&right.title))
    });

    let mut canonical_rehearsals = rehearsals
        .iter()
        .map(|run| RehearsalCommitmentEntry {
            id: run.id.trim().to_string(),
            proposal_id: run.proposal_id.trim().to_string(),
            testnet_name: run.testnet_name.trim().to_string(),
            node_count: run.node_count,
            client_diversity: run.normalized_client_diversity(),
            started_at_unix: run.started_at_unix,
            ended_at_unix: run.ended_at_unix,
            outcome: run.outcome,
            blocks_post_fork: run.blocks_post_fork,
            finality_disruption_slots: run.finality_disruption_slots,
            rollback_triggered: run.rollback_triggered,
        })
        .collect::<Vec<_>>();

    canonical_rehearsals.sort_by(|left, right| {
        left.proposal_id
            .cmp(&right.proposal_id)
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.started_at_unix.cmp(&right.started_at_unix))
    });

    let payload = StatsCommitmentPayload {
        version: "upgrade_governance_stats:v1",
        stats: StatsCommitmentFields {
            total_proposals: stats.total_proposals,
            accepted_proposals: stats.accepted_proposals,
            rehearsals_run: stats.rehearsals_run,
            rehearsal_success_rate: round_six_decimals(stats.rehearsal_success_rate),
            avg_signal_pct: round_six_decimals(stats.avg_signal_pct),
            avg_rehearsal_duration_s: round_six_decimals(stats.avg_rehearsal_duration_s),
        },
        proposals: canonical_proposals,
        rehearsals: canonical_rehearsals,
    };

    let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(payload_bytes);
    let digest = hasher.finalize();

    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

/// Pushes a field-scoped validation error.
fn push_validation_error(
    errors: &mut Vec<UpgradeGovernanceValidationError>,
    field: &str,
    message: &str,
) {
    errors.push(UpgradeGovernanceValidationError {
        field: field.to_string(),
        message: message.to_string(),
    });
}

/// Returns arithmetic mean over finite values, or `0.0` when empty.
fn average(values: impl Iterator<Item = f64>) -> f64 {
    let mut count = 0usize;
    let mut sum = 0.0;

    for value in values {
        if value.is_finite() {
            sum += value;
            count += 1;
        }
    }

    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}

/// Returns `numerator / denominator` for finite ratio computation.
fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

/// Normalizes percentages to finite values in range `[0, 100]`.
fn normalized_pct(value: f64) -> f64 {
    if !value.is_finite() {
        0.0
    } else {
        value.clamp(0.0, 100.0)
    }
}

/// Reduces floating-point noise for stable commitment payload snapshots.
fn round_six_decimals(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    let scale = 1_000_000.0;
    (value * scale).round() / scale
}

impl UpgradePhase {
    /// Returns true for phases where governance has crossed acceptance.
    fn is_accepted_or_later(self) -> bool {
        matches!(
            self,
            UpgradePhase::Accepted
                | UpgradePhase::Rehearsal
                | UpgradePhase::Activation
                | UpgradePhase::PostMortem
        )
    }
}
