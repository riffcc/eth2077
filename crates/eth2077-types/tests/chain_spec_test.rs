use eth2077_types::chain_spec::{
    compute_chain_spec_commitment, compute_chain_spec_stats, default_chain_spec_config,
    validate_chain_spec_config, ChainSpecConfig, ConsensusType, ForkEntry, ForkPhase,
    NetworkKind, SpecStatus,
};
use std::collections::HashMap;

fn fork(name: &str, phase: ForkPhase, epoch: u64, eips: &[&str], consensus_change: bool) -> ForkEntry {
    ForkEntry {
        name: name.to_string(),
        phase,
        activation_epoch: epoch,
        eips: eips.iter().map(|value| value.to_string()).collect(),
        consensus_change,
        metadata: HashMap::new(),
    }
}

#[test]
fn enum_variants_are_constructible() {
    let networks = [NetworkKind::Mainnet, NetworkKind::Testnet, NetworkKind::Devnet, NetworkKind::Shadow, NetworkKind::Local, NetworkKind::Custom];
    let phases = [ForkPhase::Scheduled, ForkPhase::Activated, ForkPhase::Deprecated, ForkPhase::Cancelled, ForkPhase::Pending, ForkPhase::Emergency];
    let consensus = [ConsensusType::ProofOfStake, ConsensusType::CitadelOob, ConsensusType::Hybrid, ConsensusType::ProofOfAuthority, ConsensusType::Delegated, ConsensusType::Experimental];
    let status = [SpecStatus::Draft, SpecStatus::Frozen, SpecStatus::Released, SpecStatus::Superseded, SpecStatus::Revoked, SpecStatus::Archived];
    assert_eq!(networks.len(), 6);
    assert_eq!(phases.len(), 6);
    assert_eq!(consensus.len(), 6);
    assert_eq!(status.len(), 6);
}

#[test]
fn default_config_is_valid() {
    let config = default_chain_spec_config();
    assert_eq!(config.network, NetworkKind::Testnet);
    assert_eq!(config.consensus, ConsensusType::ProofOfStake);
    assert_eq!(validate_chain_spec_config(&config), Ok(()));
}

#[test]
fn validation_reports_multiple_errors() {
    let mut config = default_chain_spec_config();
    config.chain_id = 0;
    config.genesis_time = 0;
    config.slots_per_epoch = 0;
    config.seconds_per_slot = 0;
    config.network = NetworkKind::Custom;
    config.metadata.clear();
    config.metadata.insert(" ".to_string(), "x".to_string());

    let mut bad = fork(" ", ForkPhase::Scheduled, 0, &["", "EIP-1", "eip-1"], false);
    bad.metadata.insert(" ".to_string(), "x".to_string());
    config.forks = vec![bad, fork("dup", ForkPhase::Activated, 10, &["EIP-2"], false), fork("dup", ForkPhase::Pending, 9, &["EIP-3"], true)];

    let errors = validate_chain_spec_config(&config).unwrap_err();
    assert!(errors.iter().any(|e| e.field == "chain_id"));
    assert!(errors.iter().any(|e| e.field == "genesis_time"));
    assert!(errors.iter().any(|e| e.field == "slots_per_epoch"));
    assert!(errors.iter().any(|e| e.field == "seconds_per_slot"));
    assert!(errors.iter().any(|e| e.field == "metadata.spec_version"));
    assert!(errors.iter().any(|e| e.field == "metadata.network_name"));
    assert!(errors.iter().any(|e| e.field == "forks.name"));
    assert!(errors.iter().any(|e| e.field == "forks.activation_epoch"));
    assert!(errors.iter().any(|e| e.field == "forks.eips"));
    assert!(errors.iter().any(|e| e.field == "forks.metadata"));
}

#[test]
fn stats_computation_uses_unique_eips_and_version() {
    let mut config = ChainSpecConfig {
        chain_id: 9_001,
        network: NetworkKind::Testnet,
        consensus: ConsensusType::Hybrid,
        status: SpecStatus::Frozen,
        genesis_time: 1,
        slots_per_epoch: 8,
        seconds_per_slot: 3,
        forks: vec![
            fork("a", ForkPhase::Activated, 0, &["EIP-1", "eip-2"], false),
            fork("b", ForkPhase::Scheduled, 4, &["EIP-2", "EIP-3"], true),
            fork("c", ForkPhase::Pending, 8, &["EIP-3", "EIP-4"], false),
        ],
        metadata: HashMap::new(),
    };
    config.metadata.insert("spec_version".to_string(), "2026.2-rc1".to_string());

    let stats = compute_chain_spec_stats(&config);
    assert_eq!(stats.total_forks, 3);
    assert_eq!(stats.active_forks, 1);
    assert_eq!(stats.pending_forks, 2);
    assert_eq!(stats.eip_count, 4);
    assert!(stats.has_consensus_changes);
    assert_eq!(stats.spec_version, "2026.2-rc1");
}

#[test]
fn commitment_is_stable_and_data_sensitive() {
    let mut left = default_chain_spec_config();
    left.metadata.clear();
    left.metadata.insert("spec_version".to_string(), "v1".to_string());
    left.metadata.insert("b".to_string(), "2".to_string());
    left.metadata.insert("a".to_string(), "1".to_string());

    let mut right = left.clone();
    right.metadata.clear();
    right.metadata.insert("a".to_string(), "1".to_string());
    right.metadata.insert("spec_version".to_string(), "v1".to_string());
    right.metadata.insert("b".to_string(), "2".to_string());

    let left_commitment = compute_chain_spec_commitment(&left);
    let right_commitment = compute_chain_spec_commitment(&right);

    assert_eq!(left_commitment, right_commitment);
    assert_eq!(left_commitment.len(), 64);
    assert!(left_commitment.chars().all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase()));

    right.forks[0].activation_epoch += 1;
    assert_ne!(left_commitment, compute_chain_spec_commitment(&right));
}
