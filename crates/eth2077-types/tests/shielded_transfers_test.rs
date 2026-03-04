use eth2077_types::shielded_transfers::*;
use std::collections::HashSet;

fn assert_close(left: f64, right: f64, eps: f64) {
    assert!(
        (left - right).abs() <= eps,
        "left={left}, right={right}, eps={eps}"
    );
}

fn fast_config() -> ShieldedTransferConfig {
    ShieldedTransferConfig {
        protocol: PrivacyProtocol::StealthAddress,
        privacy_level: PrivacyLevel::AmountPrivate,
        compliance_mode: ComplianceMode::ViewKeyOptIn,
        proof_size_bytes: 4_096,
        proving_time_ms: 220.0,
        verification_time_ms: 8.0,
        max_anonymity_set: 8_192,
        supports_programmability: true,
    }
}

#[test]
fn default_config_is_valid() {
    let config = default_shielded_config();

    assert_eq!(config.protocol, PrivacyProtocol::ZKShielded);
    assert_eq!(config.privacy_level, PrivacyLevel::FullyShielded);
    assert_eq!(config.compliance_mode, ComplianceMode::ZKCompliance);
    assert!(config.proof_size_bytes > 0);
    assert!(config.proving_time_ms > 0.0);
    assert!(config.max_anonymity_set >= 64);
    assert!(config.supports_programmability);
    assert_eq!(validate_shielded_config(&config), Ok(()));
}

#[test]
fn validation_rejects_oversized_proof() {
    let mut config = default_shielded_config();
    config.proof_size_bytes = 200_000;

    let errors = validate_shielded_config(&config).expect_err("expected validation failure");
    assert!(
        errors.contains(&ShieldedValidationError::ProofSizeTooLarge {
            size: 200_000,
            max: 131_072
        })
    );
}

#[test]
fn validation_rejects_slow_proving() {
    let mut config = default_shielded_config();
    config.proving_time_ms = 25_000.0;

    let errors = validate_shielded_config(&config).expect_err("expected validation failure");
    assert!(
        errors.contains(&ShieldedValidationError::ProvingTimeTooHigh {
            ms: 25_000.0,
            max: 20_000.0
        })
    );
}

#[test]
fn validation_rejects_small_anonymity_set() {
    let mut config = default_shielded_config();
    config.max_anonymity_set = 16;

    let errors = validate_shielded_config(&config).expect_err("expected validation failure");
    assert!(errors.contains(&ShieldedValidationError::AnonymitySetTooSmall { size: 16, min: 64 }));
}

#[test]
fn stats_show_positive_throughput() {
    let config = fast_config();
    let stats = compute_shielded_stats(&config);

    assert!(stats.throughput_tps > 0.0);
    assert!(stats.proof_overhead_pct > 0.0);
    assert!((0.0..=100.0).contains(&stats.privacy_score));
    assert!(!stats.bottleneck.is_empty());
    assert!(stats.gas_cost_estimate > 21_000);
    assert!(stats.effective_anonymity_set > 0);
    assert!(stats.compliance_compatible);
}

#[test]
fn compare_protocols_returns_all_variants() {
    let config = default_shielded_config();
    let compared = compare_privacy_protocols(&config);

    assert_eq!(compared.len(), 6);
    let names: HashSet<String> = compared.iter().map(|(name, _)| name.clone()).collect();
    let expected: HashSet<String> = vec![
        "ZKShielded".to_string(),
        "RingSignature".to_string(),
        "StealthAddress".to_string(),
        "ConfidentialTransaction".to_string(),
        "MixerBased".to_string(),
        "FullHomomorphic".to_string(),
    ]
    .into_iter()
    .collect();
    assert_eq!(names, expected);

    for (_, stats) in compared {
        assert!(stats.throughput_tps > 0.0);
        assert!(stats.gas_cost_estimate > 0);
    }
}

#[test]
fn anonymity_set_varies_by_protocol() {
    let max_set = 10_000;
    let zk = estimate_anonymity_set(PrivacyProtocol::ZKShielded, max_set);
    let ring = estimate_anonymity_set(PrivacyProtocol::RingSignature, max_set);
    let stealth = estimate_anonymity_set(PrivacyProtocol::StealthAddress, max_set);
    let confidential = estimate_anonymity_set(PrivacyProtocol::ConfidentialTransaction, max_set);
    let mixer = estimate_anonymity_set(PrivacyProtocol::MixerBased, max_set);
    let fhe = estimate_anonymity_set(PrivacyProtocol::FullHomomorphic, max_set);

    assert!(stealth < confidential);
    assert!(confidential < mixer);
    assert!(mixer < zk);
    assert!(zk < fhe);
    assert!(ring < zk);
    assert!(ring > stealth);
    for value in [zk, ring, stealth, confidential, mixer, fhe] {
        assert!(value > 0);
        assert!(value <= max_set);
    }
}

