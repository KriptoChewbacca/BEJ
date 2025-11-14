//! Integration tests for Task 2: Price Stream Integration
//!
//! These tests verify that the PriceStreamManager integrates correctly
//! with the BuyEngine and provides real-time price updates.

#[cfg(test)]
mod price_stream_integration_tests {
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use bot::components::price_stream::PriceStreamManager;
    
    #[tokio::test]
    async fn test_price_stream_basic_flow() {
        // Create a price stream manager
        let price_stream = Arc::new(PriceStreamManager::new(100, Duration::from_millis(333)));
        
        // Subscribe to price updates
        let mut rx = price_stream.subscribe();
        
        // Verify we have a subscriber
        assert_eq!(price_stream.subscriber_count(), 1);
        
        // Publish a price update
        use solana_sdk::pubkey::Pubkey;
        use bot::components::price_stream::PriceUpdate;
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let mint = Pubkey::new_unique();
        let update = PriceUpdate {
            mint,
            price_sol: 0.01,
            price_usd: 1.5,
            volume_24h: 100_000.0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            source: "test".to_string(),
        };
        
        price_stream.publish_price(update.clone());
        
        // Verify the price is cached
        let cached = price_stream.get_cached_price(&mint).unwrap();
        assert_eq!(cached.price_sol, 0.01);
        assert_eq!(cached.source, "test");
        
        // Verify the subscriber received the update
        let received = rx.recv().await.unwrap();
        assert_eq!(received.mint, mint);
        assert_eq!(received.price_sol, 0.01);
    }
    
    #[tokio::test]
    async fn test_price_stream_multiple_updates() {
        let price_stream = Arc::new(PriceStreamManager::new(1000, Duration::from_millis(333)));
        let mut rx = price_stream.subscribe();
        
        use solana_sdk::pubkey::Pubkey;
        use bot::components::price_stream::PriceUpdate;
        use std::time::{SystemTime, UNIX_EPOCH};
        
        // Publish multiple price updates
        let mut mints = Vec::new();
        for i in 0..10 {
            let mint = Pubkey::new_unique();
            mints.push(mint);
            
            let update = PriceUpdate {
                mint,
                price_sol: 0.001 * (i + 1) as f64,
                price_usd: 0.15 * (i + 1) as f64,
                volume_24h: 10_000.0 * (i + 1) as f64,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                source: format!("test_{}", i),
            };
            
            price_stream.publish_price(update);
        }
        
        // Verify all prices are cached
        assert_eq!(price_stream.cache_size(), 10);
        
        // Verify all updates were received
        for i in 0..10 {
            let received = rx.recv().await.unwrap();
            assert_eq!(received.mint, mints[i]);
            assert!((received.price_sol - 0.001 * (i + 1) as f64).abs() < 1e-10);
        }
    }
    
    #[tokio::test]
    async fn test_price_stream_concurrent_subscribers() {
        let price_stream = Arc::new(PriceStreamManager::new(1000, Duration::from_millis(333)));
        
        // Create 3 subscribers
        let mut rx1 = price_stream.subscribe();
        let mut rx2 = price_stream.subscribe();
        let mut rx3 = price_stream.subscribe();
        
        assert_eq!(price_stream.subscriber_count(), 3);
        
        use solana_sdk::pubkey::Pubkey;
        use bot::components::price_stream::PriceUpdate;
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let mint = Pubkey::new_unique();
        let update = PriceUpdate {
            mint,
            price_sol: 0.01,
            price_usd: 1.5,
            volume_24h: 100_000.0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            source: "concurrent_test".to_string(),
        };
        
        price_stream.publish_price(update);
        
        // All subscribers should receive the update
        let r1 = rx1.recv().await.unwrap();
        let r2 = rx2.recv().await.unwrap();
        let r3 = rx3.recv().await.unwrap();
        
        assert_eq!(r1.mint, mint);
        assert_eq!(r2.mint, mint);
        assert_eq!(r3.mint, mint);
        
        assert_eq!(r1.source, "concurrent_test");
        assert_eq!(r2.source, "concurrent_test");
        assert_eq!(r3.source, "concurrent_test");
    }
    
    #[tokio::test]
    async fn test_price_stream_update_interval() {
        let price_stream = PriceStreamManager::new(100, Duration::from_millis(500));
        
        // Verify the update interval is configured correctly
        assert_eq!(price_stream.update_interval(), Duration::from_millis(500));
    }
    
    #[tokio::test]
    async fn test_price_stream_cache_instant_lookup() {
        let price_stream = Arc::new(PriceStreamManager::new(100, Duration::from_millis(333)));
        
        use solana_sdk::pubkey::Pubkey;
        use bot::components::price_stream::PriceUpdate;
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let mint = Pubkey::new_unique();
        let update = PriceUpdate {
            mint,
            price_sol: 0.05,
            price_usd: 7.5,
            volume_24h: 500_000.0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            source: "cache_test".to_string(),
        };
        
        price_stream.publish_price(update);
        
        // Instant lookup should work immediately
        let start = std::time::Instant::now();
        let cached = price_stream.get_cached_price(&mint).unwrap();
        let lookup_time = start.elapsed();
        
        // Lookup should be very fast (< 1ms)
        assert!(lookup_time < Duration::from_millis(1));
        assert_eq!(cached.price_sol, 0.05);
        assert_eq!(cached.volume_24h, 500_000.0);
    }
}
