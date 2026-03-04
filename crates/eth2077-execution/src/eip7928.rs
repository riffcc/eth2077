use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AccessedAccount {
    pub address: [u8; 20],
    pub storage_keys: Vec<[u8; 32]>,
    pub post_balance: Option<u128>,
    pub post_nonce: Option<u64>,
    pub post_code_hash: Option<[u8; 32]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageAccess {
    pub key: [u8; 32],
    pub pre_value: [u8; 32],
    pub post_value: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockAccessList {
    pub block_number: u64,
    pub accessed_accounts: Vec<AccessedAccount>,
    pub storage_accesses: Vec<(/* address */ [u8; 20], StorageAccess)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlockAccessListError {
    DuplicateAccount { address: [u8; 20] },
    DuplicateStorageKey { address: [u8; 20], key: [u8; 32] },
    EmptyAccessList,
    StorageAccessWithoutAccount { address: [u8; 20] },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessListStats {
    pub block_number: u64,
    pub unique_account_count: usize,
    pub total_storage_accesses: usize,
    pub modified_storage_count: usize,
    pub read_only_storage_count: usize,
}

pub fn validate_block_access_list(list: &BlockAccessList) -> Result<(), Vec<BlockAccessListError>> {
    let mut errors = Vec::new();
    let mut seen_accounts = HashSet::new();
    let mut known_accounts = HashSet::new();

    if list.accessed_accounts.is_empty() {
        errors.push(BlockAccessListError::EmptyAccessList);
    }

    for account in &list.accessed_accounts {
        if !seen_accounts.insert(account.address) {
            errors.push(BlockAccessListError::DuplicateAccount {
                address: account.address,
            });
        }

        known_accounts.insert(account.address);

        let mut seen_storage_keys = HashSet::new();
        for key in &account.storage_keys {
            if !seen_storage_keys.insert(*key) {
                errors.push(BlockAccessListError::DuplicateStorageKey {
                    address: account.address,
                    key: *key,
                });
            }
        }
    }

    let mut seen_storage_access_keys = HashSet::new();
    for (address, storage_access) in &list.storage_accesses {
        if !known_accounts.contains(address) {
            errors.push(BlockAccessListError::StorageAccessWithoutAccount { address: *address });
        }

        if !seen_storage_access_keys.insert((*address, storage_access.key)) {
            errors.push(BlockAccessListError::DuplicateStorageKey {
                address: *address,
                key: storage_access.key,
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn compute_access_list_hash(list: &BlockAccessList) -> [u8; 32] {
    let mut hasher = Sha256::new();

    hasher.update(b"eip7928");
    hasher.update(list.block_number.to_be_bytes());

    let mut accounts = list.accessed_accounts.clone();
    accounts.sort_by(|left, right| left.address.cmp(&right.address));
    hasher.update((accounts.len() as u64).to_be_bytes());
    for account in accounts {
        hasher.update(account.address);

        let mut storage_keys = account.storage_keys.clone();
        storage_keys.sort_unstable();
        hasher.update((storage_keys.len() as u64).to_be_bytes());
        for key in storage_keys {
            hasher.update(key);
        }

        match account.post_balance {
            Some(balance) => {
                hasher.update([1u8]);
                hasher.update(balance.to_be_bytes());
            }
            None => hasher.update([0u8]),
        }

        match account.post_nonce {
            Some(nonce) => {
                hasher.update([1u8]);
                hasher.update(nonce.to_be_bytes());
            }
            None => hasher.update([0u8]),
        }

        match account.post_code_hash {
            Some(code_hash) => {
                hasher.update([1u8]);
                hasher.update(code_hash);
            }
            None => hasher.update([0u8]),
        }
    }

    let mut storage_accesses = list.storage_accesses.clone();
    storage_accesses.sort_by(
        |(left_address, left_access), (right_address, right_access)| {
            left_address
                .cmp(right_address)
                .then(left_access.key.cmp(&right_access.key))
                .then(left_access.pre_value.cmp(&right_access.pre_value))
                .then(left_access.post_value.cmp(&right_access.post_value))
        },
    );

    hasher.update((storage_accesses.len() as u64).to_be_bytes());
    for (address, storage_access) in storage_accesses {
        hasher.update(address);
        hasher.update(storage_access.key);
        hasher.update(storage_access.pre_value);
        hasher.update(storage_access.post_value);
    }

    hasher.finalize().into()
}

pub fn compute_access_list_stats(list: &BlockAccessList) -> AccessListStats {
    let unique_account_count = list
        .accessed_accounts
        .iter()
        .map(|account| account.address)
        .collect::<HashSet<_>>()
        .len();
    let total_storage_accesses = list.storage_accesses.len();
    let modified_storage_count = list
        .storage_accesses
        .iter()
        .filter(|(_, storage_access)| storage_access.pre_value != storage_access.post_value)
        .count();

    AccessListStats {
        block_number: list.block_number,
        unique_account_count,
        total_storage_accesses,
        modified_storage_count,
        read_only_storage_count: total_storage_accesses.saturating_sub(modified_storage_count),
    }
}

pub fn merge_access_lists(lists: &[BlockAccessList]) -> BlockAccessList {
    let mut merged_accounts: BTreeMap<[u8; 20], AccessedAccount> = BTreeMap::new();
    let mut merged_storage_accesses: BTreeMap<([u8; 20], [u8; 32]), StorageAccess> =
        BTreeMap::new();
    let mut block_number = 0u64;

    for list in lists {
        block_number = list.block_number;

        for account in &list.accessed_accounts {
            let merged_account =
                merged_accounts
                    .entry(account.address)
                    .or_insert(AccessedAccount {
                        address: account.address,
                        storage_keys: Vec::new(),
                        post_balance: None,
                        post_nonce: None,
                        post_code_hash: None,
                    });

            merged_account
                .storage_keys
                .extend(account.storage_keys.iter().copied());
            merged_account.post_balance = account.post_balance;
            merged_account.post_nonce = account.post_nonce;
            merged_account.post_code_hash = account.post_code_hash;
        }

        for (address, storage_access) in &list.storage_accesses {
            merged_storage_accesses.insert((*address, storage_access.key), storage_access.clone());

            let merged_account = merged_accounts.entry(*address).or_insert(AccessedAccount {
                address: *address,
                storage_keys: Vec::new(),
                post_balance: None,
                post_nonce: None,
                post_code_hash: None,
            });
            merged_account.storage_keys.push(storage_access.key);
        }
    }

    for account in merged_accounts.values_mut() {
        account.storage_keys.sort_unstable();
        account.storage_keys.dedup();
    }

    BlockAccessList {
        block_number,
        accessed_accounts: merged_accounts.into_values().collect(),
        storage_accesses: merged_storage_accesses
            .into_iter()
            .map(|((address, _), storage_access)| (address, storage_access))
            .collect(),
    }
}
