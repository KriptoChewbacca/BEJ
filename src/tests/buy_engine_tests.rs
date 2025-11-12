// =========================================================================
// UNIVERSE CLASS GRADE: Comprehensive Tests for Optimizations
// =========================================================================

#[cfg(test)]
mod universe_class_tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_predictive_analytics_surge_detection() {
        let analytics = PredictiveAnalytics::new(0.5, Duration::from_secs(60));
        
        // Record baseline volume
        for i in 1..=10 {
            analytics.record_volume(100 * i).await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // No surge yet
        assert!(analytics.predict_surge().await.is_none());

        // Record surge in volume
        for i in 1..=5 {
            analytics.record_volume(1000 * i).await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Should detect surge with confidence
        let surge = analytics.predict_surge().await;
        assert!(surge.is_some());
    }

    #[tokio::test]
    async fn test_buy_config_validation() {
        // Valid config
        let valid_config = BuyConfig::default();
        assert!(valid_config.validate().is_ok());

        // Invalid slippage
        let mut invalid_config = BuyConfig::default();
        invalid_config.slippage_bps = 20000;
        assert!(invalid_config.validate().is_err());

        // Kill switch active
        let mut killed_config = BuyConfig::default();
        killed_config.kill_switch = true;
        assert!(killed_config.validate().is_err());

        // Disabled config
        let mut disabled_config = BuyConfig::default();
        disabled_config.enabled = false;
        assert!(disabled_config.validate().is_err());
    }

    #[tokio::test]
    async fn test_token_bucket_rate_limiter() {
        let limiter = TokenBucketRateLimiter::new(5, 1);  // 5 capacity, 1/sec refill
        
        // Should be able to acquire initial capacity
        assert!(limiter.try_acquire(1).await);
        assert!(limiter.try_acquire(1).await);
        assert!(limiter.try_acquire(1).await);
        assert!(limiter.try_acquire(1).await);
        assert!(limiter.try_acquire(1).await);
        
        // Should fail after depleting
        assert!(!limiter.try_acquire(1).await);
        
        // Wait for refill
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Should succeed after refill
        assert!(limiter.try_acquire(1).await);
    }

    #[tokio::test]
    async fn test_rpc_error_classification() {
        let rate_limit_err = anyhow!("Error: rate limit exceeded");
        assert_eq!(RpcErrorClass::classify(&rate_limit_err), RpcErrorClass::RateLimit);
        
        let blockhash_err = anyhow!("blockhash not found");
        assert_eq!(RpcErrorClass::classify(&blockhash_err), RpcErrorClass::BadBlockhash);
        
        let network_err = anyhow!("connection timeout");
        assert_eq!(RpcErrorClass::classify(&network_err), RpcErrorClass::NetworkError);
        
        let insufficient_err = anyhow!("insufficient funds");
        assert_eq!(RpcErrorClass::classify(&insufficient_err), RpcErrorClass::InsufficientFunds);
        
        // Test retryability
        assert!(RpcErrorClass::RateLimit.is_retryable());
        assert!(RpcErrorClass::NetworkError.is_retryable());
        assert!(!RpcErrorClass::Permanent.is_retryable());
        assert!(!RpcErrorClass::InsufficientFunds.is_retryable());
    }

    #[tokio::test]
    async fn test_exponential_backoff() {
        let backoff = ExponentialBackoff::new(100, 5000, 3);
        
        // First attempt should be base delay ~100ms
        let delay0 = backoff.calculate_delay(0);
        assert!(delay0.as_millis() >= 90 && delay0.as_millis() <= 110);
        
        // Second attempt should be ~200ms
        let delay1 = backoff.calculate_delay(1);
        assert!(delay1.as_millis() >= 180 && delay1.as_millis() <= 220);
        
        // Third attempt should be ~400ms
        let delay2 = backoff.calculate_delay(2);
        assert!(delay2.as_millis() >= 360 && delay2.as_millis() <= 440);
        
        // Should retry on transient errors
        let transient_err = anyhow!("temporary failure");
        assert!(backoff.should_retry(0, &transient_err));
        assert!(backoff.should_retry(1, &transient_err));
        assert!(backoff.should_retry(2, &transient_err));
        assert!(!backoff.should_retry(3, &transient_err));  // Max retries exceeded
    }

    #[tokio::test]
    async fn test_blockhash_manager() {
        use solana_sdk::hash::Hash;
        
        let manager = BlockhashManager::new(1000);  // 1 second max age
        
        // Initially no fresh blockhash
        assert!(manager.get_fresh_blockhash().await.is_none());
        assert!(!manager.is_fresh().await);
        
        // Update with new blockhash
        let hash = Hash::new_unique();
        manager.update_blockhash(hash, 100).await;
        
        // Should now have fresh blockhash
        assert_eq!(manager.get_fresh_blockhash().await, Some(hash));
        assert!(manager.is_fresh().await);
        
        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(1100)).await;
        
        // Should be stale now
        assert!(manager.get_fresh_blockhash().await.is_none());
        assert!(!manager.is_fresh().await);
    }

