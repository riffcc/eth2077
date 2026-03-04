use eth2077_execution::eip7928::{
    compute_access_list_hash, compute_access_list_stats, merge_access_lists,
    validate_block_access_list, AccessedAccount, BlockAccessList, BlockAccessListError,
    StorageAccess,
};

fn address(byte: u8) -> [u8; 20] {
    [byte; 20]
}

fn word(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn base_list() -> BlockAccessList {
    BlockAccessList {
        block_number: 100,
        accessed_accounts: vec![AccessedAccount {
            address: address(0xAA),
            storage_keys: vec![word(0x01), word(0x02)],
            post_balance: Some(10),
            post_nonce: Some(1),
            post_code_hash: Some(word(0x09)),
        }],
        storage_accesses: vec![
            (
                address(0xAA),
                StorageAccess {
                    key: word(0x01),
                    pre_value: word(0x00),
                    post_value: word(0x01),
                },
            ),
            (
                address(0xAA),
                StorageAccess {
                    key: word(0x02),
                    pre_value: word(0x02),
                    post_value: word(0x02),
                },
            ),
        ],
    }
}

#[test]
fn valid_list_passes_validation() {
    let list = base_list();
    assert_eq!(validate_block_access_list(&list), Ok(()));
}

#[test]
fn empty_list_rejected() {
    let list = BlockAccessList {
        block_number: 100,
        accessed_accounts: Vec::new(),
        storage_accesses: Vec::new(),
    };

    assert_eq!(
        validate_block_access_list(&list),
        Err(vec![BlockAccessListError::EmptyAccessList])
    );
}

#[test]
fn duplicate_accounts_detected() {
    let mut list = base_list();
    let duplicate = AccessedAccount {
        address: address(0xAA),
        storage_keys: vec![word(0x03)],
        post_balance: Some(11),
        post_nonce: Some(2),
        post_code_hash: Some(word(0x0A)),
    };
    list.accessed_accounts.push(duplicate);

    let errors = validate_block_access_list(&list).expect_err("duplicate accounts should fail");
    assert!(errors.contains(&BlockAccessListError::DuplicateAccount {
        address: address(0xAA),
    }));
}

#[test]
fn duplicate_storage_keys_detected() {
    let mut list = base_list();
    list.accessed_accounts[0].storage_keys.push(word(0x01));

    let errors = validate_block_access_list(&list).expect_err("duplicate storage keys should fail");
    assert!(errors.contains(&BlockAccessListError::DuplicateStorageKey {
        address: address(0xAA),
        key: word(0x01),
    }));
}

#[test]
fn storage_access_without_account_detected() {
    let mut list = base_list();
    list.storage_accesses.push((
        address(0xBB),
        StorageAccess {
            key: word(0x09),
            pre_value: word(0x01),
            post_value: word(0x02),
        },
    ));

    let errors = validate_block_access_list(&list).expect_err("orphan storage access should fail");
    assert!(
        errors.contains(&BlockAccessListError::StorageAccessWithoutAccount {
            address: address(0xBB),
        })
    );
}

#[test]
fn hash_is_deterministic_for_equivalent_content() {
    let list_a = BlockAccessList {
        block_number: 7,
        accessed_accounts: vec![
            AccessedAccount {
                address: address(0x22),
                storage_keys: vec![word(0x03), word(0x01)],
                post_balance: Some(99),
                post_nonce: Some(2),
                post_code_hash: Some(word(0xAA)),
            },
            AccessedAccount {
                address: address(0x11),
                storage_keys: vec![word(0x02)],
                post_balance: None,
                post_nonce: Some(1),
                post_code_hash: None,
            },
        ],
        storage_accesses: vec![
            (
                address(0x22),
                StorageAccess {
                    key: word(0x03),
                    pre_value: word(0x01),
                    post_value: word(0x05),
                },
            ),
            (
                address(0x11),
                StorageAccess {
                    key: word(0x02),
                    pre_value: word(0x09),
                    post_value: word(0x09),
                },
            ),
        ],
    };

    let list_b = BlockAccessList {
        block_number: 7,
        accessed_accounts: vec![
            AccessedAccount {
                address: address(0x11),
                storage_keys: vec![word(0x02)],
                post_balance: None,
                post_nonce: Some(1),
                post_code_hash: None,
            },
            AccessedAccount {
                address: address(0x22),
                storage_keys: vec![word(0x01), word(0x03)],
                post_balance: Some(99),
                post_nonce: Some(2),
                post_code_hash: Some(word(0xAA)),
            },
        ],
        storage_accesses: vec![
            (
                address(0x11),
                StorageAccess {
                    key: word(0x02),
                    pre_value: word(0x09),
                    post_value: word(0x09),
                },
            ),
            (
                address(0x22),
                StorageAccess {
                    key: word(0x03),
                    pre_value: word(0x01),
                    post_value: word(0x05),
                },
            ),
        ],
    };

    let hash_a = compute_access_list_hash(&list_a);
    let hash_b = compute_access_list_hash(&list_b);
    assert_eq!(hash_a, hash_b);
}

#[test]
fn stats_compute_modified_and_read_only_counts() {
    let list = BlockAccessList {
        block_number: 123,
        accessed_accounts: vec![
            AccessedAccount {
                address: address(0x01),
                storage_keys: vec![word(0x0A), word(0x0B)],
                post_balance: None,
                post_nonce: None,
                post_code_hash: None,
            },
            AccessedAccount {
                address: address(0x02),
                storage_keys: vec![word(0x0C)],
                post_balance: None,
                post_nonce: None,
                post_code_hash: None,
            },
        ],
        storage_accesses: vec![
            (
                address(0x01),
                StorageAccess {
                    key: word(0x0A),
                    pre_value: word(0x00),
                    post_value: word(0x01),
                },
            ),
            (
                address(0x01),
                StorageAccess {
                    key: word(0x0B),
                    pre_value: word(0x02),
                    post_value: word(0x02),
                },
            ),
            (
                address(0x02),
                StorageAccess {
                    key: word(0x0C),
                    pre_value: word(0x03),
                    post_value: word(0x04),
                },
            ),
        ],
    };

    let stats = compute_access_list_stats(&list);
    assert_eq!(stats.block_number, 123);
    assert_eq!(stats.unique_account_count, 2);
    assert_eq!(stats.total_storage_accesses, 3);
    assert_eq!(stats.modified_storage_count, 2);
    assert_eq!(stats.read_only_storage_count, 1);
}

#[test]
fn merge_overlapping_access_lists_keeps_latest_values_and_dedupes() {
    let list_one = BlockAccessList {
        block_number: 77,
        accessed_accounts: vec![
            AccessedAccount {
                address: address(0xAA),
                storage_keys: vec![word(0x01)],
                post_balance: Some(1),
                post_nonce: Some(1),
                post_code_hash: Some(word(0x11)),
            },
            AccessedAccount {
                address: address(0xBB),
                storage_keys: vec![word(0x03)],
                post_balance: Some(2),
                post_nonce: Some(2),
                post_code_hash: Some(word(0x22)),
            },
        ],
        storage_accesses: vec![
            (
                address(0xAA),
                StorageAccess {
                    key: word(0x01),
                    pre_value: word(0x00),
                    post_value: word(0x01),
                },
            ),
            (
                address(0xBB),
                StorageAccess {
                    key: word(0x03),
                    pre_value: word(0x03),
                    post_value: word(0x04),
                },
            ),
        ],
    };

    let list_two = BlockAccessList {
        block_number: 77,
        accessed_accounts: vec![AccessedAccount {
            address: address(0xAA),
            storage_keys: vec![word(0x01), word(0x02)],
            post_balance: Some(9),
            post_nonce: Some(9),
            post_code_hash: Some(word(0x99)),
        }],
        storage_accesses: vec![
            (
                address(0xAA),
                StorageAccess {
                    key: word(0x01),
                    pre_value: word(0x01),
                    post_value: word(0x05),
                },
            ),
            (
                address(0xAA),
                StorageAccess {
                    key: word(0x02),
                    pre_value: word(0x00),
                    post_value: word(0x00),
                },
            ),
        ],
    };

    let merged = merge_access_lists(&[list_one, list_two]);

    assert_eq!(merged.block_number, 77);
    assert_eq!(merged.accessed_accounts.len(), 2);
    assert_eq!(merged.storage_accesses.len(), 3);

    let merged_account_aa = merged
        .accessed_accounts
        .iter()
        .find(|account| account.address == address(0xAA))
        .expect("account 0xAA should exist");
    assert_eq!(merged_account_aa.storage_keys, vec![word(0x01), word(0x02)]);
    assert_eq!(merged_account_aa.post_balance, Some(9));
    assert_eq!(merged_account_aa.post_nonce, Some(9));
    assert_eq!(merged_account_aa.post_code_hash, Some(word(0x99)));

    let merged_aa_key1 = merged
        .storage_accesses
        .iter()
        .find(|(address_value, access)| *address_value == address(0xAA) && access.key == word(0x01))
        .expect("account 0xAA key 0x01 should exist");
    assert_eq!(merged_aa_key1.1.pre_value, word(0x01));
    assert_eq!(merged_aa_key1.1.post_value, word(0x05));
}