#[test]
fn commitment_is_deterministic_and_sensitive() {
    let config = default_shielded_config();
    let first = compute_shielded_commitment(&config);
    let second = compute_shielded_commitment(&config);
    assert_eq!(first, second);

    let mut changed_privacy = config.clone();
    changed_privacy.privacy_level = PrivacyLevel::AmountPrivate;
    let changed_privacy_hash = compute_shielded_commitment(&changed_privacy);
    assert_ne!(first, changed_privacy_hash);

    let mut changed_proof = config.clone();
    changed_proof.proof_size_bytes += 1;
    let changed_proof_hash = compute_shielded_commitment(&changed_proof);
    assert_ne!(first, changed_proof_hash);
}

#[test]
fn validation_rejects_incompatible_compliance_mode() {
    let mut config = default_shielded_config();
    config.protocol = PrivacyProtocol::FullHomomorphic;
    config.compliance_mode = ComplianceMode::RegulatoryBackdoor;

    let errors = validate_shielded_config(&config).expect_err("expected validation failure");
    assert!(errors.iter().any(|error| matches!(
        error,
        ShieldedValidationError::IncompatibleComplianceMode { protocol, mode }
        if protocol == "FullHomomorphic" && mode == "RegulatoryBackdoor"
    )));
}

#[test]
fn stats_reflect_compliance_compatibility() {
    let mut compatible = default_shielded_config();
    compatible.protocol = PrivacyProtocol::ConfidentialTransaction;
    compatible.compliance_mode = ComplianceMode::ZKCompliance;
    let ok_stats = compute_shielded_stats(&compatible);
    assert!(ok_stats.compliance_compatible);

    let mut incompatible = compatible.clone();
    incompatible.protocol = PrivacyProtocol::MixerBased;
    incompatible.compliance_mode = ComplianceMode::ZKCompliance;
    let bad_stats = compute_shielded_stats(&incompatible);
    assert!(!bad_stats.compliance_compatible);
}

#[test]
fn protocol_comparison_shows_tradeoffs() {
    let config = fast_config();
    let compared = compare_privacy_protocols(&config);

    let stealth = compared
        .iter()
        .find(|(name, _)| name == "StealthAddress")
        .expect("missing StealthAddress")
        .1
        .clone();
    let fhe = compared
        .iter()
        .find(|(name, _)| name == "FullHomomorphic")
        .expect("missing FullHomomorphic")
        .1
        .clone();
    let zk = compared
        .iter()
        .find(|(name, _)| name == "ZKShielded")
        .expect("missing ZKShielded")
        .1
        .clone();

    assert!(fhe.effective_anonymity_set >= zk.effective_anonymity_set);
    assert!(fhe.gas_cost_estimate > stealth.gas_cost_estimate);
    assert!(stealth.throughput_tps > fhe.throughput_tps);
    assert!(zk.privacy_score >= stealth.privacy_score - 5.0);
}

#[test]
fn estimates_handle_zero_cap() {
    for protocol in [
        PrivacyProtocol::ZKShielded,
        PrivacyProtocol::RingSignature,
        PrivacyProtocol::StealthAddress,
        PrivacyProtocol::ConfidentialTransaction,
        PrivacyProtocol::MixerBased,
        PrivacyProtocol::FullHomomorphic,
    ] {
        assert_eq!(estimate_anonymity_set(protocol, 0), 0);
    }
}

#[test]
fn higher_proving_time_reduces_throughput() {
    let fast = fast_config();
    let fast_stats = compute_shielded_stats(&fast);

    let mut slow = fast.clone();
    slow.proving_time_ms = fast.proving_time_ms * 10.0;
    let slow_stats = compute_shielded_stats(&slow);

    assert!(slow_stats.throughput_tps < fast_stats.throughput_tps);
    assert_close(
        slow_stats.effective_anonymity_set as f64,
        fast_stats.effective_anonymity_set as f64,
        0.0,
    );
}
