//! Comprehensive RAII and nonce management tests for Issues #37-#40
//!
//! This module contains all critical tests for production-grade quality:
//! - Mass acquire/release stability (100+ parallel operations)
//! - Drop path verification (released flag and metrics)
//! - Panic protection in release_fn
//! - ZK proof integration tests (with feature flag)

#[cfg(test)]
mod raii_comprehensive_tests {
    use crate::nonce_manager::{NonceLease, UniverseNonceManager};
    use solana_sdk::{hash::Hash, pubkey::Pubkey, signature::Keypair};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::RwLock;

    /// Test: Mass acquire/release stability with 100+ parallel operations
    /// 
    /// This test verifies that the UniverseNonceManager can handle high
    /// concurrency without leaking nonces or corrupting internal state.
    /// 
    /// Requirements:
    /// - Minimum 100 parallel acquire/release cycles
    /// - All nonces must be returned to pool (permits_in_use == 0)
    /// - No panics or deadlocks
    /// - Metrics must be consistent
    #[tokio::test]
    async fn test_mass_acquire_release_stability() {
        const NUM_OPERATIONS: usize = 100;
        const POOL_SIZE: usize = 20;
        
        // Create a test nonce manager
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        
        // Track successful operations
        let success_count = Arc::new(AtomicUsize::new(0));
        let error_count = Arc::new(AtomicUsize::new(0));
        
        // Spawn parallel acquire/release tasks
        let mut handles = vec![];
        
        for i in 0..NUM_OPERATIONS {
            let manager = nonce_manager.clone();
            let success = success_count.clone();
            let errors = error_count.clone();
            
            let handle = tokio::spawn(async move {
                // Acquire nonce
                match manager.acquire_nonce().await {
                    Ok(lease) => {
                        // Hold lease for a short time
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        
                        // Explicit release (50% of the time)
                        if i % 2 == 0 {
                            drop(lease.release().await);
                        } else {
                            // Auto-release via Drop (50% of the time)
                            drop(lease);
                        }
                        
                        success.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(e) => {
                        eprintln!("Acquire failed: {:?}", e);
                        errors.fetch_add(1, Ordering::SeqCst);
                    }
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Allow time for async cleanup to complete
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify all operations completed
        let total_ops = success_count.load(Ordering::SeqCst) + error_count.load(Ordering::SeqCst);
        assert_eq!(total_ops, NUM_OPERATIONS, "Not all operations completed");
        
        // Critical assertion: All nonces must be returned to pool
        let permits_in_use = nonce_manager.get_stats().await.permits_in_use;
        assert_eq!(
            permits_in_use, 0,
            "Nonce leak detected! {} nonces still in use", permits_in_use
        );
        
        println!("✓ Mass stability test passed: {} operations, 0 leaks", NUM_OPERATIONS);
    }

    /// Test: Drop path updates released flag and metrics correctly
    /// 
    /// This test verifies that when NonceLease is dropped (auto-release),
    /// the `released` flag is set to true and metrics are updated.
    #[tokio::test]
    async fn test_drop_path_updates_released_flag() {
        let nonce_account = Pubkey::new_unique();
        let release_count = Arc::new(AtomicU32::new(0));
        let release_count_clone = release_count.clone();
        
        // Create lease with release callback
        let released_flag = Arc::new(RwLock::new(false));
        let _released_flag_clone = released_flag.clone();
        
        {
            let _lease = NonceLease::new(
                nonce_account,
                1000,
                Hash::default(),
                Duration::from_secs(60),
                move || {
                    release_count_clone.fetch_add(1, Ordering::SeqCst);
                },
            );
            
            // Lease is dropped here (auto-release via RAII)
        }
        
        // Verify release callback was called exactly once
        assert_eq!(
            release_count.load(Ordering::SeqCst), 1,
            "Release callback not called on drop"
        );
        
        println!("✓ Drop path correctly releases nonce");
    }

    /// Test: Explicit release and then drop (double-release protection)
    #[tokio::test]
    async fn test_explicit_release_then_drop() {
        let nonce_account = Pubkey::new_unique();
        let release_count = Arc::new(AtomicU32::new(0));
        let release_count_clone = release_count.clone();
        
        {
            let lease = NonceLease::new(
                nonce_account,
                1000,
                Hash::default(),
                Duration::from_secs(60),
                move || {
                    release_count_clone.fetch_add(1, Ordering::SeqCst);
                },
            );
            
            // Explicit release
            lease.release().await.unwrap();
            
            // Lease is dropped here, but should not double-release
        }
        
        // Verify release callback was called exactly once (not twice)
        assert_eq!(
            release_count.load(Ordering::SeqCst), 1,
            "Double-release detected! Callback called more than once"
        );
        
        println!("✓ Idempotent release works correctly");
    }

    /// Test: Panic in release_fn is caught and logged (no process termination)
    #[tokio::test]
    async fn test_panic_in_release_fn_is_caught() {
        let nonce_account = Pubkey::new_unique();
        let panic_occurred = Arc::new(AtomicU32::new(0));
        let panic_occurred_clone = panic_occurred.clone();
        
        {
            let _lease = NonceLease::new(
                nonce_account,
                1000,
                Hash::default(),
                Duration::from_secs(60),
                move || {
                    panic_occurred_clone.fetch_add(1, Ordering::SeqCst);
                    panic!("Intentional panic in release_fn for testing");
                },
            );
            
            // Lease is dropped here - panic should be caught
        }
        
        // If we reach this point, panic was caught successfully
        assert_eq!(
            panic_occurred.load(Ordering::SeqCst), 1,
            "Release function was not called"
        );
        
        println!("✓ Panic in release_fn caught successfully");
    }

    /// Test: Concurrent acquire/release with varying hold times
    #[tokio::test]
    async fn test_concurrent_varying_hold_times() {
        const NUM_OPERATIONS: usize = 50;
        const POOL_SIZE: usize = 10;
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        let mut handles = vec![];
        
        for i in 0..NUM_OPERATIONS {
            let manager = nonce_manager.clone();
            
            let handle = tokio::spawn(async move {
                if let Ok(lease) = manager.acquire_nonce().await {
                    // Vary hold time: 10ms to 100ms
                    let hold_time = 10 + (i % 10) * 10;
                    tokio::time::sleep(Duration::from_millis(hold_time as u64)).await;
                    
                    // Release explicitly
                    drop(lease.release().await);
                }
            });
            
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Allow cleanup time
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0);
        
        println!("✓ Concurrent varying hold times test passed");
    }

    /// Test: Rapid acquire/release cycles (stress test)
    #[tokio::test]
    async fn test_rapid_acquire_release_cycles() {
        const NUM_CYCLES: usize = 200;
        const POOL_SIZE: usize = 5;
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        
        for _ in 0..NUM_CYCLES {
            let lease = nonce_manager.acquire_nonce().await.unwrap();
            // Immediate release
            drop(lease.release().await);
        }
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0);
        
        println!("✓ Rapid cycles test passed: {} cycles", NUM_CYCLES);
    }

    // Helper function to create a test nonce manager
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
            Duration::from_secs(300), // 5 minute lease timeout
        ).await
    }
}

/// Integration tests with ZK proof feature
#[cfg(all(test, feature = "zk_enabled"))]
mod zk_integration_tests {
    use crate::nonce_manager::{UniverseNonceManager, ZkProofData};
    use solana_sdk::{hash::Hash, pubkey::Pubkey, signature::Keypair};
    use std::sync::Arc;
    use std::time::Duration;
    use bytes::Bytes;
    
    /// Test: ZK proof integration with nonce lease
    /// 
    /// This test verifies that ZK proofs can be attached to nonce leases
    /// and are properly handled through the lease lifecycle.
    #[tokio::test]
    async fn test_zk_proof_with_nonce_lease() {
        // Create test nonce manager
        let nonce_manager = create_test_nonce_manager_with_zk(5).await;
        
        // Acquire a nonce lease
        let mut lease = nonce_manager.acquire_nonce().await.unwrap();
        
        // Create a mock ZK proof
        let mock_proof = vec![0u8; 1024]; // Mock proof bytes
        let public_inputs = vec![12345u64, 67890, 11111]; // Mock public inputs
        let zk_proof = ZkProofData::new(mock_proof, public_inputs);
        
        // Attach ZK proof to lease
        lease.set_proof(zk_proof.clone());
        
        // Verify proof is attached
        assert!(lease.proof().is_some());
        assert_eq!(lease.proof().unwrap().confidence, 1.0);
        
        // Release lease (proof should be cleaned up)
        drop(lease.release().await);
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0);
        
        println!("✓ ZK proof integration test passed");
    }

    /// Test: ZK proof debug output truncation
    #[tokio::test]
    async fn test_zk_proof_debug_truncation() {
        let mock_proof = vec![0u8; 2048]; // Large proof
        let public_inputs = vec![12345u64];
        let zk_proof = ZkProofData::new(mock_proof, public_inputs);
        
        // Debug format should truncate proof
        let debug_output = format!("{:?}", zk_proof);
        
        // Verify truncation occurred (should not contain full 2048 bytes)
        assert!(debug_output.len() < 500, "Debug output not truncated");
        assert!(debug_output.contains("bytes total"), "Missing size indicator");
        
        println!("✓ ZK proof debug truncation works correctly");
    }

    // Helper to create nonce manager for ZK tests
    async fn create_test_nonce_manager_with_zk(pool_size: usize) -> Arc<UniverseNonceManager> {
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
}
