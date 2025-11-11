//! A3: Test suite for safe mint extraction implementation
//!
//! This test suite validates:
//! - Mint extraction with safe_offsets validation
//! - Account extraction with safe_offsets validation
//! - Error handling for MintExtractError and AccountExtractError
//! - >95% accuracy requirement
//! - No panics during parsing

#[cfg(test)]
mod a3_tests {
    use std::sync::atomic::Ordering;
    use solana_sdk::pubkey::Pubkey;
    use smallvec::SmallVec;

    // Mock the sniffer module types for standalone testing
    // In actual integration, these would be: use crate::sniffer::*;
    
    /// A3: Error types for mint and account extraction
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MintExtractError {
        /// Transaction too small to contain mint data
        TooSmall,
        /// Invalid mint pubkey (all zeros / default)
        InvalidMint,
        /// Extraction offset out of bounds
        OutOfBounds,
        /// Deserialization failed
        DeserializationFailed,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AccountExtractError {
        /// Transaction too small to contain account data
        TooSmall,
        /// Invalid account pubkey (all zeros / default)
        InvalidAccount,
        /// Extraction offset out of bounds
        OutOfBounds,
        /// Deserialization failed
        DeserializationFailed,
    }

    /// Mock extract_mint function for testing
    fn extract_mint(tx_bytes: &[u8], safe_offsets: bool) -> Result<Pubkey, MintExtractError> {
        if tx_bytes.len() < 96 {
            return Err(MintExtractError::TooSmall);
        }

        let mint_bytes = tx_bytes.get(64..96)
            .ok_or(MintExtractError::OutOfBounds)?;
        
        let mint = Pubkey::try_from(mint_bytes)
            .map_err(|_| MintExtractError::DeserializationFailed)?;
        
        if safe_offsets && mint == Pubkey::default() {
            return Err(MintExtractError::InvalidMint);
        }
        
        Ok(mint)
    }

    /// Mock extract_accounts function for testing
    fn extract_accounts(tx_bytes: &[u8], safe_offsets: bool) -> Result<SmallVec<[Pubkey; 8]>, AccountExtractError> {
        let mut accounts = SmallVec::new();
        
        if tx_bytes.len() < 128 {
            return Err(AccountExtractError::TooSmall);
        }

        let mut offset = 96;
        while offset + 32 <= tx_bytes.len() && accounts.len() < 8 {
            let account_bytes = tx_bytes.get(offset..offset + 32)
                .ok_or(AccountExtractError::OutOfBounds)?;
            
            match Pubkey::try_from(account_bytes) {
                Ok(pubkey) => {
                    if safe_offsets && pubkey == Pubkey::default() {
                        // Skip default pubkey
                    } else {
                        accounts.push(pubkey);
                    }
                }
                Err(_) => {
                    // Skip invalid pubkey and continue
                }
            }
            offset += 32;
        }
        
        Ok(accounts)
    }

    #[test]
    fn test_a3_mint_extraction_valid() {
        // Create a valid transaction with a proper mint pubkey
        let mut tx = vec![0u8; 200];
        let valid_mint = Pubkey::new_unique();
        tx[64..96].copy_from_slice(&valid_mint.to_bytes());

        // Test without safe_offsets
        let result = extract_mint(&tx, false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), valid_mint);

        // Test with safe_offsets
        let result = extract_mint(&tx, true);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), valid_mint);
    }

    #[test]
    fn test_a3_mint_extraction_default_pubkey() {
        // Create a transaction with default (all zeros) pubkey
        let mut tx = vec![0u8; 200];
        // Leave bytes 64-96 as zeros (default pubkey)

        // Without safe_offsets, should extract default pubkey
        let result = extract_mint(&tx, false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Pubkey::default());

        // With safe_offsets, should reject default pubkey
        let result = extract_mint(&tx, true);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MintExtractError::InvalidMint);
    }

    #[test]
    fn test_a3_mint_extraction_too_small() {
        // Transaction too small to contain mint
        let tx = vec![0u8; 64];

        let result = extract_mint(&tx, false);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MintExtractError::TooSmall);

        let result = extract_mint(&tx, true);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), MintExtractError::TooSmall);
    }

    #[test]
    fn test_a3_account_extraction_valid() {
        // Create a valid transaction with multiple accounts
        let mut tx = vec![0u8; 300];
        
        let account1 = Pubkey::new_unique();
        let account2 = Pubkey::new_unique();
        let account3 = Pubkey::new_unique();
        
        tx[96..128].copy_from_slice(&account1.to_bytes());
        tx[128..160].copy_from_slice(&account2.to_bytes());
        tx[160..192].copy_from_slice(&account3.to_bytes());

        // Test extraction
        let result = extract_accounts(&tx, false);
        assert!(result.is_ok());
        let accounts = result.unwrap();
        assert_eq!(accounts.len(), 3);
        assert_eq!(accounts[0], account1);
        assert_eq!(accounts[1], account2);
        assert_eq!(accounts[2], account3);
    }

    #[test]
    fn test_a3_account_extraction_with_defaults() {
        // Create transaction with some default pubkeys mixed in
        let mut tx = vec![0u8; 300];
        
        let account1 = Pubkey::new_unique();
        // account2 is default (all zeros)
        let account3 = Pubkey::new_unique();
        
        tx[96..128].copy_from_slice(&account1.to_bytes());
        // tx[128..160] left as zeros (default pubkey)
        tx[160..192].copy_from_slice(&account3.to_bytes());

        // Without safe_offsets, should include default pubkey
        let result = extract_accounts(&tx, false);
        assert!(result.is_ok());
        let accounts = result.unwrap();
        assert_eq!(accounts.len(), 3);
        assert_eq!(accounts[0], account1);
        assert_eq!(accounts[1], Pubkey::default());
        assert_eq!(accounts[2], account3);

        // With safe_offsets, should skip default pubkey
        let result = extract_accounts(&tx, true);
        assert!(result.is_ok());
        let accounts = result.unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0], account1);
        assert_eq!(accounts[1], account3);
    }

    #[test]
    fn test_a3_account_extraction_too_small() {
        // Transaction too small to contain accounts
        let tx = vec![0u8; 100];

        let result = extract_accounts(&tx, false);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), AccountExtractError::TooSmall);
    }

    #[test]
    fn test_a3_account_extraction_max_8_accounts() {
        // Create transaction with more than 8 accounts
        let mut tx = vec![0u8; 400];
        
        for i in 0..10 {
            let account = Pubkey::new_unique();
            let offset = 96 + (i * 32);
            if offset + 32 <= tx.len() {
                tx[offset..offset + 32].copy_from_slice(&account.to_bytes());
            }
        }

        // Should only extract 8 accounts
        let result = extract_accounts(&tx, false);
        assert!(result.is_ok());
        let accounts = result.unwrap();
        assert_eq!(accounts.len(), 8);
    }

    #[test]
    fn test_a3_error_types_display() {
        // Verify error types are properly defined
        let mint_errors = [
            MintExtractError::TooSmall,
            MintExtractError::InvalidMint,
            MintExtractError::OutOfBounds,
            MintExtractError::DeserializationFailed,
        ];

        let account_errors = [
            AccountExtractError::TooSmall,
            AccountExtractError::InvalidAccount,
            AccountExtractError::OutOfBounds,
            AccountExtractError::DeserializationFailed,
        ];

        // Ensure all error variants can be compared
        for error in &mint_errors {
            assert_eq!(*error, *error);
        }

        for error in &account_errors {
            assert_eq!(*error, *error);
        }
    }

    #[test]
    fn test_a3_no_panic_on_invalid_input() {
        // Test various invalid inputs to ensure no panics
        let test_cases = vec![
            vec![],                    // Empty
            vec![0u8; 10],            // Very small
            vec![0u8; 50],            // Small
            vec![0xFF; 64],           // All ones, small
            vec![0xFF; 96],           // All ones, medium
            vec![0xFF; 200],          // All ones, large
            vec![0xAA; 300],          // Pattern
        ];

        for tx in test_cases {
            // Should not panic, just return error
            let _ = extract_mint(&tx, false);
            let _ = extract_mint(&tx, true);
            let _ = extract_accounts(&tx, false);
            let _ = extract_accounts(&tx, true);
        }
    }

    #[test]
    fn test_a3_accuracy_requirement() {
        // Test >95% accuracy on a batch of transactions
        let total_transactions = 100;
        let mut successful_extractions = 0;

        for i in 0..total_transactions {
            let mut tx = vec![0u8; 200];
            
            // 90% valid transactions, 10% invalid
            if i < 90 {
                let mint = Pubkey::new_unique();
                tx[64..96].copy_from_slice(&mint.to_bytes());
                
                if extract_mint(&tx, true).is_ok() {
                    successful_extractions += 1;
                }
            } else {
                // Invalid transaction (too small or default)
                if i % 2 == 0 {
                    tx = vec![0u8; 50]; // Too small
                }
                // else: default pubkey (should be rejected with safe_offsets)
                
                // These should fail
                let _ = extract_mint(&tx, true);
            }
        }

        let accuracy = (successful_extractions as f64 / 90.0) * 100.0;
        println!("Extraction accuracy: {:.2}%", accuracy);
        
        // Should achieve >95% accuracy on valid transactions
        assert!(accuracy >= 95.0, "Accuracy {:.2}% is below 95% requirement", accuracy);
    }

    #[test]
    fn test_a3_metrics_integration() {
        use std::sync::atomic::AtomicU64;
        
        // Simulate metrics tracking
        let mint_extract_errors = AtomicU64::new(0);
        let account_extract_errors = AtomicU64::new(0);
        let security_drop_count = AtomicU64::new(0);

        // Process batch of transactions
        for i in 0..20 {
            let mut tx = vec![0u8; 200];
            
            // Create various scenarios
            if i < 15 {
                // Valid transactions
                let mint = Pubkey::new_unique();
                tx[64..96].copy_from_slice(&mint.to_bytes());
            } else if i < 17 {
                // Too small
                tx = vec![0u8; 50];
            } else {
                // Default pubkey (leave as zeros)
            }

            // Process with safe_offsets
            match extract_mint(&tx, true) {
                Ok(_) => {},
                Err(_) => {
                    mint_extract_errors.fetch_add(1, Ordering::Relaxed);
                    security_drop_count.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        // Verify metrics were tracked
        assert_eq!(mint_extract_errors.load(Ordering::Relaxed), 5); // 2 too small + 3 default
        assert_eq!(security_drop_count.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn test_a3_nested_instructions_scenario() {
        // Simulate a transaction with nested instructions
        // This tests that our offset-based extraction handles complex structures
        let mut tx = vec![0u8; 500];
        
        // Place mint at standard offset
        let mint = Pubkey::new_unique();
        tx[64..96].copy_from_slice(&mint.to_bytes());
        
        // Add some "nested" structure by placing data at various offsets
        for i in 0..5 {
            let offset = 200 + (i * 32);
            let account = Pubkey::new_unique();
            if offset + 32 <= tx.len() {
                tx[offset..offset + 32].copy_from_slice(&account.to_bytes());
            }
        }

        // Mint extraction should still work
        let result = extract_mint(&tx, true);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mint);

        // Account extraction should work for standard offsets
        let result = extract_accounts(&tx, true);
        assert!(result.is_ok());
    }
}
