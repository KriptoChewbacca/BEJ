use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Health check response for HTTP endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub timestamp: u64,
    pub endpoints: Vec<EndpointHealth>,
    pub summary: HealthSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointHealth {
    pub url: String,
    pub status: String,
    pub latency_ms: f64,
    pub error_count: u64,
    pub consecutive_errors: u64,
    pub tier: String,
    pub location: Option<String>,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSummary {
    pub total_endpoints: usize,
    pub healthy: usize,
    pub degraded: usize,
    pub unhealthy: usize,
    pub overall_status: String,
}

/// Metrics export in Prometheus format
#[derive(Debug, Clone)]
pub struct PrometheusMetrics {
    metrics: Vec<String>,
}

impl PrometheusMetrics {
    pub fn new() -> Self {
        Self {
            metrics: Vec::new(),
        }
    }

    pub fn add_counter(&mut self, name: &str, value: u64, labels: &[(&str, &str)]) {
        let label_str = Self::format_labels(labels);
        self.metrics
            .push(format!("{}{{{}}} {}", name, label_str, value));
    }

    pub fn add_gauge(&mut self, name: &str, value: f64, labels: &[(&str, &str)]) {
        let label_str = Self::format_labels(labels);
        self.metrics
            .push(format!("{}{{{}}} {}", name, label_str, value));
    }

    pub fn add_histogram(
        &mut self,
        name: &str,
        sum: f64,
        count: u64,
        buckets: &[(f64, u64)],
        labels: &[(&str, &str)],
    ) {
        let label_str = Self::format_labels(labels);

        // Add histogram buckets
        for (le, count) in buckets {
            let mut bucket_labels = labels.to_vec();
            let le_string = le.to_string();
            bucket_labels.push(("le", &le_string));
            let bucket_label_str = Self::format_labels(&bucket_labels);
            self.metrics
                .push(format!("{}_bucket{{{}}} {}", name, bucket_label_str, count));
        }

        // Add +Inf bucket
        let mut inf_labels = labels.to_vec();
        inf_labels.push(("le", "+Inf"));
        let inf_label_str = Self::format_labels(&inf_labels);
        self.metrics
            .push(format!("{}_bucket{{{}}} {}", name, inf_label_str, count));

        // Add sum and count
        self.metrics
            .push(format!("{}_sum{{{}}} {}", name, label_str, sum));
        self.metrics
            .push(format!("{}_count{{{}}} {}", name, label_str, count));
    }

    fn format_labels(labels: &[(&str, &str)]) -> String {
        labels
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, v))
            .collect::<Vec<_>>()
            .join(",")
    }
}

impl fmt::Display for PrometheusMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.metrics.join("\n"))
    }
}

