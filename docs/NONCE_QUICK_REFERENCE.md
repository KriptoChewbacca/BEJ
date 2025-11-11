# Universe Class Nonce Manager - Quick Reference

## Module Overview

```
nonce_errors.rs          → Error types & classification
nonce_retry.rs           → Retry helper with backoff
nonce_signer.rs          → Async signer abstraction
nonce_lease.rs           → Lease model with watchdog
nonce_refresh.rs         → Non-blocking refresh monitor
nonce_predictive.rs      → EMA-based predictive model
nonce_manager_integrated.rs → Complete integration
```

## Common Patterns

### 1. Error Handling (Step 1)

```rust
use nonce_errors::{NonceError, NonceResult};
use nonce_retry::{retry_with_backoff, RetryConfig};

// Define operation
let result: NonceResult<Account> = retry_with_backoff(
    "get_account",
    &RetryConfig::default(),
    || async {
        rpc_client
            .get_account(&pubkey)
            .await
            .map_err(|e| NonceError::from_client_error(e, Some(endpoint)))
    },
).await;

// Handle result
match result {
    Ok(account) => { /* use account */ },
    Err(NonceError::Rpc { .. }) => { /* transient error */ },
    Err(NonceError::InvalidNonceAccount(_)) => { /* permanent error */ },
    Err(e) => { /* other errors */ },
}
```

### 2. Signer Abstraction (Step 2)

```rust
use nonce_signer::{SignerService, LocalSigner};

// Create signer
let signer: Arc<dyn SignerService> = Arc::new(LocalSigner::new(keypair));

// Get pubkey
let pubkey = signer.pubkey().await;

// Sign transaction
let mut tx = Transaction::new_with_payer(&instructions, Some(&pubkey));
tx.message.recent_blockhash = blockhash;
signer.sign_transaction(&mut tx).await?;
```

### 3. Lease Model (Step 3)

```rust
use nonce_lease::NonceLease;

// Acquire lease
let lease = manager.acquire_nonce_with_lease(
    Duration::from_secs(60),  // timeout
    network_tps,
).await?;

// Use lease
let account_pubkey = lease.account_pubkey();
let last_valid_slot = lease.last_valid_slot();

// Check expiry
if lease.is_expired() {
    warn!("Lease expired!");
}

// Option 1: Explicit release
lease.release().await?;

// Option 2: Auto-release on drop (happens automatically)
```

### 4. Non-Blocking Refresh (Step 4)

```rust
use nonce_refresh::{NonBlockingRefresh, RefreshStatus};

let refresh_manager = NonBlockingRefresh::new();

// Send refresh transaction (doesn't block)
let signature = refresh_manager.send_refresh_transaction(
    rpc_client,
    endpoint,
    &transaction,
    nonce_account,
).await?;

// Process results in background loop
tokio::spawn(async move {
    loop {
        refresh_manager.process_results(|result| {
            match result.status {
                RefreshStatus::Confirmed => {
                    // Update account state
                    if let Some(latency) = result.telemetry.latency_ms() {
                        info!("Confirmed in {}ms", latency);
                    }
                }
                RefreshStatus::Failed(err) => {
                    error!("Failed: {}", err);
                }
                RefreshStatus::Timeout => {
                    warn!("Timed out");
                }
                _ => {}
            }
        }).await;
        
        sleep(Duration::from_millis(100)).await;
    }
});
```

### 5. Atomic Slot Validation (Step 5)

```rust
// In ImprovedNonceAccount

// Update from RPC atomically
account.update_from_rpc(&rpc_client, endpoint).await?;

// Validate before use
let current_slot = rpc_client.get_slot().await?;
account.validate_not_expired(current_slot).await?;

// Check if tainted
if account.is_tainted.load(Ordering::Relaxed) {
    return Err(NonceError::NonceTainted(account.pubkey));
}
```

### 6. Predictive Model (Step 6)

```rust
use nonce_predictive::PredictiveRefreshModel;

let mut model = PredictiveRefreshModel::new();

// Record refresh events
model.record_refresh(latency_ms, slots_consumed);

// Predict failure probability
if let Some(probability) = model.predict_failure_probability(network_tps) {
    if probability > 0.7 {
        warn!("High failure probability: {:.2}", probability);
        // Consider proactive refresh
    }
} else {
    // Insufficient data, use conservative behavior
}

// Label prediction for offline learning
model.label_prediction(actual_latency_ms, actual_success);

// Export labeled data
let predictions = model.export_predictions();
```

## Configuration Examples

### Retry Configuration

