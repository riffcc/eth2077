//! Metrics dashboards, alerting rules, and tracing baseline types for ETH2077.
//!
//! This module provides portable observability type definitions, field-level
//! config validation, and deterministic stats commitments for audit workflows.
//!
//! Design choices:
//! - All public records are serializable.
//! - Validation accumulates errors instead of failing fast.
//! - `compute_stats` binds input slices and derived totals into SHA-256.
//! - Commitment includes a fixed domain separator for versioning safety.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const STATS_COMMITMENT_DOMAIN: &str = "ETH2077::METRICS_DASHBOARD_STATS::V1";
const MIN_SECONDS: u64 = 1;
const MIN_HOURS: u64 = 1;
const MIN_DAYS: u64 = 1;
const MAX_REASONABLE_SCRAPE_INTERVAL_SECONDS: u64 = 3_600;
const MAX_REASONABLE_RETENTION_DAYS: u64 = 3_650;
const MAX_REASONABLE_ALERT_COOLDOWN_SECONDS: u64 = 604_800;
const MAX_REASONABLE_TRACING_SPAN_MS: u64 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

impl MetricType {
    pub fn is_cumulative(self) -> bool {
        matches!(self, Self::Counter)
    }

    pub fn allows_negative_values(self) -> bool {
        matches!(self, Self::Gauge)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
    Debug,
}

impl AlertSeverity {
    pub fn rank(self) -> u8 {
        match self {
            Self::Critical => 1,
            Self::Warning => 2,
            Self::Info => 3,
            Self::Debug => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DashboardPanel {
    TimeSeries,
    Table,
    Stat,
    Heatmap,
    LogViewer,
}

impl DashboardPanel {
    pub fn is_log_oriented(self) -> bool {
        matches!(self, Self::LogViewer | Self::Table)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TracingBackend {
    Jaeger,
    Zipkin,
    OpenTelemetry,
    Custom,
}

impl TracingBackend {
    pub fn default_port(self) -> u16 {
        match self {
            Self::Jaeger => 14250,
            Self::Zipkin => 9411,
            Self::OpenTelemetry => 4317,
            Self::Custom => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricDefinition {
    pub name: String,
    pub metric_type: MetricType,
    pub description: String,
    pub unit: String,
    pub labels: Vec<String>,
    pub retention_days: u64,
}

impl MetricDefinition {
    pub fn normalized_name(&self) -> String {
        self.name.trim().to_ascii_lowercase()
    }

    pub fn has_label(&self, label: &str) -> bool {
        self.labels.iter().any(|candidate| candidate == label)
    }

    pub fn has_description(&self) -> bool {
        !self.description.trim().is_empty()
    }

    pub fn has_unit(&self) -> bool {
        !self.unit.trim().is_empty()
    }

    pub fn validate(&self) -> Vec<MetricsDashboardValidationError> {
        let mut errors = Vec::new();

        if self.name.trim().is_empty() {
            push_validation_error(&mut errors, "metric.name", "must not be empty");
        }
        if self.description.trim().is_empty() {
            push_validation_error(&mut errors, "metric.description", "must not be empty");
        }
        if self.unit.trim().is_empty() {
            push_validation_error(&mut errors, "metric.unit", "must not be empty");
        }
        if self.retention_days < MIN_DAYS {
            push_validation_error(
                &mut errors,
                "metric.retention_days",
                "must be greater than zero",
            );
        }

        errors
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: String,
    pub name: String,
    pub metric_name: String,
    pub condition: String,
    pub threshold: f64,
    pub severity: AlertSeverity,
    pub cooldown_seconds: u64,
    pub notification_channels: Vec<String>,
    pub enabled: bool,
}

impl AlertRule {
    pub fn is_active(&self) -> bool {
        self.enabled
    }

    pub fn has_channel(&self, channel: &str) -> bool {
        self.notification_channels
            .iter()
            .any(|candidate| candidate == channel)
    }

    pub fn uses_metric(&self, metric_name: &str) -> bool {
        self.metric_name == metric_name
    }

    pub fn validate(&self) -> Vec<MetricsDashboardValidationError> {
        let mut errors = Vec::new();

        if self.id.trim().is_empty() {
            push_validation_error(&mut errors, "alert.id", "must not be empty");
        }
        if self.name.trim().is_empty() {
            push_validation_error(&mut errors, "alert.name", "must not be empty");
        }
        if self.metric_name.trim().is_empty() {
            push_validation_error(&mut errors, "alert.metric_name", "must not be empty");
        }
        if self.condition.trim().is_empty() {
            push_validation_error(&mut errors, "alert.condition", "must not be empty");
        }
        if !self.threshold.is_finite() {
            push_validation_error(&mut errors, "alert.threshold", "must be finite");
        }
        if self.cooldown_seconds < MIN_SECONDS {
            push_validation_error(
                &mut errors,
                "alert.cooldown_seconds",
                "must be greater than zero",
            );
        }
        if self.enabled && self.notification_channels.is_empty() {
            push_validation_error(
                &mut errors,
                "alert.notification_channels",
                "must not be empty when alert is enabled",
            );
        }

        errors
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dashboard {
    pub id: String,
    pub title: String,
    pub panels: Vec<(String, DashboardPanel)>,
    pub refresh_interval_seconds: u64,
    pub time_range_hours: u64,
    pub owner: String,
    pub shared: bool,
}

impl Dashboard {
    pub fn panel_count(&self) -> usize {
        self.panels.len()
    }

    pub fn has_owner(&self) -> bool {
        !self.owner.trim().is_empty()
    }

    pub fn contains_panel_title(&self, panel_title: &str) -> bool {
        self.panels
            .iter()
            .any(|(title, _)| title.as_str() == panel_title)
    }

    pub fn validate(&self) -> Vec<MetricsDashboardValidationError> {
        let mut errors = Vec::new();

        if self.id.trim().is_empty() {
            push_validation_error(&mut errors, "dashboard.id", "must not be empty");
        }
        if self.title.trim().is_empty() {
            push_validation_error(&mut errors, "dashboard.title", "must not be empty");
        }
        if self.refresh_interval_seconds < MIN_SECONDS {
            push_validation_error(
                &mut errors,
                "dashboard.refresh_interval_seconds",
                "must be greater than zero",
            );
        }
        if self.time_range_hours < MIN_HOURS {
            push_validation_error(
                &mut errors,
                "dashboard.time_range_hours",
                "must be greater than zero",
            );
        }
        if self.owner.trim().is_empty() {
            push_validation_error(&mut errors, "dashboard.owner", "must not be empty");
        }

        errors
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TracingConfig {
    pub backend: TracingBackend,
    pub sample_rate: f64,
    pub max_span_duration_ms: u64,
    pub propagation_format: String,
    pub endpoint: String,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            backend: TracingBackend::OpenTelemetry,
            sample_rate: 0.1,
            max_span_duration_ms: 60_000,
            propagation_format: "w3c".to_string(),
            endpoint: "http://localhost:4317".to_string(),
        }
    }
}

impl TracingConfig {
    pub fn is_sampling_enabled(&self) -> bool {
        self.sample_rate > 0.0
    }

    pub fn validate(&self) -> Vec<MetricsDashboardValidationError> {
        let mut errors = Vec::new();

        if !(0.0..=1.0).contains(&self.sample_rate) {
            push_validation_error(
                &mut errors,
                "tracing_config.sample_rate",
                "must be between 0.0 and 1.0",
            );
        }
        if !self.sample_rate.is_finite() {
            push_validation_error(&mut errors, "tracing_config.sample_rate", "must be finite");
        }
        if self.max_span_duration_ms < 1 {
            push_validation_error(
                &mut errors,
                "tracing_config.max_span_duration_ms",
                "must be greater than zero",
            );
        }
        if self.max_span_duration_ms > MAX_REASONABLE_TRACING_SPAN_MS {
            push_validation_error(
                &mut errors,
                "tracing_config.max_span_duration_ms",
                "is unreasonably high",
            );
        }
        if self.propagation_format.trim().is_empty() {
            push_validation_error(
                &mut errors,
                "tracing_config.propagation_format",
                "must not be empty",
            );
        }
        if self.endpoint.trim().is_empty() {
            push_validation_error(&mut errors, "tracing_config.endpoint", "must not be empty");
        } else if !looks_like_http_endpoint(&self.endpoint) {
            push_validation_error(
                &mut errors,
                "tracing_config.endpoint",
                "must start with http:// or https://",
            );
        }

        errors
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricsDashboardConfig {
    pub scrape_interval_seconds: u64,
    pub retention_days: u64,
    pub max_metrics: usize,
    pub max_alerts: usize,
    pub default_alert_cooldown_seconds: u64,
    pub tracing_config: TracingConfig,
    pub enable_profiling: bool,
}

impl Default for MetricsDashboardConfig {
    fn default() -> Self {
        Self {
            scrape_interval_seconds: 15,
            retention_days: 30,
            max_metrics: 10_000,
            max_alerts: 500,
            default_alert_cooldown_seconds: 300,
            tracing_config: TracingConfig::default(),
            enable_profiling: false,
        }
    }
}

impl MetricsDashboardConfig {
    pub fn validate(&self) -> Vec<MetricsDashboardValidationError> {
        let mut errors = Vec::new();

        if self.scrape_interval_seconds < MIN_SECONDS {
            push_validation_error(
                &mut errors,
                "scrape_interval_seconds",
                "must be greater than zero",
            );
        }
        if self.scrape_interval_seconds > MAX_REASONABLE_SCRAPE_INTERVAL_SECONDS {
            push_validation_error(
                &mut errors,
                "scrape_interval_seconds",
                "is unreasonably high for live dashboards",
            );
        }

        if self.retention_days < MIN_DAYS {
            push_validation_error(&mut errors, "retention_days", "must be greater than zero");
        }
        if self.retention_days > MAX_REASONABLE_RETENTION_DAYS {
            push_validation_error(
                &mut errors,
                "retention_days",
                "is unreasonably high for baseline configuration",
            );
        }

        if self.max_metrics == 0 {
            push_validation_error(&mut errors, "max_metrics", "must be greater than zero");
        }
        if self.max_alerts == 0 {
            push_validation_error(&mut errors, "max_alerts", "must be greater than zero");
        }
        if self.max_alerts > self.max_metrics {
            push_validation_error(&mut errors, "max_alerts", "must not exceed max_metrics");
        }

        if self.default_alert_cooldown_seconds < MIN_SECONDS {
            push_validation_error(
                &mut errors,
                "default_alert_cooldown_seconds",
                "must be greater than zero",
            );
        }
        if self.default_alert_cooldown_seconds > MAX_REASONABLE_ALERT_COOLDOWN_SECONDS {
            push_validation_error(
                &mut errors,
                "default_alert_cooldown_seconds",
                "is unreasonably high",
            );
        }

        errors.extend(self.tracing_config.validate());
        errors
    }

    pub fn can_store(&self, metric_count: usize, alert_count: usize) -> bool {
        metric_count <= self.max_metrics && alert_count <= self.max_alerts
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricsDashboardValidationError {
    pub field: String,
    pub message: String,
}

impl MetricsDashboardValidationError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricsDashboardStats {
    pub total_metrics: usize,
    pub total_alerts: usize,
    pub active_alerts: usize,
    pub total_dashboards: usize,
    pub avg_panels_per_dashboard: f64,
    pub tracing_sample_rate: f64,
    pub commitment: [u8; 32],
}

#[derive(Debug, Serialize)]
struct StatsCommitmentPayload<'a> {
    domain: &'a str,
    metrics: &'a [MetricDefinition],
    alerts: &'a [AlertRule],
    dashboards: &'a [Dashboard],
    total_metrics: usize,
    total_alerts: usize,
    active_alerts: usize,
    total_dashboards: usize,
    avg_panels_per_dashboard: f64,
    tracing_sample_rate: f64,
}

pub fn compute_stats(
    metrics: &[MetricDefinition],
    alerts: &[AlertRule],
    dashboards: &[Dashboard],
) -> MetricsDashboardStats {
    let total_metrics = metrics.len();
    let total_alerts = alerts.len();
    let active_alerts = alerts.iter().filter(|rule| rule.is_active()).count();
    let total_dashboards = dashboards.len();
    let total_panels = dashboards.iter().map(Dashboard::panel_count).sum::<usize>();
    let avg_panels_per_dashboard = average(total_panels, total_dashboards);

    // This module represents baseline infrastructure types. The baseline tracing
    // sample rate is the system default when no runtime override is supplied.
    let tracing_sample_rate = TracingConfig::default().sample_rate;

    let mut stats = MetricsDashboardStats {
        total_metrics,
        total_alerts,
        active_alerts,
        total_dashboards,
        avg_panels_per_dashboard,
        tracing_sample_rate,
        commitment: [0u8; 32],
    };

    stats.commitment = compute_stats_commitment(metrics, alerts, dashboards, &stats);
    stats
}

fn push_validation_error(
    errors: &mut Vec<MetricsDashboardValidationError>,
    field: &str,
    message: &str,
) {
    errors.push(MetricsDashboardValidationError::new(field, message));
}

fn average(total: usize, count: usize) -> f64 {
    if count == 0 {
        0.0
    } else {
        (total as f64) / (count as f64)
    }
}

fn looks_like_http_endpoint(endpoint: &str) -> bool {
    endpoint.starts_with("http://") || endpoint.starts_with("https://")
}

fn compute_stats_commitment(
    metrics: &[MetricDefinition],
    alerts: &[AlertRule],
    dashboards: &[Dashboard],
    stats: &MetricsDashboardStats,
) -> [u8; 32] {
    let payload = StatsCommitmentPayload {
        domain: STATS_COMMITMENT_DOMAIN,
        metrics,
        alerts,
        dashboards,
        total_metrics: stats.total_metrics,
        total_alerts: stats.total_alerts,
        active_alerts: stats.active_alerts,
        total_dashboards: stats.total_dashboards,
        avg_panels_per_dashboard: stats.avg_panels_per_dashboard,
        tracing_sample_rate: stats.tracing_sample_rate,
    };

    let encoded_payload = serialize_commitment_payload(&payload);
    let digest = Sha256::digest(encoded_payload.as_slice());

    let mut commitment = [0u8; 32];
    commitment.copy_from_slice(&digest);
    commitment
}

fn serialize_commitment_payload(payload: &StatsCommitmentPayload<'_>) -> Vec<u8> {
    match serde_json::to_vec(payload) {
        Ok(encoded) => encoded,
        Err(_) => {
            // Fallback should be unreachable for serializable payloads. This still
            // preserves deterministic output shape if serialization ever fails.
            format!(
                "{{\"domain\":\"{}\",\"total_metrics\":{},\"total_alerts\":{},\"active_alerts\":{},\"total_dashboards\":{},\"avg_panels_per_dashboard\":{},\"tracing_sample_rate\":{}}}",
                STATS_COMMITMENT_DOMAIN,
                payload.total_metrics,
                payload.total_alerts,
                payload.active_alerts,
                payload.total_dashboards,
                payload.avg_panels_per_dashboard,
                payload.tracing_sample_rate
            )
            .into_bytes()
        }
    }
}
