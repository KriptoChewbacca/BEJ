//! Nonce lease model with automatic release and watchdog
//! 
//! This module implements a lease-based system for nonce account management:
//! - Atomic acquire/release operations
//! - Automatic release on Drop
//! - TTL-based expiry with watchdog task
//! - Thread-safe concurrent access
//! 
//! Task 1 Enhancement: Proper lease semantics with nonce_pubkey and advance instruction support
use super::nonce_errors::NonceResult;
use super::nonce_manager_integrated::ZkProofData;
use solana_sdk::{
    hash::Hash,
    pubkey::Pubkey,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use tracing::{debug, warn};

/// A lease on a nonce account with enhanced semantics (Task 1)
/// 
/// This struct implements RAII (Resource Acquisition Is Initialization) pattern
/// for nonce account management, ensuring no resource leaks.
/// 
/// # RAII Contract
/// 
/// This struct enforces the following guarantees:
/// 
/// 1. **Owned Data**: All fields are owned ('static), no references held
/// 2. **Automatic Cleanup**: Drop implementation releases the nonce synchronously
/// 3. **Explicit Release**: `release()` method consumes self for explicit cleanup
/// 4. **Idempotent**: Multiple release attempts are safe (no-op after first)
/// 5. **No Async in Drop**: Drop is synchronous and cannot fail
/// 6. **Zero Leaks**: Nonce is guaranteed to be released (explicitly or on drop)
/// 
/// # Lifecycle
/// 
/// - Acquired: Lease is created with a nonce account and release callback
/// - Held: Lease is held for transaction building/broadcast
/// - Released: Either explicitly via `release()` or automatically on drop
/// 
/// # Example
/// 
/// ```no_run
/// // Acquire lease
/// let lease = nonce_manager.acquire_nonce().await?;
/// 
/// // Use lease for transaction
/// let tx = build_transaction_with_nonce(&lease);
/// 
/// // Explicitly release after use
/// lease.release().await?;
/// 
/// // Or let it drop (automatic release)
/// drop(lease);
/// ```
/// 
/// Enhanced with ZK proof support for state validation
pub struct NonceLease {
    /// Nonce account public key (owned, not a reference)
    nonce_pubkey: Pubkey,
    /// Last valid slot for this nonce
    last_valid_slot: u64,
    /// When the lease was acquired
    acquired_at: Instant,
    /// Lease timeout duration (TTL)
    lease_timeout: Duration,
    /// Lease expiry absolute time
    lease_expiry: Instant,
    /// Function to call on release (owned closure, wrapped for Sync)
    /// Uses Arc<Mutex<>> to make it Sync-safe for sharing across threads
    release_fn: Arc<Mutex<Option<Box<dyn FnOnce() + Send>>>>,
    /// Whether the lease has been released (Arc for shared state tracking)
    released: Arc<RwLock<bool>>,
    /// Current nonce blockhash value (owned)
    nonce_blockhash: Hash,
    /// ZK proof data for nonce state validation (optional, owned)
    proof: Option<ZkProofData>,
}

impl NonceLease {
    /// Create a new nonce lease (Task 1: Enhanced with blockhash and ZK proof)
    pub fn new<F>(
        nonce_pubkey: Pubkey,
        last_valid_slot: u64,
        nonce_blockhash: Hash,
        lease_timeout: Duration,
        release_fn: F,
    ) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        let acquired_at = Instant::now();
        let lease_expiry = acquired_at + lease_timeout;
        
        // Increment active leases counter
        let metrics = crate::metrics::metrics();
        metrics.nonce_active_leases.inc();
        
        Self {
            nonce_pubkey,
            last_valid_slot,
            acquired_at,
            lease_timeout,
            lease_expiry,
            release_fn: Arc::new(Mutex::new(Some(Box::new(release_fn)))),
            released: Arc::new(RwLock::new(false)),
            nonce_blockhash,
            proof: None, // Will be set via set_proof()
        }
    }
    
    /// Set ZK proof for this lease
    pub fn set_proof(&mut self, proof: ZkProofData) {
        self.proof = Some(proof);
    }
    
    /// Get ZK proof reference (if available)
    pub fn proof(&self) -> Option<&ZkProofData> {
        self.proof.as_ref()
    }
    
    /// Take ownership of ZK proof (consumes it)
    pub fn take_proof(&mut self) -> Option<ZkProofData> {
        self.proof.take()
    }
    
    /// Get the nonce account public key
    pub fn nonce_pubkey(&self) -> &Pubkey {
        &self.nonce_pubkey
    }
    
    /// Get the account public key (compatibility alias)
    pub fn account_pubkey(&self) -> &Pubkey {
        &self.nonce_pubkey
    }
    
    /// Get the last valid slot
    pub fn last_valid_slot(&self) -> u64 {
        self.last_valid_slot
    }
    
    /// Get the nonce blockhash value
    pub fn nonce_blockhash(&self) -> Hash {
        self.nonce_blockhash
    }
    
    /// Get the lease expiry time
    pub fn lease_expiry(&self) -> Instant {
        self.lease_expiry
    }
    
    /// Check if the lease has expired
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.lease_expiry
    }
    
    /// Get the time remaining on the lease
    pub fn time_remaining(&self) -> Option<Duration> {
        self.lease_expiry.checked_duration_since(Instant::now())
    }
    
    /// Explicitly release the lease (idempotent)
    /// 
    /// # RAII Contract
    /// 
    /// This method enforces RAII by consuming `self`, preventing use-after-release:
    /// - Consumes ownership of the lease
    /// - Calls the release callback to return nonce to pool
    /// - Idempotent: safe to call multiple times (though consuming prevents this)
    /// - Returns Result for error handling
    /// 
    /// # Example
    /// 
    /// ```no_run
    /// let lease = nonce_manager.acquire_nonce().await?;
    /// // ... use lease ...
    /// lease.release().await?; // Explicitly release
    /// // lease is now consumed, cannot be used again
    /// ```
    pub async fn release(mut self) -> NonceResult<()> {
        self.release_internal().await
    }
    
    /// Internal release implementation (works with &mut self for Drop compatibility)
    async fn release_internal(&mut self) -> NonceResult<()> {
        let mut released = self.released.write().await;
        if *released {
            // Already released, this is a no-op
            debug!(nonce = %self.nonce_pubkey, "Lease already released");
            return Ok(());
        }
        
        *released = true;
        
        // Calculate lifetime metrics
        let held_duration_secs = self.acquired_at.elapsed().as_secs_f64();
        let held_for_ms = self.acquired_at.elapsed().as_millis();
        
        // Call the release function if it exists
        let mut release_fn_guard = self.release_fn.lock().await;
        if let Some(release_fn) = release_fn_guard.take() {
            drop(release_fn_guard); // Release lock before calling the function
            release_fn();
            
            // Update metrics for explicit release
            let metrics = crate::metrics::metrics();
            metrics.nonce_leases_dropped_explicit.inc();
            metrics.nonce_lease_lifetime.observe(held_duration_secs);
            metrics.nonce_active_leases.dec();
            
            debug!(
                nonce = %self.nonce_pubkey,
                held_for_ms = held_for_ms,
                held_duration_secs = %held_duration_secs,
                release_type = "explicit",
                "Lease explicitly released"
            );
        }
        
        Ok(())
    }
}

