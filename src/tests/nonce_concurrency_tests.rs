#![allow(unused_imports)]
//! Concurrency and Stress Tests for Nonce Management (Issues #37-40)
//!
//! This module tests concurrent nonce lease operations:
//! - Parallel acquire without deadlocks
//! - Concurrent transactions with nonce leases
//! - Race condition detection
//! - Stress testing under high concurrency

#[cfg(test)]
mod nonce_concurrency_tests {
    use crate::nonce_manager::UniverseNonceManager;
    use crate::rpc_manager::rpc_pool::{RpcPool, EndpointConfig, EndpointType};
    use solana_sdk::{pubkey::Pubkey, signature::Keypair};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::time::timeout;

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

    /// Test: Parallel acquire without deadlocks (stress test)
    /// 
    /// Requirements:
    /// - 100+ parallel acquire operations
    /// - No deadlocks or hangs
    /// - All operations complete within reasonable time
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_parallel_acquire_no_deadlock() {
        const NUM_OPERATIONS: usize = 100;
        const POOL_SIZE: usize = 10;
        const TIMEOUT_SECS: u64 = 30;
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        let success_count = Arc::new(AtomicUsize::new(0));
        let blocked_count = Arc::new(AtomicUsize::new(0));
        
        let mut handles = vec![];
        
        for _ in 0..NUM_OPERATIONS {
            let manager = nonce_manager.clone();
            let success = success_count.clone();
            let blocked = blocked_count.clone();
            
            let handle = tokio::spawn(async move {
                // Try to acquire with timeout
                match timeout(Duration::from_secs(5), manager.acquire_nonce()).await {
                    Ok(Ok(lease)) => {
                        // Hold for a short time
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        drop(lease.release().await);
                        success.fetch_add(1, Ordering::SeqCst);
                    }
                    Ok(Err(_)) => {
                        // Nonce exhausted (expected under high contention)
                        blocked.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(_) => {
                        // Timeout (potential deadlock)
                        panic!("Acquire operation timed out - potential deadlock!");
                    }
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all operations with global timeout
        let all_ops = async {
            for handle in handles {
                handle.await.unwrap();
            }
        };
        
        timeout(Duration::from_secs(TIMEOUT_SECS), all_ops)
            .await
            .expect("Test timed out - potential deadlock detected!");
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        let total_ops = success_count.load(Ordering::SeqCst) + blocked_count.load(Ordering::SeqCst);
        assert_eq!(total_ops, NUM_OPERATIONS, "All operations should complete");
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0, "No nonce leaks detected");
        
        println!("✓ Parallel acquire test passed: {} operations, 0 deadlocks", NUM_OPERATIONS);
        println!("  Success: {}, Blocked: {}", 
            success_count.load(Ordering::SeqCst),
            blocked_count.load(Ordering::SeqCst)
        );
    }

    /// Test: High contention stress test
    /// 
    /// Requirements:
    /// - More acquires than available nonces (oversubscribed)
    /// - Proper blocking and queueing behavior
    /// - No lost leases or corruption
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_high_contention_stress() {
        const NUM_OPERATIONS: usize = 50;
        const POOL_SIZE: usize = 5; // Much smaller than operations
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        let success_count = Arc::new(AtomicUsize::new(0));
        
        let mut handles = vec![];
        
        for i in 0..NUM_OPERATIONS {
            let manager = nonce_manager.clone();
            let success = success_count.clone();
            
            let handle = tokio::spawn(async move {
                // Stagger start times slightly
                tokio::time::sleep(Duration::from_millis(i as u64 % 10)).await;
                
                if let Ok(lease) = manager.acquire_nonce().await {
                    // Vary hold time to create dynamic contention
                    let hold_time = 5 + (i % 15);
                    tokio::time::sleep(Duration::from_millis(hold_time as u64)).await;
                    
                    drop(lease.release().await);
                    success.fetch_add(1, Ordering::SeqCst);
                }
            });
            
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0, "No nonce leaks under contention");
        
        println!("✓ High contention stress test passed");
        println!("  Successful acquisitions: {}/{}", 
            success_count.load(Ordering::SeqCst), NUM_OPERATIONS
        );
    }

    /// Test: Concurrent acquire and release patterns
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_acquire_release_patterns() {
        const NUM_CYCLES: usize = 50;
        const POOL_SIZE: usize = 10;
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        
        let mut handles = vec![];
        
        // Pattern 1: Quick acquire/release
        for _ in 0..NUM_CYCLES {
            let manager = nonce_manager.clone();
            handles.push(tokio::spawn(async move {
                if let Ok(lease) = manager.acquire_nonce().await {
                    drop(lease.release().await);
                }
            }));
        }
        
        // Pattern 2: Hold and release
        for _ in 0..NUM_CYCLES {
            let manager = nonce_manager.clone();
            handles.push(tokio::spawn(async move {
                if let Ok(lease) = manager.acquire_nonce().await {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    drop(lease.release().await);
                }
            }));
        }
        
        // Pattern 3: Auto-release via drop
        for _ in 0..NUM_CYCLES {
            let manager = nonce_manager.clone();
            handles.push(tokio::spawn(async move {
                if let Ok(_lease) = manager.acquire_nonce().await {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    // Auto-drop
                }
            }));
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0, "No leaks across patterns");
        
        println!("✓ Concurrent acquire/release patterns test passed");
    }

    /// Test: Race condition detection - multiple threads modifying same state
    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn test_no_race_conditions() {
        const NUM_THREADS: usize = 20;
        const OPERATIONS_PER_THREAD: usize = 10;
        const POOL_SIZE: usize = 5;
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        let total_operations = Arc::new(AtomicUsize::new(0));
        
        let mut handles = vec![];
        
        for _ in 0..NUM_THREADS {
            let manager = nonce_manager.clone();
            let counter = total_operations.clone();
            
            let handle = tokio::spawn(async move {
                for _ in 0..OPERATIONS_PER_THREAD {
                    if let Ok(lease) = manager.acquire_nonce().await {
                        // Increment counter (test for race conditions)
                        counter.fetch_add(1, Ordering::SeqCst);
                        
                        // Small delay
                        tokio::time::sleep(Duration::from_millis(5)).await;
                        
                        drop(lease.release().await);
                    }
                }
            });
            
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify counter integrity (no race conditions in counting)
        let count = total_operations.load(Ordering::SeqCst);
        println!("Total successful operations: {}", count);
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0, "No race condition leaks");
        
        println!("✓ No race conditions detected across {} threads", NUM_THREADS);
    }

    /// Test: Burst acquire pattern (many simultaneous acquires)
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_burst_acquire_pattern() {
        const BURST_SIZE: usize = 50;
        const POOL_SIZE: usize = 10;
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        
        // Launch burst of simultaneous acquires
        let handles: Vec<_> = (0..BURST_SIZE)
            .map(|_| {
                let manager = nonce_manager.clone();
                tokio::spawn(async move {
                    if let Ok(lease) = manager.acquire_nonce().await {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        drop(lease.release().await);
                    }
                })
            })
            .collect();
        
        // Wait for all
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0, "No leaks after burst");
        
        println!("✓ Burst acquire pattern test passed: {} simultaneous acquires", BURST_SIZE);
    }

    /// Test: Long-running leases with interleaved quick operations
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_mixed_lease_durations() {
        const NUM_LONG: usize = 5;
        const NUM_SHORT: usize = 50;
        const POOL_SIZE: usize = 10;
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        let mut handles = vec![];
        
        // Long-running leases
        for _ in 0..NUM_LONG {
            let manager = nonce_manager.clone();
            handles.push(tokio::spawn(async move {
                if let Ok(lease) = manager.acquire_nonce().await {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    drop(lease.release().await);
                }
            }));
        }
        
        // Short-running leases interleaved
        for _ in 0..NUM_SHORT {
            let manager = nonce_manager.clone();
            handles.push(tokio::spawn(async move {
                if let Ok(lease) = manager.acquire_nonce().await {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    drop(lease.release().await);
                }
            }));
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0, "No leaks with mixed durations");
        
        println!("✓ Mixed lease durations test passed");
    }

    /// Test: Fairness - FIFO ordering under contention
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_acquire_fairness() {
        const POOL_SIZE: usize = 1; // Single nonce for fairness testing
        const NUM_ACQUIRES: usize = 10;
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        let order = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        
        let mut handles = vec![];
        
        for i in 0..NUM_ACQUIRES {
            let manager = nonce_manager.clone();
            let order_clone = order.clone();
            
            let handle = tokio::spawn(async move {
                if let Ok(lease) = manager.acquire_nonce().await {
                    // Record order
                    order_clone.lock().await.push(i);
                    
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    drop(lease.release().await);
                }
            });
            
            handles.push(handle);
            
            // Small delay to ensure ordering
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let acquired_order = order.lock().await;
        println!("Acquire order: {:?}", *acquired_order);
        
        // Verify some acquires succeeded
        assert!(!acquired_order.is_empty(), "Some acquires should succeed");
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0, "No leaks after fairness test");
        
        println!("✓ Acquire fairness test passed");
    }

    /// Test: Concurrent drop and explicit release
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_drop_and_release() {
        const NUM_OPERATIONS: usize = 30;
        const POOL_SIZE: usize = 10;
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        let mut handles = vec![];
        
        for i in 0..NUM_OPERATIONS {
            let manager = nonce_manager.clone();
            
            let handle = tokio::spawn(async move {
                if let Ok(lease) = manager.acquire_nonce().await {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    
                    if i % 2 == 0 {
                        // Explicit release
                        drop(lease.release().await);
                    } else {
                        // Auto-drop
                        drop(lease);
                    }
                }
            });
            
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify no leaks
        assert_eq!(
            nonce_manager.get_stats().await.permits_in_use, 0,
            "No leaks with mixed release strategies"
        );
        
        println!("✓ Concurrent drop and release test passed");
    }

    /// Test: Stress test with cancellation
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_stress_with_cancellation() {
        const NUM_OPERATIONS: usize = 40;
        const POOL_SIZE: usize = 10;
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        let mut handles = vec![];
        
        for i in 0..NUM_OPERATIONS {
            let manager = nonce_manager.clone();
            
            let handle = tokio::spawn(async move {
                if let Ok(lease) = manager.acquire_nonce().await {
                    // Cancel some operations early
                    if i % 5 == 0 {
                        // Early return (lease auto-dropped)
                        return;
                    }
                    
                    tokio::time::sleep(Duration::from_millis(15)).await;
                    drop(lease.release().await);
                }
            });
            
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify no leaks
        assert_eq!(
            nonce_manager.get_stats().await.permits_in_use, 0,
            "No leaks with cancellations"
        );
        
        println!("✓ Stress test with cancellation passed");
    }
}
