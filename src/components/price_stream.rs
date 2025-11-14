//! Price Stream Module - Real-time price updates for GUI monitoring
//!
//! This module provides a non-blocking price streaming mechanism that allows
//! the trading bot to publish price updates to multiple consumers (GUI, analytics, etc.)
//! without any performance impact on the main trading logic.
//!
//! ## Architecture
//!
//! - **Broadcast Channel**: Uses tokio::sync::broadcast for 1-to-many communication
//! - **DashMap Cache**: Lock-free concurrent cache for instant price lookups
//! - **Non-blocking**: All operations are fire-and-forget to avoid blocking the bot
//! - **333ms Update Interval**: Configurable refresh rate for GUI updates

use dashmap::DashMap;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

/// Real-time price update message
///
/// Published by the trading bot whenever a price is observed or updated.
/// Consumed by GUI, analytics, and other monitoring components.
#[derive(Clone, Debug)]
pub struct PriceUpdate {
    /// Token mint address
    pub mint: Pubkey,

    /// Current price in SOL
    pub price_sol: f64,

    /// Current price in USD (if available)
    pub price_usd: f64,

    /// 24-hour trading volume (if available)
    pub volume_24h: f64,

    /// Unix timestamp in seconds
    pub timestamp: u64,

    /// Source of the price data (e.g., "dexscreener", "jupiter", "internal")
    pub source: String,
}

/// Price stream manager for real-time price updates
///
/// Manages a broadcast channel for price updates and maintains a cache
/// for instant lookups. Designed for zero-impact integration with the trading bot.
pub struct PriceStreamManager {
    /// Broadcast channel sender for price updates
    ///
    /// Uses broadcast to support multiple consumers (GUI + analytics + logging).
    /// Each subscriber gets their own receiver that won't block others.
    price_tx: broadcast::Sender<PriceUpdate>,

    /// Update interval for GUI refresh rate
    update_interval: Duration,

    /// Lock-free cache for instant price lookups
    ///
    /// DashMap provides concurrent access without locks on the read path.
    /// The GUI can query current prices instantly without waiting for broadcasts.
    cache: Arc<DashMap<Pubkey, PriceUpdate>>,
}

impl PriceStreamManager {
    /// Create a new price stream manager
    ///
    /// # Arguments
    /// * `capacity` - Broadcast channel capacity (recommended: 1000)
    /// * `update_interval` - GUI update interval (recommended: 333ms)
    ///
    /// # Returns
    /// A new PriceStreamManager with an empty cache
    ///
    /// # Example
    /// ```no_run
    /// use std::time::Duration;
    /// # use bot::components::price_stream::PriceStreamManager;
    ///
    /// let manager = PriceStreamManager::new(1000, Duration::from_millis(333));
    /// ```
    pub fn new(capacity: usize, update_interval: Duration) -> Self {
        let (price_tx, _) = broadcast::channel(capacity);

        Self {
            price_tx,
            update_interval,
            cache: Arc::new(DashMap::new()),
        }
    }

    /// Publish a price update (non-blocking)
    ///
    /// This method is designed to be called from the hot path of the trading bot.
    /// It updates the cache synchronously (very fast) and broadcasts to subscribers
    /// without waiting for them to receive the message.
    ///
    /// # Arguments
    /// * `update` - The price update to publish
    ///
    /// # Performance
    /// - Cache update: O(1) average, lock-free
    /// - Broadcast: Non-blocking, dropped if no subscribers
    ///
    /// # Example
    /// ```no_run
    /// # use bot::components::price_stream::{PriceStreamManager, PriceUpdate};
    /// # use solana_sdk::pubkey::Pubkey;
    /// # use std::time::{Duration, SystemTime, UNIX_EPOCH};
    /// # let manager = PriceStreamManager::new(1000, Duration::from_millis(333));
    /// let update = PriceUpdate {
    ///     mint: Pubkey::new_unique(),
    ///     price_sol: 0.01,
    ///     price_usd: 1.5,
    ///     volume_24h: 100_000.0,
    ///     timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    ///     source: "internal".to_string(),
    /// };
    ///
    /// manager.publish_price(update);
    /// ```
    pub fn publish_price(&self, update: PriceUpdate) {
        // Update cache first (instant lookup for GUI)
        self.cache.insert(update.mint, update.clone());

        // Broadcast to subscribers (fire-and-forget)
        // If there are no subscribers, this is essentially free
        let _ = self.price_tx.send(update);
    }