```rust
// Aggressive (more attempts, faster)
let config = RetryConfig {
    max_attempts: 5,
    base_backoff_ms: 50,
    max_backoff_ms: 2000,
    jitter_factor: 0.3,
};

// Conservative (fewer attempts, longer delays)
let config = RetryConfig {
    max_attempts: 2,
    base_backoff_ms: 200,
    max_backoff_ms: 10000,
    jitter_factor: 0.1,
};
```

### Predictive Model Configuration

```rust
let model = PredictiveRefreshModel::with_config(
    100,   // max_history_size
    10,    // min_sample_size
    0.2,   // ema_alpha
);
```

### Watchdog Configuration

```rust
let watchdog = Arc::new(LeaseWatchdog::new(
    Duration::from_secs(5),    // check_interval
    Duration::from_secs(300),  // lease_timeout
));

watchdog.clone().start(|expired_pubkey| {
    // Handle expired lease
    mark_tainted(expired_pubkey);
}).await;
```

## Error Classification Quick Reference

| Error Type | Transient? | Action |
|------------|-----------|--------|
| `Rpc` | ✅ Yes | Retry with backoff |
| `Timeout` | ✅ Yes | Retry with backoff |
| `AdvanceFailed` | ✅ Yes | Retry with backoff |
| `InvalidNonceAccount` | ❌ No | Fail immediately |
| `NonceExpired` | ❌ No | Mark tainted, rotate |
| `PoolExhausted` | ❌ No | Wait or fail |
| `NonceLocked` | ❌ No | Try different nonce |
| `NonceTainted` | ❌ No | Use different nonce |
| `Configuration` | ❌ No | Fix configuration |

## Testing Patterns

### Unit Testing with MockSigner

```rust
use nonce_signer::MockSigner;

#[tokio::test]
async fn test_with_mock_signer() {
    let pubkey = Pubkey::new_unique();
    let signer = Arc::new(MockSigner::new(pubkey));
    
    // Test normal operation
    let result = signer.sign_transaction(&mut tx).await;
    assert!(result.is_ok());
    
    // Test failure
    let failing_signer = Arc::new(MockSigner::new_failing(pubkey));
    let result = failing_signer.sign_transaction(&mut tx).await;
    assert!(result.is_err());
}
```

### Testing Retry Behavior

```rust
#[tokio::test]
async fn test_retry_on_transient_error() {
    let attempt_count = Arc::new(AtomicU32::new(0));
    let count_clone = attempt_count.clone();
    
    let result = retry_with_backoff(
        "test",
        &RetryConfig::default(),
        || {
            let count = count_clone.fetch_add(1, Ordering::SeqCst);
            async move {
                if count < 2 {
                    Err(NonceError::Timeout(1000)) // Transient
                } else {
                    Ok(42)
                }
            }
        },
    ).await;
    
    assert_eq!(result.unwrap(), 42);
    assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
}
```

## Performance Tips

1. **Use lease model** - Automatic cleanup prevents leaks
2. **Enable retry jitter** - Prevents thundering herd
3. **Monitor predictive model** - Proactive refresh reduces failures
4. **Batch operations** - Use `sign_transactions()` for multiple txs
5. **Process results async** - Don't block on confirmation
6. **Set appropriate timeouts** - Balance responsiveness vs reliability

## Common Pitfalls

❌ **Don't:**
- Use `unwrap()` or `expect()` in production code
- Block on transaction confirmation in hot path
- Ignore predictive model warnings (>0.7 probability)
- Forget to process refresh results
- Skip slot validation before use

✅ **Do:**
- Use `NonceResult<T>` everywhere
- Rely on lease auto-release (Drop)
- Check predictive model and refresh proactively
- Process results in background loop
- Validate slots atomically

## Monitoring Metrics

Track these metrics in production:

```rust
let stats = manager.get_stats().await;

println!("Pool: {}/{}", stats.available_permits, stats.total_accounts);
println!("Tainted: {}", stats.tainted_count);
println!("Acquires: {}", stats.total_acquires);
println!("Releases: {}", stats.total_releases);
println!("Refreshes: {}", stats.total_refreshes);
println!("Model ready: {}", stats.model_has_sufficient_data);
```

## Support

For issues or questions:
1. Check `UNIVERSE_CLASS_NONCE_IMPLEMENTATION.md` for detailed docs
2. Review test cases in each module for examples
3. Enable `RUST_LOG=debug` for detailed tracing

---

**Remember:** All 6 steps are implemented and tested. Use `UniverseNonceManager` for production deployments.
