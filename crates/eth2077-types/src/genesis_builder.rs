//! Deterministic genesis builder primitives for ETH2077.
//!
//! The module provides:
//! - launch configuration and allocation data types
//! - multi-error validation for genesis settings
//! - deterministic stats for validator deposits and allocation totals
//! - stable SHA-256 commitment / artifact hashing
//!
//! To preserve determinism with `HashMap`, metadata key/value pairs are sorted
//! before hashing. Allocation records are also canonicalized and sorted, so hash
//! outputs do not depend on input ordering.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const CONFIG_DOMAIN: &[u8] = b"eth2077-genesis-builder-config-v1";
const ARTIFACT_DOMAIN: &[u8] = b"eth2077-genesis-builder-artifact-v1";
const MIN_SEED_LEN: usize = 8;

/// Type of balance allocation performed during genesis creation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AllocationKind {
    ValidatorDeposit,
    Faucet,
    Treasury,
    Bridge,
    PreMine,
    Contract,
}

/// Signature model used when attesting and sealing genesis artifacts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SigningScheme {
    Bls12381,
    Ed25519,
    Secp256k1,
    MultiSig,
    Threshold,
    Aggregate,
}

/// Operational phase for genesis workflow tracking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GenesisPhase {
    Configure,
    Allocate,
    Sign,
    Verify,
    Seal,
    Distribute,
}

/// Format used when serializing genesis artifacts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ArtifactFormat {
    Ssz,
    Json,
    Rlp,
    Binary,
    HexEncoded,
    CompressedSsz,
}

/// One deterministic genesis allocation entry.
///
/// `kind` and `is_validator` can differ intentionally in migration workflows.
/// For example, a `Bridge` record may still need validator-style minimum checks
/// if `is_validator` is set to `true`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenesisAllocation {
    pub address: String,
    pub kind: AllocationKind,
    pub amount_gwei: u64,
    pub is_validator: bool,
    pub metadata: HashMap<String, String>,
}

/// Deterministic genesis-builder configuration.
///
/// A config commitment derived from these fields can be signed or stored by
/// release automation before allocation files are finalized.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenesisBuilderConfig {
    pub chain_id: u64,
    pub genesis_time: u64,
    pub validator_count: usize,
    pub signing_scheme: SigningScheme,
    pub artifact_format: ArtifactFormat,
    pub deterministic_seed: String,
    pub min_deposit_gwei: u64,
    pub metadata: HashMap<String, String>,
}

/// Validation error that pinpoints a single malformed field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenesisBuilderValidationError {
    pub field: String,
    pub reason: String,
}

/// Rollup of computed statistics over genesis allocations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenesisBuilderStats {
    pub total_allocations: usize,
    pub validator_deposits: usize,
    pub total_gwei: u64,
    pub unique_addresses: usize,
    pub signing_complete: bool,
    pub artifact_hash: String,
}

/// Returns a baseline configuration suitable for ETH2077 testnet scaffolding.
pub fn default_genesis_builder_config() -> GenesisBuilderConfig {
    let mut metadata = HashMap::new();
    metadata.insert(
        "phase".to_string(),
        format!("{:?}", GenesisPhase::Configure),
    );
    metadata.insert("release_channel".to_string(), "testnet".to_string());
    metadata.insert("artifact_version".to_string(), "v1".to_string());
    metadata.insert(
        "aggregate.domain".to_string(),
        "eth2077-genesis".to_string(),
    );

    GenesisBuilderConfig {
        chain_id: 2077,
        genesis_time: 1_893_456_000,
        validator_count: 64,
        signing_scheme: SigningScheme::Aggregate,
        artifact_format: ArtifactFormat::Ssz,
        deterministic_seed: "eth2077-default-seed".to_string(),
        min_deposit_gwei: 32_000_000_000,
        metadata,
    }
}

