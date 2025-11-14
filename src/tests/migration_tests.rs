//! Backward Compatibility and Migration Tests
//!
//! This module tests that the multi-token migration maintains backward compatibility
//! with existing single-token behavior.

#[cfg(test)]
mod tests {
    use crate::types::{AppState, Mode, PortfolioConfig};
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn test_single_token_behavior_preserved() {
        // Default config should be single-token mode
        let state = AppState::new(Mode::Sniffing);
        
        assert_eq!(
            state.portfolio_config.enable_multi_token, false,
            "Default should be single-token mode"
        );
        assert_eq!(
            state.portfolio_config.max_concurrent_positions, 1,
            "Default max positions should be 1"
        );
        
        // Verify backward compatibility - can_buy should work like before
        assert!(state.can_buy(), "Should allow buy when no positions (backward compat)");
        
        // Add position
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
        
        // Should block second buy in single-token mode
        assert!(!state.can_buy(), "Should block buy with position (backward compat)");
    }

    #[tokio::test]
    async fn test_mode_transitions_work_correctly() {
        let state = AppState::new(Mode::Sniffing);
        
        // Start in Sniffing mode
        assert!(state.is_sniffing().await, "Should start in Sniffing mode");
        
        // Simulate buy - transition to PassiveToken
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
        
        // Set mode to PassiveToken (simulating what buy_engine does)
        *state.mode.write().await = Mode::PassiveToken(mint);
        
        // Verify mode transition
        let mode = state.get_mode().await;
        assert!(
            matches!(mode, Mode::PassiveToken(_)),
            "Should be in PassiveToken mode after buy"
        );
        
        // Simulate full sell - return to Sniffing
        state.remove_position(&mint);
        if state.active_tokens.is_empty() {
            *state.mode.write().await = Mode::Sniffing;
        }
        
        // Verify return to Sniffing
        assert!(state.is_sniffing().await, "Should return to Sniffing after full sell");
    }

    #[test]
    fn test_existing_sell_flow_unchanged() {
        let state = AppState::new(Mode::Sniffing);
        
        // Setup position
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
        
        // Verify initial holdings
        let pos = state.get_position(&mint).unwrap();
        assert_eq!(pos.holdings_percent, 1.0, "Should start with 100%");
        
        // Simulate partial sell (like existing code would do)
        if let Some(mut pos) = state.active_tokens.get_mut(&mint) {
            let sell_pct = 0.3;
            pos.holdings_percent *= (1.0 - sell_pct);
        }
        
        // Verify holdings updated
        let pos = state.get_position(&mint).unwrap();
        assert!((pos.holdings_percent - 0.7).abs() < 0.001, "Should have 70% after 30% sell");
        
        // Simulate full sell
        if let Some(mut pos) = state.active_tokens.get_mut(&mint) {
            pos.holdings_percent = 0.0;
        }
        
        // Check if should remove
        let should_remove = state.get_position(&mint)
            .map(|p| p.holdings_percent <= f64::EPSILON)
            .unwrap_or(false);
        
        assert!(should_remove, "Should remove position when holdings reach 0");
        
        // Remove position
        state.remove_position(&mint);
        
        // Verify removed
        assert_eq!(state.position_count(), 0, "Position should be removed");
    }

    #[test]
    fn test_deprecated_fields_still_accessible() {
        let state = AppState::new(Mode::Sniffing);
        
        // Verify deprecated fields exist and are initialized
        #[allow(deprecated)]
        {
            assert!(state.active_token.is_none(), "active_token should be None initially");
            assert!(state.last_buy_price.is_none(), "last_buy_price should be None initially");
            assert_eq!(state.holdings_percent, 0.0, "holdings_percent should be 0.0 initially");
        }
    }

    #[test]
    fn test_with_gui_constructor_backward_compat() {
        use std::sync::Arc;
        use crate::components::gui_bridge::GuiSnapshotProvider;
        use tokio::sync::mpsc;
        
        let (price_tx, _price_rx) = mpsc::channel(100);
        let gui_provider = Arc::new(GuiSnapshotProvider::new(price_tx));
        let state = AppState::with_gui(Mode::Production, gui_provider);
        
        // Should still work with default single-token config
        assert!(!state.portfolio_config.enable_multi_token, "Should default to single-token");
        assert_eq!(state.position_count(), 0, "Should start with no positions");
    }

    #[test]
    fn test_get_all_positions_works_empty() {
        let state = AppState::new(Mode::Sniffing);
        
        let positions = state.get_all_positions();
        assert!(positions.is_empty(), "Should return empty vec when no positions");
    }

    #[test]
    fn test_get_all_positions_returns_all() {
        let config = PortfolioConfig {
            enable_multi_token: true,
            max_concurrent_positions: 5,
            max_total_exposure_sol: 20.0,
        };
        let state = AppState::with_config(Mode::Sniffing, config);
        
        // Add 3 positions
        let mut expected_mints = vec![];
        for i in 0..3 {
            let mint = Pubkey::new_unique();
            let candidate = crate::types::PremintCandidate {
                mint,
                program: format!("program_{}", i),
                accounts: vec![],
                priority: crate::types::PriorityLevel::Medium,
                timestamp: 1234567890,
                price_hint: Some(1.0),
                signature: None,
            };
            let position = crate::types::TokenPosition::new(candidate, (i + 1) as f64);
            state.set_position(mint, position);
            expected_mints.push(mint);
        }
        
        let all_positions = state.get_all_positions();
        assert_eq!(all_positions.len(), 3, "Should return all 3 positions");
        
        // Verify all expected mints are present
        let returned_mints: Vec<_> = all_positions.iter().map(|(mint, _)| *mint).collect();
        for expected_mint in expected_mints {
            assert!(
                returned_mints.contains(&expected_mint),
                "Should contain expected mint"
            );
        }
    }
}
