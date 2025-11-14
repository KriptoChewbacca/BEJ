//! Tests for Multi-Token Sell Logic
//!
//! This module tests the multi-token sell functionality,
//! ensuring proper position updates, partial/full sells, and state transitions.

#[cfg(test)]
mod tests {
    use crate::types::{AppState, Mode, PortfolioConfig};
    use solana_sdk::pubkey::Pubkey;

    #[tokio::test]
    async fn test_partial_sell_updates_holdings() {
        let state = AppState::new(Mode::Sniffing);
        
        // Setup: Add a position
        let mint = Pubkey::new_unique();
        let candidate = crate::types::PremintCandidate {
            mint,
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::types::PriorityLevel::Medium,
            timestamp: 1234567890,
            price_hint: Some(1.0),
            signature: None,
        };
        let position = crate::types::TokenPosition::new(candidate, 1.5);
        state.set_position(mint, position);
        
        // Verify initial state
        let pos = state.get_position(&mint).unwrap();
        assert_eq!(pos.holdings_percent, 1.0, "Should start with 100%");
        
        // Simulate partial sell (50%)
        let sell_pct = 0.5;
        if let Some(mut pos) = state.active_tokens.get_mut(&mint) {
            pos.holdings_percent *= (1.0 - sell_pct);
        }
        
        // Verify updated holdings
        let pos = state.get_position(&mint).unwrap();
        assert_eq!(pos.holdings_percent, 0.5, "Should have 50% after partial sell");
        
        // Position should still exist
        assert_eq!(state.position_count(), 1, "Position should still exist");
    }

    #[tokio::test]
    async fn test_full_sell_removes_position() {
        let state = AppState::new(Mode::Sniffing);
        
        // Setup: Add a position
        let mint = Pubkey::new_unique();
        let candidate = crate::types::PremintCandidate {
            mint,
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::types::PriorityLevel::Medium,
            timestamp: 1234567890,
            price_hint: Some(1.0),
            signature: None,
        };
        let position = crate::types::TokenPosition::new(candidate, 1.5);
        state.set_position(mint, position);
        
        assert_eq!(state.position_count(), 1, "Should have 1 position");
        
        // Simulate full sell (100%)
        state.remove_position(&mint);
        
        // Verify position removed
        assert_eq!(state.position_count(), 0, "Position should be removed");
        assert!(state.get_position(&mint).is_none(), "Position should not exist");
    }

    #[tokio::test]
    async fn test_sell_one_of_multiple_tokens() {
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 3,
            max_total_exposure_sol: 15.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Setup: Add 3 positions
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();
        
        for mint in [mint1, mint2, mint3] {
            let candidate = crate::types::PremintCandidate {
                mint,
                program: "pump.fun".to_string(),
                accounts: vec![],
                priority: crate::types::PriorityLevel::Medium,
                timestamp: 1234567890,
                price_hint: Some(1.0),
                signature: None,
            };
            let position = crate::types::TokenPosition::new(candidate, 1.0);
            state.set_position(mint, position);
        }
        
        assert_eq!(state.position_count(), 3, "Should have 3 positions");
        
        // Sell position 2 (partial 60%)
        let sell_pct = 0.6;
        if let Some(mut pos) = state.active_tokens.get_mut(&mint2) {
            pos.holdings_percent *= (1.0 - sell_pct);
        }
        
        // Verify: Position 2 still exists with updated holdings
        let pos2 = state.get_position(&mint2).unwrap();
        assert!((pos2.holdings_percent - 0.4).abs() < 0.001, "Should have 40% remaining");
        
        // Verify: Other positions unchanged
        let pos1 = state.get_position(&mint1).unwrap();
        let pos3 = state.get_position(&mint3).unwrap();
        assert_eq!(pos1.holdings_percent, 1.0, "Position 1 should be unchanged");
        assert_eq!(pos3.holdings_percent, 1.0, "Position 3 should be unchanged");
        
        // Still have 3 positions
        assert_eq!(state.position_count(), 3, "Should still have 3 positions");
    }

