#![allow(unused_imports)]
//! Test Helper Utilities (Issues #37-40)
//!
//! This module provides reusable test helpers:
//! - Mock NonceLease with Send+Sync and atomic state
//! - Helper for building VersionedTransaction with nonce
//! - Test utilities for creating test nonce managers
//! - Common test fixtures

#[cfg(test)]
pub mod test_helpers {
    use solana_sdk::{
        hash::Hash,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        instruction::{Instruction, AccountMeta, CompiledInstruction},
        system_instruction,
        message::{v0::Message as MessageV0, VersionedMessage},
        transaction::VersionedTransaction,
    };
    use crate::compat::get_static_account_keys;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::{Duration, Instant};
    use tokio::sync::Mutex;

    /// Mock NonceLease for testing (Send + Sync safe)
    /// 
    /// This mock lease provides:
    /// - Atomic "released" state tracking
    /// - Send + Sync traits for concurrent testing
    /// - Optional release callback
    /// - Expiry simulation
    #[derive(Clone)]
    pub struct MockNonceLease {
        nonce_pubkey: Pubkey,
        nonce_blockhash: Hash,
        last_valid_slot: u64,
        acquired_at: Instant,
        lease_timeout: Duration,
        released: Arc<AtomicBool>,
        release_count: Arc<AtomicU64>,
        release_callback: Arc<Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>>,
    }

    impl MockNonceLease {
        /// Create a new mock nonce lease
        pub fn new(
            nonce_pubkey: Pubkey,
            nonce_blockhash: Hash,
            last_valid_slot: u64,
            lease_timeout: Duration,
        ) -> Self {
            Self {
                nonce_pubkey,
                nonce_blockhash,
                last_valid_slot,
                acquired_at: Instant::now(),
                lease_timeout,
                released: Arc::new(AtomicBool::new(false)),
                release_count: Arc::new(AtomicU64::new(0)),
                release_callback: Arc::new(Mutex::new(None)),
            }
        }

        /// Create a new mock lease with release callback
        pub fn new_with_callback<F>(
            nonce_pubkey: Pubkey,
            nonce_blockhash: Hash,
            last_valid_slot: u64,
            lease_timeout: Duration,
            callback: F,
        ) -> Self
        where
            F: FnOnce() + Send + 'static,
        {
            let mut lease = Self::new(nonce_pubkey, nonce_blockhash, last_valid_slot, lease_timeout);
            lease.release_callback = Arc::new(Mutex::new(Some(Box::new(callback))));
            lease
        }

        /// Get the nonce account public key
        pub fn nonce_pubkey(&self) -> &Pubkey {
            &self.nonce_pubkey
        }

        /// Get the nonce blockhash
        pub fn nonce_blockhash(&self) -> Hash {
            self.nonce_blockhash
        }

        /// Get the last valid slot
        pub fn last_valid_slot(&self) -> u64 {
            self.last_valid_slot
        }

        /// Check if the lease is expired
        pub fn is_expired(&self) -> bool {
            self.acquired_at.elapsed() >= self.lease_timeout
        }

        /// Check if the lease has been released
        pub fn is_released(&self) -> bool {
            self.released.load(Ordering::SeqCst)
        }

        /// Get the number of times release was called
        pub fn release_count(&self) -> u64 {
            self.release_count.load(Ordering::SeqCst)
        }

        /// Explicitly release the lease
        pub async fn release(self) -> Result<(), String> {
            // Check if already released
            if self.released.swap(true, Ordering::SeqCst) {
                return Ok(()); // Already released, idempotent
            }

            // Increment release count
            self.release_count.fetch_add(1, Ordering::SeqCst);

            // Call release callback if present
            let mut callback_guard = self.release_callback.lock().await;
            if let Some(callback) = callback_guard.take() {
                callback();
            }

            Ok(())
        }

        /// Get a handle to check release state from other threads
        pub fn release_handle(&self) -> MockNonceLeaseHandle {
            MockNonceLeaseHandle {
                released: self.released.clone(),
                release_count: self.release_count.clone(),
            }
        }
    }

    impl Drop for MockNonceLease {
        fn drop(&mut self) {
            // Auto-release on drop if not already released
            if !self.released.swap(true, Ordering::SeqCst) {
                self.release_count.fetch_add(1, Ordering::SeqCst);
                
                // Try to call callback (best effort in Drop)
                if let Ok(mut callback_guard) = self.release_callback.try_lock() {
                    if let Some(callback) = callback_guard.take() {
                        callback();
                    }
                }
            }
        }
    }

    /// Handle for checking MockNonceLease state from other threads
    #[derive(Clone)]
    pub struct MockNonceLeaseHandle {
        released: Arc<AtomicBool>,
        release_count: Arc<AtomicU64>,
    }

