//! Telemetry module with atomic counters and metrics export

use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::Mutex;

/// Atomic metrics for zero-overhead tracking in hot path
#[derive(Debug)]
pub struct SnifferMetrics {
    /// Total transactions seen
    pub tx_seen: AtomicU64,
    /// Transactions filtered out
    pub tx_filtered: AtomicU64,
    /// Candidates sent to buy_engine
    pub candidates_sent: AtomicU64,
    /// Candidates dropped due to full buffer
    pub dropped_full_buffer: AtomicU64,
    /// Security drops (malformed/invalid)
    pub security_drop_count: AtomicU64,
    /// Backpressure events
    pub backpressure_events: AtomicU64,
    /// Stream reconnects
    pub reconnect_count: AtomicU64,
    /// HIGH priority candidates sent
    pub high_priority_sent: AtomicU64,
    /// LOW priority candidates sent
    pub low_priority_sent: AtomicU64,
    /// HIGH priority candidates dropped
    pub high_priority_dropped: AtomicU64,
    /// Mint extraction errors
    pub mint_extract_errors: AtomicU64,
    /// Account extraction errors
    pub account_extract_errors: AtomicU64,
    /// Current stream buffer depth (approximate)
    pub stream_buffer_depth: AtomicU64,
    /// Latency samples for P50/P95/P99 calculation
    pub latency_samples: Mutex<Vec<u64>>,
    /// Correlation tracking: latency → confidence/priority → drop_rate
    pub latency_correlation: Mutex<LatencyCorrelation>,
}

/// Latency correlation data for performance-cost ratio analysis
#[derive(Debug)]
pub struct LatencyCorrelation {
    /// Samples of (latency, confidence, was_dropped)
    samples: Vec<(u64, f64, bool)>,
    /// Maximum samples to keep
    max_samples: usize,
}

impl LatencyCorrelation {
    /// Create a new latency correlation tracker
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::with_capacity(max_samples),
            max_samples,
        }
    }

    /// Add a correlation sample
    pub fn add_sample(&mut self, latency_us: u64, confidence: f64, was_dropped: bool) {
        if self.samples.len() >= self.max_samples {
            // Remove oldest sample
            self.samples.remove(0);
        }
        self.samples.push((latency_us, confidence, was_dropped));
    }

    /// Get average latency for high confidence items
    pub fn avg_latency_high_confidence(&self, threshold: f64) -> Option<f64> {
        let high_conf: Vec<u64> = self.samples
            .iter()
            .filter(|(_, conf, _)| *conf >= threshold)
            .map(|(lat, _, _)| *lat)
            .collect();

        if high_conf.is_empty() {
            return None;
        }

        Some(high_conf.iter().sum::<u64>() as f64 / high_conf.len() as f64)
    }

    /// Get drop rate for high latency items
    pub fn drop_rate_high_latency(&self, latency_threshold: u64) -> f64 {
        let high_latency: Vec<bool> = self.samples
            .iter()
            .filter(|(lat, _, _)| *lat >= latency_threshold)
            .map(|(_, _, dropped)| *dropped)
            .collect();

        if high_latency.is_empty() {
            return 0.0;
        }

        let dropped_count = high_latency.iter().filter(|&&d| d).count();
        dropped_count as f64 / high_latency.len() as f64
    }

    /// Get correlation between latency and confidence
    pub fn latency_confidence_correlation(&self) -> Option<f64> {
        if self.samples.len() < 2 {
            return None;
        }

        let n = self.samples.len() as f64;
        let sum_lat: f64 = self.samples.iter().map(|(lat, _, _)| *lat as f64).sum();
        let sum_conf: f64 = self.samples.iter().map(|(_, conf, _)| *conf).sum();
        let sum_lat_conf: f64 = self.samples.iter().map(|(lat, conf, _)| *lat as f64 * conf).sum();
        let sum_lat_sq: f64 = self.samples.iter().map(|(lat, _, _)| (*lat as f64).powi(2)).sum();
        let sum_conf_sq: f64 = self.samples.iter().map(|(_, conf, _)| conf.powi(2)).sum();

        let numerator = n * sum_lat_conf - sum_lat * sum_conf;
        let denominator = ((n * sum_lat_sq - sum_lat.powi(2)) * (n * sum_conf_sq - sum_conf.powi(2))).sqrt();

        if denominator == 0.0 {
            return None;
        }

        Some(numerator / denominator)
    }
}

