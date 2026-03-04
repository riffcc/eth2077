//! RPC gateway types for endpoint policy, auth controls, and rate limiting.
//!
//! This module provides a compact model for ETH2077 RPC gateway runtime
//! configuration:
//!
//! - Endpoint-level routing and timeout/cache settings.
//! - Auth method declarations for clients and defaults.
//! - Rate-limiting rule metadata.
//! - Client authorization state and namespace permissions.
//! - Aggregate statistics with a deterministic SHA-256 commitment.
//!
//! The commitment hash is designed to be stable for equivalent data even if
//! input slices are presented in a different order.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Domain separator for gateway-stats commitments.
const RPC_GATEWAY_HASH_DOMAIN: &str = "ETH2077-RPC-GATEWAY-STATS-V1";

/// Conservative upper bound for gateway request timeout values.
const MAX_REASONABLE_TIMEOUT_MS: u64 = 10 * 60 * 1_000;

/// Conservative upper bound for default rate limits.
const MAX_REASONABLE_RATE_LIMIT_RPS: u64 = 10_000_000;

/// Conservative upper bound for RPC batch size.
const MAX_REASONABLE_BATCH_SIZE: usize = 10_000;

/// Authentication method used by RPC clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    /// API key provided by the client.
    ApiKey,
    /// JWT bearer token authorization.
    JwtBearer,
    /// Mutual TLS client certificate authentication.
    MutualTls,
    /// Endpoint permits unauthenticated requests.
    NoAuth,
}

/// Algorithm used to enforce request-rate controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateLimitAlgorithm {
    /// Bucket replenished over time with burst support.
    TokenBucket,
    /// Rolling window with moving request-count boundaries.
    SlidingWindow,
    /// Fixed request window reset on periodic boundaries.
    FixedWindow,
    /// Queue-like draining behavior.
    LeakyBucket,
}

/// Standard RPC namespace classes exposed by the gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RpcNamespace {
    /// Ethereum JSON-RPC methods under `eth_*`.
    Eth,
    /// Networking namespace methods under `net_*`.
    Net,
    /// Utility namespace methods under `web3_*`.
    Web3,
    /// Debugging namespace methods under `debug_*`.
    Debug,
    /// Administrative namespace methods under `admin_*`.
    Admin,
    /// Engine API namespace methods under `engine_*`.
    Engine,
    /// Custom or extension namespace set.
    Custom,
}

/// Health classification for current gateway state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GatewayHealth {
    /// Gateway is operating within expected service bounds.
    Healthy,
    /// Gateway serves traffic but with reduced margin or quality.
    Degraded,
    /// Gateway is saturated and likely shedding work.
    Overloaded,
    /// Gateway cannot currently provide service.
    Unavailable,
}

/// Endpoint configuration for a single RPC route.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpcEndpoint {
    /// URL path exposed by the gateway.
    pub path: String,
    /// Namespace associated with this route.
    pub namespace: RpcNamespace,
    /// Indicates whether auth checks are required.
    pub auth_required: bool,
    /// Per-endpoint request-per-second limit.
    pub rate_limit_rps: u64,
    /// Request timeout budget for this endpoint.
    pub timeout_ms: u64,
    /// Optional cache TTL in seconds.
    pub cache_ttl_seconds: Option<u64>,
    /// Indicates whether this endpoint is currently enabled.
    pub enabled: bool,
}

/// Named rate-limit policy rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitRule {
    /// Human-readable rule name.
    pub name: String,
    /// Selected rate-limit algorithm.
    pub algorithm: RateLimitAlgorithm,
    /// Requests per second allowed by this rule.
    pub requests_per_second: u64,
    /// Burst capacity allowed before enforcement.
    pub burst_size: u64,
    /// Indicates if this rule applies per source IP.
    pub per_ip: bool,
    /// Indicates if this rule applies per API key.
    pub per_api_key: bool,
    /// Penalty duration in seconds when limit is violated.
    pub penalty_seconds: u64,
}

