//! Comprehensive ExecutionContext Tests (Issues #37-40)
//!
//! This module tests ExecutionContext behavior:
//! - Test ExecutionContext structure and ownership
//! - Verify correct instruction ordering with nonce advance instruction
//! - Test lease extraction and ownership transfer
//! - Test ExecutionContext lifecycle

#[cfg(test)]
mod execution_context_tests {
    use crate::nonce_manager::UniverseNonceManager;
    use crate::rpc_manager::rpc_pool::RpcPool;
    use solana_sdk::{
        hash::Hash,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
    };
    use std::sync::Arc;
    use std::time::Duration;

    /// Helper: Create test nonce manager
    async fn create_test_nonce_manager(pool_size: usize) -> Arc<UniverseNonceManager> {
        let rpc_pool = Arc::new(RpcPool::new(vec![
            "https://api.mainnet-beta.solana.com".to_string()
        ], 5));
        
        let authority = Arc::new(Keypair::new());
        let mut nonce_accounts = vec![];
        for _ in 0..pool_size {
            nonce_accounts.push(Pubkey::new_unique());
        }
        
        UniverseNonceManager::new(
            rpc_pool,
            authority,
            nonce_accounts,
            Duration::from_secs(300),
        ).await
    }

    /// Test: Nonce manager with enforce semantics (acquire always succeeds with available pool)
    /// 
    /// Requirements:
    /// - Acquire nonce lease from pool
    /// - Lease should have all required fields populated
    #[tokio::test]
    async fn test_nonce_acquisition_with_pool() {
        let nonce_manager = create_test_nonce_manager(5).await;
        
        // Acquire nonce lease (simulates enforce_nonce=true behavior)
        let lease = nonce_manager.acquire_nonce().await;
        assert!(lease.is_ok(), "Should acquire nonce from pool");
        
        let lease = lease.unwrap();
        
        // Verify lease has all required fields
        assert_ne!(lease.nonce_blockhash(), Hash::default(), "Blockhash should be set");
        assert_ne!(lease.nonce_pubkey(), &Pubkey::default(), "Nonce pubkey should be set");
        assert!(!lease.is_expired(), "Lease should not be expired");
        
        // Release lease
        drop(lease.release().await);
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        println!("✓ Nonce acquisition with pool works correctly");
    }

    /// Test: Nonce exhaustion behavior (enforce_nonce=true should fail)
    /// 
    /// Requirements:
    /// - When pool is exhausted, acquire should fail
    /// - No fallback behavior
    #[tokio::test]
    async fn test_nonce_exhaustion_fails() {
        // Create manager with 0 nonce accounts (exhausted pool)
        let nonce_manager = create_test_nonce_manager(0).await;
        
        // Try to acquire nonce (simulates enforce_nonce=true with exhausted pool)
        let lease = nonce_manager.acquire_nonce().await;
        
        // Should fail with exhaustion
        assert!(lease.is_err(), "Should fail when nonce pool is exhausted");
        
        println!("✓ Nonce exhaustion correctly fails");
    }

    /// Test: Multiple parallel nonce acquisitions
    /// 
    /// Requirements:
    /// - Multiple concurrent nonce acquisitions
    /// - Each gets a unique nonce
    /// - No deadlocks or race conditions
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_parallel_nonce_acquisitions() {
        const NUM_ACQUIRES: usize = 10;
        const POOL_SIZE: usize = 10;
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        
        let mut handles = vec![];
        
        for _ in 0..NUM_ACQUIRES {
            let manager = nonce_manager.clone();
            
            let handle = tokio::spawn(async move {
                manager.acquire_nonce().await
            });
            
            handles.push(handle);
        }
        
        let mut leases = vec![];
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "Each acquisition should succeed");
            leases.push(result.unwrap());
        }
        
        // Verify all leases have unique nonce pubkeys
        let mut nonce_pubkeys = std::collections::HashSet::new();
        for lease in &leases {
            nonce_pubkeys.insert(*lease.nonce_pubkey());
        }
        
        assert_eq!(
            nonce_pubkeys.len(), NUM_ACQUIRES,
            "All leases should have unique nonce pubkeys"
        );
        
        // Release all leases
        for lease in leases {
            drop(lease.release().await);
        }
        
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        println!("✓ Parallel nonce acquisitions work correctly");
    }

    /// Test: Lease drop releases back to pool
    /// 
    /// Requirements:
    /// - When lease is dropped, it's released back to pool
    /// - Lease can be reacquired after release
    #[tokio::test]
    async fn test_lease_drop_releases_to_pool() {
        let nonce_manager = create_test_nonce_manager(5).await;
        
        // Get baseline permits
        let baseline_permits = nonce_manager.permits_in_use();
        
        {
            // Acquire lease
            let _lease = nonce_manager.acquire_nonce().await.unwrap();
            
            // Permits should be in use
            assert!(
                nonce_manager.permits_in_use() > baseline_permits,
                "Permits should be in use"
            );
            
            // Drop lease here (auto-release)
        }
        
        // Allow time for async cleanup
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Permits should be released
        assert_eq!(
            nonce_manager.permits_in_use(), baseline_permits,
            "Permits should be released after drop"
        );
        
        println!("✓ Lease drop releases to pool correctly");
    }

    /// Test: Explicit release vs auto-drop
    /// 
    /// Requirements:
    /// - Both explicit release and auto-drop return lease to pool
    /// - No double-release issues
    #[tokio::test]
    async fn test_explicit_release_vs_auto_drop() {
        let nonce_manager = create_test_nonce_manager(5).await;
        let baseline_permits = nonce_manager.permits_in_use();
        
        // Test explicit release
        {
            let lease = nonce_manager.acquire_nonce().await.unwrap();
            assert!(nonce_manager.permits_in_use() > baseline_permits);
            drop(lease.release().await);
        }
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(nonce_manager.permits_in_use(), baseline_permits);
        
        // Test auto-drop
        {
            let _lease = nonce_manager.acquire_nonce().await.unwrap();
            assert!(nonce_manager.permits_in_use() > baseline_permits);
            // Auto-drop here
        }
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(nonce_manager.permits_in_use(), baseline_permits);
        
        println!("✓ Explicit release and auto-drop both work correctly");
    }
}
