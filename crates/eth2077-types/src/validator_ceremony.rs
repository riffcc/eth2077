use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Validator key ceremony and remote signer modeling primitives for ETH2077 testnets.
///
/// This module defines a strongly typed representation of validator onboarding
/// ceremonies where key material is generated, distributed to signer backends,
/// verified, and eventually activated for consensus duty.
///
/// The design goal is to keep the model deterministic and easy to serialize so
/// that ceremony state can be committed, replayed, and audited across multiple
/// coordinator services.
///
/// Key capabilities covered by this module:
///
/// - Ceremony lifecycle phase tracking.
/// - Signer deployment model selection.
/// - Key material status progression.
/// - Security profile declarations.
/// - Configuration validation with field-level error reporting.
/// - Aggregate key cohort statistics for dashboards.
/// - Deterministic SHA-256 commitments for ceremony configuration snapshots.
///
/// The commitment helper intentionally uses canonical ordering for metadata so
/// callers get a stable hash regardless of hash-map insertion order.
///
/// The stats helper intentionally avoids wall-clock time to keep it deterministic
/// in tests and reproducible analysis workflows. It computes key age relative to
/// the newest observed key timestamp in the provided slice.
///
/// All public types derive serde traits so they can be encoded in JSON/YAML and
/// transmitted between operators, automation agents, and validator management
/// services.
///
/// This module is intended for testnet coordinator control planes and not for
/// direct cryptographic key handling.
///
/// High-level lifecycle phases for a validator key ceremony.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CeremonyPhase {
    /// Ceremony initialization, participant enrollment, and policy lock-in.
    Setup,
    /// Distributed or local key generation and entropy contribution.
    KeyGeneration,
    /// Key shares, encrypted blobs, or signer credentials distribution.
    Distribution,
    /// Multi-party checks confirming key correctness and signer readiness.
    Verification,
    /// Validator credentials are enabled for live duty.
    Activation,
    /// Periodic or emergency key replacement cycle.
    Rotation,
}

/// Signing architecture selected for ceremony-produced validator keys.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SignerProtocol {
    /// On-host software signer with locally managed keys.
    Local,
    /// Remote signer using gRPC transport.
    RemoteGrpc,
    /// Remote signer using HTTP transport.
    RemoteHttp,
    /// Hardware Security Module backed signer.
    Hsm,
    /// Threshold BLS signer requiring a signing quorum.
    ThresholdBls,
    /// General distributed signer model spanning multiple nodes.
    Distributed,
}

/// Operational state of a validator key in the ceremony lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KeyStatus {
    /// Key material has been generated but not yet distributed.
    Generated,
    /// Key has been distributed to intended signer infrastructure.
    Distributed,
    /// Key has passed ceremony verification checks.
    Verified,
    /// Key is active for validator duties.
    Active,
    /// Key has been revoked due to incident or decommission.
    Revoked,
    /// Key is no longer valid due to policy-based expiry.
    Expired,
}

/// Security posture required for the ceremony and signer deployment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityLevel {
    /// Baseline controls suitable for routine testnet operation.
    Standard,
    /// Additional hardening requirements and operator checks.
    Enhanced,
    /// Air-gapped generation and transfer procedures.
    AirGapped,
    /// Multi-party security with separation of duties.
    MultiParty,
    /// Controls backed by formal verification artifacts.
    FormallyVerified,
    /// Forward-looking profile accounting for post-quantum concerns.
    PostQuantum,
}

/// Single validator key record created through a ceremony.
///
/// The record models operational metadata only and does not embed secret key
/// bytes. `pubkey` is represented as a string so callers can use hex, base64,
/// or another serialization format appropriate for their deployment stack.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidatorKey {
    /// Stable identifier for the key record (e.g. UUID or ceremony-local ID).
    pub id: String,
    /// Public key material in caller-defined canonical string form.
    pub pubkey: String,
    /// Current status in the validator onboarding lifecycle.
    pub status: KeyStatus,
    /// Signer protocol backing this validator key.
    pub signer: SignerProtocol,
    /// Security profile applied during key generation and storage.
    pub security: SecurityLevel,
    /// Generation timestamp in UNIX seconds.
    pub generated_at: u64,
    /// Arbitrary ceremony annotations for dashboards and audits.
    pub metadata: HashMap<String, String>,
}

