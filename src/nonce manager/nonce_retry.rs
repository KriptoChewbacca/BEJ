use super::nonce_errors::{NonceError, NonceResult, UniverseErrorType, ErrorClassification};
use rand::Rng;
use std::future::Future;
use std::time::{Duration, Instant};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::time::sleep;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Retry configuration with jitter
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of attempts (including initial attempt)
    pub max_attempts: u32,
    /// Base backoff delay in milliseconds
    pub base_backoff_ms: u64,
    /// Maximum backoff delay in milliseconds
    pub max_backoff_ms: u64,
    /// Jitter factor (0.0 to 1.0) - adds randomness to backoff
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_backoff_ms: 100,
            max_backoff_ms: 5000,
            jitter_factor: 0.2,
        }
    }
}

impl RetryConfig {
    /// Create an aggressive retry config (more attempts, shorter delays)
    pub fn aggressive() -> Self {
        Self {
            max_attempts: 5,
            base_backoff_ms: 50,
            max_backoff_ms: 2000,
            jitter_factor: 0.3,
        }
    }
    
    /// Create a conservative retry config (fewer attempts, longer delays)
    pub fn conservative() -> Self {
        Self {
            max_attempts: 2,
            base_backoff_ms: 200,
            max_backoff_ms: 10000,
            jitter_factor: 0.1,
        }
    }
    
    /// Calculate backoff delay for a given attempt (0-indexed)
    fn calculate_backoff(&self, attempt: u32) -> Duration {
        // Exponential backoff: base * 2^attempt
        let exp_backoff = (self.base_backoff_ms as f64) * 2_f64.powi(attempt as i32);
        let capped_backoff = exp_backoff.min(self.max_backoff_ms as f64);
        
        // Add jitter to prevent thundering herd
        let mut rng = rand::thread_rng();
        let jitter_range = capped_backoff * self.jitter_factor;
        let jitter = rng.gen_range(-jitter_range..=jitter_range);
        let final_backoff = (capped_backoff + jitter).max(0.0);
        
        Duration::from_millis(final_backoff as u64)
    }
}

/// Retry metrics for observability
#[derive(Debug, Clone)]
pub struct RetryMetrics {
    pub total_attempts: u32,
    pub successful: bool,
    pub final_error: Option<NonceError>,
    pub total_duration_ms: u64,
}

/// Central retry helper with jitter and error classification
///
/// This function retries an async operation according to the provided configuration.
/// It distinguishes between transient errors (which trigger retries) and permanent
/// errors (which immediately fail).
pub async fn retry_with_backoff<F, Fut, T>(
    operation_name: &str,
    config: &RetryConfig,
    mut operation: F,
) -> NonceResult<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = NonceResult<T>>,
{
    let start_time = std::time::Instant::now();
    let mut last_error = None;
    
    for attempt in 0..config.max_attempts {
        // Log attempt
        if attempt > 0 {
            debug!(
                operation = operation_name,
                attempt = attempt + 1,
                max_attempts = config.max_attempts,
                "Retrying operation"
            );
        }
        
        // Execute operation
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    debug!(
                        operation = operation_name,
                        attempts = attempt + 1,
                        duration_ms = start_time.elapsed().as_millis() as u64,
                        "Operation succeeded after retry"
                    );
                }
                return Ok(result);
            }
            Err(err) => {
                // Check if error is transient
                if !err.is_transient() {
                    warn!(
                        operation = operation_name,
                        error = %err,
                        "Permanent error, not retrying"
                    );
                    return Err(err);
                }
                
                last_error = Some(err.clone());
                
                // If this is not the last attempt, apply backoff
                if attempt + 1 < config.max_attempts {
                    let backoff = config.calculate_backoff(attempt);
                    debug!(
                        operation = operation_name,
                        attempt = attempt + 1,
                        backoff_ms = backoff.as_millis() as u64,
                        error = %err,
                        "Transient error, backing off before retry"
                    );
                    sleep(backoff).await;
                } else {
                    warn!(
                        operation = operation_name,
                        attempts = attempt + 1,
                        error = %err,
                        "All retry attempts exhausted"
                    );
                }
            }
        }
    }
    
    // All attempts failed
    Err(last_error.unwrap_or_else(|| {
        NonceError::Internal("Retry exhausted without error".to_string())
    }))
}