    /// Subscribe to price updates
    ///
    /// Creates a new broadcast receiver for consuming price updates.
    /// Multiple subscribers can coexist without blocking each other.
    ///
    /// # Returns
    /// A broadcast::Receiver that will receive all future price updates
    ///
    /// # Example
    /// ```no_run
    /// # use bot::components::price_stream::PriceStreamManager;
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() {
    /// let manager = PriceStreamManager::new(1000, Duration::from_millis(333));
    /// let mut rx = manager.subscribe();
    ///
    /// // In a GUI thread:
    /// while let Ok(price_update) = rx.recv().await {
    ///     println!("Price update: {:?}", price_update);
    /// }
    /// # }
    /// ```
    pub fn subscribe(&self) -> broadcast::Receiver<PriceUpdate> {
        self.price_tx.subscribe()
    }

    /// Get the latest cached price for a token
    ///
    /// Returns the most recent price update for a token, if available.
    /// This is an instant O(1) lookup that doesn't require waiting for broadcasts.
    ///
    /// # Arguments
    /// * `mint` - The token mint address
    ///
    /// # Returns
    /// `Some(PriceUpdate)` if a price is cached, `None` otherwise
    ///
    /// # Example
    /// ```no_run
    /// # use bot::components::price_stream::PriceStreamManager;
    /// # use solana_sdk::pubkey::Pubkey;
    /// # use std::time::Duration;
    /// # let manager = PriceStreamManager::new(1000, Duration::from_millis(333));
    /// # let mint = Pubkey::new_unique();
    /// if let Some(price) = manager.get_cached_price(&mint) {
    ///     println!("Current price: {} SOL", price.price_sol);
    /// }
    /// ```
    pub fn get_cached_price(&self, mint: &Pubkey) -> Option<PriceUpdate> {
        self.cache.get(mint).map(|entry| entry.clone())
    }

    /// Get the update interval
    ///
    /// Returns the configured GUI refresh interval.
    ///
    /// # Returns
    /// The Duration representing the update interval
    pub fn update_interval(&self) -> Duration {
        self.update_interval
    }

    /// Get the number of active subscribers
    ///
    /// Returns the current number of active broadcast receivers.
    ///
    /// # Returns
    /// Number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.price_tx.receiver_count()
    }

    /// Get the number of cached prices
    ///
    /// Returns the size of the price cache.
    ///
    /// # Returns
    /// Number of tokens with cached prices
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Clear the price cache
    ///
    /// Removes all cached prices. Useful for testing or reset scenarios.
    pub fn clear_cache(&self) {
        self.cache.clear();
    }
}

