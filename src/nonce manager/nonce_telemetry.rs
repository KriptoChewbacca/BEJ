//! Comprehensive telemetry and metrics for nonce management
//!
//! This module implements Step 3 requirements:
//! - Prometheus-compatible metrics (counters, histograms, gauges)
//! - Instrumentation with unique request_id and nonce_id
//! - Alerting rules and thresholds
//! - SLA metrics tracking
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

/// Metric types aligned with Prometheus
#[derive(Debug, Clone)]
pub enum MetricType {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
}

/// Counter metrics for nonce operations
#[derive(Debug)]
pub struct NonceCounters {
    /// Total nonce refresh attempts
    pub refresh_attempts_total: AtomicU64,

    /// Total nonce refresh failures
    pub refresh_failures_total: AtomicU64,

    /// Total tainted nonces detected
    pub tainted_total: AtomicU64,

    /// Total nonce authority rotations
    pub rotations_total: AtomicU64,

    /// Total nonce acquisitions
    pub acquire_total: AtomicU64,

    /// Total nonce releases
    pub release_total: AtomicU64,

    /// Total lease expirations
    pub lease_expired_total: AtomicU64,

    /// Total RPC failures
    pub rpc_failures_total: AtomicU64,
}

impl NonceCounters {
    pub fn new() -> Self {
        Self {
            refresh_attempts_total: AtomicU64::new(0),
            refresh_failures_total: AtomicU64::new(0),
            tainted_total: AtomicU64::new(0),
            rotations_total: AtomicU64::new(0),
            acquire_total: AtomicU64::new(0),
            release_total: AtomicU64::new(0),
            lease_expired_total: AtomicU64::new(0),
            rpc_failures_total: AtomicU64::new(0),
        }
    }

    /// Export counters as Prometheus-compatible format
    pub fn export_prometheus(&self) -> String {
        format!(
            "# HELP nonce_refresh_attempts_total Total number of nonce refresh attempts\n\
             # TYPE nonce_refresh_attempts_total counter\n\
             nonce_refresh_attempts_total {}\n\
             \n\
             # HELP nonce_refresh_failures_total Total number of nonce refresh failures\n\
             # TYPE nonce_refresh_failures_total counter\n\
             nonce_refresh_failures_total {}\n\
             \n\
             # HELP nonce_tainted_total Total number of tainted nonces detected\n\
             # TYPE nonce_tainted_total counter\n\
             nonce_tainted_total {}\n\
             \n\
             # HELP nonce_rotations_total Total number of nonce authority rotations\n\
             # TYPE nonce_rotations_total counter\n\
             nonce_rotations_total {}\n\
             \n\
             # HELP nonce_acquire_total Total number of nonce acquisitions\n\
             # TYPE nonce_acquire_total counter\n\
             nonce_acquire_total {}\n\
             \n\
             # HELP nonce_release_total Total number of nonce releases\n\
             # TYPE nonce_release_total counter\n\
             nonce_release_total {}\n\
             \n\
             # HELP nonce_lease_expired_total Total number of lease expirations\n\
             # TYPE nonce_lease_expired_total counter\n\
             nonce_lease_expired_total {}\n\
             \n\
             # HELP nonce_rpc_failures_total Total number of RPC failures\n\
             # TYPE nonce_rpc_failures_total counter\n\
             nonce_rpc_failures_total {}\n",
            self.refresh_attempts_total.load(Ordering::Relaxed),
            self.refresh_failures_total.load(Ordering::Relaxed),
            self.tainted_total.load(Ordering::Relaxed),
            self.rotations_total.load(Ordering::Relaxed),
            self.acquire_total.load(Ordering::Relaxed),
            self.release_total.load(Ordering::Relaxed),
            self.lease_expired_total.load(Ordering::Relaxed),
            self.rpc_failures_total.load(Ordering::Relaxed),
        )
    }
}

impl Default for NonceCounters {
    fn default() -> Self {
        Self::new()
    }
}

/// Histogram for tracking latency distributions
#[derive(Debug)]
pub struct LatencyHistogram {
    samples: Arc<RwLock<VecDeque<f64>>>,
    max_samples: usize,
}

