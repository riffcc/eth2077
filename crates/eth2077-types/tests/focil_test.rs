use eth2077_types::focil::{
    check_block_satisfies_inclusion_list, compute_inclusion_stats, validate_inclusion_list,
    InclusionList, InclusionListEntry, InclusionListValidationError,
};

fn hash32(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn entry(
    validator_index: u64,
    tx_byte: u8,
    gas_limit: u64,
    max_fee_per_gas: u128,
    inclusion_deadline_slot: u64,
) -> InclusionListEntry {
    InclusionListEntry {
        validator_index,
        tx_hash: hash32(tx_byte),
        gas_limit,
        max_fee_per_gas,
        inclusion_deadline_slot,
    }
}

#[test]
fn valid_list_passes_validation() {
    let list = InclusionList {
        slot: 100,
        entries: vec![
            entry(1, 1, 21_000, 100, 101),
            entry(2, 2, 30_000, 120, 102),
        ],
        aggregate_signature: Some(vec![0xAB, 0xCD]),
    };

    assert_eq!(validate_inclusion_list(&list, 100, 16, 100_000), Ok(()));
}

#[test]
fn empty_list_rejected() {
    let list = InclusionList {
        slot: 100,
        entries: vec![],
        aggregate_signature: None,
    };

    let errors = validate_inclusion_list(&list, 100, 16, 100_000).expect_err("expected errors");
    assert!(errors.contains(&InclusionListValidationError::EmptyList));
}

#[test]
fn duplicate_tx_hashes_detected() {
    let list = InclusionList {
        slot: 100,
        entries: vec![
            entry(1, 7, 21_000, 100, 101),
            entry(2, 7, 21_000, 100, 101),
        ],
        aggregate_signature: None,
    };

    let errors = validate_inclusion_list(&list, 100, 16, 100_000).expect_err("expected errors");
    assert!(errors
        .iter()
        .any(|error| matches!(error, InclusionListValidationError::DuplicateTxHashes { tx_hash } if *tx_hash == hash32(7))));
}

#[test]
fn expired_deadline_detected() {
    let list = InclusionList {
        slot: 100,
        entries: vec![entry(1, 9, 21_000, 100, 99)],
        aggregate_signature: None,
    };

    let errors = validate_inclusion_list(&list, 100, 16, 100_000).expect_err("expected errors");
    assert!(errors.iter().any(|error| matches!(
        error,
        InclusionListValidationError::ExpiredDeadline {
            tx_hash,
            deadline_slot,
            current_slot
        } if *tx_hash == hash32(9) && *deadline_slot == 99 && *current_slot == 100
    )));
}

#[test]
fn gas_limit_exceeded_detected() {
    let list = InclusionList {
        slot: 100,
        entries: vec![
            entry(1, 1, 60_000, 100, 101),
            entry(2, 2, 60_000, 120, 102),
        ],
        aggregate_signature: None,
    };

    let errors = validate_inclusion_list(&list, 100, 16, 100_000).expect_err("expected errors");
    assert!(errors.iter().any(|error| matches!(
        error,
        InclusionListValidationError::GasLimitExceeded {
            total_requested_gas,
            block_gas_limit
        } if *total_requested_gas == 120_000 && *block_gas_limit == 100_000
    )));
}

#[test]
fn block_satisfaction_check_with_all_included() {
    let required = InclusionList {
        slot: 100,
        entries: vec![entry(1, 1, 21_000, 100, 101), entry(2, 2, 21_000, 100, 101)],
        aggregate_signature: None,
    };

    let included = vec![hash32(1), hash32(2), hash32(3)];
    let missing = check_block_satisfies_inclusion_list(&included, &required);
    assert!(missing.is_empty());
}

#[test]
fn block_satisfaction_check_with_missing_entries() {
    let required = InclusionList {
        slot: 100,
        entries: vec![entry(1, 1, 21_000, 100, 101), entry(2, 2, 21_000, 100, 101)],
        aggregate_signature: None,
    };

    let included = vec![hash32(1)];
    let missing = check_block_satisfies_inclusion_list(&included, &required);
    assert_eq!(missing, vec![hash32(2)]);
}

#[test]
fn stats_computation() {
    let list = InclusionList {
        slot: 200,
        entries: vec![
            entry(1, 1, 21_000, 100, 201),
            entry(1, 2, 30_000, 200, 202),
            entry(2, 2, 40_000, 300, 203),
        ],
        aggregate_signature: Some(vec![0x01]),
    };

    let stats = compute_inclusion_stats(&list);
    assert_eq!(stats.slot, 200);
    assert_eq!(stats.entry_count, 3);
    assert_eq!(stats.unique_validator_count, 2);
    assert_eq!(stats.unique_tx_count, 2);
    assert_eq!(stats.total_gas_limit, 91_000);
    assert_eq!(stats.min_deadline_slot, Some(201));
    assert_eq!(stats.max_deadline_slot, Some(203));
    assert_eq!(stats.average_max_fee_per_gas, 200);
    assert!(stats.has_aggregate_signature);
}
