//! Metrics collection and export module

use prometheus::{Histogram, HistogramOpts, IntCounter, IntGauge, Opts, Registry};
use std::time::Instant;

/// Global metrics registry
pub struct Metrics {
    registry: Registry,

    // Counters
    pub trades_total: IntCounter,
    pub trades_success: IntCounter,
    pub trades_failed: IntCounter,
    pub candidates_received: IntCounter,
    pub candidates_filtered: IntCounter,

    // Nonce-related counters
    pub nonce_leases_dropped_auto: IntCounter,
    pub nonce_leases_dropped_explicit: IntCounter,
    pub nonce_sequence_errors: IntCounter,
    pub nonce_enforce_paths: IntCounter,

    // Gauges
    pub active_trades: IntGauge,
    pub nonce_pool_size: IntGauge,
    pub rpc_connections: IntGauge,
    pub nonce_active_leases: IntGauge,

    // Histograms
    pub trade_latency: Histogram,
    pub rpc_latency: Histogram,
    pub build_latency: Histogram,
    pub nonce_lease_lifetime: Histogram,
}

impl Metrics {
    /// Create new metrics instance
    pub fn new() -> anyhow::Result<Self> {
        let registry = Registry::new();

        let trades_total = IntCounter::with_opts(Opts::new(
            "trades_total",
            "Total number of trades attempted",
        ))?;

        let trades_success =
            IntCounter::with_opts(Opts::new("trades_success", "Number of successful trades"))?;

        let trades_failed =
            IntCounter::with_opts(Opts::new("trades_failed", "Number of failed trades"))?;

        let candidates_received = IntCounter::with_opts(Opts::new(
            "candidates_received",
            "Number of candidates received from sniffer",
        ))?;

        let candidates_filtered = IntCounter::with_opts(Opts::new(
            "candidates_filtered",
            "Number of candidates filtered out",
        ))?;

        // Nonce-related counters
        let nonce_leases_dropped_auto = IntCounter::with_opts(Opts::new(
            "nonce_leases_dropped_auto",
            "Number of nonce leases auto-released via Drop",
        ))?;

        let nonce_leases_dropped_explicit = IntCounter::with_opts(Opts::new(
            "nonce_leases_dropped_explicit",
            "Number of nonce leases explicitly released",
        ))?;

        let nonce_sequence_errors = IntCounter::with_opts(Opts::new(
            "nonce_sequence_errors",
            "Number of nonce sequence violations (debug/test)",
        ))?;

        let nonce_enforce_paths = IntCounter::with_opts(Opts::new(
            "nonce_enforce_paths",
            "Counter for different code paths in nonce enforcement",
        ))?;

        let active_trades = IntGauge::with_opts(Opts::new(
            "active_trades",
            "Number of trades currently in progress",
        ))?;

        let nonce_pool_size =
            IntGauge::with_opts(Opts::new("nonce_pool_size", "Current nonce pool size"))?;

        let rpc_connections = IntGauge::with_opts(Opts::new(
            "rpc_connections",
            "Number of active RPC connections",
        ))?;

        let nonce_active_leases = IntGauge::with_opts(Opts::new(
            "nonce_active_leases",
            "Number of currently held nonce leases",
        ))?;

        let trade_latency = Histogram::with_opts(
            HistogramOpts::new("trade_latency_seconds", "Trade execution latency")
                .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0]),
        )?;

        let rpc_latency = Histogram::with_opts(
            HistogramOpts::new("rpc_latency_seconds", "RPC call latency")
                .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]),
        )?;

        let build_latency = Histogram::with_opts(
            HistogramOpts::new("build_latency_seconds", "Transaction build latency")
                .buckets(vec![0.001, 0.005, 0.01, 0.02, 0.05, 0.1]),
        )?;

        let nonce_lease_lifetime = Histogram::with_opts(
            HistogramOpts::new(
                "nonce_lease_lifetime_seconds",
                "Duration nonce leases are held",
            )
            .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0]),
        )?;

        // Register all metrics
        registry.register(Box::new(trades_total.clone()))?;
        registry.register(Box::new(trades_success.clone()))?;
        registry.register(Box::new(trades_failed.clone()))?;
        registry.register(Box::new(candidates_received.clone()))?;
        registry.register(Box::new(candidates_filtered.clone()))?;
        registry.register(Box::new(nonce_leases_dropped_auto.clone()))?;
        registry.register(Box::new(nonce_leases_dropped_explicit.clone()))?;
        registry.register(Box::new(nonce_sequence_errors.clone()))?;
        registry.register(Box::new(nonce_enforce_paths.clone()))?;
        registry.register(Box::new(active_trades.clone()))?;
        registry.register(Box::new(nonce_pool_size.clone()))?;
        registry.register(Box::new(rpc_connections.clone()))?;
        registry.register(Box::new(nonce_active_leases.clone()))?;
        registry.register(Box::new(trade_latency.clone()))?;
        registry.register(Box::new(rpc_latency.clone()))?;
        registry.register(Box::new(build_latency.clone()))?;
        registry.register(Box::new(nonce_lease_lifetime.clone()))?;

        Ok(Self {
            registry,
            trades_total,
            trades_success,
            trades_failed,
            candidates_received,
            candidates_filtered,
            nonce_leases_dropped_auto,
            nonce_leases_dropped_explicit,
            nonce_sequence_errors,
            nonce_enforce_paths,
            active_trades,
            nonce_pool_size,
            rpc_connections,
            nonce_active_leases,
            trade_latency,
            rpc_latency,
            build_latency,
            nonce_lease_lifetime,
        })
    }

    /// Get the registry for exporting
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Increment a named counter (for dynamic counter names)
    /// Falls back to a no-op if the counter doesn't exist
    pub fn increment_counter(&self, name: &str) {
        // Map common counter names to their fields
        match name {
            "trades_total" | "buy_attempts_total" => self.trades_total.inc(),
            "trades_success" | "buy_success_total" => self.trades_success.inc(),
            "trades_failed" | "buy_failure_total" => self.trades_failed.inc(),
            "candidates_received" => self.candidates_received.inc(),
            "candidates_filtered" | "buy_attempts_filtered" => self.candidates_filtered.inc(),
            "nonce_leases_dropped_auto" => self.nonce_leases_dropped_auto.inc(),
            "nonce_leases_dropped_explicit" => self.nonce_leases_dropped_explicit.inc(),
            "nonce_sequence_errors" => self.nonce_sequence_errors.inc(),
            "nonce_enforce_paths" => self.nonce_enforce_paths.inc(),
            // For any other counter names that don't map to predefined counters,
            // we silently ignore (could log a warning in the future)
            _ => {
                // Use trades_total as a catch-all for unknown counters
                // This allows the code to compile but may not give accurate metrics
                tracing::debug!("Unknown counter name: {}", name);
            }
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new().expect("Failed to create metrics")
    }
}