/// Retry helper with metrics collection
pub async fn retry_with_metrics<F, Fut, T>(
    operation_name: &str,
    config: &RetryConfig,
    operation: F,
) -> (NonceResult<T>, RetryMetrics)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = NonceResult<T>>,
{
    let start_time = std::time::Instant::now();
    let result = retry_with_backoff(operation_name, config, operation).await;
    let duration_ms = start_time.elapsed().as_millis() as u64;
    
    let metrics = RetryMetrics {
        total_attempts: config.max_attempts,
        successful: result.is_ok(),
        final_error: result.as_ref().err().cloned(),
        total_duration_ms: duration_ms,
    };
    
    (result, metrics)
}

// ============================================================================
// CIRCUIT BREAKER PATTERN
// ============================================================================

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker for rate limiting and failure protection
#[derive(Debug)]
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_count: AtomicU64,
    success_count: AtomicU64,
    failure_threshold: u64,
    success_threshold: u64,
    last_state_change: Arc<RwLock<Instant>>,
    timeout: Duration,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(failure_threshold: u64, success_threshold: u64, timeout: Duration) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            failure_threshold,
            success_threshold,
            last_state_change: Arc::new(RwLock::new(Instant::now())),
            timeout,
        }
    }
    
    /// Create with default thresholds: 3 failures, 2 successes, 30s timeout
    pub fn default_thresholds() -> Self {
        Self::new(3, 2, Duration::from_secs(30))
    }
    
    /// Check if operation can proceed
    pub async fn can_execute(&self) -> bool {
        let state = *self.state.read().await;
        match state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true,
            CircuitState::Open => {
                // Check if timeout has elapsed
                let elapsed = self.last_state_change.read().await.elapsed();
                if elapsed >= self.timeout {
                    self.transition_to_half_open().await;
                    true
                } else {
                    false
                }
            }
        }
    }
    
    /// Record a successful operation
    pub async fn record_success(&self) {
        let state = *self.state.read().await;
        match state {
            CircuitState::HalfOpen => {
                let count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                if count >= self.success_threshold {
                    self.transition_to_closed().await;
                }
            }
            CircuitState::Closed => {
                self.failure_count.store(0, Ordering::Relaxed);
            }
            _ => {}
        }
    }
    
    /// Record a failed operation
    pub async fn record_failure(&self) {
        let state = *self.state.read().await;
        let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        
        if state == CircuitState::Closed && count >= self.failure_threshold {
            self.transition_to_open().await;
        } else if state == CircuitState::HalfOpen {
            // Immediately open on any failure in half-open state
            self.transition_to_open().await;
        }
    }
    
    /// Get current state
    pub async fn get_state(&self) -> CircuitState {
        *self.state.read().await
    }
    
    async fn transition_to_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Open;
        *self.last_state_change.write().await = Instant::now();
        debug!("Circuit breaker transitioned to OPEN");
    }
    
    async fn transition_to_half_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::HalfOpen;
        self.success_count.store(0, Ordering::Relaxed);
        self.failure_count.store(0, Ordering::Relaxed);
        *self.last_state_change.write().await = Instant::now();
        debug!("Circuit breaker transitioned to HALF_OPEN");
    }
    
    async fn transition_to_closed(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        *self.last_state_change.write().await = Instant::now();
        debug!("Circuit breaker transitioned to CLOSED");
    }
}

/// Global circuit breaker tracker for system-wide coordination
#[derive(Debug)]
pub struct GlobalCircuitBreaker {
    endpoint_breakers: Arc<RwLock<std::collections::HashMap<String, Arc<CircuitBreaker>>>>,
    tainted_endpoints: Arc<RwLock<std::collections::HashSet<String>>>,
}

impl GlobalCircuitBreaker {
    pub fn new() -> Self {
        Self {
            endpoint_breakers: Arc::new(RwLock::new(std::collections::HashMap::new())),
            tainted_endpoints: Arc::new(RwLock::new(std::collections::HashSet::new())),
        }
    }
    
    /// Get or create circuit breaker for endpoint
    pub async fn get_breaker(&self, endpoint: &str) -> Arc<CircuitBreaker> {
        let mut breakers = self.endpoint_breakers.write().await;
        breakers
            .entry(endpoint.to_string())
            .or_insert_with(|| Arc::new(CircuitBreaker::default_thresholds()))
            .clone()
    }
    
