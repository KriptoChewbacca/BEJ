//! Tests for Multi-Token State Management
//!
//! This module tests the multi-token portfolio functionality in AppState,
//! ensuring proper position tracking, limits enforcement, and backward compatibility.

#[cfg(test)]
mod tests {
    use crate::types::{AppState, Mode, PortfolioConfig, PremintCandidate, PriorityLevel, TokenPosition};
    use solana_sdk::pubkey::Pubkey;

    /// Helper function to create a test candidate
    fn create_test_candidate(mint: Pubkey) -> PremintCandidate {
        PremintCandidate {
            mint,
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: PriorityLevel::Medium,
            timestamp: 1234567890,
            price_hint: Some(1.0),
            signature: None,
        }
    }

    #[test]
    fn test_single_token_mode_enforcement() {
        // Create AppState with default config (single-token mode)
        let state = AppState::new(Mode::Sniffing);
        
        // Should be able to buy when no positions
        assert!(state.can_buy(), "Should allow buy with no positions");
        
        // Add a position
        let mint1 = Pubkey::new_unique();
        let candidate1 = create_test_candidate(mint1);
        let position1 = TokenPosition::new(candidate1, 1.0);
        state.set_position(mint1, position1);
        
        // Should NOT be able to buy another token in single-token mode
        assert!(!state.can_buy(), "Should block buy with existing position in single-token mode");
        
        // Remove position
        state.remove_position(&mint1);
        
        // Should be able to buy again
        assert!(state.can_buy(), "Should allow buy after position removed");
    }

    #[test]
    fn test_multi_token_mode_allows_multiple() {
        // Create AppState with multi-token enabled (max 3 positions)
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 3,
            max_total_exposure_sol: 10.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Should be able to buy with no positions
        assert!(state.can_buy(), "Should allow buy with no positions");
        
        // Add first position
        let mint1 = Pubkey::new_unique();
        let candidate1 = create_test_candidate(mint1);
        let position1 = TokenPosition::new(candidate1, 1.0);
        state.set_position(mint1, position1);
        
        // Should still be able to buy (1 < 3)
        assert!(state.can_buy(), "Should allow buy with 1 position (limit is 3)");
        
        // Add second position
        let mint2 = Pubkey::new_unique();
        let candidate2 = create_test_candidate(mint2);
        let position2 = TokenPosition::new(candidate2, 1.5);
        state.set_position(mint2, position2);
        
        // Should still be able to buy (2 < 3)
        assert!(state.can_buy(), "Should allow buy with 2 positions (limit is 3)");
        
        // Add third position
        let mint3 = Pubkey::new_unique();
        let candidate3 = create_test_candidate(mint3);
        let position3 = TokenPosition::new(candidate3, 2.0);
        state.set_position(mint3, position3);
        
        // Should NOT be able to buy (3 >= 3)
        assert!(!state.can_buy(), "Should block buy at position limit (3/3)");
    }

    #[test]
    fn test_can_buy_respects_limit() {
        // Test with limit of 2
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 2,
            max_total_exposure_sol: 10.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        assert!(state.can_buy(), "Should allow buy initially");
        assert_eq!(state.position_count(), 0, "Should have 0 positions");
        
        // Add first position
        let mint1 = Pubkey::new_unique();
        let candidate1 = create_test_candidate(mint1);
        let position1 = TokenPosition::new(candidate1, 1.0);
        state.set_position(mint1, position1);
        
        assert!(state.can_buy(), "Should allow buy with 1/2 positions");
        assert_eq!(state.position_count(), 1, "Should have 1 position");
        
        // Add second position
        let mint2 = Pubkey::new_unique();
        let candidate2 = create_test_candidate(mint2);
        let position2 = TokenPosition::new(candidate2, 1.5);
        state.set_position(mint2, position2);
        
        assert!(!state.can_buy(), "Should block buy at limit (2/2 positions)");
        assert_eq!(state.position_count(), 2, "Should have 2 positions");
        
        // Remove one position
        state.remove_position(&mint1);
        
        assert!(state.can_buy(), "Should allow buy after removing position (1/2)");
        assert_eq!(state.position_count(), 1, "Should have 1 position after removal");
    }

