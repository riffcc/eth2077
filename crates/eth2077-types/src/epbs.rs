use serde::{Deserialize, Serialize};

/// ePBS timeliness status for a slot's payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum PayloadTimeliness {
    Unknown,
    HeaderOnly,
    PartialReveal,
    Revealed,
    LateReveal,
    Withheld,
    PartialWithhold,
    OrphanEnvelope,
}

/// ePBS penalty lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum PenaltyState {
    Active,
    Recovered,
}

/// A registered execution payload header from a builder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PayloadHeader {
    pub slot: u64,
    pub payload_header_root: String,
    pub parent_beacon_block_root: Option<String>,
    pub execution_block_hash: Option<String>,
    pub proposer: Option<String>,
    pub bid_value_wei: u128,
    pub view_id: Option<u64>,
    pub received_at_unix_s: u64,
}

/// A revealed execution payload envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PayloadEnvelope {
    pub slot: u64,
    pub payload_header_root: String,
    pub execution_block_hash: Option<String>,
    pub payload_body_hash: Option<String>,
    pub signer: Option<String>,
    pub data_available: bool,
    pub revealed_at_unix_s: u64,
}

/// A penalty record for builder misbehavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PenaltyRecord {
    pub slot: u64,
    pub state: PenaltyState,
    pub reason: String,
    pub last_status: PayloadTimeliness,
    pub activated_at_unix_s: u64,
    pub recovered_at_unix_s: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpbsValidationError {
    HeaderSlotMismatch {
        header_slot: u64,
        expected_slot: u64,
    },
    EnvelopeSlotMismatch {
        envelope_slot: u64,
        header_slot: u64,
    },
    EnvelopeHeaderRootMismatch {
        envelope_root: String,
        header_root: String,
    },
    DuplicateHeaderRegistration {
        slot: u64,
        root: String,
    },
    ConflictingHeaderRegistration {
        slot: u64,
        root: String,
    },
    StaleSlot {
        slot: u64,
        head_slot: u64,
    },
    PenaltyAlreadyActive {
        slot: u64,
    },
    PenaltyNotActive {
        slot: u64,
    },
    RevealAfterDeadline {
        slot: u64,
        revealed_at: u64,
        deadline: u64,
    },
}

/// Validate that an envelope matches its corresponding header.
pub fn validate_envelope_against_header(
    header: &PayloadHeader,
    envelope: &PayloadEnvelope,
) -> Result<(), EpbsValidationError> {
    if envelope.slot != header.slot {
        return Err(EpbsValidationError::EnvelopeSlotMismatch {
            envelope_slot: envelope.slot,
            header_slot: header.slot,
        });
    }

    if envelope.payload_header_root != header.payload_header_root {
        return Err(EpbsValidationError::EnvelopeHeaderRootMismatch {
            envelope_root: envelope.payload_header_root.clone(),
            header_root: header.payload_header_root.clone(),
        });
    }

    Ok(())
}

/// Determine timeliness status of a payload given timing parameters.
pub fn determine_timeliness(
    header: Option<&PayloadHeader>,
    envelope: Option<&PayloadEnvelope>,
    slot_deadline_unix_s: u64,
) -> PayloadTimeliness {
    match (header, envelope) {
        (None, None) => PayloadTimeliness::Unknown,
        (Some(_), None) => PayloadTimeliness::HeaderOnly,
        (None, Some(_)) => PayloadTimeliness::OrphanEnvelope,
        (Some(_), Some(envelope)) => {
            let is_late = envelope.revealed_at_unix_s > slot_deadline_unix_s;
            if envelope.data_available {
                if is_late {
                    PayloadTimeliness::LateReveal
                } else {
                    PayloadTimeliness::Revealed
                }
            } else if is_late {
                PayloadTimeliness::Withheld
            } else {
                PayloadTimeliness::PartialReveal
            }
        }
    }
}

/// Check if a header registration conflicts with an existing one.
pub fn check_header_conflict(
    existing: &PayloadHeader,
    incoming: &PayloadHeader,
) -> Result<(), EpbsValidationError> {
    if incoming.slot != existing.slot {
        return Err(EpbsValidationError::HeaderSlotMismatch {
            header_slot: incoming.slot,
            expected_slot: existing.slot,
        });
    }

    if incoming.payload_header_root == existing.payload_header_root {
        return Ok(());
    }

    Err(EpbsValidationError::ConflictingHeaderRegistration {
        slot: incoming.slot,
        root: incoming.payload_header_root.clone(),
    })
}

/// Validate a slot-range query is within bounds.
pub fn validate_slot_range(start: u64, end: u64, max_window: u64) -> Result<(), String> {
    if end < start {
        return Err(format!(
            "invalid slot range: end ({end}) is less than start ({start})"
        ));
    }

    let window = end - start;
    if window > max_window {
        return Err(format!(
            "slot range window {window} exceeds maximum allowed {max_window}"
        ));
    }

    Ok(())
}