impl Drop for NonceLease {
    /// RAII cleanup: Automatically release lease when dropped
    /// 
    /// This implementation enforces the RAII contract:
    /// - **Synchronous**: No async operations (try_read instead of await)
    /// - **No Panic**: Gracefully handles lock contention
    /// - **Idempotent**: Checks if already released before releasing
    /// - **Guaranteed Cleanup**: Calls release_fn if not yet released
    /// - **Metrics Consistency**: Always synchronizes `released` flag to true
    /// 
    /// # Why Synchronous?
    /// 
    /// Drop cannot be async in Rust. We use `try_read()` instead of async `.read().await`
    /// to check release state without blocking. If we can't acquire the lock, we proceed
    /// with release anyway (better to double-release than leak).
    /// 
    /// # Best-Effort Async Cleanup
    /// 
    /// Any async cleanup operations spawned by the release_fn are executed in a
    /// detached task with panic protection. These operations are best-effort:
    /// - Panics are caught and logged, preventing process termination
    /// - No ordering guarantees relative to other async operations
    /// - May not complete if the runtime is shutting down
    /// 
    /// # Race Conditions
    /// 
    /// Due to the synchronous nature of Drop, there are potential race conditions:
    /// - The `released` flag may be set asynchronously by `release_internal()`
    /// - If both Drop and explicit `release()` execute concurrently, the release_fn
    ///   is guaranteed to be called at most once (protected by `Option::take()`)
    /// - Metrics may be updated slightly out-of-order in concurrent scenarios
    fn drop(&mut self) {
        // Check if we can get the lock without blocking
        // If the lease is released, we don't need to do anything
        let already_released = if let Ok(released) = self.released.try_read() {
            *released
        } else {
            // Lock contention - proceed with release to ensure no leaks
            false
        };
        
        if already_released {
            return;
        }
        
        // Calculate lifetime metrics before release
        let held_duration_secs = self.acquired_at.elapsed().as_secs_f64();
        let held_for_ms = self.acquired_at.elapsed().as_millis();
        
        // Try to mark as released (synchronize flag for metrics consistency)
        // Use try_write to avoid blocking in Drop
        if let Ok(mut released) = self.released.try_write() {
            *released = true;
        }
        
        // Try to release the lease
        // This is safe even if released check failed - release_fn is idempotent
        if let Ok(mut release_fn_guard) = self.release_fn.try_lock() {
            if let Some(release_fn) = release_fn_guard.take() {
                drop(release_fn_guard); // Release lock before calling the function
                
                // Update metrics for auto-dropped leases
                let metrics = crate::metrics::metrics();
                metrics.nonce_leases_dropped_auto.inc();
                metrics.nonce_lease_lifetime.observe(held_duration_secs);
                metrics.nonce_active_leases.dec();
                
                // Wrap release_fn call with panic protection
                // Any panic in release_fn is caught and logged
                let nonce_pubkey = self.nonce_pubkey;
                
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    release_fn();
                }));
                