    impl MockNonceLeaseHandle {
        /// Check if the lease has been released
        pub fn is_released(&self) -> bool {
            self.released.load(Ordering::SeqCst)
        }

        /// Get the number of times release was called
        pub fn release_count(&self) -> u64 {
            self.release_count.load(Ordering::SeqCst)
        }
    }

    /// Helper: Build a VersionedTransaction with nonce
    /// 
    /// This helper constructs a complete V0 transaction with:
    /// - advance_nonce instruction (first)
    /// - Compute budget instructions
    /// - Custom program instructions
    /// - Proper signature
    pub fn build_versioned_transaction_with_nonce(
        nonce_account: &Pubkey,
        nonce_authority: &Pubkey,
        nonce_blockhash: Hash,
        payer: &Keypair,
        program_instructions: Vec<Instruction>,
    ) -> VersionedTransaction {
        let mut instructions = vec![];
        
        // 1. advance_nonce instruction (MUST BE FIRST)
        instructions.push(system_instruction::advance_nonce_account(
            nonce_account,
            nonce_authority,
        ));
        
        // 2. Compute budget instructions (optional)
        instructions.push(Instruction::new_with_bytes(
            solana_sdk::compute_budget::id(),
            &[2, 0, 0, 0, 0, 200, 0, 0], // set_compute_unit_limit: 200k
            vec![],
        ));
        instructions.push(Instruction::new_with_bytes(
            solana_sdk::compute_budget::id(),
            &[3, 0, 0, 0, 100, 0, 0, 0], // set_compute_unit_price: 100 microlamports
            vec![],
        ));
        
        // 3. Program instructions
        instructions.extend(program_instructions);
        
        // Build message
        let message = MessageV0::try_compile(
            &payer.pubkey(),
            &instructions,
            &[],
            nonce_blockhash,
        ).expect("Failed to compile message");
        
        // Sign transaction
        VersionedTransaction::try_new(
            VersionedMessage::V0(message),
            &[payer],
        ).expect("Failed to create transaction")
    }

    /// Helper: Build a simple transfer instruction
    pub fn build_transfer_instruction(from: &Pubkey, to: &Pubkey, lamports: u64) -> Instruction {
        system_instruction::transfer(from, to, lamports)
    }

    /// Helper: Build a mock program instruction
    pub fn build_mock_program_instruction(
        program_id: Pubkey,
        data: Vec<u8>,
        accounts: Vec<AccountMeta>,
    ) -> Instruction {
        Instruction::new_with_bytes(program_id, &data, accounts)
    }

    /// Helper: Create a test keypair
    pub fn create_test_keypair() -> Keypair {
        Keypair::new()
    }

    /// Helper: Create multiple test keypairs
    pub fn create_test_keypairs(count: usize) -> Vec<Keypair> {
        (0..count).map(|_| Keypair::new()).collect()
    }

    /// Helper: Create test pubkeys
    pub fn create_test_pubkeys(count: usize) -> Vec<Pubkey> {
        (0..count).map(|_| Pubkey::new_unique()).collect()
    }

    /// Test fixture: Default nonce configuration
    pub struct NonceTestConfig {
        pub pool_size: usize,
        pub lease_timeout: Duration,
        pub rpc_url: String,
    }

    impl Default for NonceTestConfig {
        fn default() -> Self {
            Self {
                pool_size: 5,
                lease_timeout: Duration::from_secs(300),
                rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            }
        }
    }

    impl NonceTestConfig {
        /// Create with custom pool size
        pub fn with_pool_size(mut self, pool_size: usize) -> Self {
            self.pool_size = pool_size;
            self
        }

        /// Create with custom lease timeout
        pub fn with_lease_timeout(mut self, timeout: Duration) -> Self {
            self.lease_timeout = timeout;
            self
        }

        /// Create with custom RPC URL
        pub fn with_rpc_url(mut self, url: String) -> Self {
            self.rpc_url = url;
            self
        }
    }

    /// Helper: Verify instruction ordering in a transaction
    pub fn verify_nonce_transaction_ordering(instructions: &[Instruction]) -> Result<(), String> {
        if instructions.is_empty() {
            return Err("No instructions".to_string());
        }

        // First instruction must be advance_nonce_account
        let first = &instructions[0];
        if first.program_id != solana_sdk::system_program::id() {
            return Err(format!(
                "First instruction must be system program, got: {}",
                first.program_id
            ));
        }

        // Check for advance_nonce discriminator (4)
        if first.data.is_empty() || first.data[0] != 4 {
            return Err("First instruction must be advance_nonce_account".to_string());
        }

        // Verify no other advance_nonce instructions
        for (idx, ix) in instructions.iter().enumerate().skip(1) {
            if ix.program_id == solana_sdk::system_program::id() &&
               !ix.data.is_empty() &&
               ix.data[0] == 4 {
                return Err(format!(
                    "advance_nonce_account found at position {} (should only be first)",
                    idx
                ));
            }
        }

        Ok(())
    }

