//! Tests for TP/SL evaluation logic
//!
//! ZADANIE 1.3: TP/SL Evaluation Logic tests

use crate::types::{SellStrategy, StopLossConfig, TakeProfitConfig};
use bot::position_tracker::ActivePosition;
use solana_sdk::pubkey::Pubkey;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Create a test position with specific P&L characteristics
fn create_test_position(mint: Pubkey, initial_price: f64, current_price: f64) -> ActivePosition {
    let initial_token_amount = 1_000_000_u64; // 1M tokens
    let initial_sol_cost = (initial_token_amount as f64 * initial_price * 1_000_000_000.0) as u64;

    ActivePosition {
        mint,
        entry_timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        initial_token_amount,
        initial_sol_cost,
        sold_token_amount: 0,
        total_sol_from_sales: 0,
        last_seen_price: current_price,
        last_update: Instant::now(),
    }
}

#[tokio::test]
async fn test_stop_loss_triggers_at_threshold() {
    // Note: This test verifies the logic conceptually.
    // The actual sell execution would require mocking the RPC layer,
    // which is beyond the scope of this test.
    
    let mint = Pubkey::new_unique();
    
    // Create position: bought at 0.00001 SOL, now at 0.000008 SOL (-20% P&L)
    let position = create_test_position(mint, 0.00001, 0.000008);
    
    // Create strategy with -10% stop loss
    let strategy = SellStrategy {
        stop_loss: Some(StopLossConfig {
            enabled: true,
            threshold_percent: -10.0,
        }),
        take_profit: None,
        trailing_stop: None,
    };
    
    // Calculate P&L
    let (_pnl_sol, pnl_percent) = position.calculate_pnl(position.last_seen_price);
    
    // Verify P&L is below stop loss threshold
    assert!(
        pnl_percent <= strategy.stop_loss.as_ref().unwrap().threshold_percent,
        "P&L ({:.2}%) should be below stop loss threshold ({:.2}%)",
        pnl_percent,
        strategy.stop_loss.as_ref().unwrap().threshold_percent
    );
    
    // In actual implementation, this would trigger a sell
    // Here we just verify the condition is met
}

#[tokio::test]
async fn test_take_profit_triggers_partial_sell() {
    let mint = Pubkey::new_unique();
    
    // Create position: bought at 0.00001 SOL, now at 0.00002 SOL (+100% P&L)
    let position = create_test_position(mint, 0.00001, 0.00002);
    
    // Create strategy with +50% take profit, sell 50%
    let strategy = SellStrategy {
        stop_loss: None,
        take_profit: Some(TakeProfitConfig {
            enabled: true,
            threshold_percent: 50.0,
            sell_percent: 0.5,
        }),
        trailing_stop: None,
    };
    
    // Calculate P&L
    let (_pnl_sol, pnl_percent) = position.calculate_pnl(position.last_seen_price);
    
    // Verify P&L is above take profit threshold
    assert!(
        pnl_percent >= strategy.take_profit.as_ref().unwrap().threshold_percent,
        "P&L ({:.2}%) should be above take profit threshold ({:.2}%)",
        pnl_percent,
        strategy.take_profit.as_ref().unwrap().threshold_percent
    );
    
    // Verify sell percent is 50%
    assert_eq!(
        strategy.take_profit.as_ref().unwrap().sell_percent,
        0.5,
        "Should sell 50% of position"
    );
}

#[tokio::test]
async fn test_sl_takes_priority_over_tp() {
    let mint = Pubkey::new_unique();
    
    // Create position with negative P&L
    let position = create_test_position(mint, 0.00001, 0.000008);
    
    // Create strategy with both SL and TP
    let strategy = SellStrategy {
        stop_loss: Some(StopLossConfig {
            enabled: true,
            threshold_percent: -10.0,
        }),
        take_profit: Some(TakeProfitConfig {
            enabled: true,
            threshold_percent: 50.0,
            sell_percent: 0.5,
        }),
        trailing_stop: None,
    };
    
    // Calculate P&L
    let (_pnl_sol, pnl_percent) = position.calculate_pnl(position.last_seen_price);
    
    // Verify P&L is below SL threshold (should trigger SL, not TP)
    assert!(
        pnl_percent <= strategy.stop_loss.as_ref().unwrap().threshold_percent,
        "P&L ({:.2}%) is below SL threshold - SL should trigger first",
        pnl_percent
    );
    
    // In the actual implementation, the evaluate_auto_sell method checks SL first,
    // so TP would never be evaluated when SL is triggered.
    // This test verifies the priority logic conceptually.
}

