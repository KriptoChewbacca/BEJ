//! Tests for sell strategy management
//!
//! ZADANIE 1.4: Strategy Management API tests
//!
//! These tests verify the SellStrategy, StopLossConfig, and TakeProfitConfig types.
//! Full integration tests with BuyEngine are covered in integration tests.

use crate::types::{SellStrategy, StopLossConfig, TakeProfitConfig};

#[test]
fn test_sell_strategy_default() {
    let strategy = SellStrategy::default();
    assert!(strategy.stop_loss.is_none());
    assert!(strategy.take_profit.is_none());
}

#[test]
fn test_stop_loss_config_default() {
    let sl = StopLossConfig::default();
    assert_eq!(sl.enabled, false);
    assert_eq!(sl.threshold_percent, -10.0);
}

#[test]
fn test_take_profit_config_default() {
    let tp = TakeProfitConfig::default();
    assert_eq!(tp.enabled, false);
    assert_eq!(tp.threshold_percent, 50.0);
    assert_eq!(tp.sell_percent, 0.5);
}

#[test]
fn test_stop_loss_enabled() {
    let sl = StopLossConfig {
        enabled: true,
        threshold_percent: -15.0,
    };
    assert!(sl.enabled);
    assert_eq!(sl.threshold_percent, -15.0);
}

#[test]
fn test_take_profit_enabled() {
    let tp = TakeProfitConfig {
        enabled: true,
        threshold_percent: 100.0,
        sell_percent: 1.0,
    };
    assert!(tp.enabled);
    assert_eq!(tp.threshold_percent, 100.0);
    assert_eq!(tp.sell_percent, 1.0);
}

#[test]
fn test_sell_strategy_with_both() {
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
    
    assert!(strategy.stop_loss.is_some());
    assert!(strategy.take_profit.is_some());
    
    let sl = strategy.stop_loss.as_ref().unwrap();
    let tp = strategy.take_profit.as_ref().unwrap();
    
    assert!(sl.enabled);
    assert_eq!(sl.threshold_percent, -10.0);
    
    assert!(tp.enabled);
    assert_eq!(tp.threshold_percent, 50.0);
    assert_eq!(tp.sell_percent, 0.5);
}

#[test]
fn test_sell_strategy_clone() {
    let strategy1 = SellStrategy {
        stop_loss: Some(StopLossConfig {
            enabled: true,
            threshold_percent: -5.0,
        }),
        take_profit: None,
        trailing_stop: None,
    };
    
    let strategy2 = strategy1.clone();
    assert!(strategy2.stop_loss.is_some());
    assert_eq!(
        strategy2.stop_loss.as_ref().unwrap().threshold_percent,
        -5.0
    );
}