impl Default for PriceStreamManager {
    fn default() -> Self {
        Self::new(1000, Duration::from_millis(333))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::time::sleep;

    fn create_test_price_update(mint: Pubkey, price: f64) -> PriceUpdate {
        PriceUpdate {
            mint,
            price_sol: price,
            price_usd: price * 150.0, // Assume SOL = $150
            volume_24h: 100_000.0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            source: "test".to_string(),
        }
    }

    #[test]
    fn test_price_stream_manager_creation() {
        let manager = PriceStreamManager::new(100, Duration::from_millis(333));

        assert_eq!(manager.update_interval(), Duration::from_millis(333));
        assert_eq!(manager.cache_size(), 0);
        assert_eq!(manager.subscriber_count(), 0);
    }

    #[test]
    fn test_default_creation() {
        let manager = PriceStreamManager::default();

        assert_eq!(manager.update_interval(), Duration::from_millis(333));
        assert_eq!(manager.cache_size(), 0);
    }

    #[test]
    fn test_publish_and_cache() {
        let manager = PriceStreamManager::new(100, Duration::from_millis(333));
        let mint = Pubkey::new_unique();
        let update = create_test_price_update(mint, 0.01);

        // Publish price
        manager.publish_price(update.clone());

        // Should be cached
        assert_eq!(manager.cache_size(), 1);

        // Should be retrievable
        let cached = manager.get_cached_price(&mint).unwrap();
        assert_eq!(cached.mint, mint);
        assert_eq!(cached.price_sol, 0.01);
        assert_eq!(cached.source, "test");
    }

    #[test]
    fn test_cache_update() {
        let manager = PriceStreamManager::new(100, Duration::from_millis(333));
        let mint = Pubkey::new_unique();

        // Publish initial price
        let update1 = create_test_price_update(mint, 0.01);
        manager.publish_price(update1);

        // Publish updated price
        let update2 = create_test_price_update(mint, 0.02);
        manager.publish_price(update2);

        // Cache should still have 1 entry (same mint)
        assert_eq!(manager.cache_size(), 1);

        // Should have the updated price
        let cached = manager.get_cached_price(&mint).unwrap();
        assert_eq!(cached.price_sol, 0.02);
    }

    #[test]
    fn test_multiple_tokens() {
        let manager = PriceStreamManager::new(100, Duration::from_millis(333));

        // Publish prices for multiple tokens
        for i in 0..10 {
            let mint = Pubkey::new_unique();
            let update = create_test_price_update(mint, 0.01 * (i as f64 + 1.0));
            manager.publish_price(update);
        }

        assert_eq!(manager.cache_size(), 10);
    }

    #[test]
    fn test_clear_cache() {
        let manager = PriceStreamManager::new(100, Duration::from_millis(333));
        let mint = Pubkey::new_unique();
        let update = create_test_price_update(mint, 0.01);

        manager.publish_price(update);
        assert_eq!(manager.cache_size(), 1);

        manager.clear_cache();
        assert_eq!(manager.cache_size(), 0);
        assert!(manager.get_cached_price(&mint).is_none());
    }

    #[tokio::test]
    async fn test_subscribe_and_receive() {
        let manager = PriceStreamManager::new(100, Duration::from_millis(333));
        let mut rx = manager.subscribe();

        assert_eq!(manager.subscriber_count(), 1);

        let mint = Pubkey::new_unique();
        let update = create_test_price_update(mint, 0.01);

        // Publish price
        manager.publish_price(update.clone());

        // Should receive it
        let received = rx.recv().await.unwrap();
        assert_eq!(received.mint, mint);
        assert_eq!(received.price_sol, 0.01);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let manager = PriceStreamManager::new(100, Duration::from_millis(333));

        let mut rx1 = manager.subscribe();
        let mut rx2 = manager.subscribe();
        let mut rx3 = manager.subscribe();

        assert_eq!(manager.subscriber_count(), 3);

        let mint = Pubkey::new_unique();
        let update = create_test_price_update(mint, 0.01);

        manager.publish_price(update);

        // All subscribers should receive
        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();
        let received3 = rx3.recv().await.unwrap();

        assert_eq!(received1.mint, mint);
        assert_eq!(received2.mint, mint);
        assert_eq!(received3.mint, mint);
    }

    #[tokio::test]
    async fn test_concurrent_publish() {
        use std::sync::Arc;

        let manager = Arc::new(PriceStreamManager::new(1000, Duration::from_millis(333)));
        let mut handles = vec![];

        // Spawn 10 tasks, each publishing 100 updates
        for _ in 0..10 {
            let manager_clone = Arc::clone(&manager);
            let handle = tokio::spawn(async move {
                for _ in 0..100 {
                    let mint = Pubkey::new_unique();
                    let update = create_test_price_update(mint, 0.01);
                    manager_clone.publish_price(update);
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Should have 1000 unique prices cached
        assert_eq!(manager.cache_size(), 1000);
    }

    #[tokio::test]
    async fn test_subscribe_receive_latency() {
        let manager = PriceStreamManager::new(1000, Duration::from_millis(333));
        let mut rx = manager.subscribe();

        let mint = Pubkey::new_unique();
        let update = create_test_price_update(mint, 0.01);

        let start = std::time::Instant::now();
        manager.publish_price(update);

        let received = rx.recv().await.unwrap();
        let latency = start.elapsed();

        // Latency should be very low (< 1ms for local broadcast)
        assert!(
            latency < Duration::from_millis(10),
            "Latency too high: {:?}",
            latency
        );
        assert_eq!(received.mint, mint);
    }

    #[tokio::test]
    async fn test_no_blocking_on_slow_subscriber() {
        let manager = Arc::new(PriceStreamManager::new(10, Duration::from_millis(333)));

        // Create a slow subscriber that doesn't read
        let _slow_rx = manager.subscribe();

        // Publish many updates quickly
        for i in 0..100 {
            let mint = Pubkey::new_unique();
            let update = create_test_price_update(mint, 0.01 * i as f64);
            manager.publish_price(update);
        }

        // All updates should be cached despite slow subscriber
        assert_eq!(manager.cache_size(), 100);
    }

    #[test]
    fn test_get_cached_price_nonexistent() {
        let manager = PriceStreamManager::new(100, Duration::from_millis(333));
        let mint = Pubkey::new_unique();

        // Should return None for non-existent token
        assert!(manager.get_cached_price(&mint).is_none());
    }

    #[tokio::test]
    async fn test_dropped_subscriber() {
        let manager = PriceStreamManager::new(100, Duration::from_millis(333));

        {
            let _rx = manager.subscribe();
            assert_eq!(manager.subscriber_count(), 1);
        } // rx dropped here

        // Give tokio time to clean up
        sleep(Duration::from_millis(10)).await;

        // Subscriber count should be 0
        assert_eq!(manager.subscriber_count(), 0);
    }
}
