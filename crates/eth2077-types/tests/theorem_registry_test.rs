use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use eth2077_types::theorem_registry::{
    TheoremEntry, TheoremRegistry, TheoremRegistryValidationError, TheoremStatus, TheoremTier,
};

fn sample_registry() -> TheoremRegistry {
    TheoremRegistry {
        version: "1.0.0".to_string(),
        entries: vec![
            TheoremEntry {
                id: "THM-001".to_string(),
                name: "State root integrity".to_string(),
                description: "Applying a valid block transition yields a deterministic state root."
                    .to_string(),
                tier: TheoremTier::Tier1,
                status: TheoremStatus::Verified,
                dependencies: vec![],
                proof_artifact_path: Some(
                    "proofs/ETH2077Proofs/StateRootIntegrity.lean".to_string(),
                ),
                tags: vec!["state".to_string(), "safety".to_string()],
            },
            TheoremEntry {
                id: "THM-002".to_string(),
                name: "Transaction ordering determinism".to_string(),
                description:
                    "Given identical mempool snapshots and rules, ordering is deterministic."
                        .to_string(),
                tier: TheoremTier::Tier1,
                status: TheoremStatus::ProofInProgress,
                dependencies: vec!["THM-001".to_string()],
                proof_artifact_path: None,
                tags: vec!["ordering".to_string(), "safety".to_string()],
            },
            TheoremEntry {
                id: "THM-003".to_string(),
                name: "Finality liveness bound".to_string(),
                description:
                    "Under bounded delay and less than 1/3 Byzantine nodes, finality is eventual."
                        .to_string(),
                tier: TheoremTier::Tier2,
                status: TheoremStatus::Proposed,
                dependencies: vec!["THM-001".to_string()],
                proof_artifact_path: None,
                tags: vec!["liveness".to_string(), "finality".to_string()],
            },
            TheoremEntry {
                id: "THM-004".to_string(),
                name: "Batch commit throughput lower bound".to_string(),
                description:
                    "Batch commit strategy maintains minimum throughput under nominal load."
                        .to_string(),
                tier: TheoremTier::Tier2,
                status: TheoremStatus::ProofComplete,
                dependencies: vec![],
                proof_artifact_path: Some(
                    "proofs/ETH2077Proofs/BatchCommitThroughputLowerBound.lean".to_string(),
                ),
                tags: vec!["throughput".to_string(), "performance".to_string()],
            },
            TheoremEntry {
                id: "THM-005".to_string(),
                name: "Memory pool compaction efficiency".to_string(),
                description:
                    "Compaction preserves admission fairness while reducing memory pressure."
                        .to_string(),
                tier: TheoremTier::Tier3,
                status: TheoremStatus::Proposed,
                dependencies: vec![],
                proof_artifact_path: None,
                tags: vec!["mempool".to_string(), "optimization".to_string()],
            },
            TheoremEntry {
                id: "THM-006".to_string(),
                name: "Adaptive gossip fanout bound".to_string(),
                description: "Adaptive fanout converges while limiting redundant traffic."
                    .to_string(),
                tier: TheoremTier::Tier3,
                status: TheoremStatus::Rejected,
                dependencies: vec!["THM-004".to_string()],
                proof_artifact_path: None,
                tags: vec!["network".to_string(), "optimization".to_string()],
            },
        ],
    }
}

fn temp_registry_path() -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after UNIX_EPOCH")
        .as_nanos();
    env::temp_dir().join(format!("eth2077-theorem-registry-{ts}.json"))
}

#[test]
fn theorem_registry_json_round_trip() {
    let registry = sample_registry();
    let path = temp_registry_path();

    registry
        .save_to_json(&path)
        .expect("registry should serialize to JSON");
    let loaded = TheoremRegistry::load_from_json(&path).expect("registry should load from JSON");

    fs::remove_file(&path).expect("temporary test file should be removed");
    assert_eq!(loaded, registry);
}

#[test]
fn theorem_registry_validation_catches_duplicate_ids() {
    let mut registry = sample_registry();
    let duplicate = TheoremEntry {
        id: "THM-001".to_string(),
        name: "Duplicate theorem id".to_string(),
        description: "Duplicate id should fail validation.".to_string(),
        tier: TheoremTier::Tier1,
        status: TheoremStatus::Proposed,
        dependencies: vec![],
        proof_artifact_path: None,
        tags: vec!["test".to_string()],
    };
    registry.entries.push(duplicate);

    let errors = registry
        .validate()
        .expect_err("validation should fail for duplicate IDs");
    assert!(errors
        .iter()
        .any(|error| *error == TheoremRegistryValidationError::DuplicateId("THM-001".to_string())));
}

#[test]
fn theorem_registry_validation_catches_missing_dependency() {
    let mut registry = sample_registry();
    registry.entries[0].dependencies.push("THM-404".to_string());

    let errors = registry
        .validate()
        .expect_err("validation should fail for missing dependency");
    assert!(errors.iter().any(|error| {
        *error
            == TheoremRegistryValidationError::MissingDependency {
                theorem_id: "THM-001".to_string(),
                dependency_id: "THM-404".to_string(),
            }
    }));
}

#[test]
fn theorem_registry_filter_by_tier() {
    let registry = sample_registry();

    let tier1_entries = registry.get_by_tier(TheoremTier::Tier1);
    let tier2_entries = registry.get_by_tier(TheoremTier::Tier2);
    let tier3_entries = registry.get_by_tier(TheoremTier::Tier3);

    assert_eq!(tier1_entries.len(), 2);
    assert_eq!(tier2_entries.len(), 2);
    assert_eq!(tier3_entries.len(), 2);
    assert!(tier1_entries
        .iter()
        .all(|entry| entry.tier == TheoremTier::Tier1));
    assert!(tier2_entries
        .iter()
        .all(|entry| entry.tier == TheoremTier::Tier2));
    assert!(tier3_entries
        .iter()
        .all(|entry| entry.tier == TheoremTier::Tier3));
}
