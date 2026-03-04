use eth2077_types::validator_ceremony::{
    compute_validator_ceremony_commitment, compute_validator_ceremony_stats,
    default_validator_ceremony_config, validate_validator_ceremony_config, KeyStatus,
    SecurityLevel, SignerProtocol, ValidatorCeremonyConfig, ValidatorKey,
};
use std::collections::HashMap;

fn key(id: &str, status: KeyStatus, signer: SignerProtocol, generated_at: u64) -> ValidatorKey {
    ValidatorKey {
        id: id.to_string(),
        pubkey: format!("0xpubkey_{id}"),
        status,
        signer,
        security: SecurityLevel::MultiParty,
        generated_at,
        metadata: HashMap::new(),
    }
}

#[test]
fn default_config_matches_expected_profile_and_validates() {
    let config = default_validator_ceremony_config();
    assert_eq!(config.protocol, SignerProtocol::ThresholdBls);
    assert_eq!(config.security, SecurityLevel::MultiParty);
    assert!(config.require_air_gap);
    assert!(config.metadata.contains_key("profile"));
    assert_eq!(validate_validator_ceremony_config(&config), Ok(()));
}

#[test]
fn invalid_config_reports_multiple_field_errors() {
    let config = ValidatorCeremonyConfig {
        validator_count: 0,
        threshold: 1,
        protocol: SignerProtocol::Distributed,
        security: SecurityLevel::Standard,
        rotation_period_days: 0,
        require_air_gap: true,
        metadata: HashMap::new(),
    };
    let errors = validate_validator_ceremony_config(&config).unwrap_err();
    assert!(errors.iter().any(|e| e.field == "validator_count"));
    assert!(errors.iter().any(|e| e.field == "threshold"));
    assert!(errors.iter().any(|e| e.field == "rotation_period_days"));
    assert!(errors.iter().any(|e| e.field == "security"));
}

#[test]
fn stats_compute_distribution_age_and_completion_semantics() {
    let completed = vec![
        key("k1", KeyStatus::Active, SignerProtocol::Local, 172_800),
        key("k2", KeyStatus::Revoked, SignerProtocol::RemoteGrpc, 86_400),
        key("k3", KeyStatus::Expired, SignerProtocol::Local, 0),
    ];
    let stats = compute_validator_ceremony_stats(&completed);
    assert_eq!(stats.total_keys, 3);
    assert_eq!(stats.active_keys, 1);
    assert_eq!(stats.revoked_keys, 1);
    assert!((stats.avg_key_age_days - 1.0).abs() < 1e-12);
    assert_eq!(stats.protocol_distribution.get("Local"), Some(&2));
    assert_eq!(stats.protocol_distribution.get("RemoteGrpc"), Some(&1));
    assert!(stats.ceremony_complete);

    let incomplete = vec![
        key("k4", KeyStatus::Active, SignerProtocol::Hsm, 20),
        key("k5", KeyStatus::Generated, SignerProtocol::Hsm, 10),
    ];
    assert!(!compute_validator_ceremony_stats(&incomplete).ceremony_complete);
}

#[test]
fn commitment_is_deterministic_and_changes_when_config_changes() {
    let mut metadata_a = HashMap::new();
    metadata_a.insert("region".to_string(), "eu-west".to_string());
    metadata_a.insert("batch".to_string(), "a1".to_string());
    let mut metadata_b = HashMap::new();
    metadata_b.insert("batch".to_string(), "a1".to_string());
    metadata_b.insert("region".to_string(), "eu-west".to_string());

    let base = ValidatorCeremonyConfig {
        validator_count: 16,
        threshold: 11,
        protocol: SignerProtocol::ThresholdBls,
        security: SecurityLevel::MultiParty,
        rotation_period_days: 60,
        require_air_gap: true,
        metadata: metadata_a,
    };
    let same_semantics_different_insertion = ValidatorCeremonyConfig {
        metadata: metadata_b,
        ..base.clone()
    };
    let commitment_a = compute_validator_ceremony_commitment(&base);
    let commitment_b = compute_validator_ceremony_commitment(&same_semantics_different_insertion);
    assert_eq!(commitment_a, commitment_b);
    assert_eq!(commitment_a.len(), 64);

    let mut changed = base.clone();
    changed.threshold = 12;
    assert_ne!(
        commitment_a,
        compute_validator_ceremony_commitment(&changed)
    );
}
