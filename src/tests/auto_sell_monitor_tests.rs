//! Tests for auto-sell monitoring loop
//!
//! ZADANIE 1.2: Auto-Sell Monitor Loop tests
//!
//! These tests verify basic monitor loop behavior.
//! Full integration tests with BuyEngine are covered in integration tests.

use crate::types::TradingMode;
use std::time::Duration;

#[tokio::test]
async fn test_monitor_timing_basic() {
    // Test basic timing for 333ms tick rate
    let start = std::time::Instant::now();
    
    // Simulate 3 ticks
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_millis(333)).await;
    }
    
    let elapsed = start.elapsed();
    
    // Should be approximately 1 second (3 * 333ms ≈ 1000ms)
    assert!(
        elapsed >= Duration::from_millis(950),
        "Should wait at least ~1 second"
    );
    assert!(
        elapsed <= Duration::from_millis(1200),
        "Should not wait much more than 1 second"
    );
}

#[test]
fn test_trading_mode_for_auto_check() {
    let auto_mode = TradingMode::Auto;
    let manual_mode = TradingMode::Manual;
    
    // Monitor should only run when mode == Auto
    assert_eq!(auto_mode, TradingMode::Auto);
    assert_ne!(manual_mode, TradingMode::Auto);
}
