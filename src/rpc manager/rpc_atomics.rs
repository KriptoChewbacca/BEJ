use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use parking_lot::RwLock;
use std::time::Instant;

/// Atomic statistics for an RPC endpoint
/// Uses lock-free atomic operations for better performance under contention
#[derive(Debug)]
pub struct AtomicEndpointStats {
    /// Total number of requests made to this endpoint
    pub total_requests: AtomicU64,
    
    /// Total number of errors encountered
    pub total_errors: AtomicU64,
    
    /// Consecutive error count (resets on success)
    pub consecutive_errors: AtomicU64,
    
    /// Last known latency in microseconds (for quick reads)
    pub last_latency_us: AtomicU64,
    
    /// Whether the endpoint is currently healthy
    pub is_healthy: AtomicBool,
    
    /// Last successful request timestamp (requires RwLock for Instant)
    pub last_success: Arc<RwLock<Option<Instant>>>,
}

impl AtomicEndpointStats {
    pub fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
            consecutive_errors: AtomicU64::new(0),
            last_latency_us: AtomicU64::new(0),
            is_healthy: AtomicBool::new(true),
            last_success: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Record a successful request
    pub fn record_success(&self, latency_us: u64) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.consecutive_errors.store(0, Ordering::Relaxed);
        self.last_latency_us.store(latency_us, Ordering::Relaxed);
        self.is_healthy.store(true, Ordering::Relaxed);
        *self.last_success.write() = Some(Instant::now());
    }
    
    /// Record a failed request
    pub fn record_failure(&self, latency_us: u64) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_errors.fetch_add(1, Ordering::Relaxed);
        self.consecutive_errors.fetch_add(1, Ordering::Relaxed);
        self.last_latency_us.store(latency_us, Ordering::Relaxed);
        
        // Mark as unhealthy after 3 consecutive errors
        if self.consecutive_errors.load(Ordering::Relaxed) >= 3 {
            self.is_healthy.store(false, Ordering::Relaxed);
        }
    }
    
    /// Get current error rate (0.0 - 1.0)
    pub fn error_rate(&self) -> f64 {
        let total = self.total_requests.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let errors = self.total_errors.load(Ordering::Relaxed);
        errors as f64 / total as f64
    }
    
    /// Get consecutive error count
    pub fn consecutive_errors(&self) -> u64 {
        self.consecutive_errors.load(Ordering::Relaxed)
    }
    
    /// Check if endpoint is healthy
    pub fn is_healthy(&self) -> bool {
        self.is_healthy.load(Ordering::Relaxed)
    }
    
    /// Get last known latency in microseconds
    pub fn last_latency_us(&self) -> u64 {
        self.last_latency_us.load(Ordering::Relaxed)
    }
    
    /// Get total request count
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }
    
    /// Get total error count
    pub fn total_errors(&self) -> u64 {
        self.total_errors.load(Ordering::Relaxed)
    }
    
    /// Reset all statistics
    pub fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.total_errors.store(0, Ordering::Relaxed);
        self.consecutive_errors.store(0, Ordering::Relaxed);
        self.last_latency_us.store(0, Ordering::Relaxed);
        self.is_healthy.store(true, Ordering::Relaxed);
        *self.last_success.write() = None;
    }
    
    /// Get time since last success (if any)
    pub fn time_since_last_success(&self) -> Option<std::time::Duration> {
        self.last_success.read().as_ref().map(|instant| instant.elapsed())
    }
}

impl Default for AtomicEndpointStats {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AtomicEndpointStats {
    fn clone(&self) -> Self {
        Self {
            total_requests: AtomicU64::new(self.total_requests.load(Ordering::Relaxed)),
            total_errors: AtomicU64::new(self.total_errors.load(Ordering::Relaxed)),
            consecutive_errors: AtomicU64::new(self.consecutive_errors.load(Ordering::Relaxed)),
            last_latency_us: AtomicU64::new(self.last_latency_us.load(Ordering::Relaxed)),
            is_healthy: AtomicBool::new(self.is_healthy.load(Ordering::Relaxed)),
            last_success: Arc::new(RwLock::new(*self.last_success.read())),
        }
    }
}

/// Global atomic metrics for the entire RPC manager
#[derive(Debug, Default)]
pub struct AtomicGlobalMetrics {
    /// Total requests across all endpoints
    pub total_requests: AtomicU64,
    