/// Known client configured to access the RPC gateway.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayClient {
    /// Stable client identifier.
    pub id: String,
    /// SHA-256 hash of the client API key material.
    pub api_key_hash: [u8; 32],
    /// Auth method expected for this client.
    pub auth_method: AuthMethod,
    /// Namespaces this client is allowed to access.
    pub allowed_namespaces: Vec<RpcNamespace>,
    /// Optional request-per-second override for this client.
    pub rate_limit_override: Option<u64>,
    /// Client creation timestamp (unix seconds).
    pub created_at_unix: u64,
    /// Last seen request timestamp (unix seconds).
    pub last_request_unix: Option<u64>,
    /// Cumulative requests served for this client.
    pub total_requests: u64,
    /// Hard block switch for this client.
    pub blocked: bool,
}

/// Top-level gateway runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpcGatewayConfig {
    /// Bind address for listener sockets.
    pub listen_address: String,
    /// Bind port for listener sockets.
    pub listen_port: u16,
    /// Auth method used if endpoint/client does not override.
    pub default_auth_method: AuthMethod,
    /// Default request-per-second limit.
    pub default_rate_limit_rps: u64,
    /// Default rate-limit algorithm.
    pub rate_limit_algorithm: RateLimitAlgorithm,
    /// Maximum accepted concurrent connections.
    pub max_connections: usize,
    /// Per-request timeout budget in milliseconds.
    pub request_timeout_ms: u64,
    /// Allowed CORS origins.
    pub cors_origins: Vec<String>,
    /// Enables TLS transport for inbound traffic.
    pub tls_enabled: bool,
    /// Maximum calls accepted in a JSON-RPC batch.
    pub max_batch_size: usize,
}

/// Validation issue found in [`RpcGatewayConfig`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpcGatewayValidationError {
    /// Field name that failed validation.
    pub field: String,
    /// Human-readable validation message.
    pub message: String,
}

/// Aggregate metrics describing endpoint/client gateway state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcGatewayStats {
    /// Number of endpoint records included.
    pub total_endpoints: usize,
    /// Number of client records included.
    pub total_clients: usize,
    /// Number of blocked clients.
    pub blocked_clients: usize,
    /// Total request count across all clients.
    pub total_requests: u64,
    /// Average request count per client.
    pub avg_requests_per_client: f64,
    /// Number of distinct namespaces with at least one enabled endpoint.
    pub namespaces_enabled: usize,
    /// SHA-256 commitment for this snapshot.
    pub commitment: [u8; 32],
}

impl Default for RpcGatewayConfig {
    fn default() -> Self {
        Self {
            listen_address: "0.0.0.0".to_string(),
            listen_port: 8545,
            default_auth_method: AuthMethod::ApiKey,
            default_rate_limit_rps: 100,
            rate_limit_algorithm: RateLimitAlgorithm::TokenBucket,
            max_connections: 1_000,
            request_timeout_ms: 30_000,
            cors_origins: Vec::new(),
            tls_enabled: true,
            max_batch_size: 100,
        }
    }
}