/// Validates a genesis builder configuration and reports all discovered issues.
///
/// Checks include numeric sanity, seed quality, metadata hygiene, signing
/// scheme requirements, and artifact-format requirements.
pub fn validate_genesis_builder_config(
    config: &GenesisBuilderConfig,
) -> Result<(), Vec<GenesisBuilderValidationError>> {
    let mut errors = Vec::new();

    if config.chain_id == 0 {
        errors.push(validation_error("chain_id", "must be greater than zero"));
    }
    if config.genesis_time == 0 {
        errors.push(validation_error(
            "genesis_time",
            "must be greater than zero",
        ));
    }
    if config.validator_count == 0 {
        errors.push(validation_error(
            "validator_count",
            "must be greater than zero",
        ));
    }

    if config.deterministic_seed.trim().is_empty() {
        errors.push(validation_error(
            "deterministic_seed",
            "must not be empty or whitespace",
        ));
    } else if config.deterministic_seed.len() < MIN_SEED_LEN {
        errors.push(validation_error(
            "deterministic_seed",
            "must contain at least 8 characters",
        ));
    }

    if config.min_deposit_gwei == 0 {
        errors.push(validation_error(
            "min_deposit_gwei",
            "must be greater than zero",
        ));
    } else if config.min_deposit_gwei < 1_000_000_000 {
        errors.push(validation_error(
            "min_deposit_gwei",
            "must be at least 1_000_000_000 gwei",
        ));
    }

    for (key, value) in &config.metadata {
        if key.trim().is_empty() {
            errors.push(validation_error("metadata", "contains an empty key"));
        }
        if value.trim().is_empty() {
            errors.push(validation_error("metadata", "contains an empty value"));
        }
    }

    validate_signing_metadata(config, &mut errors);
    validate_artifact_metadata(config, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Computes deterministic statistics and artifact hash for allocation records.
///
/// `signing_complete` requires valid config, enough validator deposits, and all
/// validator deposits meeting `min_deposit_gwei`.
pub fn compute_genesis_builder_stats(
    allocations: &[GenesisAllocation],
    config: &GenesisBuilderConfig,
) -> GenesisBuilderStats {
    let mut total_gwei = 0u64;
    let mut validator_deposits = 0usize;
    let mut unique_addresses: HashMap<String, ()> = HashMap::new();
    let mut validators_meet_minimum = true;
    let mut canonical_records = Vec::with_capacity(allocations.len());

    for allocation in allocations {
        total_gwei = total_gwei.saturating_add(allocation.amount_gwei);
        unique_addresses.insert(normalize_address(&allocation.address), ());

        if is_validator_deposit(allocation) {
            validator_deposits += 1;
            if allocation.amount_gwei < config.min_deposit_gwei {
                validators_meet_minimum = false;
            }
        }

        canonical_records.push(canonical_allocation_record(allocation));
    }

    canonical_records.sort_unstable();

    let signing_complete = validate_genesis_builder_config(config).is_ok()
        && validator_deposits >= config.validator_count
        && validators_meet_minimum;

    let artifact_hash = compute_artifact_hash(config, &canonical_records);

    GenesisBuilderStats {
        total_allocations: allocations.len(),
        validator_deposits,
        total_gwei,
        unique_addresses: unique_addresses.len(),
        signing_complete,
        artifact_hash,
    }
}

/// Computes deterministic configuration commitment as SHA-256 lower-hex.
pub fn compute_genesis_builder_commitment(config: &GenesisBuilderConfig) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CONFIG_DOMAIN);
    hasher.update(config.chain_id.to_be_bytes());
    hasher.update(config.genesis_time.to_be_bytes());
    hasher.update((config.validator_count as u64).to_be_bytes());
    hasher.update([signing_scheme_discriminant(&config.signing_scheme)]);
    hasher.update([artifact_format_discriminant(&config.artifact_format)]);
    hash_len_prefixed_bytes(&mut hasher, config.deterministic_seed.as_bytes());
    hasher.update(config.min_deposit_gwei.to_be_bytes());
    hash_sorted_metadata(&mut hasher, &config.metadata);
    digest_to_lower_hex(&hasher.finalize())
}

