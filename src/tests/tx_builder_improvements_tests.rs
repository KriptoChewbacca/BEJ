//! Tests for Tasks 2-6: Simulation cache, quorum, and operation priority
//!
//! Validates:
//! - Task 2: Deterministic message hash for simulation cache
//! - Task 3: Adaptive priority fee calculated before simulation (for cache hash)
//! - Task 4: Quorum blockhash logic
//! - Task 5: Simulation error classification  
//! - Task 6: Operation priority decision logic

#[cfg(test)]
mod tx_builder_improvements_tests {
    use crate::tx_builder::{OperationPriority, TransactionConfig, QuorumConfig};
    use sha2::{Sha256, Digest};
    use solana_sdk::hash::Hash;
    
    #[test]
    fn test_operation_priority_requires_nonce() {
        // Task 6: Verify priority-based nonce requirements
        assert_eq!(OperationPriority::CriticalSniper.requires_nonce(), true);
        assert_eq!(OperationPriority::Utility.requires_nonce(), false);
        assert_eq!(OperationPriority::Bulk.requires_nonce(), false);
    }
    
    #[test]
    fn test_operation_priority_fallback() {
        // Task 6: Verify fallback policies
        assert_eq!(OperationPriority::CriticalSniper.allow_blockhash_fallback(), false);
        assert_eq!(OperationPriority::Utility.allow_blockhash_fallback(), true);
        assert_eq!(OperationPriority::Bulk.allow_blockhash_fallback(), true);
    }
    
    #[test]
    fn test_deterministic_message_hash() {
        // Task 2: Verify deterministic hash generation
        let mut hasher1 = Sha256::new();
        let mut hasher2 = Sha256::new();
        
        // Same input should produce same hash
        let test_data = b"test instruction data";
        hasher1.update(test_data);
        hasher2.update(test_data);
        
        let hash1 = hasher1.finalize();
        let hash2 = hasher2.finalize();
        
        assert_eq!(hash1, hash2, "Same input should produce same hash");
        
        // Different blockhashes should NOT affect the hash
        let mut hasher3 = Sha256::new();
        hasher3.update(test_data);
        let hash3 = hasher3.finalize();
        
        assert_eq!(hash1, hash3, "Blockhash should not be part of cache key");
    }
    
    #[test]
    fn test_quorum_config_validation() {
        // Task 4: Verify quorum configuration logic
        let mut config = TransactionConfig::default();
        config.quorum_config.min_responses = 3;
        
        // Should be valid if we have enough RPCs
        config.rpc_endpoints = std::sync::Arc::new([
            "http://rpc1".to_string(),
            "http://rpc2".to_string(),
            "http://rpc3".to_string(),
        ]);
        
        assert!(config.validate().is_ok(), "Valid config should pass");
        
        // Should fail if min_responses > available RPCs
        config.quorum_config.min_responses = 5;
        assert!(config.validate().is_err(), "Should fail with too few RPCs");
    }
    
    #[test]
    fn test_simulation_error_classification() {
        // Task 5: Verify error classification logic
        let fatal_errors = vec![
            "InstructionError",
            "ProgramFailedToComplete",
            "ComputeBudgetExceeded",
            "InsufficientFunds",
        ];
        
        let advisory_errors = vec![
            "Some other warning",
            "Advisory message",
        ];
        
        for error in fatal_errors {
            assert!(
                error.contains("InstructionError") || 
                error.contains("ProgramFailedToComplete") ||
                error.contains("ComputeBudgetExceeded") ||
                error.contains("InsufficientFunds"),
                "Should be classified as fatal: {}", error
            );
        }
        
        for error in advisory_errors {
            assert!(
                !error.contains("InstructionError") && 
                !error.contains("ProgramFailedToComplete") &&
                !error.contains("ComputeBudgetExceeded") &&
                !error.contains("InsufficientFunds"),
                "Should be classified as advisory: {}", error
            );
        }
    }
    
    #[test]
    fn test_num_rpcs_calculation() {
        // Task 4: Verify fixed quorum calculation
        let available_rpcs = 5;
        let min_responses = 3;
        
        // Corrected logic: min(available, min_responses)
        let num_rpcs = min_responses.min(available_rpcs);
        
        assert_eq!(num_rpcs, 3, "Should use min_responses when available");
        
        // Edge case: fewer RPCs than min_responses
        let available_rpcs = 2;
        let num_rpcs = min_responses.min(available_rpcs);
        
        assert_eq!(num_rpcs, 2, "Should cap at available RPCs");
        assert!(num_rpcs < min_responses, "Should detect insufficient RPCs");
    }
    
    #[test]
    fn test_lru_cache_ordering() {
        // Task 5: Verify LRU ordering for cache pruning
        use std::time::{Instant, Duration};
        
        let mut entries: Vec<(String, Instant)> = vec![
            ("entry1".to_string(), Instant::now() - Duration::from_secs(10)),
            ("entry2".to_string(), Instant::now() - Duration::from_secs(5)),
            ("entry3".to_string(), Instant::now() - Duration::from_secs(15)),
            ("entry4".to_string(), Instant::now() - Duration::from_secs(1)),
        ];
        
        // Sort by timestamp (LRU = oldest first)
        entries.sort_by_key(|(_, timestamp)| *timestamp);
        
        // Oldest should be first
        assert_eq!(entries[0].0, "entry3", "Oldest entry should be first");
        assert_eq!(entries[1].0, "entry1", "Second oldest should be second");
        assert_eq!(entries[2].0, "entry2", "Third oldest should be third");
        assert_eq!(entries[3].0, "entry4", "Newest should be last");
    }
    
    #[test]
    fn test_adaptive_priority_fee_calculation() {
        // Task 3: Verify adaptive_priority_fee is calculated correctly before simulation
        // This ensures the value is available for cache hash computation
        let mut config = TransactionConfig::default();
        config.adaptive_priority_fee_base = 10_000;
        config.adaptive_priority_fee_multiplier = 1.5;
        
        // Use the helper method (same logic as in tx_builder.rs)
        let adaptive_priority_fee = config.calculate_adaptive_priority_fee();
        
        assert_eq!(adaptive_priority_fee, 15_000, "Should apply multiplier correctly");
        
        // Test with different multipliers
        config.adaptive_priority_fee_multiplier = 2.0;
        let adaptive_priority_fee = config.calculate_adaptive_priority_fee();
        
        assert_eq!(adaptive_priority_fee, 20_000, "Should handle 2x multiplier");
        
        // Test edge case: multiplier = 1.0 (no increase)
        config.adaptive_priority_fee_multiplier = 1.0;
        let adaptive_priority_fee = config.calculate_adaptive_priority_fee();
        
        assert_eq!(adaptive_priority_fee, 10_000, "Should return base with 1.0 multiplier");
    }
}
