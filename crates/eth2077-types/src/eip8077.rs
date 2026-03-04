use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EnhancedTxAnnouncement {
    pub tx_hash: [u8; 32],
    pub tx_type: u8,
    pub tx_size: u32,
    pub sender: [u8; 20],
    pub nonce: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxAnnouncementBatch {
    pub announcements: Vec<EnhancedTxAnnouncement>,
    pub peer_id: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TxAnnouncementError {
    EmptyBatch,
    DuplicateTxHash {
        tx_hash: [u8; 32],
    },
    InvalidTxType {
        tx_type: u8,
    },
    ZeroSize {
        tx_hash: [u8; 32],
    },
    NonceGapDetected {
        sender: [u8; 20],
        expected: u64,
        actual: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NonceSummary {
    pub sender: [u8; 20],
    pub min_nonce: u64,
    pub max_nonce: u64,
    pub count: usize,
    pub has_gaps: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnnouncementBatchStats {
    pub total_announcements: usize,
    pub unique_senders: usize,
    pub total_tx_bytes: u64,
    pub nonce_summaries: Vec<NonceSummary>,
}

pub fn validate_announcement_batch(
    batch: &TxAnnouncementBatch,
) -> Result<(), Vec<TxAnnouncementError>> {
    let mut errors = Vec::new();

    if batch.announcements.is_empty() {
        errors.push(TxAnnouncementError::EmptyBatch);
    }

    let mut seen_hashes = HashSet::new();
    for announcement in &batch.announcements {
        if !seen_hashes.insert(announcement.tx_hash) {
            errors.push(TxAnnouncementError::DuplicateTxHash {
                tx_hash: announcement.tx_hash,
            });
        }

        if announcement.tx_type > 3 {
            errors.push(TxAnnouncementError::InvalidTxType {
                tx_type: announcement.tx_type,
            });
        }

        if announcement.tx_size == 0 {
            errors.push(TxAnnouncementError::ZeroSize {
                tx_hash: announcement.tx_hash,
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn detect_nonce_gaps(batch: &TxAnnouncementBatch) -> Vec<TxAnnouncementError> {
    let mut by_sender: HashMap<[u8; 20], Vec<u64>> = HashMap::new();
    for announcement in &batch.announcements {
        by_sender
            .entry(announcement.sender)
            .or_default()
            .push(announcement.nonce);
    }

    let mut errors = Vec::new();
    for (sender, mut nonces) in by_sender {
        nonces.sort_unstable();
        for window in nonces.windows(2) {
            let expected = window[0].saturating_add(1);
            let actual = window[1];
            if actual != expected {
                errors.push(TxAnnouncementError::NonceGapDetected {
                    sender,
                    expected,
                    actual,
                });
            }
        }
    }

    errors
}

pub fn compute_announcement_stats(batch: &TxAnnouncementBatch) -> AnnouncementBatchStats {
    let mut unique_senders = HashSet::new();
    let mut total_tx_bytes = 0u64;
    let mut by_sender: HashMap<[u8; 20], Vec<u64>> = HashMap::new();

    for announcement in &batch.announcements {
        unique_senders.insert(announcement.sender);
        total_tx_bytes = total_tx_bytes.saturating_add(announcement.tx_size as u64);
        by_sender
            .entry(announcement.sender)
            .or_default()
            .push(announcement.nonce);
    }

    let mut nonce_summaries = Vec::with_capacity(by_sender.len());
    for (sender, mut nonces) in by_sender {
        nonces.sort_unstable();

        let min_nonce = *nonces.first().unwrap_or(&0);
        let max_nonce = *nonces.last().unwrap_or(&0);
        let has_gaps = nonces
            .windows(2)
            .any(|window| window[1] != window[0].saturating_add(1));

        nonce_summaries.push(NonceSummary {
            sender,
            min_nonce,
            max_nonce,
            count: nonces.len(),
            has_gaps,
        });
    }
    nonce_summaries.sort_unstable_by_key(|summary| summary.sender);

    AnnouncementBatchStats {
        total_announcements: batch.announcements.len(),
        unique_senders: unique_senders.len(),
        total_tx_bytes,
        nonce_summaries,
    }
}

pub fn prioritize_announcements(
    batch: &TxAnnouncementBatch,
    known_nonces: &HashMap<[u8; 20], u64>,
) -> Vec<EnhancedTxAnnouncement> {
    let mut prioritized: Vec<EnhancedTxAnnouncement> = batch
        .announcements
        .iter()
        .filter(|announcement| {
            let known_nonce = known_nonces.get(&announcement.sender).copied().unwrap_or(0);
            announcement.nonce >= known_nonce
        })
        .cloned()
        .collect();

    prioritized.sort_unstable_by(|a, b| {
        let a_known = known_nonces.get(&a.sender).copied().unwrap_or(0);
        let b_known = known_nonces.get(&b.sender).copied().unwrap_or(0);
        let a_exec = a.nonce == a_known;
        let b_exec = b.nonce == b_known;

        b_exec
            .cmp(&a_exec)
            .then_with(|| a.nonce.cmp(&b.nonce))
            .then_with(|| a.sender.cmp(&b.sender))
            .then_with(|| a.tx_hash.cmp(&b.tx_hash))
    });

    prioritized
}

pub fn merge_announcement_batches(batches: &[TxAnnouncementBatch]) -> TxAnnouncementBatch {
    let mut seen_hashes = HashSet::new();
    let mut announcements = Vec::new();

    for batch in batches {
        for announcement in &batch.announcements {
            if seen_hashes.insert(announcement.tx_hash) {
                announcements.push(announcement.clone());
            }
        }
    }

    TxAnnouncementBatch {
        announcements,
        peer_id: [0u8; 32],
    }
}