fn validation_error(field: &str, reason: &str) -> GenesisBuilderValidationError {
    GenesisBuilderValidationError {
        field: field.to_string(),
        reason: reason.to_string(),
    }
}

fn validate_signing_metadata(
    config: &GenesisBuilderConfig,
    errors: &mut Vec<GenesisBuilderValidationError>,
) {
    match config.signing_scheme {
        SigningScheme::Bls12381 | SigningScheme::Ed25519 | SigningScheme::Secp256k1 => {}
        SigningScheme::MultiSig => validate_multisig_metadata(&config.metadata, errors),
        SigningScheme::Threshold => validate_threshold_metadata(&config.metadata, errors),
        SigningScheme::Aggregate => require_metadata_key(
            &config.metadata,
            "aggregate.domain",
            "must be present for Aggregate",
            errors,
        ),
    }
}

fn validate_multisig_metadata(
    metadata: &HashMap<String, String>,
    errors: &mut Vec<GenesisBuilderValidationError>,
) {
    let participants = parse_required_usize(
        metadata,
        "multisig.participants",
        "must be present for MultiSig",
        errors,
    );
    let threshold = parse_required_usize(
        metadata,
        "multisig.threshold",
        "must be present for MultiSig",
        errors,
    );

    if let (Some(participants), Some(threshold)) = (participants, threshold) {
        if participants == 0 {
            errors.push(validation_error(
                "metadata.multisig.participants",
                "must be greater than zero",
            ));
        }
        if threshold == 0 {
            errors.push(validation_error(
                "metadata.multisig.threshold",
                "must be greater than zero",
            ));
        }
        if threshold > participants {
            errors.push(validation_error(
                "metadata.multisig.threshold",
                "must be less than or equal to multisig.participants",
            ));
        }
    }
}

fn validate_threshold_metadata(
    metadata: &HashMap<String, String>,
    errors: &mut Vec<GenesisBuilderValidationError>,
) {
    let shares = parse_required_usize(
        metadata,
        "threshold.total_shares",
        "must be present for Threshold",
        errors,
    );
    let quorum = parse_required_usize(
        metadata,
        "threshold.quorum",
        "must be present for Threshold",
        errors,
    );

    if let (Some(shares), Some(quorum)) = (shares, quorum) {
        if shares == 0 {
            errors.push(validation_error(
                "metadata.threshold.total_shares",
                "must be greater than zero",
            ));
        }
        if quorum == 0 {
            errors.push(validation_error(
                "metadata.threshold.quorum",
                "must be greater than zero",
            ));
        }
        if quorum > shares {
            errors.push(validation_error(
                "metadata.threshold.quorum",
                "must be less than or equal to threshold.total_shares",
            ));
        }
    }
}

fn validate_artifact_metadata(
    config: &GenesisBuilderConfig,
    errors: &mut Vec<GenesisBuilderValidationError>,
) {
    match config.artifact_format {
        ArtifactFormat::CompressedSsz => require_metadata_key(
            &config.metadata,
            "compression.codec",
            "must be present when artifact_format is CompressedSsz",
            errors,
        ),
        ArtifactFormat::HexEncoded => require_metadata_key(
            &config.metadata,
            "hex.case",
            "must be present when artifact_format is HexEncoded",
            errors,
        ),
        ArtifactFormat::Ssz
        | ArtifactFormat::Json
        | ArtifactFormat::Rlp
        | ArtifactFormat::Binary => {}
    }
}

fn parse_required_usize(
    metadata: &HashMap<String, String>,
    key: &str,
    missing_reason: &str,
    errors: &mut Vec<GenesisBuilderValidationError>,
) -> Option<usize> {
    let Some(value) = metadata.get(key) else {
        errors.push(validation_error(&format!("metadata.{key}"), missing_reason));
        return None;
    };

    match value.parse::<usize>() {
        Ok(parsed) => Some(parsed),
        Err(_) => {
            errors.push(validation_error(
                &format!("metadata.{key}"),
                "must be an unsigned integer",
            ));
            None
        }
    }
}