    /// Total errors across all endpoints
    pub total_errors: AtomicU64,
    
    /// Total rate limit hits
    pub rate_limit_hits: AtomicU64,
    
    /// Total predictive switches
    pub predictive_switches: AtomicU64,
    
    /// Total circuit breaker opens
    pub circuit_breaker_opens: AtomicU64,
    
    /// Total successful requests
    pub total_successes: AtomicU64,
}

impl AtomicGlobalMetrics {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn record_request(&self, success: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if success {
            self.total_successes.fetch_add(1, Ordering::Relaxed);
        } else {
            self.total_errors.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    pub fn record_rate_limit_hit(&self) {
        self.rate_limit_hits.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_predictive_switch(&self) {
        self.predictive_switches.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_circuit_breaker_open(&self) {
        self.circuit_breaker_opens.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn get_success_rate(&self) -> f64 {
        let total = self.total_requests.load(Ordering::Relaxed);
        if total == 0 {
            return 1.0;
        }
        let successes = self.total_successes.load(Ordering::Relaxed);
        successes as f64 / total as f64
    }
    
    pub fn get_error_rate(&self) -> f64 {
        1.0 - self.get_success_rate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::Arc;
    
    #[test]
    fn test_atomic_endpoint_stats() {
        let stats = AtomicEndpointStats::new();
        
        // Record success
        stats.record_success(1000);
        assert_eq!(stats.total_requests(), 1);
        assert_eq!(stats.total_errors(), 0);
        assert_eq!(stats.consecutive_errors(), 0);
        assert!(stats.is_healthy());
        
        // Record failure
        stats.record_failure(2000);
        assert_eq!(stats.total_requests(), 2);
        assert_eq!(stats.total_errors(), 1);
        assert_eq!(stats.consecutive_errors(), 1);
        assert!(stats.is_healthy()); // Still healthy after 1 error
        
        // More failures
        stats.record_failure(3000);
        stats.record_failure(4000);
        assert_eq!(stats.consecutive_errors(), 3);
        assert!(!stats.is_healthy()); // Unhealthy after 3 errors
        
        // Recovery
        stats.record_success(500);
        assert_eq!(stats.consecutive_errors(), 0);
        assert!(stats.is_healthy());
    }
    
    #[test]
    fn test_error_rate_calculation() {
        let stats = AtomicEndpointStats::new();
        
        // No requests
        assert_eq!(stats.error_rate(), 0.0);
        
        // 2 successes, 1 failure
        stats.record_success(100);
        stats.record_success(100);
        stats.record_failure(100);
        
        let error_rate = stats.error_rate();
        assert!((error_rate - 0.333).abs() < 0.01);
    }
    
    #[test]
    fn test_concurrent_access() {
        let stats = Arc::new(AtomicEndpointStats::new());
        let mut handles = vec![];
        
        // Spawn 10 threads, each recording 100 successes
        for _ in 0..10 {
            let stats_clone = stats.clone();
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    stats_clone.record_success(1000);
                }
            });
            handles.push(handle);
        }
        
        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Should have 1000 total requests
        assert_eq!(stats.total_requests(), 1000);
        assert_eq!(stats.total_errors(), 0);
    }
    
    #[test]
    fn test_global_metrics() {
        let metrics = AtomicGlobalMetrics::new();
        
        // Record some requests
        metrics.record_request(true);
        metrics.record_request(true);
        metrics.record_request(false);
        
        assert_eq!(metrics.total_requests.load(Ordering::Relaxed), 3);
        assert_eq!(metrics.total_successes.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.total_errors.load(Ordering::Relaxed), 1);
        
        let success_rate = metrics.get_success_rate();
        assert!((success_rate - 0.666).abs() < 0.01);
    }
    
    #[test]
    fn test_stats_reset() {
        let stats = AtomicEndpointStats::new();
        
        stats.record_success(1000);
        stats.record_failure(2000);
        
        assert_eq!(stats.total_requests(), 2);
        
        stats.reset();
        
        assert_eq!(stats.total_requests(), 0);
        assert_eq!(stats.total_errors(), 0);
        assert_eq!(stats.consecutive_errors(), 0);
        assert!(stats.is_healthy());
    }
}
