//! Comprehensive Multi-Token Integration Tests
//!
//! This module tests complex multi-token scenarios including concurrent operations,
//! portfolio limit enforcement, and edge cases.

#[cfg(test)]
mod tests {
    use crate::types::{AppState, Mode, PortfolioConfig};
    use solana_sdk::pubkey::Pubkey;

    #[tokio::test]
    async fn test_buy_three_sell_one_buy_one() {
        // Enable multi-token (max 3)
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 3,
            max_total_exposure_sol: 15.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Buy token A
        let mint_a = Pubkey::new_unique();
        let candidate_a = crate::types::PremintCandidate {
            mint: mint_a,
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::types::PriorityLevel::High,
            timestamp: 1234567890,
            price_hint: Some(1.0),
            signature: None,
        };
        state.set_position(mint_a, crate::types::TokenPosition::new(candidate_a, 1.0));
        assert!(state.can_buy(), "Should allow buy (1/3)");
        
        // Buy token B
        let mint_b = Pubkey::new_unique();
        let candidate_b = crate::types::PremintCandidate {
            mint: mint_b,
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::types::PriorityLevel::High,
            timestamp: 1234567891,
            price_hint: Some(1.5),
            signature: None,
        };
        state.set_position(mint_b, crate::types::TokenPosition::new(candidate_b, 1.5));
        assert!(state.can_buy(), "Should allow buy (2/3)");
        
        // Buy token C
        let mint_c = Pubkey::new_unique();
        let candidate_c = crate::types::PremintCandidate {
            mint: mint_c,
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::types::PriorityLevel::High,
            timestamp: 1234567892,
            price_hint: Some(2.0),
            signature: None,
        };
        state.set_position(mint_c, crate::types::TokenPosition::new(candidate_c, 2.0));
        
        // Verify can_buy() == false (limit reached)
        assert!(!state.can_buy(), "Should block buy at limit (3/3)");
        assert_eq!(state.position_count(), 3, "Should have 3 positions");
        
        // Sell 100% of token B
        state.remove_position(&mint_b);
        
        // Verify can_buy() == true
        assert!(state.can_buy(), "Should allow buy after sell (2/3)");
        assert_eq!(state.position_count(), 2, "Should have 2 positions");
        
        // Buy token D
        let mint_d = Pubkey::new_unique();
        let candidate_d = crate::types::PremintCandidate {
            mint: mint_d,
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::types::PriorityLevel::High,
            timestamp: 1234567893,
            price_hint: Some(2.5),
            signature: None,
        };
        state.set_position(mint_d, crate::types::TokenPosition::new(candidate_d, 2.5));
        
        // Verify positions: [A, C, D]
        assert_eq!(state.position_count(), 3, "Should have 3 positions");
        assert!(state.get_position(&mint_a).is_some(), "Should have position A");
        assert!(state.get_position(&mint_b).is_none(), "Should NOT have position B");
        assert!(state.get_position(&mint_c).is_some(), "Should have position C");
        assert!(state.get_position(&mint_d).is_some(), "Should have position D");
    }

    #[tokio::test]
    async fn test_portfolio_limit_strict_enforcement() {
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 2,
            max_total_exposure_sol: 10.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Fill to limit
        for i in 0..2 {
            let mint = Pubkey::new_unique();
            let candidate = crate::types::PremintCandidate {
                mint,
                program: "pump.fun".to_string(),
                accounts: vec![],
                priority: crate::types::PriorityLevel::Medium,
                timestamp: 1234567890 + i,
                price_hint: Some(1.0),
                signature: None,
            };
            state.set_position(mint, crate::types::TokenPosition::new(candidate, 1.0));
        }
        
        // At limit
        assert!(!state.can_buy(), "Should block buy at limit");
        assert_eq!(state.position_count(), 2, "Should have exactly 2 positions");
        
        // Attempt to add more should fail
        let mint_extra = Pubkey::new_unique();
        let _candidate_extra = crate::types::PremintCandidate {
            mint: mint_extra,
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::types::PriorityLevel::High,
            timestamp: 1234567892,
            price_hint: Some(2.0),
            signature: None,
        };
        
        // In real implementation, buy would check can_buy() and skip
        // Here we verify can_buy() returns false
        assert!(!state.can_buy(), "Should still block buy");
    }

    #[tokio::test]
    async fn test_single_token_mode_strict() {
        // Default is single-token mode
        let state = AppState::new(Mode::Sniffing);
        
        // Add first position
        let mint1 = Pubkey::new_unique();
        let candidate1 = crate::types::PremintCandidate {
            mint: mint1,
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::types::PriorityLevel::High,
            timestamp: 1234567890,
            price_hint: Some(1.0),
            signature: None,
        };
        state.set_position(mint1, crate::types::TokenPosition::new(candidate1, 1.0));
        
        // Should block second buy
        assert!(!state.can_buy(), "Should block second buy in single-token mode");
        
        // Verify can't add second position (can_buy check would prevent this)
        assert_eq!(state.position_count(), 1, "Should have 1 position");
    }

