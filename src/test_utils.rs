//! Test Utilities Module
//! 
//! This module provides test-only utilities for mocking transaction building,
//! nonce management, and other components needed for deterministic testing.
//! 
//! These utilities are only compiled when running tests or when the 
//! `test_utils` feature is enabled.

#![cfg(any(test, feature = "test_utils"))]

use anyhow::Result;
use solana_sdk::{
    hash::Hash,
    message::Message,
    pubkey::Pubkey,
    signature::Signature,
    system_instruction,
    transaction::{Transaction, VersionedTransaction},
};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::types::PremintCandidate;

/// Mock TransactionBuilder for testing
/// 
/// This builder simulates transaction building without making actual network calls.
/// All operations are deterministic and controlled for testing purposes.
#[derive(Clone)]
pub struct MockTxBuilder {
    /// Whether to succeed or fail transaction builds
    pub should_succeed: Arc<Mutex<bool>>,
    
    /// Counter for tracking number of buy transactions built
    pub buy_count: Arc<Mutex<usize>>,
    
    /// Counter for tracking number of sell transactions built
    pub sell_count: Arc<Mutex<usize>>,
    
    /// Deterministic signature to return
    pub mock_signature: Signature,
}

impl MockTxBuilder {
    /// Create a new MockTxBuilder that succeeds by default
    pub fn new() -> Self {
        Self {
            should_succeed: Arc::new(Mutex::new(true)),
            buy_count: Arc::new(Mutex::new(0)),
            sell_count: Arc::new(Mutex::new(0)),
            mock_signature: Signature::from([1u8; 64]),
        }
    }
    
    /// Create a MockTxBuilder that always fails
    pub fn new_failing() -> Self {
        let mut builder = Self::new();
        builder.should_succeed = Arc::new(Mutex::new(false));
        builder
    }
    
    /// Set whether the builder should succeed
    pub async fn set_should_succeed(&self, should_succeed: bool) {
        *self.should_succeed.lock().await = should_succeed;
    }
    
    /// Get the number of buy transactions built
    pub async fn get_buy_count(&self) -> usize {
        *self.buy_count.lock().await
    }
    
    /// Get the number of sell transactions built
    pub async fn get_sell_count(&self) -> usize {
        *self.sell_count.lock().await
    }
    
    /// Build a mock buy transaction (deterministic, no network)
    pub async fn build_buy_transaction(
        &self,
        _candidate: &PremintCandidate,
        _sign: bool,
    ) -> Result<VersionedTransaction> {
        let should_succeed = *self.should_succeed.lock().await;
        
        if !should_succeed {
            return Err(anyhow::anyhow!("Mock build_buy_transaction failed (configured to fail)"));
        }
        
        *self.buy_count.lock().await += 1;
        
        // Create a deterministic placeholder transaction
        Ok(Self::create_placeholder_tx("buy"))
    }
    
    /// Build a mock sell transaction (deterministic, no network)
    pub async fn build_sell_transaction(
        &self,
        _mint: &Pubkey,
        _program: &str,
        _sell_percent: f64,
        _sign: bool,
    ) -> Result<VersionedTransaction> {
        let should_succeed = *self.should_succeed.lock().await;
        
        if !should_succeed {
            return Err(anyhow::anyhow!("Mock build_sell_transaction failed (configured to fail)"));
        }
        
        *self.sell_count.lock().await += 1;
        
        // Create a deterministic placeholder transaction
        Ok(Self::create_placeholder_tx("sell"))
    }
    
    /// Create a deterministic placeholder transaction
    fn create_placeholder_tx(action: &str) -> VersionedTransaction {
        // Create deterministic pubkeys based on action
        let from = if action == "buy" {
            Pubkey::new_from_array([1u8; 32])
        } else {
            Pubkey::new_from_array([2u8; 32])
        };
        
        let to = if action == "buy" {
            Pubkey::new_from_array([3u8; 32])
        } else {
            Pubkey::new_from_array([4u8; 32])
        };
        
        // TODO(migrate-system-instruction): temporary allow, full migration post-profit
        #[allow(deprecated)]
        let ix = system_instruction::transfer(&from, &to, 1);
        let msg = Message::new(&[ix], None);
        let tx = Transaction::new_unsigned(msg);
        VersionedTransaction::from(tx)
    }
    
