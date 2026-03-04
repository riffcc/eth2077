use eth2077_types::snapshot_sync::{
    compute_stats, Checkpoint, CheckpointSource, RepairStrategy, Snapshot, SnapshotFormat,
    SnapshotSyncConfig, SyncPhase, SyncSession,
};

fn bytes32(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn mk_snapshot(id: &str, size_bytes: u64, format: SnapshotFormat, seed: u8) -> Snapshot {
    Snapshot {
        id: id.to_string(),
        slot: 1,
        state_root: bytes32(seed),
        format,
        size_bytes,
        chunk_count: 100,
        created_at_unix: 1,
        source_node: "node-x".to_string(),
        signature: vec![seed; 64],
    }
}

fn mk_session(id: &str, snapshot_id: &str) -> SyncSession {
    SyncSession {
        id: id.to_string(),
        snapshot_id: snapshot_id.to_string(),
        phase: SyncPhase::Download,
        chunks_downloaded: 0,
        chunks_verified: 0,
        chunks_total: 100,
        started_at_unix: 0,
        ended_at_unix: None,
        bandwidth_bytes_per_sec: 0.0,
        repair_attempts: 0,
        repair_strategy: None,
    }
}

#[test]
fn default_config_contract_and_validity() {
    let config = SnapshotSyncConfig::default();
    assert_eq!(config.max_snapshot_age_slots, 8192);
    assert_eq!(config.min_chunk_peers, 3);
    assert_eq!(config.max_concurrent_downloads, 8);
    assert_eq!(config.verification_parallelism, 4);
    assert_eq!(config.max_repair_attempts, 3);
    assert_eq!(config.preferred_format, SnapshotFormat::Full);
    assert_eq!(
        config.checkpoint_sources,
        vec![CheckpointSource::Beacon, CheckpointSource::ExecutionLayer]
    );
    assert_eq!(config.bandwidth_limit_mbps, None);
    assert!(config.validate().is_empty());
}

#[test]
fn validation_reports_multiple_fields() {
    let bad = SnapshotSyncConfig {
        max_snapshot_age_slots: 0,
        min_chunk_peers: 0,
        max_concurrent_downloads: 0,
        verification_parallelism: 0,
        max_repair_attempts: 0,
        preferred_format: SnapshotFormat::DiffBased,
        checkpoint_sources: vec![CheckpointSource::Beacon, CheckpointSource::Beacon],
        bandwidth_limit_mbps: Some(-1.0),
    };
    let errors = bad.validate();
    for field in [
        "max_snapshot_age_slots",
        "min_chunk_peers",
        "max_concurrent_downloads",
        "verification_parallelism",
        "max_repair_attempts",
        "preferred_format",
        "checkpoint_sources",
        "bandwidth_limit_mbps",
    ] {
        assert!(errors.iter().any(|err| err.field == field));
    }
}

#[test]
fn progress_pct_behavior() {
    let partial = SyncSession {
        chunks_downloaded: 50,
        chunks_verified: 25,
        ..mk_session("s0", "snap-a")
    };
    assert!((partial.progress_pct() - 42.5).abs() < 1e-9);

    let app_phase = SyncSession {
        phase: SyncPhase::Application,
        ..partial.clone()
    };
    assert!((app_phase.progress_pct() - 80.0).abs() < 1e-9);

    let done = SyncSession {
        phase: SyncPhase::Finalization,
        ended_at_unix: Some(9),
        chunks_downloaded: 100,
        chunks_verified: 100,
        ..partial
    };
    assert!((done.progress_pct() - 100.0).abs() < 1e-9);
}

#[test]
fn stats_and_commitment_are_stable() {
    let snapshots = vec![
        mk_snapshot("snap-a", 10_000_000_000, SnapshotFormat::Full, 0xA1),
        mk_snapshot("snap-b", 5_000_000_000, SnapshotFormat::Pruned, 0xB2),
    ];
    let checkpoints = vec![
        Checkpoint {
            slot: 10,
            block_root: bytes32(0x11),
            state_root: bytes32(0x22),
            source: CheckpointSource::Beacon,
            finalized: true,
            verified_by_count: 3,
            timestamp_unix: 100,
        },
        Checkpoint {
            slot: 11,
            block_root: [0u8; 32],
            state_root: bytes32(0x23),
            source: CheckpointSource::ExecutionLayer,
            finalized: false,
            verified_by_count: 0,
            timestamp_unix: 101,
        },
    ];
    let sessions = vec![
        SyncSession {
            phase: SyncPhase::Finalization,
            chunks_downloaded: 100,
            chunks_verified: 100,
            ended_at_unix: Some(100),
            bandwidth_bytes_per_sec: 25_000_000.0,
            ..mk_session("a", "snap-a")
        },
        SyncSession {
            phase: SyncPhase::Verification,
            chunks_downloaded: 25,
            chunks_verified: 20,
            chunks_total: 50,
            bandwidth_bytes_per_sec: 12_500_000.0,
            repair_attempts: 2,
            repair_strategy: Some(RepairStrategy::PeerAssist),
            ..mk_session("b", "snap-b")
        },
        SyncSession {
            phase: SyncPhase::Discovery,
            chunks_total: 0,
            snapshot_id: "missing".to_string(),
            bandwidth_bytes_per_sec: -5.0,
            ..mk_session("c", "missing")
        },
    ];

    let stats = compute_stats(&snapshots, &checkpoints, &sessions);
    assert_eq!(stats.snapshots_available, 2);
    assert_eq!(stats.checkpoints_verified, 1);
    assert_eq!(stats.sync_sessions_completed, 1);
    assert!((stats.avg_sync_duration_s - 100.0).abs() < 1e-9);
    assert!((stats.avg_bandwidth_mbps - 100.0).abs() < 1e-9);
    assert!((stats.repair_rate - (1.0 / 3.0)).abs() < 1e-12);
    assert!((stats.total_data_synced_gb - 12.5).abs() < 1e-9);
    assert_ne!(stats.commitment, [0u8; 32]);

    let mut reversed = sessions.clone();
    reversed.reverse();
    assert_eq!(
        stats.commitment,
        compute_stats(&snapshots, &checkpoints, &reversed).commitment
    );

    let mut changed = sessions;
    changed[1].repair_attempts = 3;
    assert_ne!(
        stats.commitment,
        compute_stats(&snapshots, &checkpoints, &changed).commitment
    );
}