fn require_metadata_key(
    metadata: &HashMap<String, String>,
    key: &str,
    reason: &str,
    errors: &mut Vec<GenesisBuilderValidationError>,
) {
    if !metadata.contains_key(key) {
        errors.push(validation_error(&format!("metadata.{key}"), reason));
    }
}

fn is_validator_deposit(allocation: &GenesisAllocation) -> bool {
    allocation.is_validator || allocation.kind == AllocationKind::ValidatorDeposit
}

fn normalize_address(address: &str) -> String {
    address.trim().to_ascii_lowercase()
}

fn canonical_allocation_record(allocation: &GenesisAllocation) -> String {
    let mut out = String::new();
    out.push_str(&normalize_address(&allocation.address));
    out.push('|');
    out.push_str(allocation_kind_name(&allocation.kind));
    out.push('|');
    out.push_str(&allocation.amount_gwei.to_string());
    out.push('|');
    out.push(if allocation.is_validator { '1' } else { '0' });
    out.push('|');
    out.push_str(&canonical_metadata_fragment(&allocation.metadata));
    out
}

fn compute_artifact_hash(config: &GenesisBuilderConfig, canonical_records: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(ARTIFACT_DOMAIN);
    hasher.update(compute_genesis_builder_commitment(config).as_bytes());
    hasher.update((canonical_records.len() as u64).to_be_bytes());
    for record in canonical_records {
        hash_len_prefixed_bytes(&mut hasher, record.as_bytes());
    }
    digest_to_lower_hex(&hasher.finalize())
}

fn hash_sorted_metadata(hasher: &mut Sha256, metadata: &HashMap<String, String>) {
    let mut pairs: Vec<(&String, &String)> = metadata.iter().collect();
    pairs.sort_unstable_by(|(left_key, left_value), (right_key, right_value)| {
        left_key.cmp(right_key).then(left_value.cmp(right_value))
    });

    hasher.update((pairs.len() as u64).to_be_bytes());
    for (key, value) in pairs {
        hash_len_prefixed_bytes(hasher, key.as_bytes());
        hash_len_prefixed_bytes(hasher, value.as_bytes());
    }
}

fn canonical_metadata_fragment(metadata: &HashMap<String, String>) -> String {
    let mut pairs: Vec<(&String, &String)> = metadata.iter().collect();
    pairs.sort_unstable_by(|(left_key, left_value), (right_key, right_value)| {
        left_key.cmp(right_key).then(left_value.cmp(right_value))
    });

    let mut out = String::new();
    for (index, (key, value)) in pairs.into_iter().enumerate() {
        if index > 0 {
            out.push(';');
        }
        out.push_str(key);
        out.push('=');
        out.push_str(value);
    }
    out
}

fn hash_len_prefixed_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn digest_to_lower_hex(digest: &[u8]) -> String {
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn signing_scheme_discriminant(scheme: &SigningScheme) -> u8 {
    match scheme {
        SigningScheme::Bls12381 => 1,
        SigningScheme::Ed25519 => 2,
        SigningScheme::Secp256k1 => 3,
        SigningScheme::MultiSig => 4,
        SigningScheme::Threshold => 5,
        SigningScheme::Aggregate => 6,
    }
}

fn artifact_format_discriminant(format: &ArtifactFormat) -> u8 {
    match format {
        ArtifactFormat::Ssz => 1,
        ArtifactFormat::Json => 2,
        ArtifactFormat::Rlp => 3,
        ArtifactFormat::Binary => 4,
        ArtifactFormat::HexEncoded => 5,
        ArtifactFormat::CompressedSsz => 6,
    }
}

fn allocation_kind_name(kind: &AllocationKind) -> &'static str {
    match kind {
        AllocationKind::ValidatorDeposit => "validator_deposit",
        AllocationKind::Faucet => "faucet",
        AllocationKind::Treasury => "treasury",
        AllocationKind::Bridge => "bridge",
        AllocationKind::PreMine => "premine",
        AllocationKind::Contract => "contract",
    }
}