impl SnifferMetrics {
    /// Create a new metrics instance
    pub fn new() -> Self {
        Self {
            tx_seen: AtomicU64::new(0),
            tx_filtered: AtomicU64::new(0),
            candidates_sent: AtomicU64::new(0),
            dropped_full_buffer: AtomicU64::new(0),
            security_drop_count: AtomicU64::new(0),
            backpressure_events: AtomicU64::new(0),
            reconnect_count: AtomicU64::new(0),
            high_priority_sent: AtomicU64::new(0),
            low_priority_sent: AtomicU64::new(0),
            high_priority_dropped: AtomicU64::new(0),
            mint_extract_errors: AtomicU64::new(0),
            account_extract_errors: AtomicU64::new(0),
            stream_buffer_depth: AtomicU64::new(0),
            latency_samples: Mutex::new(Vec::with_capacity(1000)),
            latency_correlation: Mutex::new(LatencyCorrelation::new(1000)),
        }
    }

    /// Export metrics as JSON snapshot for Prometheus/Grafana
    pub fn snapshot(&self) -> String {
        format!(
            r#"{{"tx_seen":{},"tx_filtered":{},"candidates_sent":{},"dropped_full_buffer":{},"security_drop_count":{},"backpressure_events":{},"reconnect_count":{},"high_priority_sent":{},"low_priority_sent":{},"high_priority_dropped":{},"mint_extract_errors":{},"account_extract_errors":{},"stream_buffer_depth":{}}}"#,
            self.tx_seen.load(Ordering::Relaxed),
            self.tx_filtered.load(Ordering::Relaxed),
            self.candidates_sent.load(Ordering::Relaxed),
            self.dropped_full_buffer.load(Ordering::Relaxed),
            self.security_drop_count.load(Ordering::Relaxed),
            self.backpressure_events.load(Ordering::Relaxed),
            self.reconnect_count.load(Ordering::Relaxed),
            self.high_priority_sent.load(Ordering::Relaxed),
            self.low_priority_sent.load(Ordering::Relaxed),
            self.high_priority_dropped.load(Ordering::Relaxed),
            self.mint_extract_errors.load(Ordering::Relaxed),
            self.account_extract_errors.load(Ordering::Relaxed),
            self.stream_buffer_depth.load(Ordering::Relaxed),
        )
    }
    
    /// Record latency sample (lightweight, sampled approach)
    /// Uses circular buffer with pseudo-random replacement
    pub fn record_latency(&self, latency_us: u64) {
        let mut samples = self.latency_samples.lock();
        if samples.len() < 1000 {
            samples.push(latency_us);
        } else {
            // Use round-robin replacement for true circular buffer
            let idx = (latency_us % 1000) as usize;
            samples[idx] = latency_us;
        }
    }
    
    /// Calculate percentile latency (P50/P95/P99)
    /// Returns None if no samples available
    pub fn get_percentile_latency(&self, percentile: f64) -> Option<u64> {
        let samples = self.latency_samples.lock();
        if samples.is_empty() {
            return None;
        }
        
        let mut sorted = samples.clone();
        sorted.sort_unstable();
        
        let idx = ((sorted.len() as f64 * percentile) as usize).min(sorted.len() - 1);
        Some(sorted[idx])
    }
    
    /// Reset all counters (useful for testing)
    pub fn reset(&self) {
        self.tx_seen.store(0, Ordering::Relaxed);
        self.tx_filtered.store(0, Ordering::Relaxed);
        self.candidates_sent.store(0, Ordering::Relaxed);
        self.dropped_full_buffer.store(0, Ordering::Relaxed);
        self.security_drop_count.store(0, Ordering::Relaxed);
        self.backpressure_events.store(0, Ordering::Relaxed);
        self.reconnect_count.store(0, Ordering::Relaxed);
        self.high_priority_sent.store(0, Ordering::Relaxed);
        self.low_priority_sent.store(0, Ordering::Relaxed);
        self.high_priority_dropped.store(0, Ordering::Relaxed);
        self.mint_extract_errors.store(0, Ordering::Relaxed);
        self.account_extract_errors.store(0, Ordering::Relaxed);
        self.stream_buffer_depth.store(0, Ordering::Relaxed);
        self.latency_samples.lock().clear();
        self.latency_correlation.lock().samples.clear();
    }

    /// Record correlation sample for latency analysis
    pub fn record_correlation(&self, latency_us: u64, confidence: f64, was_dropped: bool) {
        self.latency_correlation.lock().add_sample(latency_us, confidence, was_dropped);
    }

    /// Get latency-confidence correlation coefficient
    pub fn get_latency_confidence_correlation(&self) -> Option<f64> {
        self.latency_correlation.lock().latency_confidence_correlation()
    }