    /// Check if global threshold is exceeded (>50% of endpoints open)
    pub async fn should_trip_global(&self) -> bool {
        let breakers = self.endpoint_breakers.read().await;
        if breakers.is_empty() {
            return false;
        }
        
        let mut open_count = 0;
        for breaker in breakers.values() {
            if *breaker.state.read().await == CircuitState::Open {
                open_count += 1;
            }
        }
        
        let open_percentage = (open_count as f64 / breakers.len() as f64) * 100.0;
        open_percentage > 50.0
    }
    
    /// Mark endpoint as tainted
    pub async fn mark_tainted(&self, endpoint: &str) {
        self.tainted_endpoints.write().await.insert(endpoint.to_string());
        warn!("Endpoint marked as tainted: {}", endpoint);
    }
    
    /// Check if endpoint is tainted
    pub async fn is_tainted(&self, endpoint: &str) -> bool {
        self.tainted_endpoints.read().await.contains(endpoint)
    }
}

// ============================================================================
// ERROR CLASSIFICATION WITH ML-BASED CLUSTERING
// ============================================================================

/// Error history for clustering
#[derive(Debug, Clone)]
struct ErrorHistoryEntry {
    error_message: String,
    timestamp: Instant,
    cluster_id: Option<u8>,
}

/// Error classifier with simple k-means clustering
pub struct ErrorClassifier {
    history: Arc<RwLock<Vec<ErrorHistoryEntry>>>,
    max_history_size: usize,
    cluster_count: u8,
}

impl ErrorClassifier {
    pub fn new(max_history_size: usize, cluster_count: u8) -> Self {
        Self {
            history: Arc::new(RwLock::new(Vec::with_capacity(max_history_size))),
            max_history_size,
            cluster_count,
        }
    }
    
    /// Classify error with confidence score
    pub async fn classify_error(&self, error: &NonceError) -> ErrorClassification {
        let error_str = error.to_string();
        
        // Pattern-based classification with regex
        let classification = self.classify_by_pattern(&error_str);
        
        // Record for ML clustering
        self.record_error(&error_str).await;
        
        // Enhance with clustering if available
        if let Some(cluster_id) = self.get_cluster_id(&error_str).await {
            let confidence = self.calculate_cluster_confidence(cluster_id).await;
            if confidence > 0.7 {
                return ErrorClassification {
                    error_type: UniverseErrorType::ClusteredAnomaly {
                        cluster_id,
                        confidence,
                    },
                    confidence,
                    is_transient: true,
                    should_taint: false,
                };
            }
        }
        
        classification
    }
    
    fn classify_by_pattern(&self, error_str: &str) -> ErrorClassification {
        let error_lower = error_str.to_lowercase();
        
        // Validator behind pattern
        if error_lower.contains("behind") || error_lower.contains("slot") && error_lower.contains("lag") {
            return ErrorClassification {
                error_type: UniverseErrorType::ValidatorBehind { slots: 10 }, // Estimate
                confidence: 0.8,
                is_transient: true,
                should_taint: false,
            };
        }
        
        // Consensus failure
        if error_lower.contains("consensus") || error_lower.contains("fork") {
            return ErrorClassification {
                error_type: UniverseErrorType::ConsensusFailure,
                confidence: 0.9,
                is_transient: false,
                should_taint: true,
            };
        }
        
        // Geyser stream errors
        if error_lower.contains("geyser") || error_lower.contains("stream") {
            return ErrorClassification {
                error_type: UniverseErrorType::GeyserStreamError,
                confidence: 0.85,
                is_transient: true,
                should_taint: false,
            };
        }
        
        // Timeout patterns
        if error_lower.contains("timeout") || error_lower.contains("timed out") {
            return ErrorClassification {
                error_type: UniverseErrorType::ShredstreamTimeout,
                confidence: 0.9,
                is_transient: true,
                should_taint: false,
            };
        }
        
        // Security violations
        if error_lower.contains("unauthorized") || error_lower.contains("signature") && error_lower.contains("invalid") {
            return ErrorClassification {
                error_type: UniverseErrorType::SecurityViolation {
                    reason: "Invalid signature or unauthorized access".to_string(),
                },
                confidence: 0.95,
                is_transient: false,
                should_taint: true,
            };
        }
        
        // Quota exceeded
        if error_lower.contains("quota") || error_lower.contains("rate limit") {
            return ErrorClassification {
                error_type: UniverseErrorType::QuotaExceeded,
                confidence: 0.9,
                is_transient: true,
                should_taint: false,
            };
        }
        
        // Congestion patterns
        if error_lower.contains("congestion") || error_lower.contains("busy") {
            return ErrorClassification {
                error_type: UniverseErrorType::ClusterCongestion { tps: 3000 }, // Estimate
                confidence: 0.8,
                is_transient: true,
                should_taint: false,
            };
        }
        
        // Default: wrap as base error
        ErrorClassification {
            error_type: UniverseErrorType::Base(Box::new(NonceError::Internal(error_str.to_string()))),
            confidence: 0.5,
            is_transient: false,
            should_taint: false,
        }
    }
    
