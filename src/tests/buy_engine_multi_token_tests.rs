//! Tests for Multi-Token Buy Engine Logic
//!
//! This module tests the multi-token buy functionality in BuyEngine,
//! ensuring proper portfolio management, position limits, and concurrent operations.

#[cfg(test)]
mod tests {
    use crate::types::{AppState, Mode, PortfolioConfig};
    use solana_sdk::pubkey::Pubkey;

    #[tokio::test]
    async fn test_buy_respects_single_token_mode() {
        // Create AppState with default config (single-token mode)
        let state = AppState::new(Mode::Sniffing);
        
        // Initially can buy
        assert!(state.can_buy(), "Should allow buy initially");
        
        // Simulate a buy - add position
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
        
        // Should NOT be able to buy another token in single-token mode
        assert!(!state.can_buy(), "Should block buy in single-token mode");
    }

    #[tokio::test]
    async fn test_buy_respects_multi_token_limit() {
        // Create AppState with multi-token enabled (max 2 positions)
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 2,
            max_total_exposure_sol: 10.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Initially can buy
        assert!(state.can_buy(), "Should allow buy initially");
        
        // Add first position
        let mint1 = Pubkey::new_unique();
        let candidate1 = crate::types::PremintCandidate {
            mint: mint1,
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::types::PriorityLevel::Medium,
            timestamp: 1234567890,
            price_hint: Some(1.0),
            signature: None,
        };
        let position1 = crate::types::TokenPosition::new(candidate1, 1.0);
        state.set_position(mint1, position1);
        
        // Should still be able to buy (1 < 2)
        assert!(state.can_buy(), "Should allow second buy");
        
        // Add second position
        let mint2 = Pubkey::new_unique();
        let candidate2 = crate::types::PremintCandidate {
            mint: mint2,
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::types::PriorityLevel::Medium,
            timestamp: 1234567890,
            price_hint: Some(1.5),
            signature: None,
        };
        let position2 = crate::types::TokenPosition::new(candidate2, 1.5);
        state.set_position(mint2, position2);
        
        // Should NOT be able to buy (2 >= 2)
        assert!(!state.can_buy(), "Should block buy at limit");
    }

    #[tokio::test]
    async fn test_position_opened_correctly() {
        let state = AppState::new(Mode::Sniffing);
        
        assert_eq!(state.position_count(), 0, "Should start with no positions");
        
        // Simulate buy
        let mint = Pubkey::new_unique();
        let entry_price = 2.5;
        let candidate = crate::types::PremintCandidate {
            mint,
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::types::PriorityLevel::High,
            timestamp: 1234567890,
            price_hint: Some(entry_price),
            signature: None,
        };
        
        let position = crate::types::TokenPosition::new(candidate.clone(), entry_price);
        state.set_position(mint, position);
        
        // Verify position
        assert_eq!(state.position_count(), 1, "Should have 1 position");
        
        let retrieved = state.get_position(&mint).unwrap();
        assert_eq!(retrieved.entry_price, entry_price, "Entry price should match");
        assert_eq!(retrieved.holdings_percent, 1.0, "Should have 100% holdings");
        assert_eq!(retrieved.candidate.mint, mint, "Mint should match");
    }

    #[tokio::test]
    async fn test_multiple_positions_tracking() {
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 5,
            max_total_exposure_sol: 20.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Add 3 different positions
        let mut mints = vec![];
        for i in 0..3 {
            let mint = Pubkey::new_unique();
            let candidate = crate::types::PremintCandidate {
                mint,
                program: format!("program_{}", i),
                accounts: vec![],
                priority: crate::types::PriorityLevel::Medium,
                timestamp: 1234567890 + i,
                price_hint: Some((i + 1) as f64),
                signature: None,
            };
            let position = crate::types::TokenPosition::new(candidate, (i + 1) as f64);
            state.set_position(mint, position);
            mints.push(mint);
        }
        
        assert_eq!(state.position_count(), 3, "Should have 3 positions");
        
        // Verify each position exists with correct data
        for (i, mint) in mints.iter().enumerate() {
            let pos = state.get_position(mint).unwrap();
            assert_eq!(pos.entry_price, (i + 1) as f64, "Entry price should match for position {}", i);
            assert_eq!(pos.holdings_percent, 1.0, "Should have 100% holdings for position {}", i);
        }
    }

    #[tokio::test]
    async fn test_can_buy_after_position_removed() {
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
        
        // At limit
        assert!(!state.can_buy(), "Should be at limit");
        assert_eq!(state.position_count(), 2, "Should have 2 positions");
        
        // Remove one position
        state.remove_position(&mint1);
        
        // Should be able to buy again
        assert!(state.can_buy(), "Should allow buy after removal");
        assert_eq!(state.position_count(), 1, "Should have 1 position");
    }
}
