# Enhanced Error Handling for Nonce Manager

## Overview

This document describes the enhanced error handling features added to the Nonce Manager module, including:

1. **UniverseErrorType Enum** - Advanced error classification
2. **Error Classification Logic** - ML-based error clustering with regex patterns
3. **Circuit Breaker Pattern** - Failure isolation and system protection
4. **Enhanced Retry Logic** - Intelligent retry with classification
5. **Integration Points** - Hooks for RPC Manager, BuyEngine, and TxBuilder

## 1. UniverseErrorType Enum

Located in `nonce_errors.rs`, this enum classifies errors into specific categories:

```rust
pub enum UniverseErrorType {
    Base(Box<NonceError>),                           // Wraps existing errors
    ValidatorBehind { slots: i64 },                  // Validator lag detected
    ConsensusFailure,                                // Consensus issues
    GeyserStreamError,                               // Geyser stream problems
    ShredstreamTimeout,                              // Timeout in shredstream
    CircuitBreakerOpen,                              // Circuit breaker triggered
    PredictiveFailure { probability: f64 },          // ML-predicted failure
    SecurityViolation { reason: String },            // Security issues
    QuotaExceeded,                                   // Rate limit hit
    ClusterCongestion { tps: u32 },                  // Network congestion
    ClusteredAnomaly { cluster_id: u8, confidence: f64 }, // ML cluster detection
}
```

### ErrorClassification

Each classified error includes:
- `error_type`: The UniverseErrorType
- `confidence`: Classification confidence (0.0 to 1.0)
- `is_transient`: Whether the error should be retried
- `should_taint`: Whether to mark the source as tainted

## 2. Error Classification Logic

### Pattern-Based Classification

The `ErrorClassifier` uses regex patterns to identify error types:

- **Validator Behind**: Keywords "behind", "slot lag"
- **Consensus Failure**: Keywords "consensus", "fork"
- **Geyser Stream**: Keywords "geyser", "stream"
- **Timeout**: Keywords "timeout", "timed out"
- **Security**: Keywords "unauthorized", "invalid signature"
- **Quota**: Keywords "quota", "rate limit"
- **Congestion**: Keywords "congestion", "busy"

### ML-Based Clustering

Simple k-means approximation clusters similar errors:

```rust
let classifier = ErrorClassifier::new(100, 5); // 100 history size, 5 clusters
let classification = classifier.classify_error(&error).await;
```

The classifier:
1. Records error history (bounded to max_history_size)
2. Performs clustering when ≥20 samples collected
3. Returns cluster ID with confidence score
4. Higher confidence for larger clusters

## 3. Circuit Breaker Pattern

### Per-Endpoint Circuit Breakers

Protects individual endpoints from cascading failures:

```rust
let breaker = CircuitBreaker::new(
    3,                              // failure_threshold
    2,                              // success_threshold  
    Duration::from_secs(30)         // timeout
);
```

**States:**
- **Closed**: Normal operation, all requests allowed
- **Open**: Too many failures, requests blocked
- **HalfOpen**: Testing if service recovered

**Transitions:**
- Closed → Open: After `failure_threshold` consecutive failures
- Open → HalfOpen: After `timeout` duration
- HalfOpen → Closed: After `success_threshold` consecutive successes
- HalfOpen → Open: On any failure

### Global Circuit Breaker

Coordinates system-wide protection:

```rust
let global = GlobalCircuitBreaker::new();
let breaker = global.get_breaker("endpoint1").await;

// Check if >50% of endpoints are open
if global.should_trip_global().await {
    // Trigger system-wide pause
}

// Mark endpoints as tainted for security issues
global.mark_tainted("endpoint1").await;
```

## 4. Enhanced Retry Logic

### Basic Usage

```rust
use retry_with_backoff_enhanced;

let config = RetryConfig::default();
let breaker = CircuitBreaker::default_thresholds();
let classifier = ErrorClassifier::new(100, 5);

let result = retry_with_backoff_enhanced(
    "operation_name",
    &config,
    Some(&breaker),      // Optional circuit breaker
    Some(&classifier),   // Optional error classifier
    || async {
        // Your async operation
        Ok(())
    }
).await;
```

### Behavior

1. **Check circuit breaker** before each attempt
2. **Classify errors** to determine if transient
3. **Abort immediately** on permanent errors (security violations)
4. **Record metrics** in circuit breaker
5. **Apply exponential backoff** with jitter for transient errors

### Retry Decision Flow

