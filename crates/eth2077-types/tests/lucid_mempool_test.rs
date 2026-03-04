use eth2077_types::lucid_mempool::{
    compute_envelope_hash, compute_lucid_stats, default_lucid_config, validate_decryption_shares,
    validate_encrypted_transaction, DecryptionShare, DecryptionTrigger, EncryptedTransaction,
    EncryptionScheme, LucidValidationError,
};

fn tx(scheme: EncryptionScheme, payload: Vec<u8>, gas_ceiling: u64) -> EncryptedTransaction {
    EncryptedTransaction {
        envelope_hash: [0xAA; 32],
        encrypted_payload: payload,
        scheme,
        decryption_trigger: DecryptionTrigger::BlockInclusion,
        sender_commitment: [0x11; 32],
        gas_ceiling,
        priority_fee_ceiling: 2_000_000_000,
    }
}

#[test]
fn default_config_values() {
    let config = default_lucid_config();

    assert_eq!(config.committee_size, 21);
    assert_eq!(config.threshold, 14);
    assert_eq!(config.default_scheme, EncryptionScheme::ThresholdBLS);
    assert_eq!(config.default_trigger, DecryptionTrigger::BlockInclusion);
    assert_eq!(config.max_encrypted_size, 131_072);
    assert_eq!(config.decryption_timeout_slots, 32);
}

#[test]
fn valid_encrypted_tx_passes() {
    let config = default_lucid_config();
    let tx = tx(EncryptionScheme::ThresholdBLS, vec![1, 2, 3], 50_000);

    assert_eq!(validate_encrypted_transaction(&tx, &config), Ok(()));
}

#[test]
fn empty_payload_rejected() {
    let config = default_lucid_config();
    let tx = tx(EncryptionScheme::ThresholdBLS, Vec::new(), 50_000);
    let errors = validate_encrypted_transaction(&tx, &config).unwrap_err();

    assert!(errors.contains(&LucidValidationError::EmptyPayload));
}

#[test]
fn payload_too_large_rejected() {
    let mut config = default_lucid_config();
    config.max_encrypted_size = 8;

    let tx = tx(EncryptionScheme::ThresholdBLS, vec![0xFF; 9], 50_000);
    let errors = validate_encrypted_transaction(&tx, &config).unwrap_err();

    assert!(
        errors
            .iter()
            .any(|error| matches!(error, LucidValidationError::PayloadTooLarge { size: 9, max: 8 }))
    );
}

#[test]
fn sufficient_decryption_shares_pass() {
    let mut config = default_lucid_config();
    config.threshold = 3;

    let shares = vec![
        DecryptionShare {
            committee_member: [1; 32],
            share_data: vec![0x01],
            slot_number: 10,
        },
        DecryptionShare {
            committee_member: [2; 32],
            share_data: vec![0x02],
            slot_number: 10,
        },
        DecryptionShare {
            committee_member: [3; 32],
            share_data: vec![0x03],
            slot_number: 10,
        },
    ];

    assert_eq!(validate_decryption_shares(&shares, &config), Ok(()));
}

#[test]
fn insufficient_shares_rejected() {
    let mut config = default_lucid_config();
    config.threshold = 3;

    let shares = vec![
        DecryptionShare {
            committee_member: [1; 32],
            share_data: vec![0x01],
            slot_number: 10,
        },
        DecryptionShare {
            committee_member: [2; 32],
            share_data: vec![0x02],
            slot_number: 10,
        },
    ];

    let errors = validate_decryption_shares(&shares, &config).unwrap_err();

    assert!(errors.contains(&LucidValidationError::InsufficientShares {
        provided: 2,
        required: 3,
    }));
}

#[test]
fn envelope_hash_deterministic() {
    let base = tx(EncryptionScheme::ShutterStyle, vec![0xAB, 0xCD], 75_000);
    let first = compute_envelope_hash(&base);
    let second = compute_envelope_hash(&base);

    assert_eq!(first, second);
}

#[test]
fn stats_computation_correct() {
    let config = default_lucid_config();
    let txs = vec![
        tx(EncryptionScheme::ThresholdBLS, vec![1, 2], 100_000),
        tx(EncryptionScheme::IdentityBased, vec![3], 200_000),
        tx(EncryptionScheme::CommitReveal, vec![4, 5, 6], 300_000),
        tx(EncryptionScheme::ShutterStyle, vec![7], 400_000),
    ];

    let stats = compute_lucid_stats(&txs, &config);

    assert_eq!(stats.total_encrypted_txs, 4);
    assert_eq!(stats.total_payload_bytes, 7);
    assert_eq!(stats.avg_gas_ceiling, 250_000.0);
    assert_eq!(stats.mev_protection_ratio, 1.0);
    assert_eq!(stats.latency_overhead_ms, 10.0);

    let dist = stats.scheme_distribution;
    assert_eq!(dist[0], ("ThresholdBLS".to_string(), 1));
    assert_eq!(dist[1], ("IdentityBased".to_string(), 1));
    assert_eq!(dist[2], ("CommitReveal".to_string(), 1));
    assert_eq!(dist[3], ("ShutterStyle".to_string(), 1));
}