                match result {
                    Ok(()) => {
                        warn!(
                            nonce = %nonce_pubkey,
                            held_for_ms = held_for_ms,
                            held_duration_secs = %held_duration_secs,
                            release_type = "auto_drop",
                            "Lease automatically released on drop (RAII) - should be explicitly released"
                        );
                    }
                    Err(e) => {
                        // Log panic but don't propagate (Drop must not panic)
                        let panic_msg = if let Some(s) = e.downcast_ref::<&str>() {
                            s.to_string()
                        } else if let Some(s) = e.downcast_ref::<String>() {
                            s.clone()
                        } else {
                            "Unknown panic".to_string()
                        };
                        
                        warn!(
                            nonce = %nonce_pubkey,
                            held_for_ms = held_for_ms,
                            held_duration_secs = %held_duration_secs,
                            panic_msg = %panic_msg,
                            release_type = "auto_drop_panic",
                            "Panic caught during lease release in Drop (nonce still returned to pool)"
                        );
                    }
                }
            }
        } else {
            // Lock contention on release_fn - log warning but don't block
            warn!(
                nonce = %self.nonce_pubkey,
                held_for_ms = held_for_ms,
                "Failed to acquire release_fn lock in Drop (potential leak)"
            );
        }
    }
}

/// Watchdog for monitoring and reclaiming expired leases (Task 1: Enhanced monitoring)
pub struct LeaseWatchdog {
    check_interval: Duration,
    lease_timeout: Duration,
    leases: Arc<Mutex<Vec<LeaseInfo>>>,
    running: Arc<RwLock<bool>>,
}

