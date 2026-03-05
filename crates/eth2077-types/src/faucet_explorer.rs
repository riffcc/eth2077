//! Faucet and explorer integration types for ETH2077.
//!
//! This module models developer onboarding infrastructure for testnet access:
//! faucet policy, explorer capabilities, portal/docs integration, validation,
//! summary stats, and deterministic configuration commitments.
//!
//! Design principles:
//! - serializable and control-plane friendly configuration shapes,
//! - field-scoped validation errors collected in one pass,
//! - stable SHA-256 commitments across non-semantic ordering changes,
//! - deterministic, compact telemetry output for dashboards and CI gates.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Access model for the testnet faucet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FaucetMode {
    OpenDrip,
    RateLimited,
    CaptchaGated,
    SocialVerified,
    WhitelistOnly,
    Disabled,
}

/// Feature set exposed by the explorer integration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExplorerFeature {
    BlockView,
    TxSearch,
    AccountLookup,
    ContractVerify,
    TokenTracker,
    Analytics,
}

/// Coarse quota strategy applied to faucet and explorer surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RateLimitPolicy {
    PerIp,
    PerAddress,
    PerSession,
    Global,
    Tiered,
    Unlimited,
}

/// Lifecycle state for the explorer integration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IntegrationStatus {
    Configured,
    Deploying,
    Live,
    Degraded,
    Maintenance,
    Retired,
}

/// Faucet policy and operational metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FaucetConfig {
    pub drip_amount_gwei: u64,
    pub mode: FaucetMode,
    pub rate_limit: RateLimitPolicy,
    pub cooldown_seconds: u64,
    pub max_balance_gwei: u64,
    pub metadata: HashMap<String, String>,
}

/// Explorer policy and capability declaration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExplorerConfig {
    pub base_url: String,
    pub features: Vec<ExplorerFeature>,
    pub status: IntegrationStatus,
    pub api_rate_limit: u64,
    pub metadata: HashMap<String, String>,
}

/// Unified onboarding configuration for faucet + explorer + portal links.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FaucetExplorerConfig {
    pub faucet: FaucetConfig,
    pub explorer: ExplorerConfig,
    pub portal_url: String,
    pub documentation_url: String,
    pub metadata: HashMap<String, String>,
}

/// Field-scoped validation issue produced by config validation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FaucetExplorerValidationError {
    pub field: String,
    pub reason: String,
}

/// Summary usage metrics for faucet and explorer onboarding services.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FaucetExplorerStats {
    pub total_drips: u64,
    pub unique_recipients: u64,
    pub explorer_queries: u64,
    pub avg_response_ms: f64,
    pub uptime_pct: f64,
    pub active_features: usize,
}

/// Returns a conservative baseline onboarding configuration for ETH2077.
///
/// Defaults are intentionally practical:
/// - a rate-limited faucet with address-based quota,
/// - an explorer with all standard onboarding features enabled,
/// - explicit portal and documentation URLs,
/// - metadata suitable for dashboards and ownership tagging.
pub fn default_faucet_explorer_config() -> FaucetExplorerConfig {
    let mut faucet_metadata = HashMap::new();
    faucet_metadata.insert("owner".to_string(), "devrel".to_string());
    faucet_metadata.insert("network".to_string(), "ETH2077-testnet".to_string());
    faucet_metadata.insert("currency".to_string(), "test-eth".to_string());

    let mut explorer_metadata = HashMap::new();
    explorer_metadata.insert("vendor".to_string(), "eth2077-explorer".to_string());
    explorer_metadata.insert("indexing".to_string(), "realtime".to_string());
    explorer_metadata.insert("api_version".to_string(), "v1".to_string());

    let mut metadata = HashMap::new();
    metadata.insert("environment".to_string(), "public-testnet".to_string());
    metadata.insert("stage".to_string(), "onboarding".to_string());
    metadata.insert("support".to_string(), "portal".to_string());

    FaucetExplorerConfig {
        faucet: FaucetConfig {
            drip_amount_gwei: 50_000_000,
            mode: FaucetMode::RateLimited,
            rate_limit: RateLimitPolicy::PerAddress,
            cooldown_seconds: 3_600,
            max_balance_gwei: 500_000_000,
            metadata: faucet_metadata,
        },
        explorer: ExplorerConfig {
            base_url: "https://explorer.testnet.eth2077.org".to_string(),
            features: vec![
                ExplorerFeature::BlockView,
                ExplorerFeature::TxSearch,
                ExplorerFeature::AccountLookup,
                ExplorerFeature::ContractVerify,
                ExplorerFeature::TokenTracker,
                ExplorerFeature::Analytics,
            ],
            status: IntegrationStatus::Live,
            api_rate_limit: 1_200,
            metadata: explorer_metadata,
        },
        portal_url: "https://portal.testnet.eth2077.org".to_string(),
        documentation_url: "https://docs.eth2077.org/testnet/onboarding".to_string(),
        metadata,
    }
}

