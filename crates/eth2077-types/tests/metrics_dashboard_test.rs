use eth2077_types::metrics_dashboard::{
    compute_stats, AlertRule, AlertSeverity, Dashboard, DashboardPanel, MetricDefinition,
    MetricType, MetricsDashboardConfig, TracingBackend, TracingConfig,
};

fn sample_metric(name: &str, metric_type: MetricType) -> MetricDefinition {
    MetricDefinition {
        name: name.to_string(),
        metric_type,
        description: format!("{name} metric"),
        unit: "count".to_string(),
        labels: vec!["node".to_string(), "region".to_string()],
        retention_days: 30,
    }
}

fn sample_alert(id: &str, enabled: bool) -> AlertRule {
    AlertRule {
        id: id.to_string(),
        name: format!("alert-{id}"),
        metric_name: "eth_block_import_total".to_string(),
        condition: "gt".to_string(),
        threshold: 100.0,
        severity: AlertSeverity::Warning,
        cooldown_seconds: 300,
        notification_channels: vec!["pagerduty".to_string()],
        enabled,
    }
}

fn sample_dashboard(id: &str, panels: Vec<(String, DashboardPanel)>) -> Dashboard {
    Dashboard {
        id: id.to_string(),
        title: format!("dashboard-{id}"),
        panels,
        refresh_interval_seconds: 15,
        time_range_hours: 6,
        owner: "sre@eth2077.local".to_string(),
        shared: true,
    }
}

#[test]
fn default_tracing_config_matches_requested_values() {
    let tracing = TracingConfig::default();
    assert_eq!(tracing.backend, TracingBackend::OpenTelemetry);
    assert!((tracing.sample_rate - 0.1).abs() < f64::EPSILON);
    assert_eq!(tracing.max_span_duration_ms, 60_000);
    assert_eq!(tracing.propagation_format, "w3c");
    assert_eq!(tracing.endpoint, "http://localhost:4317");
}

#[test]
fn default_metrics_dashboard_config_matches_requested_values() {
    let config = MetricsDashboardConfig::default();
    assert_eq!(config.scrape_interval_seconds, 15);
    assert_eq!(config.retention_days, 30);
    assert_eq!(config.max_metrics, 10_000);
    assert_eq!(config.max_alerts, 500);
    assert_eq!(config.default_alert_cooldown_seconds, 300);
    assert!(!config.enable_profiling);
    assert_eq!(config.tracing_config, TracingConfig::default());
}

#[test]
fn config_validation_reports_multiple_invalid_fields() {
    let mut config = MetricsDashboardConfig::default();
    config.scrape_interval_seconds = 0;
    config.retention_days = 0;
    config.max_metrics = 0;
    config.max_alerts = 2;
    config.default_alert_cooldown_seconds = 0;
    config.tracing_config.sample_rate = 1.5;
    config.tracing_config.max_span_duration_ms = 0;
    config.tracing_config.propagation_format = "   ".to_string();
    config.tracing_config.endpoint = "grpc://collector:4317".to_string();

    let errors = config.validate();
    assert!(errors.iter().any(|e| e.field == "scrape_interval_seconds"));
    assert!(errors.iter().any(|e| e.field == "retention_days"));
    assert!(errors.iter().any(|e| e.field == "max_metrics"));
    assert!(errors.iter().any(|e| e.field == "max_alerts"));
    assert!(errors
        .iter()
        .any(|e| e.field == "default_alert_cooldown_seconds"));
    assert!(errors
        .iter()
        .any(|e| e.field == "tracing_config.sample_rate"));
    assert!(errors
        .iter()
        .any(|e| e.field == "tracing_config.max_span_duration_ms"));
    assert!(errors
        .iter()
        .any(|e| e.field == "tracing_config.propagation_format"));
    assert!(errors.iter().any(|e| e.field == "tracing_config.endpoint"));
}

#[test]
fn compute_stats_counts_averages_and_baseline_sample_rate() {
    let metrics = vec![
        sample_metric("eth_block_import_total", MetricType::Counter),
        sample_metric("eth_mempool_size", MetricType::Gauge),
        sample_metric("eth_state_commit_duration_ms", MetricType::Histogram),
    ];
    let alerts = vec![sample_alert("a1", true), sample_alert("a2", false)];
    let dashboards = vec![
        sample_dashboard(
            "d1",
            vec![
                ("Block Import".to_string(), DashboardPanel::TimeSeries),
                ("Mempool".to_string(), DashboardPanel::Stat),
            ],
        ),
        sample_dashboard("d2", vec![("Logs".to_string(), DashboardPanel::LogViewer)]),
    ];

    let stats = compute_stats(&metrics, &alerts, &dashboards);
    assert_eq!(stats.total_metrics, 3);
    assert_eq!(stats.total_alerts, 2);
    assert_eq!(stats.active_alerts, 1);
    assert_eq!(stats.total_dashboards, 2);
    assert!((stats.avg_panels_per_dashboard - 1.5).abs() < 1e-12);
    assert!((stats.tracing_sample_rate - 0.1).abs() < f64::EPSILON);
}

#[test]
fn compute_stats_commitment_is_deterministic_and_input_sensitive() {
    let metrics = vec![sample_metric("eth_block_import_total", MetricType::Counter)];
    let alerts = vec![sample_alert("a1", true)];
    let dashboards = vec![sample_dashboard(
        "d1",
        vec![("Block Import".to_string(), DashboardPanel::TimeSeries)],
    )];

    let first = compute_stats(&metrics, &alerts, &dashboards);
    let second = compute_stats(&metrics, &alerts, &dashboards);
    assert_eq!(first.commitment, second.commitment);

    let modified_alerts = vec![sample_alert("a1", false)];
    let changed = compute_stats(&metrics, &modified_alerts, &dashboards);
    assert_ne!(first.commitment, changed.commitment);
}