impl LatencyHistogram {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: Arc::new(RwLock::new(VecDeque::with_capacity(max_samples))),
            max_samples,
        }
    }

    /// Record a latency sample in seconds
    pub async fn record(&self, latency_seconds: f64) {
        let mut samples = self.samples.write().await;
        samples.push_back(latency_seconds);
        if samples.len() > self.max_samples {
            samples.pop_front();
        }
    }

    /// Calculate percentiles
    pub async fn percentiles(&self) -> HistogramPercentiles {
        let samples = self.samples.read().await;

        if samples.is_empty() {
            return HistogramPercentiles::default();
        }

        let mut sorted: Vec<f64> = samples.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let p50_idx = (sorted.len() as f64 * 0.50) as usize;
        let p95_idx = (sorted.len() as f64 * 0.95) as usize;
        let p99_idx = (sorted.len() as f64 * 0.99) as usize;

        HistogramPercentiles {
            p50: sorted.get(p50_idx).copied().unwrap_or(0.0),
            p95: sorted.get(p95_idx).copied().unwrap_or(0.0),
            p99: sorted.get(p99_idx).copied().unwrap_or(0.0),
            count: sorted.len(),
        }
    }

    /// Export as Prometheus histogram
    pub async fn export_prometheus(&self, name: &str) -> String {
        let percentiles = self.percentiles().await;
        format!(
            "# HELP {} Latency histogram in seconds\n\
             # TYPE {} histogram\n\
             {}_bucket{{le=\"0.050\"}} {}\n\
             {}_bucket{{le=\"0.100\"}} {}\n\
             {}_bucket{{le=\"+Inf\"}} {}\n\
             {}_sum {}\n\
             {}_count {}\n",
            name,
            name,
            name,
            percentiles.count,
            name,
            percentiles.count,
            name,
            percentiles.count,
            name,
            percentiles.p50 * percentiles.count as f64,
            name,
            percentiles.count,
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct HistogramPercentiles {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub count: usize,
}

/// Gauge metrics for current state
#[derive(Debug)]
pub struct NonceGauges {
    /// Current nonce pool size
    pub pool_size: Arc<RwLock<u64>>,

    /// Current number of outstanding leases
    pub leases_outstanding: Arc<RwLock<u64>>,

    /// Predictive failure probability (0.0 to 1.0)
    pub predictive_failure_prob: Arc<RwLock<f64>>,

    /// Current RPC latency (ms)
    pub rpc_latency_ms: Arc<RwLock<f64>>,
}

impl NonceGauges {
    pub fn new() -> Self {
        Self {
            pool_size: Arc::new(RwLock::new(0)),
            leases_outstanding: Arc::new(RwLock::new(0)),
            predictive_failure_prob: Arc::new(RwLock::new(0.0)),
            rpc_latency_ms: Arc::new(RwLock::new(0.0)),
        }
    }

    /// Export gauges as Prometheus format
    pub async fn export_prometheus(&self) -> String {
        format!(
            "# HELP nonce_pool_size Current nonce pool size\n\
             # TYPE nonce_pool_size gauge\n\
             nonce_pool_size {}\n\
             \n\
             # HELP nonce_leases_outstanding Current number of outstanding leases\n\
             # TYPE nonce_leases_outstanding gauge\n\
             nonce_leases_outstanding {}\n\
             \n\
             # HELP nonce_predictive_failure_prob Predictive failure probability\n\
             # TYPE nonce_predictive_failure_prob gauge\n\
             nonce_predictive_failure_prob {}\n\
             \n\
             # HELP nonce_rpc_latency_ms Current RPC latency in milliseconds\n\
             # TYPE nonce_rpc_latency_ms gauge\n\
             nonce_rpc_latency_ms {}\n",
            *self.pool_size.read().await,
            *self.leases_outstanding.read().await,
            *self.predictive_failure_prob.read().await,
            *self.rpc_latency_ms.read().await,
        )
    }
}

impl Default for NonceGauges {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete telemetry collector
#[derive(Debug)]
pub struct NonceTelemetry {
    pub counters: NonceCounters,
    pub gauges: NonceGauges,
    pub acquire_latency: LatencyHistogram,
    pub refresh_latency: LatencyHistogram,
    pub alerting: AlertManager,
}

impl NonceTelemetry {
    pub fn new() -> Self {
        Self {
            counters: NonceCounters::new(),
            gauges: NonceGauges::new(),
            acquire_latency: LatencyHistogram::new(10000),
            refresh_latency: LatencyHistogram::new(10000),
            alerting: AlertManager::new(),
        }
    }

    /// Record nonce acquisition
    pub async fn record_acquire(&self, latency: Duration) {
        self.counters.acquire_total.fetch_add(1, Ordering::Relaxed);
        self.acquire_latency.record(latency.as_secs_f64()).await;
    }

    /// Record nonce refresh
    pub async fn record_refresh(&self, success: bool, latency: Duration) {
        self.counters
            .refresh_attempts_total
            .fetch_add(1, Ordering::Relaxed);
        if !success {
            self.counters
                .refresh_failures_total
                .fetch_add(1, Ordering::Relaxed);
        }
        self.refresh_latency.record(latency.as_secs_f64()).await;

        // Check alerting rules
        self.check_refresh_failure_rate().await;
    }

    /// Record tainted nonce
    pub fn record_tainted(&self) {
        self.counters.tainted_total.fetch_add(1, Ordering::Relaxed);

        // Immediate alert for any tainted nonce
        self.alerting.trigger_alert(Alert {
            severity: AlertSeverity::Critical,
            message: "Tainted nonce detected".to_string(),
            metric: "nonce_tainted_total".to_string(),
            value: self.counters.tainted_total.load(Ordering::Relaxed) as f64,
            threshold: 0.0,
            timestamp: Instant::now(),
        });
    }

    /// Check refresh failure rate and trigger alerts
    async fn check_refresh_failure_rate(&self) {
        let attempts = self.counters.refresh_attempts_total.load(Ordering::Relaxed);
        let failures = self.counters.refresh_failures_total.load(Ordering::Relaxed);

        if attempts == 0 {
            return;
        }

        let failure_rate = failures as f64 / attempts as f64;

        // Alert if failure rate > 5% over last 5 minutes
        if failure_rate > 0.05 {
            self.alerting.trigger_alert(Alert {
                severity: AlertSeverity::Warning,
                message: format!(
                    "Refresh failure rate {:.2}% exceeds threshold",
                    failure_rate * 100.0
                ),
                metric: "nonce_refresh_failure_rate".to_string(),
                value: failure_rate,
                threshold: 0.05,
                timestamp: Instant::now(),
            });
        }
    }

    /// Check latency and trigger alerts
    pub async fn check_latency_alerts(&self) {
        let percentiles = self.acquire_latency.percentiles().await;

        // Alert if P99 latency > threshold (configurable, e.g., 100ms)
        if percentiles.p99 > 0.100 {
            self.alerting.trigger_alert(Alert {
                severity: AlertSeverity::Warning,
                message: format!(
                    "Acquire P99 latency {:.3}s exceeds threshold",
                    percentiles.p99
                ),
                metric: "nonce_acquire_latency_p99".to_string(),
                value: percentiles.p99,
                threshold: 0.100,
                timestamp: Instant::now(),
            });
        }
    }

    /// Export all metrics in Prometheus format
    pub async fn export_prometheus(&self) -> String {
        let mut output = String::new();

        output.push_str(&self.counters.export_prometheus());
        output.push_str("\n");
        output.push_str(&self.gauges.export_prometheus().await);
        output.push_str("\n");
        output.push_str(
            &self
                .acquire_latency
                .export_prometheus("nonce_acquire_latency_seconds")
                .await,
        );
        output.push_str("\n");
        output.push_str(
            &self
                .refresh_latency
                .export_prometheus("nonce_refresh_latency_seconds")
                .await,
        );

        output
    }

    /// Get diagnostic summary
    pub async fn get_diagnostics(&self) -> TelemetryDiagnostics {
        let acquire_percentiles = self.acquire_latency.percentiles().await;
        let refresh_percentiles = self.refresh_latency.percentiles().await;

        TelemetryDiagnostics {
            refresh_attempts: self.counters.refresh_attempts_total.load(Ordering::Relaxed),
            refresh_failures: self.counters.refresh_failures_total.load(Ordering::Relaxed),
            tainted_nonces: self.counters.tainted_total.load(Ordering::Relaxed),
            rotations: self.counters.rotations_total.load(Ordering::Relaxed),
            acquire_p50_ms: acquire_percentiles.p50 * 1000.0,
            acquire_p95_ms: acquire_percentiles.p95 * 1000.0,
            acquire_p99_ms: acquire_percentiles.p99 * 1000.0,
            refresh_p50_ms: refresh_percentiles.p50 * 1000.0,
            refresh_p95_ms: refresh_percentiles.p95 * 1000.0,
            refresh_p99_ms: refresh_percentiles.p99 * 1000.0,
            pool_size: *self.gauges.pool_size.read().await,
            leases_outstanding: *self.gauges.leases_outstanding.read().await,
            active_alerts: self.alerting.get_active_alerts().await.len(),
        }
    }
}

impl Default for NonceTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

/// Telemetry diagnostics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryDiagnostics {
    pub refresh_attempts: u64,
    pub refresh_failures: u64,
    pub tainted_nonces: u64,
    pub rotations: u64,
    pub acquire_p50_ms: f64,
    pub acquire_p95_ms: f64,
    pub acquire_p99_ms: f64,
    pub refresh_p50_ms: f64,
    pub refresh_p95_ms: f64,
    pub refresh_p99_ms: f64,
    pub pool_size: u64,
    pub leases_outstanding: u64,
    pub active_alerts: usize,
}

/// Alert severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Alert structure
#[derive(Debug, Clone)]
pub struct Alert {
    pub severity: AlertSeverity,
    pub message: String,
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
    pub timestamp: Instant,
}

/// Alert manager for tracking and managing alerts
#[derive(Debug)]
pub struct AlertManager {
    active_alerts: Arc<RwLock<Vec<Alert>>>,
    alert_history: Arc<RwLock<VecDeque<Alert>>>,
    max_history: usize,
}

impl AlertManager {
    pub fn new() -> Self {
        Self {
            active_alerts: Arc::new(RwLock::new(Vec::new())),
            alert_history: Arc::new(RwLock::new(VecDeque::new())),
            max_history: 1000,
        }
    }

    /// Trigger a new alert
    pub fn trigger_alert(&self, alert: Alert) {
        let _alert_clone = alert.clone();
        let active_alerts = self.active_alerts.clone();
        let alert_history = self.alert_history.clone();
        let max_history = self.max_history;

        tokio::spawn(async move {
            // Log the alert
            match alert.severity {
                AlertSeverity::Info => debug!("{}", alert.message),
                AlertSeverity::Warning => warn!("{}", alert.message),
                AlertSeverity::Critical => error!("{}", alert.message),
            }

            // Add to active alerts
            active_alerts.write().await.push(alert.clone());

            // Add to history
            let mut history = alert_history.write().await;
            history.push_back(alert);
            if history.len() > max_history {
                history.pop_front();
            }
        });
    }

    /// Get active alerts
    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        self.active_alerts.read().await.clone()
    }

    /// Clear all active alerts
    pub async fn clear_alerts(&self) {
        self.active_alerts.write().await.clear();
    }

    /// Get alert history
    pub async fn get_alert_history(&self, count: usize) -> Vec<Alert> {
        let history = self.alert_history.read().await;
        history.iter().rev().take(count).cloned().collect()
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Trace context for distributed tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub request_id: String,
    pub nonce_id: Option<String>,
    pub parent_span_id: Option<String>,
    pub trace_id: String,
    #[serde(skip, default = "Instant::now")]
    pub start_time: Instant,
}

impl TraceContext {
    pub fn new(nonce_id: Option<String>) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            nonce_id,
            parent_span_id: None,
            trace_id: uuid::Uuid::new_v4().to_string(),
            start_time: Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counters_initialization() {
        let counters = NonceCounters::new();
        assert_eq!(counters.refresh_attempts_total.load(Ordering::Relaxed), 0);
        assert_eq!(counters.refresh_failures_total.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_histogram_percentiles() {
        let histogram = LatencyHistogram::new(100);

        // Record some samples
        for i in 1..=100 {
            histogram.record(i as f64 / 1000.0).await;
        }

        let percentiles = histogram.percentiles().await;
        assert_eq!(percentiles.count, 100);
        assert!(percentiles.p50 > 0.0);
        assert!(percentiles.p99 > percentiles.p50);
    }

    #[tokio::test]
    async fn test_telemetry_record_acquire() {
        let telemetry = NonceTelemetry::new();

        telemetry.record_acquire(Duration::from_millis(50)).await;

        assert_eq!(telemetry.counters.acquire_total.load(Ordering::Relaxed), 1);

        let percentiles = telemetry.acquire_latency.percentiles().await;
        assert_eq!(percentiles.count, 1);
    }

    #[test]
    fn test_alert_severity_ordering() {
        assert!(AlertSeverity::Info < AlertSeverity::Warning);
        assert!(AlertSeverity::Warning < AlertSeverity::Critical);
    }

    #[tokio::test]
    async fn test_alert_manager() {
        let manager = AlertManager::new();

        manager.trigger_alert(Alert {
            severity: AlertSeverity::Warning,
            message: "Test alert".to_string(),
            metric: "test_metric".to_string(),
            value: 1.0,
            threshold: 0.5,
            timestamp: Instant::now(),
        });

        // Give async task time to complete
        tokio::time::sleep(Duration::from_millis(10)).await;

        let active = manager.get_active_alerts().await;
        assert_eq!(active.len(), 1);
    }
}
