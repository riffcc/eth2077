use eth2077_types::faucet_explorer::{
    compute_faucet_explorer_commitment, compute_faucet_explorer_stats,
    default_faucet_explorer_config, validate_faucet_explorer_config, ExplorerFeature,
    RateLimitPolicy,
};
use std::collections::HashMap;

#[test]
fn default_config_is_valid() {
    let config = default_faucet_explorer_config();
    assert_eq!(validate_faucet_explorer_config(&config), Ok(()));
}

#[test]
fn validation_reports_multiple_field_errors() {
    let mut config = default_faucet_explorer_config();
    config.faucet.drip_amount_gwei = 0;
    config.faucet.cooldown_seconds = 0;
    config.faucet.rate_limit = RateLimitPolicy::Unlimited;
    config.explorer.base_url = "explorer.local".to_string();
    config.explorer.features.push(ExplorerFeature::TxSearch);
    config.explorer.api_rate_limit = 0;
    config.portal_url = "not-a-url".to_string();
    config.documentation_url = config.portal_url.clone();
    config.metadata.insert("".to_string(), "bad".to_string());

    let errors = validate_faucet_explorer_config(&config).unwrap_err();
    for field in [
        "faucet.drip_amount_gwei",
        "faucet.cooldown_seconds",
        "faucet.rate_limit",
        "explorer.base_url",
        "explorer.features",
        "explorer.api_rate_limit",
        "portal_url",
        "documentation_url",
        "metadata",
    ] {
        assert!(errors.iter().any(|error| error.field == field));
    }
}

#[test]
fn stats_are_deterministic_and_sane() {
    let stats_a = compute_faucet_explorer_stats(10_000, 8_000, 250_000);
    let stats_b = compute_faucet_explorer_stats(10_000, 8_000, 250_000);

    assert_eq!(stats_a, stats_b);
    assert_eq!(stats_a.total_drips, 10_000);
    assert_eq!(stats_a.unique_recipients, 8_000);
    assert_eq!(stats_a.explorer_queries, 250_000);
    assert!(stats_a.avg_response_ms > 0.0);
    assert!((95.0..=99.99).contains(&stats_a.uptime_pct));
    assert!((1..=6).contains(&stats_a.active_features));
}

#[test]
fn commitment_is_deterministic_and_order_stable() {
    let config = default_faucet_explorer_config();
    let baseline = compute_faucet_explorer_commitment(&config);
    assert_eq!(baseline, compute_faucet_explorer_commitment(&config));

    let mut reordered = config.clone();
    reordered.explorer.features.reverse();
    reordered.metadata = HashMap::from([
        ("support".to_string(), "portal".to_string()),
        ("environment".to_string(), "public-testnet".to_string()),
        ("stage".to_string(), "onboarding".to_string()),
    ]);
    reordered.faucet.metadata = HashMap::from([
        ("network".to_string(), "ETH2077-testnet".to_string()),
        ("owner".to_string(), "devrel".to_string()),
        ("currency".to_string(), "test-eth".to_string()),
    ]);

    assert_eq!(baseline, compute_faucet_explorer_commitment(&reordered));
}

#[test]
fn commitment_changes_on_semantic_update() {
    let config = default_faucet_explorer_config();
    let baseline = compute_faucet_explorer_commitment(&config);

    let mut changed = config.clone();
    changed.faucet.cooldown_seconds += 60;

    assert_ne!(baseline, compute_faucet_explorer_commitment(&changed));
}