impl Default for PrometheusMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// JSON metrics export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonMetrics {
    pub timestamp: u64,
    pub global: GlobalMetrics,
    pub endpoints: Vec<EndpointMetrics>,
    pub tiers: HashMap<String, TierMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalMetrics {
    pub total_requests: u64,
    pub total_errors: u64,
    pub success_rate: f64,
    pub error_rate: f64,
    pub rate_limit_hits: u64,
    pub predictive_switches: u64,
    pub circuit_breaker_opens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointMetrics {
    pub url: String,
    pub tier: String,
    pub location: Option<String>,
    pub total_requests: u64,
    pub total_errors: u64,
    pub consecutive_errors: u64,
    pub success_rate: f64,
    pub latency_p50: f64,
    pub latency_p95: f64,
    pub latency_p99: f64,
    pub avg_latency_ms: f64,
    pub health_status: String,
    pub circuit_breaker_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierMetrics {
    pub tier: String,
    pub total_endpoints: usize,
    pub healthy_endpoints: usize,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub circuit_breaker_state: String,
}

/// Alert definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub name: String,
    pub severity: AlertSeverity,
    pub description: String,
    pub triggered_at: u64,
    pub value: f64,
    pub threshold: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Alert manager for tracking and evaluating alerts
#[derive(Debug, Clone)]
pub struct AlertManager {
    /// Alert definitions and their thresholds
    alert_configs: HashMap<String, AlertConfig>,

    /// Currently active alerts
    active_alerts: HashMap<String, Alert>,
}

#[derive(Debug, Clone)]
pub struct AlertConfig {
    pub name: String,
    pub severity: AlertSeverity,
    pub threshold: f64,
    pub description: String,
    pub evaluation_fn: fn(f64, f64) -> bool,
}

impl AlertManager {
    pub fn new() -> Self {
        let mut manager = Self {
            alert_configs: HashMap::new(),
            active_alerts: HashMap::new(),
        };

        // Register default alerts
        manager.register_default_alerts();
        manager
    }

    fn register_default_alerts(&mut self) {
        // Consecutive failures alert
        self.add_alert(AlertConfig {
            name: "consecutive_failures_high".to_string(),
            severity: AlertSeverity::Warning,
            threshold: 5.0,
            description: "Endpoint has 5+ consecutive failures".to_string(),
            evaluation_fn: |value, threshold| value >= threshold,
        });

        // P99 latency alert
        self.add_alert(AlertConfig {
            name: "p99_latency_high".to_string(),
            severity: AlertSeverity::Warning,
            threshold: 1000.0, // 1 second
            description: "P99 latency exceeds 1000ms".to_string(),
            evaluation_fn: |value, threshold| value >= threshold,
        });

        // Error rate alert
        self.add_alert(AlertConfig {
            name: "error_rate_high".to_string(),
            severity: AlertSeverity::Critical,
            threshold: 0.1, // 10%
            description: "Error rate exceeds 10%".to_string(),
            evaluation_fn: |value, threshold| value >= threshold,
        });

        // Circuit breaker alert
        self.add_alert(AlertConfig {
            name: "circuit_breaker_open".to_string(),
            severity: AlertSeverity::Critical,
            threshold: 1.0,
            description: "Circuit breaker is open".to_string(),
            evaluation_fn: |value, threshold| value >= threshold,
        });
    }

    pub fn add_alert(&mut self, config: AlertConfig) {
        self.alert_configs.insert(config.name.clone(), config);
    }

    pub fn evaluate(&mut self, metric_name: &str, value: f64) -> Option<Alert> {
        if let Some(config) = self.alert_configs.get(metric_name) {
            let should_alert = (config.evaluation_fn)(value, config.threshold);

            if should_alert {
                let alert = Alert {
                    name: config.name.clone(),
                    severity: config.severity,
                    description: config.description.clone(),
                    triggered_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    value,
                    threshold: config.threshold,
                };

                self.active_alerts
                    .insert(config.name.clone(), alert.clone());
                return Some(alert);
            }
            // Clear alert if condition no longer met
            self.active_alerts.remove(metric_name);
        }

        None
    }

    pub fn get_active_alerts(&self) -> Vec<Alert> {
        self.active_alerts.values().cloned().collect()
    }

    pub fn clear_alert(&mut self, name: &str) {
        self.active_alerts.remove(name);
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prometheus_metrics_counter() {
        let mut metrics = PrometheusMetrics::new();
        metrics.add_counter(
            "rpc_requests_total",
            100,
            &[("endpoint", "test"), ("tier", "tier0")],
        );

        let output = metrics.to_string();
        assert!(output.contains("rpc_requests_total"));
        assert!(output.contains("endpoint=\"test\""));
        assert!(output.contains("100"));
    }

    #[test]
    fn test_prometheus_metrics_gauge() {
        let mut metrics = PrometheusMetrics::new();
        metrics.add_gauge("rpc_latency_ms", 123.45, &[("endpoint", "test")]);

        let output = metrics.to_string();
        assert!(output.contains("rpc_latency_ms"));
        assert!(output.contains("123.45"));
    }

    #[test]
    fn test_prometheus_metrics_histogram() {
        let mut metrics = PrometheusMetrics::new();
        let buckets = vec![(10.0, 5), (50.0, 15), (100.0, 30), (500.0, 45)];
        metrics.add_histogram("rpc_latency", 5000.0, 50, &buckets, &[("endpoint", "test")]);

        let output = metrics.to_string();
        assert!(output.contains("rpc_latency_bucket"));
        assert!(output.contains("rpc_latency_sum"));
        assert!(output.contains("rpc_latency_count"));
        assert!(output.contains("le=\"+Inf\""));
    }

    #[test]
    fn test_alert_manager() {
        let mut manager = AlertManager::new();

        // Should not trigger
        let alert = manager.evaluate("consecutive_failures_high", 3.0);
        assert!(alert.is_none());

        // Should trigger
        let alert = manager.evaluate("consecutive_failures_high", 6.0);
        assert!(alert.is_some());

        let alert = alert.unwrap();
        assert_eq!(alert.name, "consecutive_failures_high");
        assert_eq!(alert.severity, AlertSeverity::Warning);
        assert_eq!(alert.value, 6.0);
        assert_eq!(alert.threshold, 5.0);

        // Should be in active alerts
        let active = manager.get_active_alerts();
        assert_eq!(active.len(), 1);

        // Clear alert
        manager.clear_alert("consecutive_failures_high");
        assert_eq!(manager.get_active_alerts().len(), 0);
    }

    #[test]
    fn test_json_metrics_serialization() {
        let metrics = JsonMetrics {
            timestamp: 1234567890,
            global: GlobalMetrics {
                total_requests: 1000,
                total_errors: 10,
                success_rate: 0.99,
                error_rate: 0.01,
                rate_limit_hits: 5,
                predictive_switches: 2,
                circuit_breaker_opens: 0,
            },
            endpoints: vec![],
            tiers: HashMap::new(),
        };

        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("total_requests"));
        assert!(json.contains("1000"));
    }
}