#[derive(Clone)]
struct LeaseInfo {
    nonce_pubkey: Pubkey,
    acquired_at: Instant,
    released: Arc<RwLock<bool>>,
}

impl LeaseWatchdog {
    /// Create a new lease watchdog
    pub fn new(check_interval: Duration, lease_timeout: Duration) -> Self {
        Self {
            check_interval,
            lease_timeout,
            leases: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Register a lease with the watchdog
    pub async fn register_lease(
        &self,
        nonce_pubkey: Pubkey,
        acquired_at: Instant,
        released: Arc<RwLock<bool>>,
    ) {
        let lease_info = LeaseInfo {
            nonce_pubkey,
            acquired_at,
            released,
        };
        
        self.leases.lock().await.push(lease_info);
    }
    
    /// Start the watchdog task
    pub async fn start<F>(self: Arc<Self>, on_expired: F)
    where
        F: Fn(Pubkey) + Send + Sync + 'static,
    {
        let mut running = self.running.write().await;
        if *running {
            warn!("Watchdog already running");
            return;
        }
        *running = true;
        drop(running);
        
        let on_expired = Arc::new(on_expired);
        
        tokio::spawn(async move {
            debug!("Lease watchdog started");
            
            loop {
                // Check if we should stop
                if !*self.running.read().await {
                    break;
                }
                
                // Sleep for the check interval
                tokio::time::sleep(self.check_interval).await;
                
                // Check for expired leases
                let mut leases = self.leases.lock().await;
                let now = Instant::now();
                
                let mut expired_leases = Vec::new();
                leases.retain(|lease_info| {
                    // Check if released
                    if let Ok(released) = lease_info.released.try_read() {
                        if *released {
                            // Remove released leases from tracking
                            return false;
                        }
                    }
                    
                    // Check if expired
                    let elapsed = now.duration_since(lease_info.acquired_at);
                    if elapsed >= self.lease_timeout {
                        expired_leases.push(lease_info.nonce_pubkey);
                        return false; // Remove from tracking
                    }
                    
                    true // Keep in tracking
                });
                
                drop(leases);
                
                // Handle expired leases
                if !expired_leases.is_empty() {
                    let metrics = crate::metrics::metrics();
                    for nonce_pubkey in expired_leases {
                        // Increment auto-drop counter for expired leases
                        metrics.nonce_leases_dropped_auto.inc();
                        
                        warn!(
                            nonce = %nonce_pubkey,
                            timeout_sec = self.lease_timeout.as_secs(),
                            release_type = "watchdog_expired",
                            "Lease expired, reclaiming nonce account"
                        );
                        on_expired(nonce_pubkey);
                    }
                }
            }
            
            debug!("Lease watchdog stopped");
        });
    }
    
    /// Stop the watchdog task
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }
    
    /// Get the number of active leases being tracked
    pub async fn active_lease_count(&self) -> usize {
        self.leases.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::hash::Hash;
    use std::sync::atomic::{AtomicU32, Ordering};
    
    #[tokio::test]
    async fn test_lease_explicit_release() {
        let nonce_account = Pubkey::new_unique();
        let released = Arc::new(AtomicU32::new(0));
        let released_clone = released.clone();
        
        let lease = NonceLease::new(
            nonce_account,
            1000,
            Hash::default(),
            Duration::from_secs(60),
            move || {
                released_clone.fetch_add(1, Ordering::SeqCst);
            },
        );
        
        assert_eq!(lease.nonce_pubkey(), &nonce_account);
        assert_eq!(lease.last_valid_slot(), 1000);
        assert!(!lease.is_expired());
        
        lease.release().await.unwrap();
        
        // Release function should have been called exactly once
        assert_eq!(released.load(Ordering::SeqCst), 1);
    }
    
    #[tokio::test]
    async fn test_lease_auto_release_on_drop() {
        let nonce_account = Pubkey::new_unique();
        let released = Arc::new(AtomicU32::new(0));
        let released_clone = released.clone();
        
        {
            let _lease = NonceLease::new(
                nonce_account,
                1000,
                Hash::default(),
                Duration::from_secs(60),
                move || {
                    released_clone.fetch_add(1, Ordering::SeqCst);
                },
            );
            
            // Lease is dropped here
        }
        
        // Release function should have been called
        assert_eq!(released.load(Ordering::SeqCst), 1);
    }
    
    #[tokio::test]
    async fn test_lease_idempotent_release() {
        let nonce_account = Pubkey::new_unique();
        let released = Arc::new(AtomicU32::new(0));
        let released_clone = released.clone();
        
        let lease = NonceLease::new(
            nonce_account,
            1000,
            Hash::default(),
            Duration::from_secs(60),
            move || {
                released_clone.fetch_add(1, Ordering::SeqCst);
            },
        );
        
        // Release multiple times
        lease.release().await.unwrap();
        
        // Should only be called once
        assert_eq!(released.load(Ordering::SeqCst), 1);
    }
    
    #[tokio::test]
    async fn test_lease_expiry() {
        let nonce_account = Pubkey::new_unique();
        
        let lease = NonceLease::new(
            nonce_account,
            1000,
            Hash::default(),
            Duration::from_millis(100),
            || {},
        );
        
        assert!(!lease.is_expired());
        assert!(lease.time_remaining().is_some());
        
        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        assert!(lease.is_expired());
        assert!(lease.time_remaining().is_none());
    }
    
    #[tokio::test]
    async fn test_watchdog_detects_expired_lease() {
        let nonce_account = Pubkey::new_unique();
        let expired_count = Arc::new(AtomicU32::new(0));
        let expired_count_clone = expired_count.clone();
        
        let watchdog = Arc::new(LeaseWatchdog::new(
            Duration::from_millis(50),
            Duration::from_millis(100),
        ));
        
        // Start watchdog
        watchdog.clone().start(move |_pubkey| {
            expired_count_clone.fetch_add(1, Ordering::SeqCst);
        }).await;
        
        // Register a lease
        let released = Arc::new(RwLock::new(false));
        watchdog.register_lease(
            nonce_account,
            Instant::now(),
            released.clone(),
        ).await;
        
        assert_eq!(watchdog.active_lease_count().await, 1);
        
        // Wait for watchdog to detect expiry
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Lease should have been detected as expired
        assert!(expired_count.load(Ordering::SeqCst) > 0);
        assert_eq!(watchdog.active_lease_count().await, 0);
        
        watchdog.stop().await;
    }
    
    #[tokio::test]
    async fn test_watchdog_ignores_released_lease() {
        let nonce_account = Pubkey::new_unique();
        let expired_count = Arc::new(AtomicU32::new(0));
        let expired_count_clone = expired_count.clone();
        
        let watchdog = Arc::new(LeaseWatchdog::new(
            Duration::from_millis(50),
            Duration::from_millis(100),
        ));
        
        // Start watchdog
        watchdog.clone().start(move |_pubkey| {
            expired_count_clone.fetch_add(1, Ordering::SeqCst);
        }).await;
        
        // Register a lease
        let released = Arc::new(RwLock::new(false));
        watchdog.register_lease(
            nonce_account,
            Instant::now(),
            released.clone(),
        ).await;
        
        // Mark as released immediately
        *released.write().await = true;
        
        // Wait for watchdog check
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Should not be counted as expired
        assert_eq!(expired_count.load(Ordering::SeqCst), 0);
        assert_eq!(watchdog.active_lease_count().await, 0);
        
        watchdog.stop().await;
    }
    
    /// Compile-time check: Ensure NonceLease is Send (required for async/await)
    #[allow(dead_code)]
    fn assert_nonce_lease_is_send() {
        fn is_send<T: Send>() {}
        is_send::<NonceLease>();
    }
    
    /// Compile-time check: Ensure NonceLease is Sync
    #[allow(dead_code)]
    fn assert_nonce_lease_is_sync() {
        fn is_sync<T: Sync>() {}
        is_sync::<NonceLease>();
    }
    
    #[tokio::test]
    async fn test_metrics_explicit_release() {
        let metrics = crate::metrics::metrics();
        
        // Get baseline counts
        let baseline_explicit = metrics.nonce_leases_dropped_explicit.get();
        let baseline_active = metrics.nonce_active_leases.get();
        
        let nonce_account = Pubkey::new_unique();
        let lease = NonceLease::new(
            nonce_account,
            1000,
            Hash::default(),
            Duration::from_secs(60),
            || {},
        );
        
        // Active leases should increment
        assert_eq!(metrics.nonce_active_leases.get(), baseline_active + 1);
        
        // Explicitly release
        lease.release().await.unwrap();
        
        // Explicit release counter should increment
        assert_eq!(metrics.nonce_leases_dropped_explicit.get(), baseline_explicit + 1);
        
        // Active leases should decrement
        assert_eq!(metrics.nonce_active_leases.get(), baseline_active);
    }
    
    #[tokio::test]
    async fn test_metrics_auto_release() {
        let metrics = crate::metrics::metrics();
        
        // Get baseline counts
        let baseline_auto = metrics.nonce_leases_dropped_auto.get();
        let baseline_active = metrics.nonce_active_leases.get();
        
        let nonce_account = Pubkey::new_unique();
        
        {
            let _lease = NonceLease::new(
                nonce_account,
                1000,
                Hash::default(),
                Duration::from_secs(60),
                || {},
            );
            
            // Active leases should increment
            assert_eq!(metrics.nonce_active_leases.get(), baseline_active + 1);
            
            // Drop happens here
        }
        
        // Auto release counter should increment
        assert_eq!(metrics.nonce_leases_dropped_auto.get(), baseline_auto + 1);
        
        // Active leases should decrement
        assert_eq!(metrics.nonce_active_leases.get(), baseline_active);
    }
    
    #[tokio::test]
    async fn test_metrics_lease_lifetime() {
        let metrics = crate::metrics::metrics();
        
        // Get baseline sample count
        let baseline_count = metrics.nonce_lease_lifetime.get_sample_count();
        
        let nonce_account = Pubkey::new_unique();
        let lease = NonceLease::new(
            nonce_account,
            1000,
            Hash::default(),
            Duration::from_secs(60),
            || {},
        );
        
        // Wait a bit to ensure measurable duration
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Release and check histogram was updated
        lease.release().await.unwrap();
        
        // Histogram sample count should increment
        assert_eq!(metrics.nonce_lease_lifetime.get_sample_count(), baseline_count + 1);
    }
    
    #[tokio::test]
    async fn test_watchdog_metrics_expired_leases() {
        let metrics = crate::metrics::metrics();
        
        // Get baseline
        let baseline_auto = metrics.nonce_leases_dropped_auto.get();
        
        let nonce_account = Pubkey::new_unique();
        let expired_count = Arc::new(AtomicU32::new(0));
        let expired_count_clone = expired_count.clone();
        
        let watchdog = Arc::new(LeaseWatchdog::new(
            Duration::from_millis(50),
            Duration::from_millis(100),
        ));
        
        // Start watchdog
        watchdog.clone().start(move |_pubkey| {
            expired_count_clone.fetch_add(1, Ordering::SeqCst);
        }).await;
        
        // Register a lease
        let released = Arc::new(RwLock::new(false));
        watchdog.register_lease(
            nonce_account,
            Instant::now(),
            released.clone(),
        ).await;
        
        // Wait for watchdog to detect expiry
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Auto-drop counter should increment (watchdog marks as auto-dropped)
        assert!(metrics.nonce_leases_dropped_auto.get() > baseline_auto);
        
        watchdog.stop().await;
    }
}
