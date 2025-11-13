#![allow(unused_imports)]
//! Tests for Task 1: NonceLease semantics and RAII
//!
//! Validates:
//! - Lease creation with all required fields (nonce_pubkey, nonce_blockhash, lease_expiry)
//! - Auto-release on Drop (RAII semantics)
//! - Explicit release (idempotent)
//! - Lease expiry detection (TTL-based)
//! - Watchdog monitoring and reclamation

#[cfg(test)]
mod nonce_lease_tests {
    //! Unit tests for NonceLease implementation
    //!
    //! These tests verify the Task 1 requirements:
    //! 1. NonceLease contains nonce_pubkey, last_valid_slot, and lease_expiry
    //! 2. RAII semantics with auto-release on Drop
    //! 3. Watchdog monitors and reclaims expired leases
    //! 4. TTL-based expiry detection

    use crate::nonce_manager::nonce_lease::LeaseWatchdog;
    use crate::nonce_manager::{NonceError, NonceLease, NonceResult};
    use solana_sdk::{hash::Hash, pubkey::Pubkey};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_lease_contains_nonce_blockhash() {
        // Task 1: Verify lease includes nonce_blockhash
        let nonce_pubkey = Pubkey::new_unique();
        let nonce_blockhash = Hash::new_unique();
        let last_valid_slot = 1000;

        let lease = NonceLease::new(
            nonce_pubkey,
            last_valid_slot,
            nonce_blockhash,
            Duration::from_secs(60),
            || {},
        );

        assert_eq!(lease.nonce_pubkey(), &nonce_pubkey);
        assert_eq!(lease.nonce_blockhash(), nonce_blockhash);
        assert_eq!(lease.last_valid_slot(), last_valid_slot);
    }

    #[tokio::test]
    async fn test_lease_expiry_detection() {
        // Task 1: Verify lease expiry detection
        let nonce_pubkey = Pubkey::new_unique();
        let nonce_blockhash = Hash::new_unique();

        let lease = NonceLease::new(
            nonce_pubkey,
            1000,
            nonce_blockhash,
            Duration::from_millis(100),
            || {},
        );

        assert!(
            !lease.is_expired(),
            "Lease should not be expired immediately"
        );
        assert!(
            lease.time_remaining().is_some(),
            "Should have time remaining"
        );

        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(150)).await;

        assert!(lease.is_expired(), "Lease should be expired after TTL");
        assert!(
            lease.time_remaining().is_none(),
            "Should have no time remaining"
        );
    }

    #[tokio::test]
    async fn test_lease_auto_release_on_drop() {
        // Task 1: Verify RAII auto-release
        let released = Arc::new(AtomicU32::new(0));
        let released_clone = released.clone();

        {
            let _lease = NonceLease::new(
                Pubkey::new_unique(),
                1000,
                Hash::new_unique(),
                Duration::from_secs(60),
                move || {
                    released_clone.fetch_add(1, Ordering::SeqCst);
                },
            );

            assert_eq!(released.load(Ordering::SeqCst), 0, "Not released yet");
        } // Lease dropped here

        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(
            released.load(Ordering::SeqCst),
            1,
            "Should be auto-released on drop"
        );
    }

    #[tokio::test]
    async fn test_watchdog_reclaims_expired_lease() {
        // Task 1: Verify watchdog detects and reclaims expired leases
        let expired_count = Arc::new(AtomicU32::new(0));
        let expired_count_clone = expired_count.clone();

        let watchdog = Arc::new(LeaseWatchdog::new(
            Duration::from_millis(50),  // Check every 50ms
            Duration::from_millis(100), // Timeout after 100ms
        ));

        watchdog
            .clone()
            .start(move |_pubkey| {
                expired_count_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        let nonce_pubkey = Pubkey::new_unique();
        let released = Arc::new(RwLock::new(false));

        watchdog
            .register_lease(nonce_pubkey, Instant::now(), released.clone())
            .await;

        assert_eq!(watchdog.active_lease_count().await, 1);

        // Wait for watchdog to detect expiry
        tokio::time::sleep(Duration::from_millis(200)).await;

        assert!(
            expired_count.load(Ordering::SeqCst) > 0,
            "Watchdog should detect expiry"
        );
        assert_eq!(
            watchdog.active_lease_count().await,
            0,
            "Lease should be removed"
        );

        watchdog.stop().await;
    }
}