    #[tokio::test]
    async fn test_simulation_policy() {
        let success = SimulationResult::Success;
        assert!(success.should_proceed(SimulationPolicy::BlockOnCritical));
        assert!(success.should_proceed(SimulationPolicy::WarnOnAdvisory));
        assert!(success.should_proceed(SimulationPolicy::AlwaysAllow));
        
        let critical = SimulationResult::CriticalFailure("test".to_string());
        assert!(!critical.should_proceed(SimulationPolicy::BlockOnCritical));
        assert!(critical.should_proceed(SimulationPolicy::AlwaysAllow));
        
        let advisory = SimulationResult::AdvisoryFailure("test".to_string());
        assert!(advisory.should_proceed(SimulationPolicy::BlockOnCritical));
        assert!(advisory.should_proceed(SimulationPolicy::WarnOnAdvisory));
        assert!(advisory.should_proceed(SimulationPolicy::AlwaysAllow));
    }

    #[tokio::test]
    async fn test_transaction_queue() {
        use solana_sdk::{pubkey::Pubkey, signature::Signature, transaction::VersionedTransaction};
        use solana_sdk::{message::Message, transaction::Transaction};
        // TODO(migrate-system-instruction): temporary allow, full migration post-profit
        #[allow(deprecated)]
        use solana_sdk::system_instruction;
        
        let queue = TransactionQueue::new(10);
        
        // Create dummy transaction
        let from = Pubkey::new_unique();
        let to = Pubkey::new_unique();
        let ix = system_instruction::transfer(&from, &to, 1);
        let msg = Message::new(&[ix], None);
        let tx = Transaction::new_unsigned(msg);
        let vtx = VersionedTransaction::from(tx);
        
        let queued = QueuedTransaction {
            tx: vtx.clone(),
            candidate: PremintCandidate {
                mint: Pubkey::new_unique(),
                creator: Pubkey::new_unique(),
                program: "test".to_string(),
                slot: 0,
                timestamp: 0,
                instruction_summary: None,
                is_jito_bundle: None,
            },
            created_at: Instant::now(),
            blockhash_fetch_time: Some(Instant::now()),
            attempts: 0,
            correlation_id: "test123".to_string(),
        };
        
        // Initially empty
        assert_eq!(queue.len().await, 0);
        
        // Push transaction
        assert!(queue.push(queued.clone()).await.is_ok());
        assert_eq!(queue.len().await, 1);
        
        // Pop transaction
        let popped = queue.pop().await;
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().correlation_id, "test123");
        assert_eq!(queue.len().await, 0);
    }

    #[tokio::test]
    async fn test_queue_stale_cleanup() {
        use solana_sdk::{pubkey::Pubkey, message::Message, transaction::Transaction, transaction::VersionedTransaction};
        // TODO(migrate-system-instruction): temporary allow, full migration post-profit
        #[allow(deprecated)]
        use solana_sdk::system_instruction;
        
        let queue = TransactionQueue::new(10);
        
        // Create old transaction
        let from = Pubkey::new_unique();
        let to = Pubkey::new_unique();
        let ix = system_instruction::transfer(&from, &to, 1);
        let msg = Message::new(&[ix], None);
        let tx = Transaction::new_unsigned(msg);
        let vtx = VersionedTransaction::from(tx);
        
        let old_queued = QueuedTransaction {
            tx: vtx,
            candidate: PremintCandidate {
                mint: Pubkey::new_unique(),
                creator: Pubkey::new_unique(),
                program: "test".to_string(),
                slot: 0,
                timestamp: 0,
                instruction_summary: None,
                is_jito_bundle: None,
            },
            created_at: Instant::now() - Duration::from_secs(20),  // Old
            blockhash_fetch_time: Some(Instant::now()),
            attempts: 0,
            correlation_id: "old".to_string(),
        };
        
        queue.push(old_queued).await.unwrap();
        assert_eq!(queue.len().await, 1);
        
        // Clean up items older than 10 seconds
        let removed = queue.clear_stale(Duration::from_secs(10)).await;
        assert_eq!(removed, 1);
        assert_eq!(queue.len().await, 0);
    }

    #[tokio::test]
    async fn test_universe_metrics_enhanced() {
        let metrics = UniverseMetrics::new();
        
        // Test RPC error recording
        metrics.record_rpc_error("RateLimit");
        metrics.record_rpc_error("RateLimit");
        metrics.record_rpc_error("NetworkError");
        
        assert_eq!(
            metrics.rpc_error_counts.get("RateLimit").unwrap().load(Ordering::Relaxed),
            2
        );
        assert_eq!(
            metrics.rpc_error_counts.get("NetworkError").unwrap().load(Ordering::Relaxed),
            1
        );
        
        // Test simulation failure recording
        metrics.record_simulation_failure(false);  // Advisory
        metrics.record_simulation_failure(true);   // Critical
        metrics.record_simulation_failure(true);   // Critical
        
        assert_eq!(metrics.simulate_failures.load(Ordering::Relaxed), 3);
        assert_eq!(metrics.simulate_critical_failures.load(Ordering::Relaxed), 2);
        
        // Test retry count recording
        metrics.record_retry_count(3).await;
        metrics.record_retry_count(1).await;
        metrics.record_retry_count(5).await;
        
        let retries = metrics.retries_per_tx.read().await;
        assert_eq!(retries.len(), 3);
        
        // Test blockhash age recording
        metrics.record_blockhash_age(500).await;
        metrics.record_blockhash_age(1200).await;
        
        let ages = metrics.blockhash_age_at_signing.read().await;
        assert_eq!(ages.len(), 2);
        
        // Test inflight queue depth
        metrics.increment_inflight();
        metrics.increment_inflight();
        metrics.increment_inflight();
        assert_eq!(metrics.get_inflight_depth(), 3);
        
        metrics.decrement_inflight();
        assert_eq!(metrics.get_inflight_depth(), 2);
        
        // Test mempool rejection
        metrics.record_mempool_rejection();
        assert_eq!(metrics.mempool_rejections.load(Ordering::Relaxed), 1);
        
        // Test slippage recording
        metrics.record_slippage(25.5).await;
        metrics.record_slippage(30.2).await;
        
        let slippage = metrics.realized_slippage.read().await;
        assert_eq!(slippage.len(), 2);
    }

    #[tokio::test]
    async fn test_percentile_latency_calculation() {
        const LATENCY_SAMPLES: u64 = 100;
        
        let metrics = UniverseMetrics::new();
        
        // Record some latencies
        for i in 1..=LATENCY_SAMPLES {
            metrics.record_latency("sniff_to_buy", i * 1000).await;
        }
        
        // Check P50
        let p50 = metrics.get_percentile_latency("sniff_to_buy", 0.50).await;
        assert!(p50.is_some());
        assert!(p50.unwrap() >= 49_000 && p50.unwrap() <= 51_000);
        
        // Check P90
        let p90 = metrics.get_percentile_latency("sniff_to_buy", 0.90).await;
        assert!(p90.is_some());
        assert!(p90.unwrap() >= 89_000 && p90.unwrap() <= 91_000);
        
        // Check P99
        let p99 = metrics.get_percentile_latency("sniff_to_buy", 0.99).await;
        assert!(p99.is_some());
        assert!(p99.unwrap() >= 98_000 && p99.unwrap() <= 100_000);
    }
}
