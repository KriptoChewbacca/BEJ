//! Tests for trading mode management
//!
//! ZADANIE 1.1: Trading mode infrastructure tests

use crate::buy_engine::BuyEngine;
use crate::types::{AppState, Mode, TradingMode};
use std::sync::Arc;
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
async fn test_default_mode_is_hybrid() {
    let engine = create_test_engine().await;
    let mode = engine.get_trading_mode().await;
    assert_eq!(mode, TradingMode::Hybrid, "Default mode should be Hybrid");
}

#[tokio::test]
async fn test_mode_change_persists() {
    let engine = create_test_engine().await;

    // Change to Auto
    engine.set_trading_mode(TradingMode::Auto).await;
    let mode = engine.get_trading_mode().await;
    assert_eq!(mode, TradingMode::Auto);

    // Change to Manual
    engine.set_trading_mode(TradingMode::Manual).await;
    let mode = engine.get_trading_mode().await;
    assert_eq!(mode, TradingMode::Manual);

    // Change back to Hybrid
    engine.set_trading_mode(TradingMode::Hybrid).await;
    let mode = engine.get_trading_mode().await;
    assert_eq!(mode, TradingMode::Hybrid);
}

#[tokio::test]
async fn test_concurrent_mode_access() {
    let engine = create_test_engine().await;

    // Spawn multiple tasks that concurrently read the mode
    let mut handles = vec![];
    for _ in 0..10 {
        let engine_clone = Arc::clone(&engine);
        let handle = tokio::spawn(async move {
            let _mode = engine_clone.get_trading_mode().await;
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Mode should still be accessible
    let mode = engine.get_trading_mode().await;
    assert_eq!(mode, TradingMode::Hybrid);
}

#[tokio::test]
async fn test_mode_changes_are_thread_safe() {
    let engine = create_test_engine().await;

    // Spawn multiple tasks that concurrently change the mode
    let mut handles = vec![];
    
    for i in 0..20 {
        let engine_clone = Arc::clone(&engine);
        let handle = tokio::spawn(async move {
            let mode = match i % 3 {
                0 => TradingMode::Auto,
                1 => TradingMode::Manual,
                _ => TradingMode::Hybrid,
            };
            engine_clone.set_trading_mode(mode).await;
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Mode should be one of the valid modes (any is fine, just checking no panic)
    let mode = engine.get_trading_mode().await;
    assert!(
        matches!(mode, TradingMode::Auto | TradingMode::Manual | TradingMode::Hybrid),
        "Mode should be a valid TradingMode variant"
    );
}