    #[test]
    fn test_position_tracking() {
        let state = AppState::new(Mode::Sniffing);
        
        // Initially no positions
        assert_eq!(state.position_count(), 0, "Should start with no positions");
        assert!(state.get_all_positions().is_empty(), "Should have empty position list");
        
        // Add position
        let mint = Pubkey::new_unique();
        let candidate = create_test_candidate(mint);
        let entry_price = 1.5;
        let position = TokenPosition::new(candidate.clone(), entry_price);
        
        state.set_position(mint, position);
        
        // Verify position exists
        assert_eq!(state.position_count(), 1, "Should have 1 position");
        
        let retrieved = state.get_position(&mint);
        assert!(retrieved.is_some(), "Position should exist");
        
        let retrieved_pos = retrieved.unwrap();
        assert_eq!(retrieved_pos.entry_price, entry_price, "Entry price should match");
        assert_eq!(retrieved_pos.holdings_percent, 1.0, "Holdings should be 100%");
        assert_eq!(retrieved_pos.candidate.mint, mint, "Mint should match");
        
        // Verify all positions
        let all_positions = state.get_all_positions();
        assert_eq!(all_positions.len(), 1, "Should have 1 position in list");
        assert_eq!(all_positions[0].0, mint, "Position mint should match");
        
        // Remove position
        let removed = state.remove_position(&mint);
        assert!(removed.is_some(), "Should return removed position");
        assert_eq!(state.position_count(), 0, "Should have 0 positions after removal");
        assert!(state.get_position(&mint).is_none(), "Position should not exist after removal");
    }

    #[test]
    fn test_get_position_nonexistent() {
        let state = AppState::new(Mode::Sniffing);
        let mint = Pubkey::new_unique();
        
        // Should return None for non-existent position
        assert!(state.get_position(&mint).is_none(), "Should return None for non-existent position");
    }

    #[test]
    fn test_update_position() {
        let state = AppState::new(Mode::Sniffing);
        let mint = Pubkey::new_unique();
        let candidate = create_test_candidate(mint);
        
        // Add initial position
        let initial_position = TokenPosition::new(candidate.clone(), 1.0);
        state.set_position(mint, initial_position);
        
        // Verify initial state
        let pos = state.get_position(&mint).unwrap();
        assert_eq!(pos.holdings_percent, 1.0, "Initial holdings should be 100%");
        assert_eq!(pos.entry_price, 1.0, "Initial entry price should be 1.0");
        
        // Update position (simulate partial sell)
        let mut updated_position = state.get_position(&mint).unwrap();
        updated_position.holdings_percent = 0.5;
        state.set_position(mint, updated_position);
        
        // Verify updated state
        let pos = state.get_position(&mint).unwrap();
        assert_eq!(pos.holdings_percent, 0.5, "Updated holdings should be 50%");
    }

    #[test]
    fn test_multiple_positions_different_tokens() {
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 5,
            max_total_exposure_sol: 20.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Add multiple positions
        let mut mints = vec![];
        for i in 0..3 {
            let mint = Pubkey::new_unique();
            let candidate = create_test_candidate(mint);
            let position = TokenPosition::new(candidate, (i + 1) as f64);
            state.set_position(mint, position);
            mints.push(mint);
        }
        
        assert_eq!(state.position_count(), 3, "Should have 3 positions");
        
        // Verify each position
        for (i, mint) in mints.iter().enumerate() {
            let pos = state.get_position(mint);
            assert!(pos.is_some(), "Position should exist for mint {}", i);
            let pos = pos.unwrap();
            assert_eq!(pos.entry_price, (i + 1) as f64, "Entry price should match for position {}", i);
        }
        
        // Verify get_all_positions returns all
        let all = state.get_all_positions();
        assert_eq!(all.len(), 3, "Should return all 3 positions");
    }

    #[test]
    fn test_with_config_constructor() {
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 10,
            max_total_exposure_sol: 50.0,
        };
        let state = AppState::with_config(Mode::Production, config.clone());
        
        assert_eq!(state.portfolio_config.enable_multi_token, true, "Should have multi-token enabled");
        assert_eq!(state.portfolio_config.max_concurrent_positions, 10, "Should have correct limit");
        assert_eq!(state.portfolio_config.max_total_exposure_sol, 50.0, "Should have correct exposure limit");
    }

    #[test]
    fn test_token_position_creation() {
        let mint = Pubkey::new_unique();
        let candidate = create_test_candidate(mint);
        let entry_price = 2.5;
        
        let position = TokenPosition::new(candidate.clone(), entry_price);
        
        assert_eq!(position.entry_price, entry_price, "Entry price should match");
        assert_eq!(position.holdings_percent, 1.0, "Should default to 100% holdings");
        assert_eq!(position.candidate.mint, mint, "Candidate mint should match");
        assert!(position.entry_timestamp > 0, "Should have valid timestamp");
    }
}
