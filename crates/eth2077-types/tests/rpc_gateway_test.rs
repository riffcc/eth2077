use eth2077_types::rpc_gateway::*;

fn endpoint(
    path: &str,
    namespace: RpcNamespace,
    enabled: bool,
    auth_required: bool,
    rate_limit_rps: u64,
) -> RpcEndpoint {
    RpcEndpoint {
        path: path.to_string(),
        namespace,
        auth_required,
        rate_limit_rps,
        timeout_ms: 12_000,
        cache_ttl_seconds: Some(15),
        enabled,
    }
}

fn client(
    id: &str,
    allowed_namespaces: Vec<RpcNamespace>,
    blocked: bool,
    total_requests: u64,
) -> GatewayClient {
    GatewayClient {
        id: id.to_string(),
        api_key_hash: [7_u8; 32],
        auth_method: AuthMethod::ApiKey,
        allowed_namespaces,
        rate_limit_override: None,
        created_at_unix: 1_766_000_000,
        last_request_unix: Some(1_766_000_777),
        total_requests,
        blocked,
    }
}

#[test]
fn default_config_matches_requested_values_and_validates() {
    let config = RpcGatewayConfig::default();

    assert_eq!(config.listen_address, "0.0.0.0");
    assert_eq!(config.listen_port, 8545);
    assert_eq!(config.default_auth_method, AuthMethod::ApiKey);
    assert_eq!(config.default_rate_limit_rps, 100);
    assert_eq!(config.rate_limit_algorithm, RateLimitAlgorithm::TokenBucket);
    assert_eq!(config.max_connections, 1_000);
    assert_eq!(config.request_timeout_ms, 30_000);
    assert!(config.tls_enabled);
    assert_eq!(config.max_batch_size, 100);

    let errors = config.validate();
    assert!(
        errors.is_empty(),
        "unexpected validation errors: {errors:?}"
    );
}

#[test]
fn validation_reports_multiple_configuration_issues() {
    let mut config = RpcGatewayConfig::default();
    config.listen_address = "   ".to_string();
    config.listen_port = 0;
    config.default_rate_limit_rps = 0;
    config.max_connections = 0;
    config.request_timeout_ms = 0;
    config.max_batch_size = 0;
    config.tls_enabled = false;
    config.default_auth_method = AuthMethod::MutualTls;
    config.cors_origins = vec![
        " ".to_string(),
        "ftp://bad.example".to_string(),
        "https://api.eth2077.org".to_string(),
        "HTTPS://api.eth2077.org".to_string(),
    ];

    let errors = config.validate();

    assert!(errors.iter().any(|e| e.field == "listen_address"));
    assert!(errors.iter().any(|e| e.field == "listen_port"));
    assert!(errors.iter().any(|e| e.field == "default_rate_limit_rps"));
    assert!(errors.iter().any(|e| e.field == "max_connections"));
    assert!(errors.iter().any(|e| e.field == "request_timeout_ms"));
    assert!(errors.iter().any(|e| e.field == "max_batch_size"));
    assert!(errors.iter().any(|e| e.field == "default_auth_method"));
    assert!(errors.iter().any(|e| e.field == "cors_origins[0]"));
    assert!(errors.iter().any(|e| e.field == "cors_origins[1]"));
    assert!(errors.iter().any(|e| e.field == "cors_origins[3]"));
}

#[test]
fn compute_stats_aggregates_counts_and_produces_stable_commitment() {
    let endpoints = vec![
        endpoint("/", RpcNamespace::Web3, true, false, 300),
        endpoint("/eth", RpcNamespace::Eth, true, true, 200),
        endpoint("/admin", RpcNamespace::Admin, false, true, 5),
    ];
    let clients = vec![
        client(
            "alice",
            vec![RpcNamespace::Eth, RpcNamespace::Net],
            false,
            25,
        ),
        client("bob", vec![RpcNamespace::Web3], true, 10),
        client(
            "carol",
            vec![RpcNamespace::Eth, RpcNamespace::Web3],
            false,
            5,
        ),
    ];

    let stats = compute_stats(&endpoints, &clients);
    assert_eq!(stats.total_endpoints, 3);
    assert_eq!(stats.total_clients, 3);
    assert_eq!(stats.blocked_clients, 1);
    assert_eq!(stats.total_requests, 40);
    assert!((stats.avg_requests_per_client - 13.333_333_333).abs() < 1e-9);
    assert_eq!(stats.namespaces_enabled, 2);
    assert_ne!(stats.commitment, [0_u8; 32]);

    let reordered_endpoints = vec![
        endpoints[2].clone(),
        endpoints[0].clone(),
        endpoints[1].clone(),
    ];
    let reordered_clients = vec![clients[1].clone(), clients[2].clone(), clients[0].clone()];
    let stats_reordered = compute_stats(&reordered_endpoints, &reordered_clients);
    assert_eq!(stats.commitment, stats_reordered.commitment);

    let mut changed_clients = clients.clone();
    changed_clients[0].total_requests = 26;
    let changed = compute_stats(&endpoints, &changed_clients);
    assert_ne!(stats.commitment, changed.commitment);
}

#[test]
fn namespace_access_control_respects_block_state_and_membership() {
    let allowed = client("dora", vec![RpcNamespace::Eth, RpcNamespace::Net], false, 0);
    assert!(allowed.is_allowed(RpcNamespace::Eth));
    assert!(allowed.is_allowed(RpcNamespace::Net));
    assert!(!allowed.is_allowed(RpcNamespace::Web3));

    let blocked = client("erin", vec![RpcNamespace::Eth, RpcNamespace::Web3], true, 0);
    assert!(!blocked.is_allowed(RpcNamespace::Eth));
    assert!(!blocked.is_allowed(RpcNamespace::Web3));
}