    async fn record_error(&self, error_str: &str) {
        let mut history = self.history.write().await;
        history.push(ErrorHistoryEntry {
            error_message: error_str.to_string(),
            timestamp: Instant::now(),
            cluster_id: None,
        });
        
        // Maintain bounded size
        if history.len() > self.max_history_size {
            history.remove(0);
        }
        
        // Perform clustering if enough samples
        if history.len() >= 20 {
            self.perform_clustering(&mut history);
        }
    }
    
    /// Simple k-means clustering approximation on error strings
    fn perform_clustering(&self, history: &mut Vec<ErrorHistoryEntry>) {
        // Simplified clustering based on error string similarity
        // In production, use proper feature extraction and k-means
        
        // Group by first word (simple heuristic)
        let mut clusters: Vec<Vec<usize>> = vec![Vec::new(); self.cluster_count as usize];
        
        for (idx, entry) in history.iter().enumerate() {
            let first_word = entry.error_message.split_whitespace().next().unwrap_or("");
            let hash = first_word.chars().fold(0u32, |acc, c| acc.wrapping_add(c as u32));
            let cluster_id = (hash % self.cluster_count as u32) as u8;
            
            if let Some(cluster) = clusters.get_mut(cluster_id as usize) {
                cluster.push(idx);
            }
        }
        
        // Assign cluster IDs
        for (cluster_id, indices) in clusters.iter().enumerate() {
            for &idx in indices {
                if let Some(entry) = history.get_mut(idx) {
                    entry.cluster_id = Some(cluster_id as u8);
                }
            }
        }
    }
    
    async fn get_cluster_id(&self, error_str: &str) -> Option<u8> {
        let history = self.history.read().await;
        
        // Find most similar error in history
        let first_word = error_str.split_whitespace().next().unwrap_or("");
        
        for entry in history.iter().rev() {
            let entry_first_word = entry.error_message.split_whitespace().next().unwrap_or("");
            if first_word == entry_first_word {
                return entry.cluster_id;
            }
        }
        
        None
    }
    
    async fn calculate_cluster_confidence(&self, cluster_id: u8) -> f64 {
        let history = self.history.read().await;
        let cluster_size = history.iter().filter(|e| e.cluster_id == Some(cluster_id)).count();
        
        // Confidence based on cluster size
        if cluster_size >= 10 {
            0.9
        } else if cluster_size >= 5 {
            0.7
        } else {
            0.5
        }
    }
}

