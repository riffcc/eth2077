use eth2077_types::genesis_builder::{
    compute_genesis_builder_commitment, compute_genesis_builder_stats,
    default_genesis_builder_config, validate_genesis_builder_config, AllocationKind,
    ArtifactFormat, GenesisAllocation, GenesisBuilderConfig, SigningScheme,
};
use std::collections::HashMap;

fn alloc(
    address: &str,
    kind: AllocationKind,
    amount_gwei: u64,
    is_validator: bool,
) -> GenesisAllocation {
    GenesisAllocation {
        address: address.to_string(),
        kind,
        amount_gwei,
        is_validator,
        metadata: HashMap::new(),
    }
}

#[test]
fn default_and_validation_basics() {
    let config = default_genesis_builder_config();
    assert_eq!(config.chain_id, 2077);
    assert_eq!(config.signing_scheme, SigningScheme::Aggregate);
    assert_eq!(config.artifact_format, ArtifactFormat::Ssz);
    assert_eq!(validate_genesis_builder_config(&config), Ok(()));
}

#[test]
fn validation_collects_multiple_errors() {
    let config = GenesisBuilderConfig {
        chain_id: 0,
        genesis_time: 0,
        validator_count: 0,
        signing_scheme: SigningScheme::MultiSig,
        artifact_format: ArtifactFormat::HexEncoded,
        deterministic_seed: " ".to_string(),
        min_deposit_gwei: 0,
        metadata: HashMap::new(),
    };

    let errors = validate_genesis_builder_config(&config).unwrap_err();
    assert!(errors.iter().any(|e| e.field == "chain_id"));
    assert!(errors.iter().any(|e| e.field == "genesis_time"));
    assert!(errors.iter().any(|e| e.field == "validator_count"));
    assert!(errors.iter().any(|e| e.field == "deterministic_seed"));
    assert!(errors.iter().any(|e| e.field == "min_deposit_gwei"));
    assert!(errors
        .iter()
        .any(|e| e.field == "metadata.multisig.participants"));
    assert!(errors
        .iter()
        .any(|e| e.field == "metadata.multisig.threshold"));
    assert!(errors.iter().any(|e| e.field == "metadata.hex.case"));
}

#[test]
fn stats_are_deterministic_and_capture_signing_readiness() {
    let mut config = default_genesis_builder_config();
    config.validator_count = 2;

    let ordered = vec![
        alloc(
            "0xAAA0000000000000000000000000000000000000",
            AllocationKind::ValidatorDeposit,
            32_000_000_000,
            true,
        ),
        alloc(
            "0xbbb0000000000000000000000000000000000000",
            AllocationKind::ValidatorDeposit,
            32_000_000_000,
            true,
        ),
        alloc(
            "0xAaA0000000000000000000000000000000000000",
            AllocationKind::Treasury,
            5_000_000_000,
            false,
        ),
    ];
    let reversed = vec![ordered[2].clone(), ordered[1].clone(), ordered[0].clone()];

    let stats_ordered = compute_genesis_builder_stats(&ordered, &config);
    let stats_reversed = compute_genesis_builder_stats(&reversed, &config);
    assert_eq!(stats_ordered.total_allocations, 3);
    assert_eq!(stats_ordered.validator_deposits, 2);
    assert_eq!(stats_ordered.total_gwei, 69_000_000_000);
    assert_eq!(stats_ordered.unique_addresses, 2);
    assert!(stats_ordered.signing_complete);
    assert_eq!(stats_ordered.artifact_hash.len(), 64);
    assert_eq!(stats_ordered.artifact_hash, stats_reversed.artifact_hash);

    let mut insufficient = ordered.clone();
    insufficient[1].amount_gwei = 1;
    let stats_insufficient = compute_genesis_builder_stats(&insufficient, &config);
    assert!(!stats_insufficient.signing_complete);
}

#[test]
fn commitment_is_deterministic_and_sensitive() {
    let mut first = default_genesis_builder_config();
    first.metadata.clear();
    first.metadata.insert("b".to_string(), "2".to_string());
    first.metadata.insert("a".to_string(), "1".to_string());

    let mut second = first.clone();
    second.metadata.clear();
    second.metadata.insert("a".to_string(), "1".to_string());
    second.metadata.insert("b".to_string(), "2".to_string());

    let h1 = compute_genesis_builder_commitment(&first);
    let h2 = compute_genesis_builder_commitment(&second);
    assert_eq!(h1, h2);

    second.chain_id += 1;
    let h3 = compute_genesis_builder_commitment(&second);
    assert_ne!(h1, h3);
}
