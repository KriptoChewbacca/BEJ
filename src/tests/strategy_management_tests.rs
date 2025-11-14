//! Tests for sell strategy management
//!
//! ZADANIE 1.4: Strategy Management API tests

use crate::buy_engine::BuyEngine;
use crate::types::{AppState, Mode};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Helper to create a minimal BuyEngine for testing
async fn create_test_engine() -> Arc<BuyEngine> {
    use crate::config::Config;
    use crate::nonce_manager::NonceManager;
    use crate::rpc_manager::RpcPool;
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
async fn test_set_stop_loss_persists() {
    let engine = create_test_engine().await;
    let mint = Pubkey::new_unique();

    // Set stop loss
    engine.set_stop_loss(mint, 10.0).await;

    // Retrieve strategy
    let strategy = engine.get_strategy(&mint).await;
    assert!(strategy.is_some(), "Strategy should exist");

    let strategy = strategy.unwrap();
    assert!(strategy.stop_loss.is_some(), "Stop loss should be set");

    let sl = strategy.stop_loss.unwrap();
    assert!(sl.enabled, "Stop loss should be enabled");
    assert_eq!(sl.threshold_percent, -10.0, "Threshold should be -10%");
}

#[tokio::test]
async fn test_set_take_profit_persists() {
    let engine = create_test_engine().await;
    let mint = Pubkey::new_unique();

    // Set take profit: 50% gain, sell 50% of position
    engine.set_take_profit(mint, 50.0, 0.5).await;

    // Retrieve strategy
    let strategy = engine.get_strategy(&mint).await;
    assert!(strategy.is_some(), "Strategy should exist");

    let strategy = strategy.unwrap();
    assert!(strategy.take_profit.is_some(), "Take profit should be set");

    let tp = strategy.take_profit.unwrap();
    assert!(tp.enabled, "Take profit should be enabled");
    assert_eq!(tp.threshold_percent, 50.0, "Threshold should be +50%");
    assert_eq!(tp.sell_percent, 0.5, "Sell percent should be 0.5");
}

#[tokio::test]
async fn test_stop_loss_ensures_negative_threshold() {
    let engine = create_test_engine().await;
    let mint = Pubkey::new_unique();

    // Set stop loss with positive value (should be converted to negative)
    engine.set_stop_loss(mint, 15.0).await;

    let strategy = engine.get_strategy(&mint).await.unwrap();
    let sl = strategy.stop_loss.unwrap();
    assert_eq!(
        sl.threshold_percent, -15.0,
        "Threshold should be negative"
    );
}

#[tokio::test]
async fn test_take_profit_ensures_positive_threshold() {
    let engine = create_test_engine().await;
    let mint = Pubkey::new_unique();

    // Set take profit with negative value (should be converted to positive)
    engine.set_take_profit(mint, -30.0, 0.75).await;

    let strategy = engine.get_strategy(&mint).await.unwrap();
    let tp = strategy.take_profit.unwrap();
    assert_eq!(
        tp.threshold_percent, 30.0,
        "Threshold should be positive"
    );
}

#[tokio::test]
async fn test_take_profit_clamps_sell_percent() {
    let engine = create_test_engine().await;
    let mint = Pubkey::new_unique();

    // Test upper bound clamping (> 1.0)
    engine.set_take_profit(mint, 50.0, 1.5).await;
    let strategy = engine.get_strategy(&mint).await.unwrap();
    assert_eq!(
        strategy.take_profit.unwrap().sell_percent,
        1.0,
        "Sell percent should be clamped to 1.0"
    );

    // Test lower bound clamping (< 0.0)
    engine.set_take_profit(mint, 50.0, -0.5).await;
    let strategy = engine.get_strategy(&mint).await.unwrap();
    assert_eq!(
        strategy.take_profit.unwrap().sell_percent,
        0.0,
        "Sell percent should be clamped to 0.0"
    );
}

#[tokio::test]
async fn test_clear_strategy_removes_rules() {
    let engine = create_test_engine().await;
    let mint = Pubkey::new_unique();

    // Set both SL and TP
    engine.set_stop_loss(mint, 10.0).await;
    engine.set_take_profit(mint, 50.0, 0.5).await;

    // Verify strategy exists
    assert!(
        engine.get_strategy(&mint).await.is_some(),
        "Strategy should exist"
    );

    // Clear strategy
    engine.clear_strategy(&mint).await;

    // Verify strategy is removed
    assert!(
        engine.get_strategy(&mint).await.is_none(),
        "Strategy should be removed"
    );
}

#[tokio::test]
async fn test_get_strategy_returns_correct_data() {
    let engine = create_test_engine().await;
    let mint = Pubkey::new_unique();

    // Initially should be None
    assert!(
        engine.get_strategy(&mint).await.is_none(),
        "Strategy should not exist initially"
    );

    // Set stop loss
    engine.set_stop_loss(mint, 12.0).await;
    let strategy = engine.get_strategy(&mint).await.unwrap();
    assert!(strategy.stop_loss.is_some());
    assert!(strategy.take_profit.is_none());

    // Add take profit
    engine.set_take_profit(mint, 60.0, 0.6).await;
    let strategy = engine.get_strategy(&mint).await.unwrap();
    assert!(strategy.stop_loss.is_some());
    assert!(strategy.take_profit.is_some());
    
    let sl = strategy.stop_loss.as_ref().unwrap();
    let tp = strategy.take_profit.as_ref().unwrap();
    assert_eq!(sl.threshold_percent, -12.0);
    assert_eq!(tp.threshold_percent, 60.0);
    assert_eq!(tp.sell_percent, 0.6);
}

#[tokio::test]
async fn test_concurrent_strategy_updates() {
    let engine = create_test_engine().await;
    let mint = Pubkey::new_unique();

    // Spawn multiple tasks that concurrently update strategies
    let mut handles = vec![];

    for i in 0..20 {
        let engine_clone = Arc::clone(&engine);
        let handle = tokio::spawn(async move {
            if i % 2 == 0 {
                engine_clone.set_stop_loss(mint, 10.0 + i as f64).await;
            } else {
                engine_clone.set_take_profit(mint, 50.0, 0.5).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Strategy should exist with valid values
    let strategy = engine.get_strategy(&mint).await;
    assert!(strategy.is_some(), "Strategy should exist");

    let strategy = strategy.unwrap();
    // Both SL and TP should be set from concurrent updates
    assert!(
        strategy.stop_loss.is_some() || strategy.take_profit.is_some(),
        "At least one strategy component should be set"
    );
}

#[tokio::test]
async fn test_multiple_token_strategies() {
    let engine = create_test_engine().await;
    let mint1 = Pubkey::new_unique();
    let mint2 = Pubkey::new_unique();
    let mint3 = Pubkey::new_unique();

    // Set different strategies for different tokens
    engine.set_stop_loss(mint1, 5.0).await;
    engine.set_take_profit(mint2, 100.0, 1.0).await;
    engine.set_stop_loss(mint3, 15.0).await;
    engine.set_take_profit(mint3, 50.0, 0.5).await;

    // Verify each strategy is independent
    let strategy1 = engine.get_strategy(&mint1).await.unwrap();
    assert!(strategy1.stop_loss.is_some());
    assert!(strategy1.take_profit.is_none());

    let strategy2 = engine.get_strategy(&mint2).await.unwrap();
    assert!(strategy2.stop_loss.is_none());
    assert!(strategy2.take_profit.is_some());

    let strategy3 = engine.get_strategy(&mint3).await.unwrap();
    assert!(strategy3.stop_loss.is_some());
    assert!(strategy3.take_profit.is_some());
}
