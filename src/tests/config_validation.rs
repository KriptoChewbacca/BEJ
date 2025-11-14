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
    assert_eq!(mode, TradingMode::Single);
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
    assert_ne!(TradingMode::Single, TradingMode::Multi);
    assert_ne!(TradingMode::Single, TradingMode::Hybrid);
    assert_ne!(TradingMode::Multi, TradingMode::Hybrid);
}

#[test]
fn test_stop_loss_config_default() {
    let config = StopLossConfig::default();
    
    assert_eq!(config.percentage, 0.10); // 10% stop loss
    assert!(!config.time_based);
    assert!(config.time_limit_seconds.is_none());
}

#[test]
fn test_stop_loss_config_custom() {
    let config = StopLossConfig {
        percentage: 0.05,
        time_based: true,
        time_limit_seconds: Some(3600), // 1 hour
    };
    
    assert_eq!(config.percentage, 0.05);
    assert!(config.time_based);
    assert_eq!(config.time_limit_seconds, Some(3600));
}

#[test]
fn test_take_profit_config_default() {
    let config = TakeProfitConfig::default();
    
    assert_eq!(config.percentage, 0.50); // 50% take profit
    assert!(config.partial_levels.is_empty());
}

#[test]
fn test_take_profit_config_with_partials() {
    let config = TakeProfitConfig {
        percentage: 1.0, // 100%
        partial_levels: vec![
            (0.25, 0.25), // At 25% gain, sell 25%
            (0.50, 0.50), // At 50% gain, sell 50%
        ],
    };
    
    assert_eq!(config.percentage, 1.0);
    assert_eq!(config.partial_levels.len(), 2);
    assert_eq!(config.partial_levels[0], (0.25, 0.25));
    assert_eq!(config.partial_levels[1], (0.50, 0.50));
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
            percentage: 0.15,
            time_based: true,
            time_limit_seconds: Some(7200),
        }),
        take_profit: Some(TakeProfitConfig {
            percentage: 0.75,
            partial_levels: vec![(0.30, 0.30)],
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
    assert_eq!(stop_loss.percentage, 0.15);
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
    let mode = TradingMode::Multi;
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
        percentage: 0.0, // Edge case: immediate stop
        time_based: false,
        time_limit_seconds: None,
    };
    
    assert_eq!(config.percentage, 0.0);
}

#[test]
fn test_take_profit_very_high() {
    let config = TakeProfitConfig {
        percentage: 10.0, // 1000% gain
        partial_levels: vec![],
    };
    
    assert_eq!(config.percentage, 10.0);
}
