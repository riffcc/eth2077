use eth2077_types::epbs::{
    check_header_conflict, determine_timeliness, validate_envelope_against_header,
    validate_slot_range, EpbsValidationError, PayloadEnvelope, PayloadHeader, PayloadTimeliness,
};

fn sample_header(slot: u64, root: &str) -> PayloadHeader {
    PayloadHeader {
        slot,
        payload_header_root: root.to_string(),
        parent_beacon_block_root: Some("0xparent".to_string()),
        execution_block_hash: Some("0xexec".to_string()),
        proposer: Some("0xproposer".to_string()),
        bid_value_wei: 1000,
        view_id: Some(7),
        received_at_unix_s: 1_700_000_000,
    }
}

fn sample_envelope(
    slot: u64,
    root: &str,
    data_available: bool,
    revealed_at_unix_s: u64,
) -> PayloadEnvelope {
    PayloadEnvelope {
        slot,
        payload_header_root: root.to_string(),
        execution_block_hash: Some("0xexec".to_string()),
        payload_body_hash: Some("0xbody".to_string()),
        signer: Some("0xsigner".to_string()),
        data_available,
        revealed_at_unix_s,
    }
}

#[test]
fn test_envelope_matches_header() {
    let header = sample_header(42, "0xroot");
    let envelope = sample_envelope(42, "0xroot", true, 1_700_000_010);

    assert_eq!(validate_envelope_against_header(&header, &envelope), Ok(()));
}

#[test]
fn test_envelope_slot_mismatch() {
    let header = sample_header(42, "0xroot");
    let envelope = sample_envelope(43, "0xroot", true, 1_700_000_010);

    assert_eq!(
        validate_envelope_against_header(&header, &envelope),
        Err(EpbsValidationError::EnvelopeSlotMismatch {
            envelope_slot: 43,
            header_slot: 42,
        })
    );
}

#[test]
fn test_envelope_root_mismatch() {
    let header = sample_header(42, "0xroot-a");
    let envelope = sample_envelope(42, "0xroot-b", true, 1_700_000_010);

    assert_eq!(
        validate_envelope_against_header(&header, &envelope),
        Err(EpbsValidationError::EnvelopeHeaderRootMismatch {
            envelope_root: "0xroot-b".to_string(),
            header_root: "0xroot-a".to_string(),
        })
    );
}

#[test]
fn test_timeliness_unknown() {
    assert_eq!(
        determine_timeliness(None, None, 1_700_000_100),
        PayloadTimeliness::Unknown
    );
}

#[test]
fn test_timeliness_header_only() {
    let header = sample_header(42, "0xroot");
    assert_eq!(
        determine_timeliness(Some(&header), None, 1_700_000_100),
        PayloadTimeliness::HeaderOnly
    );
}

#[test]
fn test_timeliness_revealed() {
    let header = sample_header(42, "0xroot");
    let envelope = sample_envelope(42, "0xroot", true, 1_700_000_050);
    assert_eq!(
        determine_timeliness(Some(&header), Some(&envelope), 1_700_000_100),
        PayloadTimeliness::Revealed
    );
}

#[test]
fn test_timeliness_late_reveal() {
    let header = sample_header(42, "0xroot");
    let envelope = sample_envelope(42, "0xroot", true, 1_700_000_200);
    assert_eq!(
        determine_timeliness(Some(&header), Some(&envelope), 1_700_000_100),
        PayloadTimeliness::LateReveal
    );
}

#[test]
fn test_timeliness_orphan_envelope() {
    let envelope = sample_envelope(42, "0xroot", true, 1_700_000_050);
    assert_eq!(
        determine_timeliness(None, Some(&envelope), 1_700_000_100),
        PayloadTimeliness::OrphanEnvelope
    );
}

#[test]
fn test_header_conflict_duplicate_ok() {
    let existing = sample_header(42, "0xroot");
    let incoming = sample_header(42, "0xroot");

    assert_eq!(check_header_conflict(&existing, &incoming), Ok(()));
}

#[test]
fn test_header_conflict_different_root_fails() {
    let existing = sample_header(42, "0xroot-a");
    let incoming = sample_header(42, "0xroot-b");

    assert_eq!(
        check_header_conflict(&existing, &incoming),
        Err(EpbsValidationError::ConflictingHeaderRegistration {
            slot: 42,
            root: "0xroot-b".to_string(),
        })
    );
}

#[test]
fn test_slot_range_validation() {
    assert_eq!(validate_slot_range(10, 20, 10), Ok(()));
    assert!(validate_slot_range(20, 10, 10).is_err());
    assert!(validate_slot_range(10, 30, 10).is_err());
}