/// Validates a faucet+explorer onboarding configuration.
///
/// Rules:
/// - faucet amounts must be coherent and non-zero unless disabled,
/// - disabled faucet must have zero drip amount,
/// - URL fields must be absolute HTTP(S),
/// - active explorer states need features and API rate limit,
/// - retired explorer must not publish active features,
/// - explorer features must be duplicate-free,
/// - metadata keys/values must be non-empty when present.
pub fn validate_faucet_explorer_config(
    config: &FaucetExplorerConfig,
) -> Result<(), Vec<FaucetExplorerValidationError>> {
    let mut errors = Vec::new();

    validate_faucet_config(config, &mut errors);
    validate_explorer_config(config, &mut errors);
    validate_url_field(&config.portal_url, "portal_url", &mut errors);
    validate_url_field(&config.documentation_url, "documentation_url", &mut errors);

    if config.portal_url == config.documentation_url {
        push_validation_error(
            &mut errors,
            "documentation_url",
            "documentation_url should differ from portal_url",
        );
    }

    validate_metadata(&config.metadata, "metadata", &mut errors, false, false);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Computes deterministic summary metrics from high-level traffic counters.
///
/// Because only counters are provided, `avg_response_ms` and `uptime_pct`
/// are derived using fixed heuristics that are stable across runs.
pub fn compute_faucet_explorer_stats(
    drip_count: u64,
    unique_addrs: u64,
    queries: u64,
) -> FaucetExplorerStats {
    let total_drips = drip_count;
    let unique_recipients = unique_addrs.min(total_drips);
    let explorer_queries = queries;

    let drip_base = total_drips.max(1) as f64;
    let query_pressure = explorer_queries as f64 / drip_base;

    let avg_response_ms = if explorer_queries == 0 {
        0.0
    } else {
        (65.0 + (query_pressure * 18.0)).clamp(40.0, 600.0)
    };

    let recipient_base = unique_recipients.max(1) as f64;
    let saturation = unique_recipients as f64 / drip_base;
    let query_density = explorer_queries as f64 / recipient_base;

    let uptime_pct = if total_drips == 0 && explorer_queries == 0 {
        99.95
    } else {
        (99.90 - (saturation * 0.20) - (query_density * 0.05)).clamp(95.0, 99.99)
    };

    let active_features = if explorer_queries == 0 {
        1
    } else if explorer_queries < 100 {
        2
    } else if explorer_queries < 1_000 {
        3
    } else if explorer_queries < 10_000 {
        4
    } else if explorer_queries < 100_000 {
        5
    } else {
        6
    };

    FaucetExplorerStats {
        total_drips,
        unique_recipients,
        explorer_queries,
        avg_response_ms,
        uptime_pct,
        active_features,
    }
}

/// Computes a deterministic SHA-256 commitment of onboarding configuration.
///
/// The commitment input includes:
/// - faucet scalar values and enum labels,
/// - explorer scalar values and enum labels,
/// - sorted explorer feature labels,
/// - sorted faucet/explorer/top-level metadata,
/// - portal and documentation URLs.
///
/// Sorting ensures hash stability independent of map insertion order.
pub fn compute_faucet_explorer_commitment(config: &FaucetExplorerConfig) -> String {
    let mut hasher = Sha256::new();

    hasher.update(
        format!(
            "faucet.drip_amount_gwei={}|faucet.mode={}|faucet.rate_limit={}|faucet.cooldown_seconds={}|faucet.max_balance_gwei={}|",
            config.faucet.drip_amount_gwei,
            faucet_mode_label(&config.faucet.mode),
            rate_limit_policy_label(&config.faucet.rate_limit),
            config.faucet.cooldown_seconds,
            config.faucet.max_balance_gwei,
        )
        .as_bytes(),
    );

    hasher.update(
        format!(
            "explorer.base_url={}|explorer.status={}|explorer.api_rate_limit={}|",
            config.explorer.base_url,
            integration_status_label(&config.explorer.status),
            config.explorer.api_rate_limit,
        )
        .as_bytes(),
    );

    let mut feature_labels: Vec<&'static str> = config
        .explorer
        .features
        .iter()
        .map(explorer_feature_label)
        .collect();
    feature_labels.sort_unstable();
    for label in feature_labels {
        hasher.update(b"feature=");
        hasher.update(label.as_bytes());
        hasher.update(b";");
    }

    append_sorted_metadata(&mut hasher, "faucet.metadata", &config.faucet.metadata);
    append_sorted_metadata(&mut hasher, "explorer.metadata", &config.explorer.metadata);
    append_sorted_metadata(&mut hasher, "metadata", &config.metadata);

    hasher.update(format!("portal_url={}|", config.portal_url).as_bytes());
    hasher.update(format!("documentation_url={}|", config.documentation_url).as_bytes());

    let digest = hasher.finalize();
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push_str(&format!("{:02x}", byte));
    }
    output
}