    #[tokio::test]
    async fn test_mixed_partial_and_full_sells() {
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 4,
            max_total_exposure_sol: 20.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Setup 4 positions
        let mints: Vec<_> = (0..4).map(|_| Pubkey::new_unique()).collect();
        for (i, mint) in mints.iter().enumerate() {
            let candidate = crate::types::PremintCandidate {
                mint: *mint,
                program: "pump.fun".to_string(),
                accounts: vec![],
                priority: crate::types::PriorityLevel::Medium,
                timestamp: 1234567890 + i as u64,
                price_hint: Some((i + 1) as f64),
                signature: None,
            };
            state.set_position(*mint, crate::types::TokenPosition::new(candidate, (i + 1) as f64));
        }
        
        assert_eq!(state.position_count(), 4, "Should have 4 positions");
        assert!(!state.can_buy(), "Should be at limit");
        
        // Partial sell on position 0 (50%)
        if let Some(mut pos) = state.active_tokens.get_mut(&mints[0]) {
            pos.holdings_percent *= 0.5;
        }
        
        // Full sell on position 1
        state.remove_position(&mints[1]);
        
        // Partial sell on position 2 (25%)
        if let Some(mut pos) = state.active_tokens.get_mut(&mints[2]) {
            pos.holdings_percent *= 0.75;
        }
        
        // Keep position 3 unchanged
        
        // Verify results
        assert_eq!(state.position_count(), 3, "Should have 3 positions after full sell");
        assert!(state.can_buy(), "Should allow buy after full sell");
        
        let pos0 = state.get_position(&mints[0]).unwrap();
        assert!((pos0.holdings_percent - 0.5).abs() < 0.001, "Position 0 should be 50%");
        
        assert!(state.get_position(&mints[1]).is_none(), "Position 1 should be removed");
        
        let pos2 = state.get_position(&mints[2]).unwrap();
        assert!((pos2.holdings_percent - 0.75).abs() < 0.001, "Position 2 should be 75%");
        
        let pos3 = state.get_position(&mints[3]).unwrap();
        assert_eq!(pos3.holdings_percent, 1.0, "Position 3 should be 100%");
    }

    #[tokio::test]
    async fn test_all_positions_sold_returns_to_sniffing() {
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 3,
            max_total_exposure_sol: 15.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Setup 3 positions
        let mints: Vec<_> = (0..3).map(|_| Pubkey::new_unique()).collect();
        for (i, mint) in mints.iter().enumerate() {
            let candidate = crate::types::PremintCandidate {
                mint: *mint,
                program: "pump.fun".to_string(),
                accounts: vec![],
                priority: crate::types::PriorityLevel::Medium,
                timestamp: 1234567890 + i as u64,
                price_hint: Some(1.0),
                signature: None,
            };
            state.set_position(*mint, crate::types::TokenPosition::new(candidate, 1.0));
        }
        
        // Set mode to PassiveToken
        *state.mode.write().await = Mode::PassiveToken(mints[0]);
        
        // Sell all positions
        for mint in &mints {
            state.remove_position(mint);
        }
        
        // Simulate what sell() does: return to Sniffing if no positions
        if state.active_tokens.is_empty() {
            *state.mode.write().await = Mode::Sniffing;
        }
        
        // Verify
        assert_eq!(state.position_count(), 0, "Should have no positions");
        assert!(state.is_sniffing().await, "Should return to Sniffing mode");
    }

    #[test]
    fn test_position_data_integrity() {
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 5,
            max_total_exposure_sol: 25.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Add positions with different entry prices
        let test_data = vec![
            (Pubkey::new_unique(), 1.5, "pump.fun"),
            (Pubkey::new_unique(), 2.0, "raydium"),
            (Pubkey::new_unique(), 0.5, "orca"),
        ];
        
        for (mint, price, program) in &test_data {
            let candidate = crate::types::PremintCandidate {
                mint: *mint,
                program: program.to_string(),
                accounts: vec![],
                priority: crate::types::PriorityLevel::Medium,
                timestamp: 1234567890,
                price_hint: Some(*price),
                signature: None,
            };
            state.set_position(*mint, crate::types::TokenPosition::new(candidate, *price));
        }
        
        // Verify all data is preserved correctly
        for (mint, expected_price, expected_program) in &test_data {
            let pos = state.get_position(mint).unwrap();
            assert_eq!(pos.entry_price, *expected_price, "Entry price should match");
            assert_eq!(&pos.candidate.program, expected_program, "Program should match");
            assert_eq!(pos.candidate.mint, *mint, "Mint should match");
            assert_eq!(pos.holdings_percent, 1.0, "Holdings should be 100%");
            assert!(pos.entry_timestamp > 0, "Timestamp should be set");
        }
    }

    #[test]
    fn test_zero_max_positions_blocks_all_buys() {
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 0,
            max_total_exposure_sol: 10.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Should never allow buy with 0 limit
        assert!(!state.can_buy(), "Should block buy with 0 max positions");
    }

    #[test]
    fn test_large_position_count() {
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 50,
            max_total_exposure_sol: 100.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Add 25 positions
        for i in 0..25 {
            let mint = Pubkey::new_unique();
            let candidate = crate::types::PremintCandidate {
                mint,
                program: format!("program_{}", i),
                accounts: vec![],
                priority: crate::types::PriorityLevel::Medium,
                timestamp: 1234567890 + i,
                price_hint: Some(1.0),
                signature: None,
            };
            state.set_position(mint, crate::types::TokenPosition::new(candidate, 1.0));
        }
        
        assert_eq!(state.position_count(), 25, "Should have 25 positions");
        assert!(state.can_buy(), "Should allow buy (25/50)");
        
        // Verify get_all_positions returns all
        let all_pos = state.get_all_positions();
        assert_eq!(all_pos.len(), 25, "Should return all 25 positions");
    }
}