    /// Reset all counters
    pub async fn reset(&self) {
        *self.buy_count.lock().await = 0;
        *self.sell_count.lock().await = 0;
        *self.should_succeed.lock().await = true;
    }
}

impl Default for MockTxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock NonceManager for testing
/// 
/// This provides a test-only nonce manager that doesn't make RPC calls
/// and has deterministic behavior.
pub mod mock_nonce {
    use super::*;
    use crate::nonce_manager::{NonceManager, NonceLease, NonceError, NonceResult};
    use std::sync::atomic::{AtomicU64, Ordering};
    
    /// Create a NonceManager configured for testing
    /// 
    /// This creates a nonce manager with:
    /// - No RPC refresh (high TTL)
    /// - Stub backend without network calls
    /// - Deterministic behavior
    pub async fn create_test_nonce_manager(pool_size: usize) -> Arc<NonceManager> {
        // Create a test nonce manager with no RPC calls
        // The manager will use in-memory nonces for testing
        Arc::new(NonceManager::new_for_testing(pool_size))
    }
    
    /// Mock NonceLease for testing
    #[derive(Clone)]
    pub struct MockNonceLease {
        nonce_pubkey: Pubkey,
        nonce_hash: Hash,
        released: Arc<Mutex<bool>>,
    }
    
    impl MockNonceLease {
        /// Create a new mock nonce lease
        pub fn new() -> Self {
            Self {
                nonce_pubkey: Pubkey::new_unique(),
                nonce_hash: Hash::new_unique(),
                released: Arc::new(Mutex::new(false)),
            }
        }
        
        /// Get the nonce pubkey
        pub fn nonce_pubkey(&self) -> &Pubkey {
            &self.nonce_pubkey
        }
        
        /// Get the nonce hash
        pub fn nonce_hash(&self) -> Hash {
            self.nonce_hash
        }
        
        /// Release the lease
        pub async fn release(self) -> Result<()> {
            let mut released = self.released.lock().await;
            if *released {
                return Err(anyhow::anyhow!("Lease already released"));
            }
            *released = true;
            Ok(())
        }
        
        /// Check if the lease is released
        pub async fn is_released(&self) -> bool {
            *self.released.lock().await
        }
    }
    
    impl Default for MockNonceLease {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mock_tx_builder_success() {
        let builder = MockTxBuilder::new();
        
        let candidate = PremintCandidate {
            mint: Pubkey::new_unique(),
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::sniffer::PriorityLevel::High,
            timestamp: 0,
            price_hint: None,
            signature: None,
        };
        
        // Should succeed
        let result = builder.build_buy_transaction(&candidate, false).await;
        assert!(result.is_ok());
        assert_eq!(builder.get_buy_count().await, 1);
        
        // Build sell
        let result = builder.build_sell_transaction(&Pubkey::new_unique(), "pump.fun", 1.0, false).await;
        assert!(result.is_ok());
        assert_eq!(builder.get_sell_count().await, 1);
    }
    
    #[tokio::test]
    async fn test_mock_tx_builder_failure() {
        let builder = MockTxBuilder::new_failing();
        
        let candidate = PremintCandidate {
            mint: Pubkey::new_unique(),
            program: "pump.fun".to_string(),
            accounts: vec![],
            priority: crate::sniffer::PriorityLevel::High,
            timestamp: 0,
            price_hint: None,
            signature: None,
        };
        
        // Should fail
        let result = builder.build_buy_transaction(&candidate, false).await;
        assert!(result.is_err());
        assert_eq!(builder.get_buy_count().await, 0);
    }
    
    #[tokio::test]
    async fn test_mock_nonce_lease() {
        let lease = mock_nonce::MockNonceLease::new();
        assert!(!lease.is_released().await);
        
        lease.clone().release().await.unwrap();
        assert!(lease.is_released().await);
    }
}
