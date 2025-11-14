//! Configuration validation tests for multi-token portfolio features
//!
//! These tests validate the new configuration structures for future multi-token support.
//! Note: These features are not yet integrated into the main trading logic.

use crate::types::{
    PortfolioConfig, TakeProfitConfig, TradingMode, SellStrategy, 
    StopLossConfig, TrailingStopConfig,
};

#[test]
fn test_portfolio_config_default() {
    let config = PortfolioConfig::default();
    
    // Verify safe defaults
    assert_eq!(config.max_concurrent_positions, 1);
    assert!(!config.enable_multi_token);
    assert_eq!(config.max_total_exposure_sol, 10.0);
}

#[test]
fn test_portfolio_config_custom() {
    let config = PortfolioConfig {
        enable_multi_token: true,
        max_concurrent_positions: 5,
        max_total_exposure_sol: 50.0,
    };
    
    assert!(config.enable_multi_token);
    assert_eq!(config.max_concurrent_positions, 5);
    assert_eq!(config.max_total_exposure_sol, 50.0);
}

#[test]
fn test_trading_mode_default() {
    let mode = TradingMode::default();
    assert_eq!(mode, TradingMode::Hybrid);
}

#[test]
fn test_trading_mode_serialization() {
    let mode = TradingMode::Hybrid;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, "\"Hybrid\"");
    
    // Test deserialization
    let deserialized: TradingMode = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, TradingMode::Hybrid);
}

#[test]
fn test_trading_mode_variants() {
    // Verify all variants are distinct
    assert_ne!(TradingMode::Auto, TradingMode::Manual);
    assert_ne!(TradingMode::Auto, TradingMode::Hybrid);
    assert_ne!(TradingMode::Manual, TradingMode::Hybrid);
}

#[test]
fn test_stop_loss_config_default() {
    let config = StopLossConfig::default();
    
    assert_eq!(config.threshold_percent, -10.0); // -10% stop loss
    assert!(!config.enabled);
}

#[test]
fn test_stop_loss_config_custom() {
    let config = StopLossConfig {
        enabled: true,
        threshold_percent: -5.0,
    };
    
    assert_eq!(config.threshold_percent, -5.0);
    assert!(config.enabled);
}

#[test]
fn test_take_profit_config_default() {
    let config = TakeProfitConfig::default();
    
    assert_eq!(config.threshold_percent, 50.0); // 50% take profit
    assert_eq!(config.sell_percent, 0.5);
    assert!(!config.enabled);
}

#[test]
fn test_take_profit_config_custom() {
    let config = TakeProfitConfig {
        enabled: true,
        threshold_percent: 100.0, // 100%
        sell_percent: 1.0,
    };
    
    assert_eq!(config.threshold_percent, 100.0);
    assert_eq!(config.sell_percent, 1.0);
    assert!(config.enabled);
}

#[test]
fn test_trailing_stop_config_default() {
    let config = TrailingStopConfig::default();
    
    assert_eq!(config.percentage, 0.05); // 5% from peak
    assert_eq!(config.activation_threshold, 0.20); // Activate at 20%
}

#[test]
fn test_sell_strategy_default() {
    let strategy = SellStrategy::default();
    
    assert!(strategy.stop_loss.is_none());
    assert!(strategy.take_profit.is_none());
    assert!(strategy.trailing_stop.is_none());
}

#[test]
fn test_sell_strategy_with_all_configs() {
    let strategy = SellStrategy {
        stop_loss: Some(StopLossConfig::default()),
        take_profit: Some(TakeProfitConfig::default()),
        trailing_stop: Some(TrailingStopConfig::default()),
    };
    
    assert!(strategy.stop_loss.is_some());
    assert!(strategy.take_profit.is_some());
    assert!(strategy.trailing_stop.is_some());
}

#[test]
fn test_portfolio_config_serialization() {
    let config = PortfolioConfig {
        enable_multi_token: true,
        max_concurrent_positions: 3,
        max_total_exposure_sol: 15.0,
    };
    
    // Test JSON serialization
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: PortfolioConfig = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.enable_multi_token, config.enable_multi_token);
    assert_eq!(deserialized.max_concurrent_positions, config.max_concurrent_positions);
    assert_eq!(deserialized.max_total_exposure_sol, config.max_total_exposure_sol);
}

#[test]
fn test_sell_strategy_serialization() {
    let strategy = SellStrategy {
        stop_loss: Some(StopLossConfig {
            enabled: true,
            threshold_percent: -15.0,
        }),
        take_profit: Some(TakeProfitConfig {
            enabled: true,
            threshold_percent: 75.0,
            sell_percent: 0.75,
        }),
        trailing_stop: None,
    };
    
    // Test JSON serialization
    let json = serde_json::to_string(&strategy).unwrap();
    let deserialized: SellStrategy = serde_json::from_str(&json).unwrap();
    
    assert!(deserialized.stop_loss.is_some());
    assert!(deserialized.take_profit.is_some());
    assert!(deserialized.trailing_stop.is_none());
    
    let stop_loss = deserialized.stop_loss.unwrap();
    assert_eq!(stop_loss.threshold_percent, -15.0);
}

#[test]
fn test_portfolio_config_clone() {
    let config = PortfolioConfig::default();
    let cloned = config.clone();
    
    assert_eq!(config.enable_multi_token, cloned.enable_multi_token);
    assert_eq!(config.max_concurrent_positions, cloned.max_concurrent_positions);
}

#[test]
fn test_trading_mode_clone() {
    let mode = TradingMode::Auto;
    let cloned = mode;
    
    assert_eq!(mode, cloned);
}

// Edge case tests

#[test]
fn test_portfolio_config_zero_positions() {
    let config = PortfolioConfig {
        enable_multi_token: false,
        max_concurrent_positions: 0, // Edge case: no positions allowed
        max_total_exposure_sol: 10.0,
    };
    
    // Should not panic, just store the value
    assert_eq!(config.max_concurrent_positions, 0);
}

#[test]
fn test_portfolio_config_large_exposure() {
    let config = PortfolioConfig {
        enable_multi_token: true,
        max_concurrent_positions: 100,
        max_total_exposure_sol: 1000.0,
    };
    
    assert_eq!(config.max_total_exposure_sol, 1000.0);
}

#[test]
fn test_stop_loss_zero_percentage() {
    let config = StopLossConfig {
        enabled: true,
        threshold_percent: 0.0,
    };
    
    assert_eq!(config.threshold_percent, 0.0);
}

#[test]
fn test_take_profit_very_high() {
    let config = TakeProfitConfig {
        enabled: true,
        threshold_percent: 1000.0, // 1000% gain
        sell_percent: 1.0,
    };
    
    assert_eq!(config.threshold_percent, 1000.0);
    assert_eq!(config.sell_percent, 1.0);
}