/// Validates faucet-related constraints.
fn validate_faucet_config(
    config: &FaucetExplorerConfig,
    errors: &mut Vec<FaucetExplorerValidationError>,
) {
    if config.faucet.drip_amount_gwei == 0 && config.faucet.mode != FaucetMode::Disabled {
        push_validation_error(
            errors,
            "faucet.drip_amount_gwei",
            "must be greater than 0 unless faucet mode is Disabled",
        );
    }

    if config.faucet.cooldown_seconds == 0 && config.faucet.mode != FaucetMode::Disabled {
        push_validation_error(
            errors,
            "faucet.cooldown_seconds",
            "must be greater than 0 unless faucet mode is Disabled",
        );
    }

    if config.faucet.max_balance_gwei == 0 {
        push_validation_error(errors, "faucet.max_balance_gwei", "must be greater than 0");
    }

    if config.faucet.max_balance_gwei < config.faucet.drip_amount_gwei {
        push_validation_error(
            errors,
            "faucet.max_balance_gwei",
            "must be greater than or equal to faucet.drip_amount_gwei",
        );
    }

    if config.faucet.mode == FaucetMode::Disabled && config.faucet.drip_amount_gwei != 0 {
        push_validation_error(
            errors,
            "faucet.drip_amount_gwei",
            "must be 0 when faucet mode is Disabled",
        );
    }

    if config.faucet.mode == FaucetMode::RateLimited
        && config.faucet.rate_limit == RateLimitPolicy::Unlimited
    {
        push_validation_error(
            errors,
            "faucet.rate_limit",
            "cannot be Unlimited when faucet mode is RateLimited",
        );
    }

    validate_metadata(
        &config.faucet.metadata,
        "faucet.metadata",
        errors,
        false,
        true,
    );
}

/// Validates explorer-related constraints.
fn validate_explorer_config(
    config: &FaucetExplorerConfig,
    errors: &mut Vec<FaucetExplorerValidationError>,
) {
    validate_url_field(&config.explorer.base_url, "explorer.base_url", errors);

    if config.explorer.features.is_empty() && is_active_status(&config.explorer.status) {
        push_validation_error(
            errors,
            "explorer.features",
            "must include at least one feature when explorer is active",
        );
    }

    let mut seen_features: HashMap<String, usize> = HashMap::new();
    for feature in &config.explorer.features {
        let label = explorer_feature_label(feature).to_string();
        let count = seen_features.entry(label.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            push_validation_error(
                errors,
                "explorer.features",
                &format!("contains duplicate feature `{label}`"),
            );
        }
    }

    if config.explorer.api_rate_limit == 0 && is_active_status(&config.explorer.status) {
        push_validation_error(
            errors,
            "explorer.api_rate_limit",
            "must be greater than 0 while explorer status is active",
        );
    }

    if config.explorer.status == IntegrationStatus::Retired && !config.explorer.features.is_empty()
    {
        push_validation_error(
            errors,
            "explorer.features",
            "must be empty when explorer status is Retired",
        );
    }

    validate_metadata(
        &config.explorer.metadata,
        "explorer.metadata",
        errors,
        false,
        true,
    );
}