/// Configuration for a validator key ceremony run.
///
/// `threshold` is interpreted as the minimum participant quorum required for
/// threshold or distributed signing modes. For local/HSM mode it can still be
/// used as an organizational approval threshold.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidatorCeremonyConfig {
    /// Number of validator keys targeted by the ceremony.
    pub validator_count: usize,
    /// Required approval/signing threshold.
    pub threshold: usize,
    /// Signer transport or architecture mode.
    pub protocol: SignerProtocol,
    /// Security controls expected for this ceremony.
    pub security: SecurityLevel,
    /// Planned key rotation period in days.
    pub rotation_period_days: u64,
    /// Whether the ceremony requires fully air-gapped handling.
    pub require_air_gap: bool,
    /// Additional free-form configuration annotations.
    pub metadata: HashMap<String, String>,
}

/// Field-level configuration validation error.
///
/// This shape is intentionally simple so UI layers can easily map issues to
/// form controls or API fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidatorCeremonyValidationError {
    /// The invalid field name.
    pub field: String,
    /// Human-readable reason for rejection.
    pub reason: String,
}

/// Aggregated statistics for a collection of validator keys.
///
/// Stats are intended for control-plane observability and operator reporting.
/// The protocol distribution maps protocol name to key count.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidatorCeremonyStats {
    /// Number of keys in the input slice.
    pub total_keys: usize,
    /// Number of keys currently marked active.
    pub active_keys: usize,
    /// Number of keys currently marked revoked.
    pub revoked_keys: usize,
    /// Mean key age in days, measured against newest observed key timestamp.
    pub avg_key_age_days: f64,
    /// Mapping from signer protocol label to key count.
    pub protocol_distribution: HashMap<String, usize>,
    /// True when all keys reached a terminal state and at least one is active.
    pub ceremony_complete: bool,
}

/// Returns a conservative default configuration for ETH2077 validator onboarding.
///
/// Default profile rationale:
///
/// - `validator_count`: 128 for moderate testnet cohorts.
/// - `threshold`: 86 (roughly two-thirds quorum).
/// - `protocol`: `ThresholdBls` to model shared-signer coordination.
/// - `security`: `MultiParty` to reflect ceremony operator separation.
/// - `rotation_period_days`: 90 for quarterly rollover exercises.
/// - `require_air_gap`: `true` for stricter generation practice.
///
/// Callers are expected to override these values for specific testnet programs.
pub fn default_validator_ceremony_config() -> ValidatorCeremonyConfig {
    ValidatorCeremonyConfig {
        validator_count: 128,
        threshold: 86,
        protocol: SignerProtocol::ThresholdBls,
        security: SecurityLevel::MultiParty,
        rotation_period_days: 90,
        require_air_gap: true,
        metadata: HashMap::from([
            ("profile".to_string(), "eth2077-testnet-default".to_string()),
            (
                "verification_policy".to_string(),
                "dual-operator-check".to_string(),
            ),
        ]),
    }
}

