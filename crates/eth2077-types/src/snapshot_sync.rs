//! Snapshot/checkpoint synchronization and state repair types for ETH2077.
//! Includes config validation, session progress tracking, and SHA-256 stats commitments.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotFormat {
    Full,
    Pruned,
    Archive,
    DiffBased,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncPhase {
    Discovery,
    Download,
    Verification,
    Application,
    Finalization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepairStrategy {
    Redownload,
    Reconstruct,
    PeerAssist,
    ManualIntervention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointSource {
    Beacon,
    ExecutionLayer,
    ExternalOracle,
    BootstrapNode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub slot: u64,
    pub state_root: [u8; 32],
    pub format: SnapshotFormat,
    pub size_bytes: u64,
    pub chunk_count: usize,
    pub created_at_unix: u64,
    pub source_node: String,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub slot: u64,
    pub block_root: [u8; 32],
    pub state_root: [u8; 32],
    pub source: CheckpointSource,
    pub finalized: bool,
    pub verified_by_count: usize,
    pub timestamp_unix: u64,
}

impl Checkpoint {
    pub fn is_verified(&self) -> bool {
        self.verified_by_count > 0 && !is_zero_32(&self.block_root) && !is_zero_32(&self.state_root)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncSession {
    pub id: String,
    pub snapshot_id: String,
    pub phase: SyncPhase,
    pub chunks_downloaded: usize,
    pub chunks_verified: usize,
    pub chunks_total: usize,
    pub started_at_unix: u64,
    pub ended_at_unix: Option<u64>,
    pub bandwidth_bytes_per_sec: f64,
    pub repair_attempts: usize,
    pub repair_strategy: Option<RepairStrategy>,
}

impl SyncSession {
    pub fn is_completed(&self) -> bool {
        self.ended_at_unix.is_some()
    }

    pub fn duration_s(&self) -> Option<u64> {
        self.ended_at_unix
            .map(|ended| ended.saturating_sub(self.started_at_unix))
    }

    pub fn bandwidth_mbps(&self) -> f64 {
        if !self.bandwidth_bytes_per_sec.is_finite() || self.bandwidth_bytes_per_sec <= 0.0 {
            0.0
        } else {
            bytes_per_sec_to_mbps(self.bandwidth_bytes_per_sec)
        }
    }

    pub fn progress_pct(&self) -> f64 {
        if self.chunks_total == 0 {
            if matches!(self.phase, SyncPhase::Finalization) && self.ended_at_unix.is_some() {
                return 100.0;
            }
            return 0.0;
        }

        let downloaded_ratio =
            self.chunks_downloaded.min(self.chunks_total) as f64 / self.chunks_total as f64;
        let verified_ratio =
            self.chunks_verified.min(self.chunks_total) as f64 / self.chunks_total as f64;

        let weighted_progress = downloaded_ratio * 70.0 + verified_ratio * 30.0;
        let phase_floor = match self.phase {
            SyncPhase::Discovery => 0.0,
            SyncPhase::Download => 5.0,
            SyncPhase::Verification => 40.0,
            SyncPhase::Application => 80.0,
            SyncPhase::Finalization => 95.0,
        };

        let mut progress = weighted_progress.max(phase_floor);
        if matches!(self.phase, SyncPhase::Finalization) && self.ended_at_unix.is_some() {
            progress = 100.0;
        }

        progress.clamp(0.0, 100.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotSyncConfig {
    pub max_snapshot_age_slots: u64,
    pub min_chunk_peers: usize,
    pub max_concurrent_downloads: usize,
    pub verification_parallelism: usize,
    pub max_repair_attempts: usize,
    pub preferred_format: SnapshotFormat,
    pub checkpoint_sources: Vec<CheckpointSource>,
    pub bandwidth_limit_mbps: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotSyncValidationError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotSyncStats {
    pub snapshots_available: usize,
    pub checkpoints_verified: usize,
    pub sync_sessions_completed: usize,
    pub avg_sync_duration_s: f64,
    pub avg_bandwidth_mbps: f64,
    pub repair_rate: f64,
    pub total_data_synced_gb: f64,
    pub commitment: [u8; 32],
}

impl Default for SnapshotSyncConfig {
    fn default() -> Self {
        Self {
            max_snapshot_age_slots: 8192,
            min_chunk_peers: 3,
            max_concurrent_downloads: 8,
            verification_parallelism: 4,
            max_repair_attempts: 3,
            preferred_format: SnapshotFormat::Full,
            checkpoint_sources: vec![CheckpointSource::Beacon, CheckpointSource::ExecutionLayer],
            bandwidth_limit_mbps: None,
        }
    }
}

impl SnapshotSyncConfig {
    pub fn validate(&self) -> Vec<SnapshotSyncValidationError> {
        let mut errors = Vec::new();

        if self.max_snapshot_age_slots == 0 {
            push_error(
                &mut errors,
                "max_snapshot_age_slots",
                "must be greater than 0",
            );
        }

        if self.min_chunk_peers == 0 {
            push_error(&mut errors, "min_chunk_peers", "must be greater than 0");
        }

        if self.max_concurrent_downloads == 0 {
            push_error(
                &mut errors,
                "max_concurrent_downloads",
                "must be greater than 0",
            );
        }

        if self.verification_parallelism == 0 {
            push_error(
                &mut errors,
                "verification_parallelism",
                "must be greater than 0",
            );
        }

        if self.max_repair_attempts == 0 {
            push_error(&mut errors, "max_repair_attempts", "must be greater than 0");
        }

        if self.min_chunk_peers > self.max_concurrent_downloads.saturating_mul(4) {
            push_error(
                &mut errors,
                "min_chunk_peers",
                "is too high relative to max_concurrent_downloads",
            );
        }

        if self.verification_parallelism > self.max_concurrent_downloads.saturating_mul(8) {
            push_error(
                &mut errors,
                "verification_parallelism",
                "is too high relative to max_concurrent_downloads",
            );
        }

        if self.checkpoint_sources.is_empty() {
            push_error(
                &mut errors,
                "checkpoint_sources",
                "must contain at least one source",
            );
        }

        if has_duplicate_checkpoint_sources(&self.checkpoint_sources) {
            push_error(
                &mut errors,
                "checkpoint_sources",
                "must not contain duplicates",
            );
        }

        if matches!(self.preferred_format, SnapshotFormat::DiffBased) && self.min_chunk_peers < 2 {
            push_error(
                &mut errors,
                "preferred_format",
                "DiffBased format requires min_chunk_peers >= 2",
            );
        }

        if let Some(limit) = self.bandwidth_limit_mbps {
            if !limit.is_finite() || limit <= 0.0 {
                push_error(
                    &mut errors,
                    "bandwidth_limit_mbps",
                    "must be finite and greater than 0 when set",
                );
            }

            if limit > 1_000_000.0 {
                push_error(
                    &mut errors,
                    "bandwidth_limit_mbps",
                    "is unrealistically high; expected <= 1,000,000 Mbps",
                );
            }
        }

        errors
    }
}

pub fn compute_stats(
    snapshots: &[Snapshot],
    checkpoints: &[Checkpoint],
    sessions: &[SyncSession],
) -> SnapshotSyncStats {
    let snapshots_available = snapshots.len();
    let checkpoints_verified = checkpoints
        .iter()
        .filter(|checkpoint| checkpoint.is_verified())
        .count();

    let completed_sessions: Vec<&SyncSession> = sessions
        .iter()
        .filter(|session| session.is_completed())
        .collect();
    let sync_sessions_completed = completed_sessions.len();

    let avg_sync_duration_s = if completed_sessions.is_empty() {
        0.0
    } else {
        let duration_sum: u128 = completed_sessions
            .iter()
            .map(|session| session.duration_s().unwrap_or(0) as u128)
            .sum();
        duration_sum as f64 / completed_sessions.len() as f64
    };

    let bandwidth_samples: Vec<f64> = sessions
        .iter()
        .map(SyncSession::bandwidth_mbps)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .collect();
    let avg_bandwidth_mbps = average_or_zero(&bandwidth_samples);

    let repair_sessions = sessions
        .iter()
        .filter(|session| session.repair_attempts > 0)
        .count();
    let repair_rate = if sessions.is_empty() {
        0.0
    } else {
        repair_sessions as f64 / sessions.len() as f64
    };

    let snapshot_size_by_id: HashMap<&str, u64> = snapshots
        .iter()
        .map(|snapshot| (snapshot.id.as_str(), snapshot.size_bytes))
        .collect();
    let synced_bytes_total: f64 = sessions
        .iter()
        .map(|session| estimate_synced_bytes(session, &snapshot_size_by_id))
        .sum();
    let total_data_synced_gb = bytes_to_gigabytes(synced_bytes_total);

    let base_stats = SnapshotSyncStats {
        snapshots_available,
        checkpoints_verified,
        sync_sessions_completed,
        avg_sync_duration_s,
        avg_bandwidth_mbps,
        repair_rate,
        total_data_synced_gb,
        commitment: [0u8; 32],
    };

    let commitment = compute_commitment_payload(snapshots, checkpoints, sessions, &base_stats);
    SnapshotSyncStats {
        commitment,
        ..base_stats
    }
}

#[derive(Debug, Clone, Serialize)]
struct SnapshotCommitmentView {
    id: String,
    slot: u64,
    state_root: [u8; 32],
    format: SnapshotFormat,
    size_bytes: u64,
    chunk_count: usize,
    created_at_unix: u64,
    source_node: String,
    signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
struct CheckpointCommitmentView {
    slot: u64,
    block_root: [u8; 32],
    state_root: [u8; 32],
    source: CheckpointSource,
    finalized: bool,
    verified_by_count: usize,
    timestamp_unix: u64,
}

#[derive(Debug, Clone, Serialize)]
struct SessionCommitmentView {
    id: String,
    snapshot_id: String,
    phase: SyncPhase,
    chunks_downloaded: usize,
    chunks_verified: usize,
    chunks_total: usize,
    started_at_unix: u64,
    ended_at_unix: Option<u64>,
    bandwidth_bytes_per_sec: f64,
    repair_attempts: usize,
    repair_strategy: Option<RepairStrategy>,
    progress_pct: f64,
}

#[derive(Debug, Serialize)]
struct StatsCommitmentView {
    snapshots_available: usize,
    checkpoints_verified: usize,
    sync_sessions_completed: usize,
    avg_sync_duration_s: f64,
    avg_bandwidth_mbps: f64,
    repair_rate: f64,
    total_data_synced_gb: f64,
}

#[derive(Debug, Serialize)]
struct CommitmentPayload {
    snapshots: Vec<SnapshotCommitmentView>,
    checkpoints: Vec<CheckpointCommitmentView>,
    sessions: Vec<SessionCommitmentView>,
    stats: StatsCommitmentView,
}

fn compute_commitment_payload(
    snapshots: &[Snapshot],
    checkpoints: &[Checkpoint],
    sessions: &[SyncSession],
    stats: &SnapshotSyncStats,
) -> [u8; 32] {
    let mut normalized_snapshots: Vec<SnapshotCommitmentView> = snapshots
        .iter()
        .map(|snapshot| SnapshotCommitmentView {
            id: snapshot.id.clone(),
            slot: snapshot.slot,
            state_root: snapshot.state_root,
            format: snapshot.format,
            size_bytes: snapshot.size_bytes,
            chunk_count: snapshot.chunk_count,
            created_at_unix: snapshot.created_at_unix,
            source_node: snapshot.source_node.clone(),
            signature: snapshot.signature.clone(),
        })
        .collect();
    normalized_snapshots.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then(left.slot.cmp(&right.slot))
            .then(left.created_at_unix.cmp(&right.created_at_unix))
    });

    let mut normalized_checkpoints: Vec<CheckpointCommitmentView> = checkpoints
        .iter()
        .map(|checkpoint| CheckpointCommitmentView {
            slot: checkpoint.slot,
            block_root: checkpoint.block_root,
            state_root: checkpoint.state_root,
            source: checkpoint.source,
            finalized: checkpoint.finalized,
            verified_by_count: checkpoint.verified_by_count,
            timestamp_unix: checkpoint.timestamp_unix,
        })
        .collect();
    normalized_checkpoints.sort_by(|left, right| {
        left.slot
            .cmp(&right.slot)
            .then(source_order(left.source).cmp(&source_order(right.source)))
            .then(left.timestamp_unix.cmp(&right.timestamp_unix))
    });

    let mut normalized_sessions: Vec<SessionCommitmentView> = sessions
        .iter()
        .map(|session| SessionCommitmentView {
            id: session.id.clone(),
            snapshot_id: session.snapshot_id.clone(),
            phase: session.phase,
            chunks_downloaded: session.chunks_downloaded,
            chunks_verified: session.chunks_verified,
            chunks_total: session.chunks_total,
            started_at_unix: session.started_at_unix,
            ended_at_unix: session.ended_at_unix,
            bandwidth_bytes_per_sec: sanitize_f64(session.bandwidth_bytes_per_sec),
            repair_attempts: session.repair_attempts,
            repair_strategy: session.repair_strategy,
            progress_pct: sanitize_f64(session.progress_pct()),
        })
        .collect();
    normalized_sessions.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then(left.snapshot_id.cmp(&right.snapshot_id))
            .then(left.started_at_unix.cmp(&right.started_at_unix))
    });

    let payload = CommitmentPayload {
        snapshots: normalized_snapshots,
        checkpoints: normalized_checkpoints,
        sessions: normalized_sessions,
        stats: StatsCommitmentView {
            snapshots_available: stats.snapshots_available,
            checkpoints_verified: stats.checkpoints_verified,
            sync_sessions_completed: stats.sync_sessions_completed,
            avg_sync_duration_s: sanitize_f64(stats.avg_sync_duration_s),
            avg_bandwidth_mbps: sanitize_f64(stats.avg_bandwidth_mbps),
            repair_rate: sanitize_f64(stats.repair_rate),
            total_data_synced_gb: sanitize_f64(stats.total_data_synced_gb),
        },
    };

    let serialized = serde_json::to_vec(&payload)
        .expect("commitment payload serialization should not fail for concrete types");
    let digest = Sha256::digest(serialized);

    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn push_error(errors: &mut Vec<SnapshotSyncValidationError>, field: &str, message: &str) {
    errors.push(SnapshotSyncValidationError {
        field: field.to_string(),
        message: message.to_string(),
    });
}

fn has_duplicate_checkpoint_sources(sources: &[CheckpointSource]) -> bool {
    let mut seen = [false; 4];
    for source in sources {
        let idx = source_order(*source);
        if seen[idx] {
            return true;
        }
        seen[idx] = true;
    }
    false
}

fn source_order(source: CheckpointSource) -> usize {
    match source {
        CheckpointSource::Beacon => 0,
        CheckpointSource::ExecutionLayer => 1,
        CheckpointSource::ExternalOracle => 2,
        CheckpointSource::BootstrapNode => 3,
    }
}

fn sanitize_f64(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn average_or_zero(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum: f64 = samples.iter().sum();
    if !sum.is_finite() {
        0.0
    } else {
        sum / samples.len() as f64
    }
}

fn estimate_synced_bytes(session: &SyncSession, snapshot_size_by_id: &HashMap<&str, u64>) -> f64 {
    let Some(size_bytes) = snapshot_size_by_id.get(session.snapshot_id.as_str()) else {
        return 0.0;
    };

    if *size_bytes == 0 {
        return 0.0;
    }

    if session.chunks_total == 0 {
        if session.is_completed() {
            return *size_bytes as f64;
        }
        return *size_bytes as f64 * (session.progress_pct() / 100.0);
    }

    let completed_chunks = session
        .chunks_downloaded
        .max(session.chunks_verified)
        .min(session.chunks_total);

    let ratio = completed_chunks as f64 / session.chunks_total as f64;
    *size_bytes as f64 * ratio.clamp(0.0, 1.0)
}

fn bytes_per_sec_to_mbps(bytes_per_sec: f64) -> f64 {
    (bytes_per_sec * 8.0) / 1_000_000.0
}

fn bytes_to_gigabytes(bytes: f64) -> f64 {
    if !bytes.is_finite() || bytes <= 0.0 {
        0.0
    } else {
        bytes / 1_000_000_000.0
    }
}

fn is_zero_32(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}