impl RpcGatewayConfig {
    /// Validates gateway config and returns all discovered issues.
    ///
    /// Validation is intentionally conservative and rejects clearly unsafe or
    /// malformed settings while avoiding policy assumptions outside this type.
    pub fn validate(&self) -> Vec<RpcGatewayValidationError> {
        let mut errors = Vec::new();

        if self.listen_address.trim().is_empty() {
            push_error(
                &mut errors,
                "listen_address",
                "listen_address must be non-empty",
            );
        }

        if self.listen_port == 0 {
            push_error(
                &mut errors,
                "listen_port",
                "listen_port must be greater than zero",
            );
        }

        if self.default_rate_limit_rps == 0 {
            push_error(
                &mut errors,
                "default_rate_limit_rps",
                "default_rate_limit_rps must be greater than zero",
            );
        }
        if self.default_rate_limit_rps > MAX_REASONABLE_RATE_LIMIT_RPS {
            push_error(
                &mut errors,
                "default_rate_limit_rps",
                "default_rate_limit_rps exceeds conservative upper bound",
            );
        }

        if self.max_connections == 0 {
            push_error(
                &mut errors,
                "max_connections",
                "max_connections must be greater than zero",
            );
        }

        if self.request_timeout_ms == 0 {
            push_error(
                &mut errors,
                "request_timeout_ms",
                "request_timeout_ms must be greater than zero",
            );
        }
        if self.request_timeout_ms > MAX_REASONABLE_TIMEOUT_MS {
            push_error(
                &mut errors,
                "request_timeout_ms",
                "request_timeout_ms exceeds conservative upper bound",
            );
        }

        if self.max_batch_size == 0 {
            push_error(
                &mut errors,
                "max_batch_size",
                "max_batch_size must be greater than zero",
            );
        }
        if self.max_batch_size > MAX_REASONABLE_BATCH_SIZE {
            push_error(
                &mut errors,
                "max_batch_size",
                "max_batch_size exceeds conservative upper bound",
            );
        }

        if self.default_auth_method == AuthMethod::MutualTls && !self.tls_enabled {
            push_error(
                &mut errors,
                "default_auth_method",
                "MutualTls requires tls_enabled=true",
            );
        }

        let mut seen_origins = BTreeSet::new();
        for (index, origin) in self.cors_origins.iter().enumerate() {
            let trimmed = origin.trim();
            if trimmed.is_empty() {
                push_error(
                    &mut errors,
                    &format!("cors_origins[{index}]"),
                    "CORS origin must be non-empty",
                );
                continue;
            }

            if !is_valid_cors_origin(trimmed) {
                push_error(
                    &mut errors,
                    &format!("cors_origins[{index}]"),
                    "CORS origin must be '*' or start with http:// or https://",
                );
            }

            let dedupe_key = trimmed.to_ascii_lowercase();
            if !seen_origins.insert(dedupe_key) {
                push_error(
                    &mut errors,
                    &format!("cors_origins[{index}]"),
                    "duplicate CORS origin",
                );
            }
        }

        errors
    }
}

impl GatewayClient {
    /// Returns true when the client can access the requested namespace.
    ///
    /// Blocked clients are always denied regardless of namespace membership.
    pub fn is_allowed(&self, namespace: RpcNamespace) -> bool {
        if self.blocked {
            return false;
        }

        self.allowed_namespaces
            .iter()
            .any(|allowed| *allowed == namespace)
    }
}

/// Computes aggregate RPC gateway stats and a deterministic SHA-256 commitment.
pub fn compute_stats(endpoints: &[RpcEndpoint], clients: &[GatewayClient]) -> RpcGatewayStats {
    let total_endpoints = endpoints.len();
    let total_clients = clients.len();
    let blocked_clients = clients.iter().filter(|client| client.blocked).count();
    let total_requests = clients.iter().fold(0_u64, |acc, client| {
        acc.saturating_add(client.total_requests)
    });

    let avg_requests_per_client = if total_clients == 0 {
        0.0
    } else {
        total_requests as f64 / total_clients as f64
    };

    let namespaces_enabled = endpoints
        .iter()
        .filter(|endpoint| endpoint.enabled)
        .map(|endpoint| namespace_rank(endpoint.namespace))
        .collect::<BTreeSet<_>>()
        .len();

    let commitment = compute_stats_commitment(
        endpoints,
        clients,
        total_endpoints,
        total_clients,
        blocked_clients,
        total_requests,
        namespaces_enabled,
    );

    RpcGatewayStats {
        total_endpoints,
        total_clients,
        blocked_clients,
        total_requests,
        avg_requests_per_client,
        namespaces_enabled,
        commitment,
    }
}

fn push_error(errors: &mut Vec<RpcGatewayValidationError>, field: &str, message: &str) {
    errors.push(RpcGatewayValidationError {
        field: field.to_string(),
        message: message.to_string(),
    });
}

fn is_valid_cors_origin(origin: &str) -> bool {
    if origin == "*" {
        return true;
    }

    if origin.chars().any(char::is_whitespace) {
        return false;
    }

    origin.starts_with("http://") || origin.starts_with("https://")
}

