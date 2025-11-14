//! Tests for trading mode management
//!
//! ZADANIE 1.1: Trading mode infrastructure tests
//!
//! These tests verify the TradingMode enum and basic behavior.
//! Full integration tests with BuyEngine are covered in integration tests.

use crate::types::TradingMode;

#[test]
fn test_default_mode_is_hybrid() {
    let mode = TradingMode::default();
    assert_eq!(mode, TradingMode::Hybrid, "Default mode should be Hybrid");
}

#[test]
fn test_trading_mode_variants_exist() {
    let _auto = TradingMode::Auto;
    let _manual = TradingMode::Manual;
    let _hybrid = TradingMode::Hybrid;
    
    // Test passes if all variants compile
}

#[test]
fn test_trading_mode_equality() {
    assert_eq!(TradingMode::Auto, TradingMode::Auto);
    assert_eq!(TradingMode::Manual, TradingMode::Manual);
    assert_eq!(TradingMode::Hybrid, TradingMode::Hybrid);
    
    assert_ne!(TradingMode::Auto, TradingMode::Manual);
    assert_ne!(TradingMode::Auto, TradingMode::Hybrid);
    assert_ne!(TradingMode::Manual, TradingMode::Hybrid);
}

#[test]
fn test_trading_mode_debug() {
    let mode = TradingMode::Auto;
    let debug_str = format!("{:?}", mode);
    assert!(debug_str.contains("Auto"));
}

#[test]
fn test_trading_mode_clone() {
    let mode1 = TradingMode::Auto;
    let mode2 = mode1; // Copy (since it's Copy)
    assert_eq!(mode1, mode2);
}