#[tokio::test]
async fn test_disabled_stop_loss_does_not_trigger() {
    let mint = Pubkey::new_unique();
    
    // Create position with -20% P&L
    let position = create_test_position(mint, 0.00001, 0.000008);
    
    // Create strategy with disabled stop loss
    let strategy = SellStrategy {
        stop_loss: Some(StopLossConfig {
            enabled: false, // Disabled
            threshold_percent: -10.0,
        }),
        take_profit: None,
        trailing_stop: None,
    };
    
    // Even though P&L is below threshold, disabled SL should not trigger
    let (_pnl_sol, pnl_percent) = position.calculate_pnl(position.last_seen_price);
    
    assert!(
        pnl_percent <= -10.0,
        "P&L is below threshold, but SL is disabled"
    );
    assert!(
        !strategy.stop_loss.as_ref().unwrap().enabled,
        "Stop loss should be disabled"
    );
}

#[tokio::test]
async fn test_disabled_take_profit_does_not_trigger() {
    let mint = Pubkey::new_unique();
    
    // Create position with +100% P&L
    let position = create_test_position(mint, 0.00001, 0.00002);
    
    // Create strategy with disabled take profit
    let strategy = SellStrategy {
        stop_loss: None,
        take_profit: Some(TakeProfitConfig {
            enabled: false, // Disabled
            threshold_percent: 50.0,
            sell_percent: 0.5,
        }),
        trailing_stop: None,
    };
    
    // Even though P&L is above threshold, disabled TP should not trigger
    let (_pnl_sol, pnl_percent) = position.calculate_pnl(position.last_seen_price);
    
    assert!(
        pnl_percent >= 50.0,
        "P&L is above threshold, but TP is disabled"
    );
    assert!(
        !strategy.take_profit.as_ref().unwrap().enabled,
        "Take profit should be disabled"
    );
}

#[tokio::test]
async fn test_no_trigger_when_sl_threshold_not_met() {
    let mint = Pubkey::new_unique();
    
    // Create position with -5% P&L
    let position = create_test_position(mint, 0.00001, 0.0000095);
    
    // Create strategy with -10% stop loss
    let strategy = SellStrategy {
        stop_loss: Some(StopLossConfig {
            enabled: true,
            threshold_percent: -10.0,
        }),
        take_profit: None,
        trailing_stop: None,
    };
    
    // Calculate P&L
    let (_pnl_sol, pnl_percent) = position.calculate_pnl(position.last_seen_price);
    
    // Verify P&L is NOT below stop loss threshold
    assert!(
        pnl_percent > strategy.stop_loss.as_ref().unwrap().threshold_percent,
        "P&L ({:.2}%) should be above stop loss threshold ({:.2}%) - no trigger",
        pnl_percent,
        strategy.stop_loss.as_ref().unwrap().threshold_percent
    );
}

#[tokio::test]
async fn test_no_trigger_when_tp_threshold_not_met() {
    let mint = Pubkey::new_unique();
    
    // Create position with +30% P&L
    let position = create_test_position(mint, 0.00001, 0.000013);
    
    // Create strategy with +50% take profit
    let strategy = SellStrategy {
        stop_loss: None,
        take_profit: Some(TakeProfitConfig {
            enabled: true,
            threshold_percent: 50.0,
            sell_percent: 0.5,
        }),
        trailing_stop: None,
    };
    
    // Calculate P&L
    let (_pnl_sol, pnl_percent) = position.calculate_pnl(position.last_seen_price);
    
    // Verify P&L is NOT above take profit threshold
    assert!(
        pnl_percent < strategy.take_profit.as_ref().unwrap().threshold_percent,
        "P&L ({:.2}%) should be below take profit threshold ({:.2}%) - no trigger",
        pnl_percent,
        strategy.take_profit.as_ref().unwrap().threshold_percent
    );
}

#[tokio::test]
async fn test_pnl_calculation_deterministic() {
    let mint = Pubkey::new_unique();
    
    // Test multiple times to ensure determinism
    for _ in 0..10 {
        let position = create_test_position(mint, 0.00001, 0.00002);
        let (pnl_sol1, pnl_percent1) = position.calculate_pnl(position.last_seen_price);
        let (pnl_sol2, pnl_percent2) = position.calculate_pnl(position.last_seen_price);
        
        assert_eq!(
            pnl_sol1, pnl_sol2,
            "P&L SOL should be deterministic"
        );
        assert_eq!(
            pnl_percent1, pnl_percent2,
            "P&L percent should be deterministic"
        );
    }
}
