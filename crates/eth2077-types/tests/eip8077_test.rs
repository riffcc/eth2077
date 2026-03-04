use eth2077_types::eip8077::{
    compute_announcement_stats, detect_nonce_gaps, merge_announcement_batches,
    prioritize_announcements, validate_announcement_batch, AnnouncementBatchStats,
    EnhancedTxAnnouncement, NonceSummary, TxAnnouncementBatch, TxAnnouncementError,
};
use std::collections::{HashMap, HashSet};

fn bytes32(value: u8) -> [u8; 32] {
    [value; 32]
}

fn bytes20(value: u8) -> [u8; 20] {
    [value; 20]
}

fn announcement(
    tx_hash: [u8; 32],
    tx_type: u8,
    tx_size: u32,
    sender: [u8; 20],
    nonce: u64,
) -> EnhancedTxAnnouncement {
    EnhancedTxAnnouncement {
        tx_hash,
        tx_type,
        tx_size,
        sender,
        nonce,
    }
}

#[test]
fn valid_batch_passes() {
    let batch = TxAnnouncementBatch {
        announcements: vec![
            announcement(bytes32(0x01), 2, 120, bytes20(0xA1), 7),
            announcement(bytes32(0x02), 0, 95, bytes20(0xA2), 3),
        ],
        peer_id: bytes32(0x10),
    };

    assert_eq!(validate_announcement_batch(&batch), Ok(()));
}

#[test]
fn empty_batch_rejected() {
    let batch = TxAnnouncementBatch {
        announcements: Vec::new(),
        peer_id: bytes32(0x11),
    };

    let errors = validate_announcement_batch(&batch).unwrap_err();
    assert!(errors.contains(&TxAnnouncementError::EmptyBatch));
}

#[test]
fn duplicate_hash_detected() {
    let dup_hash = bytes32(0xAB);
    let batch = TxAnnouncementBatch {
        announcements: vec![
            announcement(dup_hash, 2, 100, bytes20(0x01), 1),
            announcement(dup_hash, 2, 110, bytes20(0x02), 2),
        ],
        peer_id: bytes32(0x12),
    };

    let errors = validate_announcement_batch(&batch).unwrap_err();
    assert!(errors.contains(&TxAnnouncementError::DuplicateTxHash { tx_hash: dup_hash }));
}

#[test]
fn invalid_tx_type_detected() {
    let batch = TxAnnouncementBatch {
        announcements: vec![announcement(bytes32(0xCC), 9, 100, bytes20(0x01), 0)],
        peer_id: bytes32(0x13),
    };

    let errors = validate_announcement_batch(&batch).unwrap_err();
    assert!(errors.contains(&TxAnnouncementError::InvalidTxType { tx_type: 9 }));
}

#[test]
fn nonce_gap_detection() {
    let sender = bytes20(0xEF);
    let batch = TxAnnouncementBatch {
        announcements: vec![
            announcement(bytes32(0x01), 2, 100, sender, 5),
            announcement(bytes32(0x02), 2, 100, sender, 7),
            announcement(bytes32(0x03), 2, 100, bytes20(0xFE), 1),
        ],
        peer_id: bytes32(0x14),
    };

    let errors = detect_nonce_gaps(&batch);
    assert!(errors.contains(&TxAnnouncementError::NonceGapDetected {
        sender,
        expected: 6,
        actual: 7,
    }));
}

#[test]
fn prioritization_puts_executable_first() {
    let sender = bytes20(0x11);
    let executable_hash = bytes32(0xB2);
    let batch = TxAnnouncementBatch {
        announcements: vec![
            announcement(bytes32(0xB1), 2, 100, sender, 4), // filtered out
            announcement(executable_hash, 2, 100, sender, 5),
            announcement(bytes32(0xB3), 2, 100, sender, 6),
            announcement(bytes32(0xC1), 2, 100, bytes20(0x22), 1),
        ],
        peer_id: bytes32(0x15),
    };

    let mut known_nonces = HashMap::new();
    known_nonces.insert(sender, 5);

    let prioritized = prioritize_announcements(&batch, &known_nonces);
    assert_eq!(prioritized[0].tx_hash, executable_hash);
    assert!(!prioritized.iter().any(|tx| tx.tx_hash == bytes32(0xB1)));
}

#[test]
fn stats_computation_correct() {
    let sender_a = bytes20(0xAA);
    let sender_b = bytes20(0xBB);
    let batch = TxAnnouncementBatch {
        announcements: vec![
            announcement(bytes32(0x01), 2, 100, sender_a, 1),
            announcement(bytes32(0x02), 2, 120, sender_a, 2),
            announcement(bytes32(0x03), 2, 50, sender_b, 10),
            announcement(bytes32(0x04), 2, 80, sender_b, 12),
        ],
        peer_id: bytes32(0x16),
    };

    let stats: AnnouncementBatchStats = compute_announcement_stats(&batch);
    assert_eq!(stats.total_announcements, 4);
    assert_eq!(stats.unique_senders, 2);
    assert_eq!(stats.total_tx_bytes, 350);
    assert_eq!(stats.nonce_summaries.len(), 2);

    let summary_a = stats
        .nonce_summaries
        .iter()
        .find(|summary| summary.sender == sender_a)
        .unwrap();
    assert_eq!(
        summary_a,
        &NonceSummary {
            sender: sender_a,
            min_nonce: 1,
            max_nonce: 2,
            count: 2,
            has_gaps: false,
        }
    );

    let summary_b = stats
        .nonce_summaries
        .iter()
        .find(|summary| summary.sender == sender_b)
        .unwrap();
    assert_eq!(
        summary_b,
        &NonceSummary {
            sender: sender_b,
            min_nonce: 10,
            max_nonce: 12,
            count: 2,
            has_gaps: true,
        }
    );
}

#[test]
fn merge_deduplicates_by_hash() {
    let duplicate_hash = bytes32(0xD1);
    let batch_one = TxAnnouncementBatch {
        announcements: vec![
            announcement(duplicate_hash, 2, 100, bytes20(0x01), 1),
            announcement(bytes32(0xD2), 2, 110, bytes20(0x01), 2),
        ],
        peer_id: bytes32(0xA1),
    };
    let batch_two = TxAnnouncementBatch {
        announcements: vec![
            announcement(duplicate_hash, 2, 100, bytes20(0x02), 3),
            announcement(bytes32(0xD3), 2, 90, bytes20(0x02), 4),
        ],
        peer_id: bytes32(0xA2),
    };

    let merged = merge_announcement_batches(&[batch_one, batch_two]);
    assert_eq!(merged.peer_id, [0u8; 32]);
    assert_eq!(merged.announcements.len(), 3);

    let unique_hashes: HashSet<[u8; 32]> =
        merged.announcements.iter().map(|tx| tx.tx_hash).collect();
    assert_eq!(unique_hashes.len(), merged.announcements.len());
}