/// Validates an onboarding ceremony configuration.
///
/// Validation rules are intentionally explicit and return all discovered issues
/// in one pass so operators can fix configuration holistically.
///
/// Rules:
///
/// - `validator_count` must be greater than zero.
/// - `threshold` must be greater than zero.
/// - `threshold` cannot exceed `validator_count`.
/// - `rotation_period_days` must be in `[1, 3650]`.
/// - `require_air_gap` implies `security` must be `AirGapped` or stronger.
/// - `security=AirGapped` cannot be paired with `protocol=RemoteHttp`.
/// - `protocol=ThresholdBls` and `protocol=Distributed` require `threshold >= 2`.
/// - `security=PostQuantum` requires `rotation_period_days <= 180`.
///
/// On success returns `Ok(())`; otherwise returns every field-level issue.
pub fn validate_validator_ceremony_config(
    config: &ValidatorCeremonyConfig,
) -> Result<(), Vec<ValidatorCeremonyValidationError>> {
    let mut errors = Vec::new();

    if config.validator_count == 0 {
        errors.push(ValidatorCeremonyValidationError {
            field: "validator_count".to_string(),
            reason: "validator_count must be greater than zero".to_string(),
        });
    }

    if config.threshold == 0 {
        errors.push(ValidatorCeremonyValidationError {
            field: "threshold".to_string(),
            reason: "threshold must be greater than zero".to_string(),
        });
    }

    if config.threshold > config.validator_count {
        errors.push(ValidatorCeremonyValidationError {
            field: "threshold".to_string(),
            reason: "threshold cannot exceed validator_count".to_string(),
        });
    }

    if config.rotation_period_days == 0 {
        errors.push(ValidatorCeremonyValidationError {
            field: "rotation_period_days".to_string(),
            reason: "rotation_period_days must be at least 1".to_string(),
        });
    }

    if config.rotation_period_days > 3_650 {
        errors.push(ValidatorCeremonyValidationError {
            field: "rotation_period_days".to_string(),
            reason: "rotation_period_days must not exceed 3650".to_string(),
        });
    }

    if config.require_air_gap && !security_meets_air_gap_requirement(&config.security) {
        errors.push(ValidatorCeremonyValidationError {
            field: "security".to_string(),
            reason: "require_air_gap=true requires AirGapped, MultiParty, FormallyVerified, or PostQuantum security"
                .to_string(),
        });
    }

    if matches!(config.security, SecurityLevel::AirGapped)
        && matches!(config.protocol, SignerProtocol::RemoteHttp)
    {
        errors.push(ValidatorCeremonyValidationError {
            field: "protocol".to_string(),
            reason: "RemoteHttp is incompatible with AirGapped security".to_string(),
        });
    }

    if matches!(
        config.protocol,
        SignerProtocol::ThresholdBls | SignerProtocol::Distributed
    ) && config.threshold < 2
    {
        errors.push(ValidatorCeremonyValidationError {
            field: "threshold".to_string(),
            reason: "threshold must be at least 2 for ThresholdBls or Distributed protocols"
                .to_string(),
        });
    }

    if matches!(config.security, SecurityLevel::PostQuantum) && config.rotation_period_days > 180 {
        errors.push(ValidatorCeremonyValidationError {
            field: "rotation_period_days".to_string(),
            reason: "PostQuantum profile requires rotation_period_days <= 180".to_string(),
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Computes aggregate statistics over validator key records.
///
/// Behavior details:
///
/// - `total_keys`: number of items in `keys`.
/// - `active_keys`: number of keys with `status=Active`.
/// - `revoked_keys`: number of keys with `status=Revoked`.
/// - `avg_key_age_days`: mean `(max_generated_at - generated_at) / 86400.0`.
/// - `protocol_distribution`: protocol name counts.
/// - `ceremony_complete`: true when all keys are in terminal statuses
///   (`Active`, `Revoked`, or `Expired`) and at least one key is active.
///
/// The age metric uses the newest observed key timestamp as a deterministic
/// cohort reference point, which avoids non-deterministic wall-clock behavior
/// inside type-level utility code.
pub fn compute_validator_ceremony_stats(keys: &[ValidatorKey]) -> ValidatorCeremonyStats {
    let total_keys = keys.len();
    let mut active_keys = 0_usize;
    let mut revoked_keys = 0_usize;
    let mut protocol_distribution: HashMap<String, usize> = HashMap::new();

    let newest_timestamp = keys.iter().map(|key| key.generated_at).max().unwrap_or(0);

    let mut age_days_sum = 0.0_f64;
    let mut all_terminal_statuses = true;

    for key in keys {
        if matches!(key.status, KeyStatus::Active) {
            active_keys += 1;
        }

        if matches!(key.status, KeyStatus::Revoked) {
            revoked_keys += 1;
        }

        let protocol_name = protocol_label(&key.signer).to_string();
        *protocol_distribution.entry(protocol_name).or_insert(0) += 1;

        let age_seconds = newest_timestamp.saturating_sub(key.generated_at);
        age_days_sum += age_seconds as f64 / 86_400.0;

        if !is_terminal_key_status(&key.status) {
            all_terminal_statuses = false;
        }
    }

    let avg_key_age_days = if total_keys == 0 {
        0.0
    } else {
        age_days_sum / total_keys as f64
    };

    let ceremony_complete = total_keys > 0 && all_terminal_statuses && active_keys > 0;

    ValidatorCeremonyStats {
        total_keys,
        active_keys,
        revoked_keys,
        avg_key_age_days,
        protocol_distribution,
        ceremony_complete,
    }
}

/// Computes a deterministic SHA-256 commitment for a ceremony configuration.
///
/// Canonicalization strategy:
///
/// - Scalar numeric fields are encoded in little-endian bytes.
/// - Enum fields are encoded via stable textual labels.
/// - `require_air_gap` is encoded as `0` or `1` byte.
/// - Metadata entries are sorted lexicographically by key then value.
/// - Key/value entries are delimited to avoid accidental concatenation
///   collisions when adjacent fields vary in length.
///
/// Returns a lowercase hexadecimal string with 64 characters.
pub fn compute_validator_ceremony_commitment(config: &ValidatorCeremonyConfig) -> String {
    let mut hasher = Sha256::new();

    hasher.update((config.validator_count as u64).to_le_bytes());
    hasher.update((config.threshold as u64).to_le_bytes());
    hasher.update(protocol_label(&config.protocol).as_bytes());
    hasher.update([0_u8]);
    hasher.update(security_label(&config.security).as_bytes());
    hasher.update([0_u8]);
    hasher.update(config.rotation_period_days.to_le_bytes());
    hasher.update([u8::from(config.require_air_gap)]);

    let mut metadata_entries: Vec<(&String, &String)> = config.metadata.iter().collect();
    metadata_entries.sort_unstable_by(|(key_a, value_a), (key_b, value_b)| {
        key_a.cmp(key_b).then_with(|| value_a.cmp(value_b))
    });

    for (key, value) in metadata_entries {
        hasher.update(key.as_bytes());
        hasher.update([0_u8]);
        hasher.update(value.as_bytes());
        hasher.update([0xFF_u8]);
    }

    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(nibble_to_hex((byte >> 4) & 0x0F));
        hex.push(nibble_to_hex(byte & 0x0F));
    }
    hex
}

/// Returns `true` when the security level satisfies air-gap constraints.
fn security_meets_air_gap_requirement(security: &SecurityLevel) -> bool {
    matches!(
        security,
        SecurityLevel::AirGapped
            | SecurityLevel::MultiParty
            | SecurityLevel::FormallyVerified
            | SecurityLevel::PostQuantum
    )
}

/// Returns `true` if a key status is considered terminal for ceremony completion.
fn is_terminal_key_status(status: &KeyStatus) -> bool {
    matches!(
        status,
        KeyStatus::Active | KeyStatus::Revoked | KeyStatus::Expired
    )
}

/// Stable protocol labels used in stats and commitment serialization.
fn protocol_label(protocol: &SignerProtocol) -> &'static str {
    match protocol {
        SignerProtocol::Local => "Local",
        SignerProtocol::RemoteGrpc => "RemoteGrpc",
        SignerProtocol::RemoteHttp => "RemoteHttp",
        SignerProtocol::Hsm => "Hsm",
        SignerProtocol::ThresholdBls => "ThresholdBls",
        SignerProtocol::Distributed => "Distributed",
    }
}

/// Stable security labels used in commitment serialization.
fn security_label(security: &SecurityLevel) -> &'static str {
    match security {
        SecurityLevel::Standard => "Standard",
        SecurityLevel::Enhanced => "Enhanced",
        SecurityLevel::AirGapped => "AirGapped",
        SecurityLevel::MultiParty => "MultiParty",
        SecurityLevel::FormallyVerified => "FormallyVerified",
        SecurityLevel::PostQuantum => "PostQuantum",
    }
}

/// Converts a 4-bit nibble into lowercase hexadecimal character.
fn nibble_to_hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => '0',
    }
}