    #[tokio::test]
    async fn test_sell_sequence_partial_then_full() {
        let state = AppState::new(Mode::Sniffing);
        
        // Setup: Add a position
        let mint = Pubkey::new_unique();
        let candidate = crate::types::PremintCandidate {
            mint,
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::types::PriorityLevel::Medium,
            timestamp: 1234567890,
            price_hint: Some(1.0),
            signature: None,
        };
        let position = crate::types::TokenPosition::new(candidate, 1.5);
        state.set_position(mint, position);
        
        // First sell: 30%
        if let Some(mut pos) = state.active_tokens.get_mut(&mint) {
            pos.holdings_percent *= (1.0 - 0.3);
        }
        
        let pos = state.get_position(&mint).unwrap();
        assert!((pos.holdings_percent - 0.7).abs() < 0.001, "Should have 70% after first sell");
        
        // Second sell: 50% of remaining (0.5 * 0.7 = 0.35 sold, 0.35 remaining)
        if let Some(mut pos) = state.active_tokens.get_mut(&mint) {
            pos.holdings_percent *= (1.0 - 0.5);
        }
        
        let pos = state.get_position(&mint).unwrap();
        assert!((pos.holdings_percent - 0.35).abs() < 0.001, "Should have 35% after second sell");
        
        // Third sell: 100% (full exit)
        state.remove_position(&mint);
        
        assert_eq!(state.position_count(), 0, "Position should be removed");
        assert!(state.get_position(&mint).is_none(), "Position should not exist");
    }

    #[tokio::test]
    async fn test_multiple_tokens_independent_sells() {
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 3,
            max_total_exposure_sol: 15.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Setup: Add 3 positions with different entry prices
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        let mint3 = Pubkey::new_unique();
        let mints = [mint1, mint2, mint3];
        let prices = [1.0, 2.0, 3.0];
        
        for (mint, price) in mints.iter().zip(prices.iter()) {
            let candidate = crate::types::PremintCandidate {
                mint: *mint,
                program: "pump.fun".to_string(),
                accounts: vec![],
                priority: crate::types::PriorityLevel::Medium,
                timestamp: 1234567890,
                price_hint: Some(*price),
                signature: None,
            };
            let position = crate::types::TokenPosition::new(candidate, *price);
            state.set_position(*mint, position);
        }
        
        // Sell 50% of token 1
        if let Some(mut pos) = state.active_tokens.get_mut(&mint1) {
            pos.holdings_percent *= 0.5;
        }
        
        // Sell 100% of token 2
        state.remove_position(&mint2);
        
        // Sell 25% of token 3
        if let Some(mut pos) = state.active_tokens.get_mut(&mint3) {
            pos.holdings_percent *= 0.75;
        }
        
        // Verify results
        assert_eq!(state.position_count(), 2, "Should have 2 positions (1 fully sold)");
        
        let pos1 = state.get_position(&mint1).unwrap();
        assert!((pos1.holdings_percent - 0.5).abs() < 0.001, "Token 1 should have 50%");
        assert_eq!(pos1.entry_price, 1.0, "Token 1 entry price should be unchanged");
        
        assert!(state.get_position(&mint2).is_none(), "Token 2 should be removed");
        
        let pos3 = state.get_position(&mint3).unwrap();
        assert!((pos3.holdings_percent - 0.75).abs() < 0.001, "Token 3 should have 75%");
        assert_eq!(pos3.entry_price, 3.0, "Token 3 entry price should be unchanged");
    }

    #[tokio::test]
    async fn test_can_buy_after_full_sell() {
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 2,
            max_total_exposure_sol: 10.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Fill to limit
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        
        for mint in [mint1, mint2] {
            let candidate = crate::types::PremintCandidate {
                mint,
                program: "pump.fun".to_string(),
                accounts: vec![],
                priority: crate::types::PriorityLevel::Medium,
                timestamp: 1234567890,
                price_hint: Some(1.0),
                signature: None,
            };
            let position = crate::types::TokenPosition::new(candidate, 1.0);
            state.set_position(mint, position);
        }
        
        assert!(!state.can_buy(), "Should be at limit");
        
        // Fully sell one position
        state.remove_position(&mint1);
        
        // Should be able to buy again
        assert!(state.can_buy(), "Should allow buy after full sell");
        assert_eq!(state.position_count(), 1, "Should have 1 position remaining");
    }
}