    /// Helper: Assert nonce transaction is valid
    pub fn assert_valid_nonce_transaction(tx: &VersionedTransaction) {
        // Extract instructions from message - both Legacy and V0 have CompiledInstruction
        let (compiled_instructions, account_keys) = match &tx.message {
            VersionedMessage::V0(msg) => (&msg.instructions, get_static_account_keys(&tx.message)),
            VersionedMessage::Legacy(msg) => (&msg.instructions, get_static_account_keys(&tx.message)),
        };
        
        // Convert compiled instructions to regular instructions for verification
        let instructions: Vec<Instruction> = compiled_instructions.iter().map(|ix| {
            let program_id = account_keys[ix.program_id_index as usize];
            Instruction {
                program_id,
                accounts: vec![], // Simplified for testing
                data: ix.data.clone(),
            }
        }).collect();

        // Verify ordering
        verify_nonce_transaction_ordering(&instructions)
            .expect("Invalid nonce transaction ordering");

        // Verify transaction is signed
        assert!(!tx.signatures.is_empty(), "Transaction must be signed");
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_mock_nonce_lease_creation() {
            let lease = MockNonceLease::new(
                Pubkey::new_unique(),
                Hash::new_unique(),
                1000,
                Duration::from_secs(60),
            );

            assert!(!lease.is_released());
            assert!(!lease.is_expired());
            assert_eq!(lease.release_count(), 0);
        }

        #[tokio::test]
        async fn test_mock_nonce_lease_release() {
            let release_called = Arc::new(AtomicBool::new(false));
            let release_called_clone = release_called.clone();

            let lease = MockNonceLease::new_with_callback(
                Pubkey::new_unique(),
                Hash::new_unique(),
                1000,
                Duration::from_secs(60),
                move || {
                    release_called_clone.store(true, Ordering::SeqCst);
                },
            );

            let handle = lease.release_handle();

            // Release lease
            lease.release().await.unwrap();

            // Verify release
            assert!(handle.is_released());
            assert_eq!(handle.release_count(), 1);
            assert!(release_called.load(Ordering::SeqCst));
        }

        #[tokio::test]
        async fn test_mock_nonce_lease_idempotent_release() {
            let release_count = Arc::new(AtomicU64::new(0));
            let release_count_clone = release_count.clone();

            let lease = MockNonceLease::new_with_callback(
                Pubkey::new_unique(),
                Hash::new_unique(),
                1000,
                Duration::from_secs(60),
                move || {
                    release_count_clone.fetch_add(1, Ordering::SeqCst);
                },
            );

            // Release multiple times
            lease.clone().release().await.unwrap();
            lease.clone().release().await.unwrap();

            // Callback should only be called once
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert_eq!(release_count.load(Ordering::SeqCst), 1);
        }

        #[test]
        fn test_build_versioned_transaction_with_nonce() {
            let nonce_account = Pubkey::new_unique();
            let nonce_authority = Pubkey::new_unique();
            let nonce_blockhash = Hash::new_unique();
            let payer = Keypair::new();

            let program_instructions = vec![
                system_instruction::transfer(&payer.pubkey(), &Pubkey::new_unique(), 1_000_000)
            ];

            let tx = build_versioned_transaction_with_nonce(
                &nonce_account,
                &nonce_authority,
                nonce_blockhash,
                &payer,
                program_instructions,
            );

            // Verify transaction structure
            assert!(!tx.signatures.is_empty());
            assert_valid_nonce_transaction(&tx);
        }

        #[test]
        fn test_nonce_test_config_builder() {
            let config = NonceTestConfig::default()
                .with_pool_size(10)
                .with_lease_timeout(Duration::from_secs(120));

            assert_eq!(config.pool_size, 10);
            assert_eq!(config.lease_timeout, Duration::from_secs(120));
        }

        #[test]
        fn test_verify_valid_nonce_ordering() {
            let nonce_account = Pubkey::new_unique();
            let nonce_authority = Pubkey::new_unique();

            let instructions = vec![
                system_instruction::advance_nonce_account(&nonce_account, &nonce_authority),
                Instruction::new_with_bytes(
                    solana_sdk::compute_budget::id(),
                    &[2, 0, 0, 0, 0, 200, 0, 0],
                    vec![],
                ),
            ];

            let result = verify_nonce_transaction_ordering(&instructions);
            assert!(result.is_ok());
        }

        #[test]
        fn test_verify_invalid_nonce_ordering() {
            let instructions = vec![
                Instruction::new_with_bytes(
                    Pubkey::new_unique(),
                    &[1, 2, 3],
                    vec![],
                ),
            ];

            let result = verify_nonce_transaction_ordering(&instructions);
            assert!(result.is_err());
        }
    }
}
