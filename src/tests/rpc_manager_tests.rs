#[cfg(test)]
mod rpc_manager_integration_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::time::Duration;
    
    /// Mock RPC server that can simulate various failure scenarios
    struct MockRpcServer {
        /// How many requests this server has handled
        request_count: Arc<AtomicUsize>,
        /// Simulated latency in milliseconds
        latency_ms: u64,
        /// Failure rate (0.0 - 1.0)
        failure_rate: f64,
        /// Whether to simulate timeouts
        timeout: bool,
    }
    
    impl MockRpcServer {
        fn new(latency_ms: u64, failure_rate: f64) -> Self {
            Self {
                request_count: Arc::new(AtomicUsize::new(0)),
                latency_ms,
                failure_rate,
                timeout: false,
            }
        }
        
        fn with_timeout(mut self) -> Self {
            self.timeout = true;
            self
        }
        
        async fn handle_request(&self) -> Result<(), &'static str> {
            self.request_count.fetch_add(1, Ordering::SeqCst);
            
            if self.timeout {
                // Simulate a very long delay
                tokio::time::sleep(Duration::from_secs(30)).await;
                return Err("timeout");
            }
            
            // Simulate latency
            tokio::time::sleep(Duration::from_millis(self.latency_ms)).await;
            
            // Simulate failure
            if rand::random::<f64>() < self.failure_rate {
                return Err("simulated failure");
            }
            
            Ok(())
        }
        
        fn get_request_count(&self) -> usize {
            self.request_count.load(Ordering::SeqCst)
        }
    }
    
    #[tokio::test]
    async fn test_no_panics_with_errors() {
        // This test ensures that error conditions don't cause panics
        
        // Test with invalid URLs
        let invalid_urls = vec![
            "not-a-url".to_string(),
            "http://".to_string(),
            "".to_string(),
        ];
        
        // Should not panic when creating manager with invalid URLs
        let manager = RpcManager::new(&invalid_urls);
        
        // Should not panic when trying to get healthy client
        let result = manager.get_healthy_client().await;
        assert!(result.is_err()); // Expected to fail, but shouldn't panic
    }
    
    #[tokio::test]
    async fn test_fibonacci_backoff_no_panic() {
        let mut backoff = FibonacciBackoff::new(5, 100, 5000);
        
        // Test normal progression
        for i in 0..5 {
            let delay = backoff.next_delay();
            assert!(delay.is_some(), "Delay {} should be Some", i);
        }
        
        // Test exhaustion
        let delay = backoff.next_delay();
        assert!(delay.is_none(), "Should be exhausted");
        
        // Test reset
        backoff.reset();
        let delay = backoff.next_delay();
        assert!(delay.is_some(), "Should work after reset");
    }
    
    #[tokio::test]
    async fn test_circuit_breaker_state_transitions() {
        let mut cb = TierCircuitBreaker::new(3, 2, Duration::from_millis(100));
        
        // Initially closed
        assert_eq!(cb.get_state(), CircuitState::Closed);
        assert!(cb.can_execute());
        
        // Record failures to open
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        
        // Should be open now
        assert_eq!(cb.get_state(), CircuitState::Open);
        assert!(!cb.can_execute());
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Should transition to half-open
        cb.record_failure(); // Trigger state check
        assert_eq!(cb.get_state(), CircuitState::Open); // Failed recovery
        
        // Wait again
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Success should lead to recovery
        cb.record_success(); // Check state
        assert_eq!(cb.get_state(), CircuitState::HalfOpen);
        
        cb.record_success();
        cb.record_success(); // Need success_threshold successes
        
        // Should be closed now
        assert_eq!(cb.get_state(), CircuitState::Closed);
        assert!(cb.can_execute());
    }
    
    #[tokio::test]
    async fn test_predictive_model_no_panic() {
        let mut model = PredictiveHealthModel::new(100, 0.7);
        
        // Should handle empty history
        let prob = model.predict_failure_probability();
        assert!(prob >= 0.0 && prob <= 1.0);
        
        // Should handle small history
        for i in 0..5 {
            model.record_observation(100.0 + i as f64, 0.0, 0);
        }
        let prob = model.predict_failure_probability();
        assert!(prob >= 0.0 && prob <= 1.0);
        
        // Should handle increasing latency
        for i in 0..100 {
            model.record_observation(100.0 + (i as f64 * 10.0), i as f64 / 100.0, i);
        }
        let prob = model.predict_failure_probability();
        assert!(prob >= 0.0 && prob <= 1.0);
    }
    
    #[tokio::test]
    async fn test_perf_stats_ewma() {
        let mut stats = PerfStats::new(0.3);
        
        // Record successful request
        stats.record_request(100.0, true);
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.total_errors, 0);
        assert!(stats.success_rate() > 0.9);
        
        // Record failed request
        stats.record_request(200.0, false);
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.total_errors, 1);
        assert!(stats.success_rate() < 1.0);
        
        // Record confirmation
        stats.record_confirmation(500.0);
        assert!(stats.confirmation_speed_ms() > 0.0);
    }
    
    #[tokio::test]
    async fn test_universe_metrics() {
        let metrics = UniverseMetrics::new();
        
        // Record requests
        metrics.record_request(RpcTier::Tier0Ultra, true);
        metrics.record_request(RpcTier::Tier0Ultra, true);
        metrics.record_request(RpcTier::Tier0Ultra, false);
        
        assert_eq!(*metrics.total_requests.read(), 3);
        assert_eq!(*metrics.total_errors.read(), 1);
        
        // Record predictive switch
        metrics.record_predictive_switch();
        assert_eq!(*metrics.predictive_switches.read(), 1);
        
        // Record rate limit
        metrics.record_rate_limit_hit();
        assert_eq!(*metrics.rate_limit_hits.read(), 1);
    }
    
    #[test]
    fn test_tier_inference() {
        // Test Tier0
        assert_eq!(
            RpcManager::infer_tier_from_url("https://block-engine.jito.wtf"),
            RpcTier::Tier0Ultra
        );
        assert_eq!(
            RpcManager::infer_tier_from_url("https://private-rpc.com"),
            RpcTier::Tier0Ultra
        );
        
        // Test Tier1
        assert_eq!(
            RpcManager::infer_tier_from_url("https://mainnet.helius-rpc.com"),
            RpcTier::Tier1Premium
        );
        assert_eq!(
            RpcManager::infer_tier_from_url("https://rpc.quicknode.com"),
            RpcTier::Tier1Premium
        );
        
        // Test Tier2
        assert_eq!(
            RpcManager::infer_tier_from_url("https://api.mainnet-beta.solana.com"),
            RpcTier::Tier2Public
        );
    }
    
    #[test]
    fn test_location_inference() {
        assert_eq!(
            RpcManager::infer_location_from_url("https://mainnet.helius-rpc.com"),
            Some("us-east".to_string())
        );
        assert_eq!(
            RpcManager::infer_location_from_url("https://rpc.triton.one"),
            Some("us-west".to_string())
        );
        assert_eq!(
            RpcManager::infer_location_from_url("https://unknown.com"),
            None
        );
    }
    
    #[tokio::test]
    async fn test_manager_initialization() {
        let urls = vec![
            "https://api.devnet.solana.com".to_string(),
            "https://api.testnet.solana.com".to_string(),
        ];
        
        let manager = RpcManager::new(&urls);
        
        // Verify endpoints were created
        let endpoints = manager.endpoints.read();
        assert_eq!(endpoints.len(), 2);
        assert_eq!(endpoints[0].url, "https://api.devnet.solana.com");
        assert_eq!(endpoints[1].url, "https://api.testnet.solana.com");
        
        // Verify initial health
        assert_eq!(endpoints[0].health, RpcHealth::Healthy);
        assert_eq!(endpoints[1].health, RpcHealth::Healthy);
    }
    
    #[tokio::test]
    async fn test_hot_add_remove_endpoint() {
        let urls = vec!["https://api.devnet.solana.com".to_string()];
        let manager = RpcManager::new(&urls);
        
        // Add endpoint
        let new_url = "https://api.testnet.solana.com".to_string();
        let result = manager.add_endpoint_hot(new_url.clone()).await;
        assert!(result.is_ok());
        
        // Verify addition
        {
            let endpoints = manager.endpoints.read();
            assert_eq!(endpoints.len(), 2);
        }
        
        // Remove endpoint
        let result = manager.remove_endpoint_hot(&new_url).await;
        assert!(result.is_ok());
        
        // Verify removal
        {
            let endpoints = manager.endpoints.read();
            assert_eq!(endpoints.len(), 1);
        }
    }
    
    #[tokio::test]
    async fn test_safe_non_zero_u32() {
        // Test clamping to minimum
        let result = safe_non_zero_u32(0);
        assert_eq!(result.get(), MIN_RATE_LIMIT_RPS);
        
        // Test normal value
        let result = safe_non_zero_u32(100);
        assert_eq!(result.get(), 100);
        
        // Test clamping to maximum
        let result = safe_non_zero_u32(999999);
        assert_eq!(result.get(), MAX_RATE_LIMIT_RPS);
    }
    
    #[tokio::test]
    async fn test_error_classification() {
        use solana_client::client_error::{ClientError, ClientErrorKind};
        
        let manager = RpcManager::new(&vec!["https://test.com".to_string()]);
        
        // Create a mock error
        let err = anyhow::anyhow!("Rate limit exceeded");
        let err_type = RpcManager::classify_error(&*err);
        
        // Should classify as rate limited or other
        assert!(matches!(err_type, RpcErrorType::RateLimited | RpcErrorType::Other));
    }
    
    /// Stress test: Simulate 1000 sends without panics
    #[tokio::test]
    async fn test_1000_sends_no_panic() {
        let urls = vec![
            "https://api.devnet.solana.com".to_string(),
            "https://api.testnet.solana.com".to_string(),
        ];
        
        let manager = Arc::new(RpcManager::new(&urls));
        let mut tasks = vec![];
        
        for i in 0..1000 {
            let manager_clone = manager.clone();
            let task = tokio::spawn(async move {
                // Simulate various operations that might fail
                let _ = manager_clone.get_healthy_client().await;
                
                // Record some metrics
                manager_clone.record_rpc_result(
                    "https://api.devnet.solana.com",
                    100.0 + (i as f64),
                    i % 10 != 0, // Fail every 10th request
                );
                
                // Try to get ranked endpoints
                let _ = manager_clone.get_ranked_rpc_endpoints(3).await;
            });
            tasks.push(task);
        }
        
        // Wait for all tasks
        for task in tasks {
            assert!(task.await.is_ok(), "Task should not panic");
        }
        
        // Verify metrics were recorded
        let metrics = manager.get_universe_metrics();
        let total = *metrics.total_requests.read();
        // Should have recorded at least some requests
        assert!(total > 0, "Should have recorded requests");
    }
    
    #[tokio::test]
    async fn test_concurrent_endpoint_access() {
        let urls = vec![
            "https://api.devnet.solana.com".to_string(),
            "https://api.testnet.solana.com".to_string(),
            "https://api.mainnet-beta.solana.com".to_string(),
        ];
        
        let manager = Arc::new(RpcManager::new(&urls));
        let mut tasks = vec![];
        
        // Spawn 100 concurrent tasks
        for i in 0..100 {
            let manager_clone = manager.clone();
            let task = tokio::spawn(async move {
                // Read endpoint data
                let _ = manager_clone.get_health_stats().await;
                
                // Record results
                let url = if i % 3 == 0 {
                    "https://api.devnet.solana.com"
                } else if i % 3 == 1 {
                    "https://api.testnet.solana.com"
                } else {
                    "https://api.mainnet-beta.solana.com"
                };
                
                manager_clone.record_rpc_result(url, 100.0, i % 5 != 0);
            });
            tasks.push(task);
        }
        
        // All tasks should complete without panic
        for task in tasks {
            assert!(task.await.is_ok());
        }
    }
}