#[derive(Debug, Clone, Serialize)]
struct CommitmentEndpoint {
    path: String,
    namespace: u8,
    auth_required: bool,
    rate_limit_rps: u64,
    timeout_ms: u64,
    cache_ttl_seconds: Option<u64>,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
struct CommitmentClient {
    id: String,
    api_key_hash: [u8; 32],
    auth_method: u8,
    allowed_namespaces: Vec<u8>,
    rate_limit_override: Option<u64>,
    created_at_unix: u64,
    last_request_unix: Option<u64>,
    total_requests: u64,
    blocked: bool,
}

#[derive(Debug, Clone, Serialize)]
struct CommitmentPayload {
    domain: &'static str,
    total_endpoints: usize,
    total_clients: usize,
    blocked_clients: usize,
    total_requests: u64,
    namespaces_enabled: usize,
    endpoints: Vec<CommitmentEndpoint>,
    clients: Vec<CommitmentClient>,
}

fn compute_stats_commitment(
    endpoints: &[RpcEndpoint],
    clients: &[GatewayClient],
    total_endpoints: usize,
    total_clients: usize,
    blocked_clients: usize,
    total_requests: u64,
    namespaces_enabled: usize,
) -> [u8; 32] {
    let mut endpoint_rows = endpoints
        .iter()
        .map(|endpoint| CommitmentEndpoint {
            path: endpoint.path.clone(),
            namespace: namespace_rank(endpoint.namespace),
            auth_required: endpoint.auth_required,
            rate_limit_rps: endpoint.rate_limit_rps,
            timeout_ms: endpoint.timeout_ms,
            cache_ttl_seconds: endpoint.cache_ttl_seconds,
            enabled: endpoint.enabled,
        })
        .collect::<Vec<_>>();

    endpoint_rows.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.namespace.cmp(&right.namespace))
            .then_with(|| left.auth_required.cmp(&right.auth_required))
            .then_with(|| left.rate_limit_rps.cmp(&right.rate_limit_rps))
            .then_with(|| left.timeout_ms.cmp(&right.timeout_ms))
            .then_with(|| left.cache_ttl_seconds.cmp(&right.cache_ttl_seconds))
            .then_with(|| left.enabled.cmp(&right.enabled))
    });

    let mut client_rows = clients
        .iter()
        .map(|client| CommitmentClient {
            id: client.id.clone(),
            api_key_hash: client.api_key_hash,
            auth_method: auth_method_rank(client.auth_method),
            allowed_namespaces: sorted_namespace_ranks(&client.allowed_namespaces),
            rate_limit_override: client.rate_limit_override,
            created_at_unix: client.created_at_unix,
            last_request_unix: client.last_request_unix,
            total_requests: client.total_requests,
            blocked: client.blocked,
        })
        .collect::<Vec<_>>();

    client_rows.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.api_key_hash.cmp(&right.api_key_hash))
            .then_with(|| left.auth_method.cmp(&right.auth_method))
            .then_with(|| left.allowed_namespaces.cmp(&right.allowed_namespaces))
            .then_with(|| left.rate_limit_override.cmp(&right.rate_limit_override))
            .then_with(|| left.created_at_unix.cmp(&right.created_at_unix))
            .then_with(|| left.last_request_unix.cmp(&right.last_request_unix))
            .then_with(|| left.total_requests.cmp(&right.total_requests))
            .then_with(|| left.blocked.cmp(&right.blocked))
    });

    let payload = CommitmentPayload {
        domain: RPC_GATEWAY_HASH_DOMAIN,
        total_endpoints,
        total_clients,
        blocked_clients,
        total_requests,
        namespaces_enabled,
        endpoints: endpoint_rows,
        clients: client_rows,
    };

    let encoded = serde_json::to_vec(&payload)
        .expect("rpc gateway commitment payload serialization should not fail");
    let digest = Sha256::digest(encoded);

    let mut commitment = [0_u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn sorted_namespace_ranks(namespaces: &[RpcNamespace]) -> Vec<u8> {
    let mut ranks = namespaces
        .iter()
        .map(|namespace| namespace_rank(*namespace))
        .collect::<Vec<_>>();
    ranks.sort_unstable();
    ranks.dedup();
    ranks
}

fn auth_method_rank(method: AuthMethod) -> u8 {
    match method {
        AuthMethod::ApiKey => 0,
        AuthMethod::JwtBearer => 1,
        AuthMethod::MutualTls => 2,
        AuthMethod::NoAuth => 3,
    }
}

fn namespace_rank(namespace: RpcNamespace) -> u8 {
    match namespace {
        RpcNamespace::Eth => 0,
        RpcNamespace::Net => 1,
        RpcNamespace::Web3 => 2,
        RpcNamespace::Debug => 3,
        RpcNamespace::Admin => 4,
        RpcNamespace::Engine => 5,
        RpcNamespace::Custom => 6,
    }
}
