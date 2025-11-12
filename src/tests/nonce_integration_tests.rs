//! Integration Tests for Nonce Transaction Building (Issues #37-40)
//!
//! This module provides end-to-end integration tests:
//! - Success paths with proper lease release
//! - Error paths with proper lease cleanup
//! - Full transaction building with nonce
//! - Real-world scenarios

#[cfg(test)]
mod nonce_integration_tests {
    use crate::nonce_manager::UniverseNonceManager;
    use crate::rpc_manager::rpc_pool::{RpcPool, EndpointConfig, EndpointType};
    use solana_sdk::{
        hash::Hash,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        instruction::Instruction,
        system_instruction,
        message::{v0::Message as MessageV0, VersionedMessage},
        transaction::VersionedTransaction,
    };
    use std::sync::Arc;
    use std::time::Duration;

    /// Helper: Create test nonce manager
    async fn create_test_nonce_manager(pool_size: usize) -> Arc<UniverseNonceManager> {
        use crate::nonce_manager::{UniverseNonceManager, LocalSigner};
        
        let signer = Arc::new(LocalSigner::new(Keypair::new()));
        let mut nonce_accounts = vec![];
        for _ in 0..pool_size {
            nonce_accounts.push(Pubkey::new_unique());
        }
        
        UniverseNonceManager::new_for_testing(
            signer,
            nonce_accounts,
            Duration::from_secs(300),
        ).await
    }

    /// Helper: Build a complete VersionedTransaction with nonce
    fn build_versioned_transaction_with_nonce(
        nonce_account: &Pubkey,
        nonce_authority: &Pubkey,
        nonce_blockhash: Hash,
        payer: &Keypair,
    ) -> VersionedTransaction {
        let mut instructions = vec![];
        
        // 1. advance_nonce instruction (MUST BE FIRST)
        instructions.push(system_instruction::advance_nonce_account(
            nonce_account,
            nonce_authority,
        ));
        
        // 2. Compute budget instructions
        instructions.push(Instruction::new_with_bytes(
            solana_sdk::compute_budget::id(),
            &[2, 0, 0, 0, 0, 200, 0, 0], // set_compute_unit_limit
            vec![],
        ));
        
        // 3. Simple transfer instruction (example)
        instructions.push(system_instruction::transfer(
            &payer.pubkey(),
            &Pubkey::new_unique(),
            1_000_000, // 0.001 SOL
        ));
        
        // Build message
        let message = MessageV0::try_compile(
            &payer.pubkey(),
            &instructions,
            &[],
            nonce_blockhash, // Use nonce blockhash
        ).unwrap();
        
        // Sign transaction
        VersionedTransaction::try_new(
            VersionedMessage::V0(message),
            &[payer],
        ).unwrap()
    }

    /// Test: End-to-end transaction building with nonce (success path)
    #[tokio::test]
    async fn test_e2e_transaction_with_nonce_success() {
        let nonce_manager = create_test_nonce_manager(5).await;
        let payer = Keypair::new();
        
        // Acquire nonce lease
        let lease = nonce_manager.acquire_nonce().await.unwrap();
        let nonce_pubkey = *lease.nonce_pubkey();
        let nonce_authority = Keypair::new().pubkey(); // Mock authority
        let nonce_blockhash = lease.nonce_blockhash();
        
        // Build transaction
        let tx = build_versioned_transaction_with_nonce(
            &nonce_pubkey,
            &nonce_authority,
            nonce_blockhash,
            &payer,
        );
        
        // Verify transaction structure
        assert!(tx.signatures.len() > 0, "Transaction should be signed");
        
        let message = match &tx.message {
            VersionedMessage::V0(msg) => msg,
            _ => panic!("Expected V0 message"),
        };
        
        assert!(!message.instructions.is_empty(), "Should have instructions");
        
        // Verify blockhash matches nonce
        assert_eq!(message.recent_blockhash, nonce_blockhash);
        
        // Explicitly release lease
        drop(lease.release().await);
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0);
        