/// Validates that a URL is non-empty and starts with HTTP(S).
fn validate_url_field(value: &str, field: &str, errors: &mut Vec<FaucetExplorerValidationError>) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        push_validation_error(errors, field, "must not be empty");
        return;
    }

    let is_http = trimmed.starts_with("http://") || trimmed.starts_with("https://");
    if !is_http {
        push_validation_error(errors, field, "must start with http:// or https://");
    }
}

/// Validates metadata key/value hygiene.
fn validate_metadata(
    metadata: &HashMap<String, String>,
    prefix: &str,
    errors: &mut Vec<FaucetExplorerValidationError>,
    require_non_empty: bool,
    enforce_value_non_empty: bool,
) {
    if require_non_empty && metadata.is_empty() {
        push_validation_error(errors, prefix, "must include at least one metadata entry");
    }

    for (key, value) in metadata {
        if key.trim().is_empty() {
            push_validation_error(errors, prefix, "metadata keys must not be empty");
        }

        if enforce_value_non_empty && value.trim().is_empty() {
            push_validation_error(
                errors,
                prefix,
                &format!("metadata value for key `{key}` must not be empty"),
            );
        }
    }
}

/// Appends sorted metadata pairs into the commitment hasher.
fn append_sorted_metadata(hasher: &mut Sha256, scope: &str, metadata: &HashMap<String, String>) {
    let mut entries: Vec<(&String, &String)> = metadata.iter().collect();
    entries.sort_by(|(ka, va), (kb, vb)| ka.cmp(kb).then(va.cmp(vb)));

    for (key, value) in entries {
        hasher.update(scope.as_bytes());
        hasher.update(b".");
        hasher.update(key.as_bytes());
        hasher.update(b"=");
        hasher.update(value.as_bytes());
        hasher.update(b";");
    }
}

/// Appends a single validation error.
fn push_validation_error(
    errors: &mut Vec<FaucetExplorerValidationError>,
    field: &str,
    reason: &str,
) {
    errors.push(FaucetExplorerValidationError {
        field: field.to_string(),
        reason: reason.to_string(),
    });
}

/// Returns true for statuses expected to handle active developer traffic.
fn is_active_status(status: &IntegrationStatus) -> bool {
    matches!(
        status,
        IntegrationStatus::Configured
            | IntegrationStatus::Deploying
            | IntegrationStatus::Live
            | IntegrationStatus::Degraded
    )
}

/// Stable label mapping for `FaucetMode`.
fn faucet_mode_label(mode: &FaucetMode) -> &'static str {
    match mode {
        FaucetMode::OpenDrip => "OpenDrip",
        FaucetMode::RateLimited => "RateLimited",
        FaucetMode::CaptchaGated => "CaptchaGated",
        FaucetMode::SocialVerified => "SocialVerified",
        FaucetMode::WhitelistOnly => "WhitelistOnly",
        FaucetMode::Disabled => "Disabled",
    }
}

/// Stable label mapping for `ExplorerFeature`.
fn explorer_feature_label(feature: &ExplorerFeature) -> &'static str {
    match feature {
        ExplorerFeature::BlockView => "BlockView",
        ExplorerFeature::TxSearch => "TxSearch",
        ExplorerFeature::AccountLookup => "AccountLookup",
        ExplorerFeature::ContractVerify => "ContractVerify",
        ExplorerFeature::TokenTracker => "TokenTracker",
        ExplorerFeature::Analytics => "Analytics",
    }
}

/// Stable label mapping for `RateLimitPolicy`.
fn rate_limit_policy_label(policy: &RateLimitPolicy) -> &'static str {
    match policy {
        RateLimitPolicy::PerIp => "PerIp",
        RateLimitPolicy::PerAddress => "PerAddress",
        RateLimitPolicy::PerSession => "PerSession",
        RateLimitPolicy::Global => "Global",
        RateLimitPolicy::Tiered => "Tiered",
        RateLimitPolicy::Unlimited => "Unlimited",
    }
}

/// Stable label mapping for `IntegrationStatus`.
fn integration_status_label(status: &IntegrationStatus) -> &'static str {
    match status {
        IntegrationStatus::Configured => "Configured",
        IntegrationStatus::Deploying => "Deploying",
        IntegrationStatus::Live => "Live",
        IntegrationStatus::Degraded => "Degraded",
        IntegrationStatus::Maintenance => "Maintenance",
        IntegrationStatus::Retired => "Retired",
    }
}