/// Enhanced retry with circuit breaker and error classification
pub async fn retry_with_backoff_enhanced<F, Fut, T>(
    operation_name: &str,
    config: &RetryConfig,
    breaker: Option<&CircuitBreaker>,
    classifier: Option<&ErrorClassifier>,
    mut operation: F,
) -> NonceResult<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = NonceResult<T>>,
{
    let start_time = std::time::Instant::now();
    let mut last_error = None;
    
    for attempt in 0..config.max_attempts {
        // Check circuit breaker
        if let Some(cb) = breaker {
            if !cb.can_execute().await {
                warn!(
                    operation = operation_name,
                    "Circuit breaker is open, aborting operation"
                );
                return Err(NonceError::Internal(
                    "Circuit breaker open".to_string()
                ));
            }
        }
        
        // Log attempt
        if attempt > 0 {
            debug!(
                operation = operation_name,
                attempt = attempt + 1,
                max_attempts = config.max_attempts,
                "Retrying operation"
            );
        }
        
        // Execute operation
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    debug!(
                        operation = operation_name,
                        attempts = attempt + 1,
                        duration_ms = start_time.elapsed().as_millis() as u64,
                        "Operation succeeded after retry"
                    );
                }
                
                // Record success in circuit breaker
                if let Some(cb) = breaker {
                    cb.record_success().await;
                }
                
                return Ok(result);
            }
            Err(err) => {
                // Classify error if classifier available
                let classification = if let Some(clf) = classifier {
                    Some(clf.classify_error(&err).await)
                } else {
                    None
                };
                
                // Check if error is transient
                let is_transient = classification
                    .as_ref()
                    .map(|c| c.is_transient)
                    .unwrap_or_else(|| err.is_transient());
                
                if !is_transient {
                    warn!(
                        operation = operation_name,
                        error = %err,
                        "Permanent error, not retrying"
                    );
                    
                    // Record failure in circuit breaker
                    if let Some(cb) = breaker {
                        cb.record_failure().await;
                    }
                    
                    return Err(err);
                }
                
                last_error = Some(err.clone());
                
                // Record failure in circuit breaker
                if let Some(cb) = breaker {
                    cb.record_failure().await;
                }
                
                // If this is not the last attempt, apply backoff
                if attempt + 1 < config.max_attempts {
                    let backoff = config.calculate_backoff(attempt);
                    debug!(
                        operation = operation_name,
                        attempt = attempt + 1,
                        backoff_ms = backoff.as_millis() as u64,
                        error = %err,
                        classified_type = ?classification.as_ref().map(|c| &c.error_type),
                        "Transient error, backing off before retry"
                    );
                    sleep(backoff).await;
                } else {
                    warn!(
                        operation = operation_name,
                        attempts = attempt + 1,
                        error = %err,
                        "All retry attempts exhausted"
                    );
                }
            }
        }
    }
    
    // All attempts failed
    Err(last_error.unwrap_or_else(|| {
        NonceError::Internal("Retry exhausted without error".to_string())
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    
    #[tokio::test]
    async fn test_retry_succeeds_on_first_attempt() {
        let config = RetryConfig::default();
        let result = retry_with_backoff("test_op", &config, || async {
            Ok::<i32, NonceError>(42)
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
    
    #[tokio::test]
    async fn test_retry_succeeds_after_transient_errors() {
        let config = RetryConfig::default();
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();
        
        let result = retry_with_backoff("test_op", &config, || {
            let count = attempt_count_clone.fetch_add(1, Ordering::SeqCst);
            async move {
                if count < 2 {
                    // First two attempts fail with transient error
                    Err(NonceError::Rpc {
                        endpoint: Some("test".to_string()),
                        message: "transient error".to_string(),
                    })
                } else {
                    // Third attempt succeeds
                    Ok(42)
                }
            }
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }
    
    #[tokio::test]
    async fn test_retry_fails_on_permanent_error() {
        let config = RetryConfig::default();
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();
        
        let result = retry_with_backoff("test_op", &config, || {
            let _ = attempt_count_clone.fetch_add(1, Ordering::SeqCst);
            async {
                Err(NonceError::InvalidNonceAccount(
                    "permanent error".to_string()
                ))
            }
        }).await;
        
        assert!(result.is_err());
        // Should only attempt once for permanent error
        assert_eq!(attempt_count.load(Ordering::SeqCst), 1);
    }
    
    #[tokio::test]
    async fn test_retry_exhausts_all_attempts() {
        let config = RetryConfig {
            max_attempts: 3,
            base_backoff_ms: 10,
            max_backoff_ms: 50,
            jitter_factor: 0.1,
        };
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();
        
        let result = retry_with_backoff("test_op", &config, || {
            let _ = attempt_count_clone.fetch_add(1, Ordering::SeqCst);
            async {
                Err(NonceError::Timeout(1000))
            }
        }).await;
        
        assert!(result.is_err());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }
    
    #[tokio::test]
    async fn test_backoff_calculation() {
        let config = RetryConfig {
            max_attempts: 5,
            base_backoff_ms: 100,
            max_backoff_ms: 2000,
            jitter_factor: 0.2,
        };
        
        // Test exponential growth
        let delay0 = config.calculate_backoff(0);
        let delay1 = config.calculate_backoff(1);
        let delay2 = config.calculate_backoff(2);
        
        // Delays should generally increase (accounting for jitter)
        assert!(delay0.as_millis() >= 80 && delay0.as_millis() <= 120); // ~100ms ± 20%
        assert!(delay1.as_millis() >= 160 && delay1.as_millis() <= 240); // ~200ms ± 20%
        assert!(delay2.as_millis() >= 320 && delay2.as_millis() <= 480); // ~400ms ± 20%
        
        // Test capping
        let delay_large = config.calculate_backoff(10);
        assert!(delay_large.as_millis() <= 2400); // Max 2000ms + 20% jitter
    }
    
    #[tokio::test]
    async fn test_retry_with_metrics() {
        let config = RetryConfig::default();
        let (result, metrics) = retry_with_metrics("test_op", &config, || async {
            Ok::<i32, NonceError>(42)
        }).await;
        
        assert!(result.is_ok());
        assert!(metrics.successful);
        assert!(metrics.final_error.is_none());
    }
    
    #[tokio::test]
    async fn test_circuit_breaker_transitions() {
        let breaker = CircuitBreaker::new(3, 2, Duration::from_millis(100));
        
        // Initial state should be Closed
        assert_eq!(breaker.get_state().await, CircuitState::Closed);
        assert!(breaker.can_execute().await);
        
        // Record failures to open circuit
        for _ in 0..3 {
            breaker.record_failure().await;
        }
        
        assert_eq!(breaker.get_state().await, CircuitState::Open);
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Should transition to HalfOpen after timeout
        assert!(breaker.can_execute().await);
        assert_eq!(breaker.get_state().await, CircuitState::HalfOpen);
        
        // Record successes to close circuit
        for _ in 0..2 {
            breaker.record_success().await;
        }
        
        assert_eq!(breaker.get_state().await, CircuitState::Closed);
    }
    
    #[tokio::test]
    async fn test_circuit_breaker_halfopen_failure() {
        let breaker = CircuitBreaker::new(3, 2, Duration::from_millis(100));
        
        // Open the breaker
        for _ in 0..3 {
            breaker.record_failure().await;
        }
        assert_eq!(breaker.get_state().await, CircuitState::Open);
        
        // Wait for timeout to transition to HalfOpen
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(breaker.can_execute().await);
        assert_eq!(breaker.get_state().await, CircuitState::HalfOpen);
        
        // Any failure in HalfOpen should immediately open
        breaker.record_failure().await;
        assert_eq!(breaker.get_state().await, CircuitState::Open);
    }
    
    #[tokio::test]
    async fn test_global_circuit_breaker() {
        let global = GlobalCircuitBreaker::new();
        
        // Get breakers for different endpoints
        let breaker1 = global.get_breaker("endpoint1").await;
        let breaker2 = global.get_breaker("endpoint2").await;
        
        // Open one breaker
        for _ in 0..3 {
            breaker1.record_failure().await;
        }
        
        // Should not trip global (only 50% open)
        assert!(!global.should_trip_global().await);
        
        // Open second breaker
        for _ in 0..3 {
            breaker2.record_failure().await;
        }
        
        // Should trip global (100% open)
        assert!(global.should_trip_global().await);
    }
    
    #[tokio::test]
    async fn test_error_classification() {
        let classifier = ErrorClassifier::new(100, 5);
        
        // Test timeout classification
        let timeout_err = NonceError::Timeout(1000);
        let classification = classifier.classify_error(&timeout_err).await;
        assert!(classification.is_transient);
        assert!(classification.confidence > 0.8);
        
        // Test security violation
        let sec_err = NonceError::Signing("Invalid signature".to_string());
        let classification = classifier.classify_error(&sec_err).await;
        assert!(!classification.is_transient);
        
        // Test RPC error (transient)
        let rpc_err = NonceError::Rpc {
            endpoint: Some("test".to_string()),
            message: "timeout occurred".to_string(),
        };
        let classification = classifier.classify_error(&rpc_err).await;
        assert!(classification.is_transient);
    }
    
    #[tokio::test]
    async fn test_retry_with_circuit_breaker() {
        let config = RetryConfig::default();
        let breaker = CircuitBreaker::new(2, 1, Duration::from_secs(1));
        let classifier = ErrorClassifier::new(100, 5);
        
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();
        
        // This will fail and open the circuit
        let result = retry_with_backoff_enhanced(
            "test_op",
            &config,
            Some(&breaker),
            Some(&classifier),
            || {
                let _ = attempt_count_clone.fetch_add(1, Ordering::SeqCst);
                async {
                    Err(NonceError::Timeout(1000))
                }
            }
        ).await;
        
        assert!(result.is_err());
        assert_eq!(breaker.get_state().await, CircuitState::Open);
    }
}