        println!("✓ E2E transaction with nonce (success path) passed");
    }

    /// Test: Error path - transaction building fails, lease is cleaned up
    #[tokio::test]
    async fn test_e2e_transaction_error_cleanup() {
        let nonce_manager = create_test_nonce_manager(5).await;
        
        // Acquire nonce lease
        let lease = nonce_manager.acquire_nonce().await.unwrap();
        
        // Simulate error during transaction building
        // (lease goes out of scope without explicit release)
        drop(lease);
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Verify lease was auto-released (no leak)
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0);
        
        println!("✓ E2E error path with cleanup passed");
    }

    /// Test: Multiple transactions in sequence
    #[tokio::test]
    async fn test_sequential_transactions_with_nonce() {
        const NUM_TRANSACTIONS: usize = 10;
        
        let nonce_manager = create_test_nonce_manager(5).await;
        let payer = Keypair::new();
        
        for i in 0..NUM_TRANSACTIONS {
            // Acquire lease
            let lease = nonce_manager.acquire_nonce().await.unwrap();
            let nonce_pubkey = *lease.nonce_pubkey();
            let nonce_authority = Keypair::new().pubkey();
            let nonce_blockhash = lease.nonce_blockhash();
            
            // Build transaction
            let _tx = build_versioned_transaction_with_nonce(
                &nonce_pubkey,
                &nonce_authority,
                nonce_blockhash,
                &payer,
            );
            
            // Release lease
            drop(lease.release().await);
            
            // Small delay
            tokio::time::sleep(Duration::from_millis(10)).await;
            
            // Verify no leaks after each iteration
            assert_eq!(
                nonce_manager.get_stats().await.permits_in_use, 0,
                "Leak detected at iteration {}", i
            );
        }
        
        println!("✓ Sequential transactions test passed: {} transactions", NUM_TRANSACTIONS);
    }

    /// Test: Parallel transaction building
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_parallel_transaction_building() {
        const NUM_TRANSACTIONS: usize = 20;
        const POOL_SIZE: usize = 10;
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        let mut handles = vec![];
        
        for _ in 0..NUM_TRANSACTIONS {
            let manager = nonce_manager.clone();
            
            let handle = tokio::spawn(async move {
                let payer = Keypair::new();
                
                // Acquire lease
                if let Ok(lease) = manager.acquire_nonce().await {
                    let nonce_pubkey = *lease.nonce_pubkey();
                    let nonce_authority = Keypair::new().pubkey();
                    let nonce_blockhash = lease.nonce_blockhash();
                    
                    // Build transaction
                    let _tx = build_versioned_transaction_with_nonce(
                        &nonce_pubkey,
                        &nonce_authority,
                        nonce_blockhash,
                        &payer,
                    );
                    
                    // Release lease
                    drop(lease.release().await);
                }
            });
            
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0);
        
        println!("✓ Parallel transaction building test passed");
    }

    /// Test: Transaction with early return (error simulation)
    #[tokio::test]
    async fn test_transaction_early_return_cleanup() {
        let nonce_manager = create_test_nonce_manager(5).await;
        
        // Simulate function that acquires lease but returns early on error
        async fn build_transaction_with_error(
            manager: Arc<UniverseNonceManager>,
        ) -> Result<(), String> {
            let _lease = manager.acquire_nonce().await.map_err(|e| format!("{:?}", e))?;
            
            // Simulate error before transaction is built
            return Err("Simulated error".to_string());
            
            // Lease is auto-dropped here
        }
        
        let result = build_transaction_with_error(nonce_manager.clone()).await;
        assert!(result.is_err(), "Should return error");
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Verify lease was cleaned up
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0);
        
        println!("✓ Early return cleanup test passed");
    }

    /// Test: Transaction building with panic recovery
    #[tokio::test]
    async fn test_transaction_panic_recovery() {
        let nonce_manager = create_test_nonce_manager(5).await;
        
        // Spawn task that panics while holding lease
        let manager_clone = nonce_manager.clone();
        let handle = tokio::spawn(async move {
            let _lease = manager_clone.acquire_nonce().await.unwrap();
            
            // Simulate panic
            panic!("Intentional panic for testing");
        });
        
        // Expect panic
        let result = handle.await;
        assert!(result.is_err(), "Task should panic");
        
        // Allow cleanup (lease should be dropped despite panic)
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Verify lease was cleaned up
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0);
        
        println!("✓ Panic recovery test passed");
    }

    /// Test: Long-running transaction building with lease expiry
    #[tokio::test]
    async fn test_lease_expiry_during_transaction() {
        use crate::nonce_manager::LocalSigner;
        
        // Create manager with short lease timeout
        let signer = Arc::new(LocalSigner::new(Keypair::new()));
        let nonce_accounts = vec![Pubkey::new_unique()];
        
        let nonce_manager = UniverseNonceManager::new_for_testing(
            signer,
            nonce_accounts,
            Duration::from_millis(100), // Very short timeout
        ).await;
        
        // Acquire lease
        let lease = nonce_manager.acquire_nonce().await.unwrap();
        
        assert!(!lease.is_expired(), "Lease should not be expired initially");
        
        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        assert!(lease.is_expired(), "Lease should be expired");
        
        // Release expired lease (should still work)
        drop(lease.release().await);
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0);
        
        println!("✓ Lease expiry during transaction test passed");
    }

    /// Test: Complex transaction with multiple operations
    #[tokio::test]
    async fn test_complex_multi_operation_transaction() {
        let nonce_manager = create_test_nonce_manager(5).await;
        let payer = Keypair::new();
        
        // Acquire lease
        let lease = nonce_manager.acquire_nonce().await.unwrap();
        let nonce_pubkey = *lease.nonce_pubkey();
        let nonce_authority = Keypair::new().pubkey();
        let nonce_blockhash = lease.nonce_blockhash();
        
        // Build complex transaction with multiple instructions
        let mut instructions = vec![];
        
        // 1. advance_nonce
        instructions.push(system_instruction::advance_nonce_account(
            &nonce_pubkey,
            &nonce_authority,
        ));
        
        // 2. Compute budget
        instructions.push(Instruction::new_with_bytes(
            solana_sdk::compute_budget::id(),
            &[2, 0, 0, 0, 0, 200, 0, 0],
            vec![],
        ));
        
        // 3. Multiple transfers
        for _ in 0..5 {
            instructions.push(system_instruction::transfer(
                &payer.pubkey(),
                &Pubkey::new_unique(),
                100_000,
            ));
        }
        
        // Build message
        let message = MessageV0::try_compile(
            &payer.pubkey(),
            &instructions,
            &[],
            nonce_blockhash,
        ).unwrap();
        
        // Build transaction
        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(message),
            &[&payer],
        ).unwrap();
        
        // Verify transaction
        assert!(tx.signatures.len() > 0);
        
        // Release lease
        drop(lease.release().await);
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0);
        
        println!("✓ Complex multi-operation transaction test passed");
    }

    /// Test: Retry pattern with nonce leases
    #[tokio::test]
    async fn test_retry_pattern_with_nonce() {
        const MAX_RETRIES: usize = 3;
        
        let nonce_manager = create_test_nonce_manager(5).await;
        let payer = Keypair::new();
        
        for attempt in 0..MAX_RETRIES {
            // Acquire new lease for each retry
            let lease = nonce_manager.acquire_nonce().await.unwrap();
            let nonce_pubkey = *lease.nonce_pubkey();
            let nonce_authority = Keypair::new().pubkey();
            let nonce_blockhash = lease.nonce_blockhash();
            
            // Build transaction
            let _tx = build_versioned_transaction_with_nonce(
                &nonce_pubkey,
                &nonce_authority,
                nonce_blockhash,
                &payer,
            );
            
            // Simulate retry logic
            if attempt < MAX_RETRIES - 1 {
                // "Transaction failed", release and retry
                drop(lease.release().await);
            } else {
                // "Transaction succeeded"
                drop(lease.release().await);
            }
            
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        
        // Allow final cleanup
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0);
        
        println!("✓ Retry pattern test passed: {} retries", MAX_RETRIES);
    }
}