```
┌─────────────────┐
│  Start Attempt  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐      NO      ┌──────────────┐
│ Circuit Open?   │──────────────>│ Try Operation│
└────────┬────────┘               └──────┬───────┘
         │YES                            │
         │                               ▼
         ▼                      ┌─────────────────┐
┌─────────────────┐      YES    │   Success?      │
│ Return Error    │<─────────────┤                 │
└─────────────────┘              └────────┬────────┘
                                          │NO
                                          ▼
                                 ┌─────────────────┐
                          NO     │  Is Transient?  │    YES
                        ┌────────┤                 ├────────┐
                        │        └─────────────────┘        │
                        ▼                                   ▼
               ┌─────────────────┐              ┌──────────────────┐
               │ Record Failure  │              │ Backoff & Retry  │
               │  Return Error   │              └──────────────────┘
               └─────────────────┘
```

## 5. Integration Points

### nonce_manager.rs

Added `update_from_rpc()` method that uses error classification:

```rust
pub async fn update_from_rpc(
    &self,
    index: usize,
    rpc_client: &RpcClient,
    classifier: Option<&ErrorClassifier>,
) -> Result<(), NonceError>
```

**Features:**
- Uses `retry_with_backoff_enhanced` for RPC calls
- Classifies errors and taints nonces on security violations
- Integrates seamlessly with existing nonce pool

### Integration with Other Components

#### RPC Manager
```rust
// Share circuit breaker state
let breaker = global_breaker.get_breaker("rpc_endpoint").await;
rpc_manager.set_circuit_breaker(breaker).await;
```

#### BuyEngine
```rust
// Feed error classifications to backoff state
if let Err(e) = operation {
    let classification = classifier.classify_error(&e).await;
    if matches!(classification.error_type, UniverseErrorType::CircuitBreakerOpen) {
        buy_engine.pause_operations().await;
    }
}
```

#### TxBuilder
```rust
// Use classification in build_transaction_with_nonce
let classification = classifier.classify_error(&error).await;
if let UniverseErrorType::PredictiveFailure { probability } = classification.error_type {
    if probability > 0.5 {
        // Abort bundle construction
        return Err(error);
    }
}
```

## Configuration

### RetryConfig

```rust
RetryConfig {
    max_attempts: 3,           // Maximum retry attempts
    base_backoff_ms: 100,      // Initial backoff delay
    max_backoff_ms: 5000,      // Maximum backoff delay
    jitter_factor: 0.2,        // Randomness (0.0-1.0)
}
```

### Circuit Breaker Defaults

```rust
CircuitBreaker::default_thresholds()
// Equivalent to:
CircuitBreaker::new(
    3,                          // failure_threshold
    2,                          // success_threshold
    Duration::from_secs(30)     // timeout
)
```

## Testing

Comprehensive test coverage in `nonce_retry.rs`:

- ✅ `test_circuit_breaker_transitions` - State machine transitions
- ✅ `test_circuit_breaker_halfopen_failure` - HalfOpen failure handling
- ✅ `test_global_circuit_breaker` - Global coordination
- ✅ `test_error_classification` - Pattern-based classification
- ✅ `test_retry_with_circuit_breaker` - Integration test

Run tests:
```bash
cargo test --bin Ultra nonce_retry::
```

## Examples

See `examples_circuit_breaker.rs` for detailed usage examples:

1. Basic circuit breaker usage
2. Error classification
3. Enhanced retry logic
4. Global circuit breaker coordination

## Performance Considerations

- **Bounded History**: Error classifier maintains max 100 entries
- **Lock-Free Operations**: Circuit breaker uses atomics where possible
- **Efficient Pattern Matching**: Simple string comparisons (no regex compilation overhead)
- **O(1) Cluster Assignment**: Hash-based clustering for constant time

## Future Enhancements

Potential improvements (not yet implemented):

1. **Advanced ML Models**: Replace simple k-means with proper feature extraction
2. **Adaptive Thresholds**: Dynamically adjust circuit breaker thresholds based on historical performance
3. **Distributed Coordination**: Share circuit breaker state across multiple instances
4. **Metrics Export**: Prometheus/Grafana integration for observability
5. **Predictive Circuit Breaking**: Use ML to predict failures before they occur

## API Reference

### Types

- `UniverseErrorType` - Error classification enum
- `ErrorClassification` - Classification result with confidence
- `CircuitBreaker` - Per-endpoint failure protection
- `GlobalCircuitBreaker` - System-wide coordination
- `ErrorClassifier` - ML-based error clustering
- `CircuitState` - Breaker state (Closed/Open/HalfOpen)

### Functions

- `retry_with_backoff_enhanced()` - Enhanced retry with classification
- `classify_error()` - Classify error with confidence score
- `can_execute()` - Check if circuit breaker allows execution
- `record_success()` / `record_failure()` - Update circuit breaker state

## Conclusion

The enhanced error handling provides:

✅ **Robustness** - Circuit breakers prevent cascading failures  
✅ **Intelligence** - ML-based error classification  
✅ **Flexibility** - Configurable thresholds and timeouts  
✅ **Integration** - Clean integration points with existing code  
✅ **Observability** - Detailed error classifications for debugging  

These improvements make the Nonce Manager more resilient to network issues, security threats, and system-wide failures.