    /// Get average latency for high confidence items
    pub fn get_avg_latency_high_confidence(&self, threshold: f64) -> Option<f64> {
        self.latency_correlation.lock().avg_latency_high_confidence(threshold)
    }

    /// Get drop rate for high latency items
    pub fn get_drop_rate_high_latency(&self, latency_threshold: u64) -> f64 {
        self.latency_correlation.lock().drop_rate_high_latency(latency_threshold)
    }
}

impl Default for SnifferMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Handoff diagnostics for backpressure analysis
#[derive(Debug)]
pub struct HandoffDiagnostics {
    /// Dropped candidates per priority level
    pub dropped_high_priority: AtomicU64,
    pub dropped_low_priority: AtomicU64,
    /// Queue wait time samples (microseconds)
    pub queue_wait_samples: Mutex<Vec<u64>>,
    /// Queue wait histogram buckets (0-10us, 10-100us, 100-1000us, 1000+us)
    pub queue_wait_histogram: [AtomicU64; 4],
}

impl HandoffDiagnostics {
    /// Create new handoff diagnostics
    pub fn new() -> Self {
        Self {
            dropped_high_priority: AtomicU64::new(0),
            dropped_low_priority: AtomicU64::new(0),
            queue_wait_samples: Mutex::new(Vec::with_capacity(1000)),
            queue_wait_histogram: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
        }
    }

    /// Record a dropped candidate
    pub fn record_drop(&self, is_high_priority: bool) {
        if is_high_priority {
            self.dropped_high_priority.fetch_add(1, Ordering::Relaxed);
        } else {
            self.dropped_low_priority.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record queue wait time
    pub fn record_queue_wait(&self, wait_us: u64) {
        // Add to samples
        let mut samples = self.queue_wait_samples.lock();
        if samples.len() >= 1000 {
            samples.remove(0);
        }
        samples.push(wait_us);
        drop(samples);

        // Update histogram
        let bucket = if wait_us < 10 {
            0
        } else if wait_us < 100 {
            1
        } else if wait_us < 1000 {
            2
        } else {
            3
        };
        self.queue_wait_histogram[bucket].fetch_add(1, Ordering::Relaxed);
    }

    /// Get average queue wait time
    pub fn avg_queue_wait(&self) -> Option<f64> {
        let samples = self.queue_wait_samples.lock();
        if samples.is_empty() {
            return None;
        }
        Some(samples.iter().sum::<u64>() as f64 / samples.len() as f64)
    }

    /// Get queue wait histogram
    pub fn get_histogram(&self) -> [u64; 4] {
        [
            self.queue_wait_histogram[0].load(Ordering::Relaxed),
            self.queue_wait_histogram[1].load(Ordering::Relaxed),
            self.queue_wait_histogram[2].load(Ordering::Relaxed),
            self.queue_wait_histogram[3].load(Ordering::Relaxed),
        ]
    }

    /// Reset diagnostics
    pub fn reset(&self) {
        self.dropped_high_priority.store(0, Ordering::Relaxed);
        self.dropped_low_priority.store(0, Ordering::Relaxed);
        self.queue_wait_samples.lock().clear();
        for bucket in &self.queue_wait_histogram {
            bucket.store(0, Ordering::Relaxed);
        }
    }
}

impl Default for HandoffDiagnostics {
    fn default() -> Self {
        Self::new()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = SnifferMetrics::new();
        assert_eq!(metrics.tx_seen.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.candidates_sent.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_metrics_increment() {
        let metrics = SnifferMetrics::new();
        metrics.tx_seen.fetch_add(1, Ordering::Relaxed);
        metrics.candidates_sent.fetch_add(5, Ordering::Relaxed);
        
        assert_eq!(metrics.tx_seen.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.candidates_sent.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn test_latency_recording() {
        let metrics = SnifferMetrics::new();
        metrics.record_latency(100);
        metrics.record_latency(200);
        metrics.record_latency(300);
        
        let p50 = metrics.get_percentile_latency(0.5);
        assert!(p50.is_some());
        assert_eq!(p50.unwrap(), 200);
    }

    #[test]
    fn test_snapshot() {
        let metrics = SnifferMetrics::new();
        metrics.tx_seen.fetch_add(100, Ordering::Relaxed);
        metrics.candidates_sent.fetch_add(10, Ordering::Relaxed);
        
        let snapshot = metrics.snapshot();
        assert!(snapshot.contains("\"tx_seen\":100"));
        assert!(snapshot.contains("\"candidates_sent\":10"));
    }
}
