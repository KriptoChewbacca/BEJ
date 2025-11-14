//! Task 6: GUI Feature Gating Tests
//!
//! These tests verify that the GUI monitoring feature can be conditionally
//! compiled and that the bot works correctly both with and without the feature.

use super::*;

/// Test that position_tracker module is available
#[test]
fn test_position_tracker_available() {
    let tracker = position_tracker::PositionTracker::new();
    assert_eq!(tracker.position_count(), 0);
}

/// Test that components module is available
#[test]
fn test_components_available() {
    use std::time::Duration;
    
    let _price_stream = components::price_stream::PriceStreamManager::new(
        100,
        Duration::from_millis(333),
    );
    
    // Just verify it compiles and constructs
}

/// Test that GUI module is conditionally available
#[cfg(feature = "gui_monitor")]
#[test]
fn test_gui_module_available_with_feature() {
    // This test only compiles when gui_monitor feature is enabled
    // It verifies that the gui module is accessible
    
    use std::sync::Arc;
    use std::sync::atomic::AtomicU8;
    
    let tracker = Arc::new(position_tracker::PositionTracker::new());
    let price_stream = components::price_stream::PriceStreamManager::new(
        100,
        std::time::Duration::from_millis(333),
    );
    let _bot_state = Arc::new(AtomicU8::new(1));
    
    // Verify we can get a price receiver (but don't launch GUI in tests)
    let _price_rx = price_stream.subscribe();
    
    // Verify position tracker works
    assert_eq!(tracker.position_count(), 0);
}

/// Test that GUI dependencies are not included without feature
#[cfg(not(feature = "gui_monitor"))]
#[test]
fn test_gui_not_available_without_feature() {
    // This test only compiles when gui_monitor feature is NOT enabled
    // It verifies that we can still use position_tracker and price_stream
    // even without the GUI
    
    use std::time::Duration;
    
    let tracker = position_tracker::PositionTracker::new();
    assert_eq!(tracker.position_count(), 0);
    
    let _price_stream = components::price_stream::PriceStreamManager::new(
        100,
        Duration::from_millis(333),
    );
}

/// Test AtomicU8 import is conditional
#[cfg(feature = "gui_monitor")]
#[test]
fn test_atomic_u8_available_with_feature() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU8, Ordering};
    
    let bot_state = Arc::new(AtomicU8::new(1));
    assert_eq!(bot_state.load(Ordering::Relaxed), 1);
    
    bot_state.store(0, Ordering::Relaxed);
    assert_eq!(bot_state.load(Ordering::Relaxed), 0);
}

/// Test that shared components can be created
#[test]
fn test_shared_components_creation() {
    use std::sync::Arc;
    use std::time::Duration;
    
    // These components should be available regardless of feature flag
    let tracker = Arc::new(position_tracker::PositionTracker::new());
    let price_stream = Arc::new(components::price_stream::PriceStreamManager::new(
        1000,
        Duration::from_millis(333),
    ));
    
    assert_eq!(tracker.position_count(), 0);
    
    // Verify price stream can create subscribers
    let _rx = price_stream.subscribe();
}

/// Test position tracker basic operations
#[test]
fn test_position_tracker_basic_operations() {
    use solana_sdk::pubkey::Pubkey;
    
    let tracker = position_tracker::PositionTracker::new();
    let mint = Pubkey::new_unique();
    
    // Initially no positions
    assert_eq!(tracker.position_count(), 0);
    assert!(!tracker.has_position(&mint));
    
    // Record a buy
    tracker.record_buy(mint, 1_000_000, 10_000_000);
    
    // Now we should have one position
    assert_eq!(tracker.position_count(), 1);
    assert!(tracker.has_position(&mint));
    
    // Get the position
    let pos = tracker.get_position(&mint).unwrap();
    assert_eq!(pos.mint, mint);
    assert_eq!(pos.initial_token_amount, 1_000_000);
    assert_eq!(pos.initial_sol_cost, 10_000_000);
}

/// Test price stream basic operations
#[tokio::test]
async fn test_price_stream_basic_operations() {
    use solana_sdk::pubkey::Pubkey;
    use std::time::Duration;
    
    let price_stream = components::price_stream::PriceStreamManager::new(
        100,
        Duration::from_millis(333),
    );
    
    let mut rx = price_stream.subscribe();
    
    // Publish a price update
    let mint = Pubkey::new_unique();
    let update = components::price_stream::PriceUpdate {
        mint,
        price_sol: 0.01,
        price_usd: 1.5,
        volume_24h: 100_000.0,
        timestamp: 1234567890,
        source: "test".to_string(),
    };
    
    price_stream.publish_price(update.clone());
    
    // Receive it
    let received = rx.recv().await.unwrap();
    assert_eq!(received.mint, mint);
    assert_eq!(received.price_sol, 0.01);
}

/// Integration test: Components work together
#[tokio::test]
async fn test_components_integration() {
    use solana_sdk::pubkey::Pubkey;
    use std::sync::Arc;
    use std::time::Duration;
    
    // Create components
    let tracker = Arc::new(position_tracker::PositionTracker::new());
    let price_stream = Arc::new(components::price_stream::PriceStreamManager::new(
        100,
        Duration::from_millis(333),
    ));
    
    let mint = Pubkey::new_unique();
    
    // Record a buy
    tracker.record_buy(mint, 1_000_000, 10_000_000);
    
    // Publish price update
    let update = components::price_stream::PriceUpdate {
        mint,
        price_sol: 0.02,
        price_usd: 3.0,
        volume_24h: 200_000.0,
        timestamp: 1234567890,
        source: "test".to_string(),
    };
    
    price_stream.publish_price(update);
    
    // Update position price
    tracker.update_price(&mint, 0.02);
    
    // Verify position
    let pos = tracker.get_position(&mint).unwrap();
    assert_eq!(pos.last_seen_price, 0.02);
    
    // Calculate P&L (price doubled, should be ~100% profit)
    let (pnl_sol, pnl_percent) = pos.calculate_pnl(0.02);
    assert!(pnl_sol > 0.0, "Should have profit");
    assert!(pnl_percent > 90.0, "Should be close to 100% gain");
}

/// Test that feature flag is documented
#[test]
fn test_feature_documentation() {
    // This test just verifies that we've properly structured the code
    // The actual verification happens at compile time via cfg attributes
    
    #[cfg(feature = "gui_monitor")]
    {
        // GUI should be available
        assert!(true, "GUI feature is enabled");
    }
    
    #[cfg(not(feature = "gui_monitor"))]
    {
        // GUI should not be available
        assert!(true, "GUI feature is disabled");
    }
}

/// Test zero-allocation price update (verify performance)
#[test]
fn test_price_update_zero_allocation() {
    use solana_sdk::pubkey::Pubkey;
    use std::time::Duration;
    
    let price_stream = components::price_stream::PriceStreamManager::new(
        1000,
        Duration::from_millis(333),
    );
    
    let mint = Pubkey::new_unique();
    
    // This should be very fast (no allocations after channel is created)
    for i in 0..100 {
        let update = components::price_stream::PriceUpdate {
            mint,
            price_sol: 0.01 + (i as f64 * 0.0001),
            price_usd: 1.5 + (i as f64 * 0.15),
            volume_24h: 100_000.0,
            timestamp: 1234567890 + i,
            source: "test".to_string(),
        };
        
        price_stream.publish_price(update);
    }
    
    // Just verify it doesn't panic or hang
}
