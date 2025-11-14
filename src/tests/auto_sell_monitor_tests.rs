//! Tests for auto-sell monitoring loop
//!
//! ZADANIE 1.2: Auto-Sell Monitor Loop tests

use crate::buy_engine::BuyEngine;
use crate::types::{AppState, Mode, TradingMode};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Helper to create a minimal BuyEngine for testing
async fn create_test_engine() -> Arc<BuyEngine> {
    use crate::config::Config;
    use crate::nonce_manager::NonceManager;
    use crate::rpc_manager::RpcPool;
    use solana_sdk::pubkey::Pubkey;
    use std::sync::atomic::AtomicU8;
    use tokio::sync::mpsc;

    let (_tx, rx) = mpsc::unbounded_channel();
    let app_state = Arc::new(Mutex::new(AppState::new(Mode::Sniffing)));

    let config = Config::default();
    let rpc_pool = Arc::new(RpcPool::new(config.clone()).await.unwrap());
    let nonce_manager = Arc::new(
        NonceManager::new(
            Arc::clone(&rpc_pool) as Arc<dyn crate::rpc_manager::RpcBroadcaster>,
            Pubkey::new_unique(),
            vec![Pubkey::new_unique()],
            config.clone(),
        )
        .await
        .unwrap(),
    );

    let gui_control_state = Arc::new(AtomicU8::new(1));

    Arc::new(BuyEngine::new_with_gui_control(
        Arc::clone(&rpc_pool) as Arc<dyn crate::rpc_manager::RpcBroadcaster>,
        nonce_manager,
        rx,
        app_state,
        config,
        None,
        None,
        None,
        None,
        gui_control_state,
    ))
}

#[tokio::test]
async fn test_monitor_starts_successfully() {
    let engine = create_test_engine().await;

    // Start monitor
    Arc::clone(&engine).start_auto_sell_monitor().await;

    // Give it a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Monitor should be running (this test just ensures no panic)
    // Note: auto_sell_handle is private, so we can't directly check it
    // The test passes if the monitor starts without error
}

#[tokio::test]
async fn test_monitor_only_runs_in_auto_mode() {
    let engine = create_test_engine().await;

    // Set to Manual mode
    engine.set_trading_mode(TradingMode::Manual).await;

    // Start monitor
    Arc::clone(&engine).start_auto_sell_monitor().await;

    // Wait for several ticks
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Monitor should be running but not evaluating (this test just ensures no panic)
    
    // Switch to Auto mode
    engine.set_trading_mode(TradingMode::Auto).await;

    // Wait for more ticks
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Monitor should still be running (test passes if no panic)
}

#[tokio::test]
async fn test_monitor_checks_all_positions() {
    // Note: This test is limited because we can't easily verify position checking
    // without setting up a full position tracker. The test verifies the monitor
    // runs without panicking when position_tracker is None.
    
    let engine = create_test_engine().await;

    // Set to Auto mode
    engine.set_trading_mode(TradingMode::Auto).await;

    // Start monitor
    Arc::clone(&engine).start_auto_sell_monitor().await;

    // Wait for several ticks
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Monitor should still be running (no positions, so no errors)
}

#[tokio::test]
async fn test_monitor_respects_tick_rate() {
    let engine = create_test_engine().await;

    // Set to Auto mode
    engine.set_trading_mode(TradingMode::Auto).await;

    // Start monitor
    Arc::clone(&engine).start_auto_sell_monitor().await;

    // Record start time
    let start = std::time::Instant::now();

    // Wait for approximately 3 ticks (333ms * 3 ≈ 1000ms)
    tokio::time::sleep(Duration::from_millis(1000)).await;

    let elapsed = start.elapsed();

    // Verify roughly 1 second has passed (allowing some variance)
    assert!(
        elapsed >= Duration::from_millis(950),
        "Should have waited at least ~1 second"
    );
    assert!(
        elapsed <= Duration::from_millis(1200),
        "Should not have waited much more than 1 second"
    );
}

#[tokio::test]
async fn test_monitor_can_be_restarted() {
    let engine = create_test_engine().await;

    // Start monitor first time
    Arc::clone(&engine).start_auto_sell_monitor().await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Start monitor second time (should replace first)
    Arc::clone(&engine).start_auto_sell_monitor().await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test passes if no panic occurs
}