/// Global metrics instance
pub fn metrics() -> &'static Metrics {
    static METRICS: once_cell::sync::Lazy<Metrics> =
        once_cell::sync::Lazy::new(|| Metrics::new().expect("Failed to initialize metrics"));
    &METRICS
}

/// Timer helper for measuring operation duration
pub struct Timer {
    start: Instant,
    histogram_name: Option<String>,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            histogram_name: None,
        }
    }

    /// Create a timer with a histogram name for automatic recording
    pub fn with_name(histogram_name: &str) -> Self {
        Self {
            start: Instant::now(),
            histogram_name: Some(histogram_name.to_string()),
        }
    }

    pub fn observe_duration(&self, histogram: &Histogram) {
        let duration = self.start.elapsed();
        histogram.observe(duration.as_secs_f64());
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// Finish the timer and record to the associated histogram
    pub fn finish(self) {
        if let Some(name) = self.histogram_name {
            let duration = self.start.elapsed().as_secs_f64();
            // Map histogram names to actual histograms
            match name.as_str() {
                "buy_latency_seconds" | "trade_latency_seconds" => {
                    metrics().trade_latency.observe(duration);
                }
                "rpc_latency_seconds" => {
                    metrics().rpc_latency.observe(duration);
                }
                "build_latency_seconds" => {
                    metrics().build_latency.observe(duration);
                }
                _ => {
                    tracing::debug!("Unknown histogram name: {}", name);
                }
            }
        }
    }
}
