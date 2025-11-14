# Jito Bundler Integration Guide

## Overview

Task 7 implements integration with Jito MEV bundler for MEV-protected transaction submission. This guide explains how to use the bundler in the BuyEngine.

## Architecture

The bundler system consists of:

- **Bundler trait** - Abstract interface for bundle submission
- **JitoBundler** - Production implementation (requires Jito SDK)
- **MockBundler** - Testing implementation
- **BuyEngine integration** - Automatic path selection (bundler vs RPC)

## Basic Usage

### 1. Create a Bundler

```rust
use std::sync::Arc;
use bot::tx_builder::{MockBundler, Bundler};

// For testing - always succeeds
let bundler: Arc<dyn Bundler> = Arc::new(MockBundler::new_success());

// For testing - always fails
let bundler: Arc<dyn Bundler> = Arc::new(MockBundler::new_failure());

// For testing - custom behavior
let bundler: Arc<dyn Bundler> = Arc::new(MockBundler::new_custom(
    true,  // should_succeed
    3,     // tip_multiplier (3x)
));
```

### 2. Configure JitoBundler (Production)

```rust
use bot::tx_builder::{BundleConfig, BundleEndpoint, JitoBundler};

let config = BundleConfig {
    endpoints: vec![
        BundleEndpoint {
            region: "ny".to_string(),
            url: "https://ny.jito.wtf".to_string(),
            priority: 1, // Highest priority
        },
        BundleEndpoint {
            region: "ams".to_string(),
            url: "https://ams.jito.wtf".to_string(),
            priority: 2,
        },
        BundleEndpoint {
            region: "tokyo".to_string(),
            url: "https://tokyo.jito.wtf".to_string(),
            priority: 3,
        },
    ],
    default_tip_lamports: 10_000,
    max_tip_lamports: 100_000,
};

// Note: JitoBundler requires RPC client - see implementation for details
// let bundler = Arc::new(JitoBundler::new(config, rpc_client));
```

### 3. Create BuyEngine with Bundler

```rust
use bot::buy_engine::BuyEngine;

// Without bundler (existing behavior)
let engine = BuyEngine::new(
    rpc,
    nonce_manager,
    candidate_rx,
    app_state,
    config,
    tx_builder,
);

// With bundler (MEV-protected)
let engine = BuyEngine::new_with_bundler(
    rpc,
    nonce_manager,
    candidate_rx,
    app_state,
    config,
    tx_builder,
    Some(bundler), // Optional bundler
);
```

## How It Works

### Transaction Submission Flow

When `try_buy_universe` is called:

1. **Check if bundler is available**
   ```rust
   if bundler.is_available() {
       // Use bundler path
   } else {
       // Fallback to RPC
   }
   ```

2. **Calculate dynamic tip**
   ```rust
   let base_tip = self.calculate_dynamic_tip().await;
   let dynamic_tip = bundler.calculate_dynamic_tip(base_tip);
   ```

3. **Submit bundle**
   ```rust
   bundler.submit_bundle(
       vec![transaction],
       dynamic_tip,
       &trace_ctx
   ).await
   ```

4. **Automatic fallback on error**
   - If bundler submission fails, engine automatically falls back to RPC
   - Metrics track bundler_submission_failed and bundler_unavailable_fallback

### RAII Nonce Management

The bundler integration maintains proper RAII nonce management:

```rust
// Nonce acquired and held in TxBuildOutput
let buy_output = self.create_buy_transaction_output(&candidate).await?;

// Submit via bundler (nonce guard still held)
let result = bundler.submit_bundle(...).await;

// On success: explicit release
buy_output.release_nonce().await?;

// On error: automatic release via Drop
drop(buy_output);
```

## Metrics

The bundler integration adds the following metrics:

- `bundler_submission_attempt` - Counter for bundle submission attempts
- `bundler_submission_failed` - Counter for failed submissions
- `bundler_unavailable_fallback` - Counter for RPC fallbacks
- `prepare_bundle_ms` - Histogram of bundle preparation time
- `mock_bundler_success` - Counter for successful mock submissions
- `mock_bundler_failure` - Counter for failed mock submissions

Access metrics via:
```rust
use bot::metrics::metrics;

metrics().increment_counter("bundler_submission_attempt");
```

## Testing

### Integration Tests

See `tests/bundler_integration_test.rs` for comprehensive examples:

```rust
#[tokio::test]
async fn test_mock_bundler_success() {
    let bundler = MockBundler::new_success();
    let trace_ctx = TraceContext::new("test");
    
    let result = bundler.submit_bundle(vec![], 10_000, &trace_ctx).await;
    assert!(result.is_ok());
}
```

### Testing with BuyEngine

```rust
// Create mock bundler that always succeeds
let bundler = Arc::new(MockBundler::new_success());

// Create BuyEngine with bundler
let engine = BuyEngine::new_with_bundler(
    rpc,
    nonce_manager,
    candidate_rx,
    app_state,
    config,
    tx_builder,
    Some(bundler),
);

// Engine will now use bundler for MEV-protected submissions
```

## Configuration Best Practices

### 1. Endpoint Priority

Set priorities based on latency to your deployment:
- Priority 1: Closest region
- Priority 2: Secondary region
- Priority 3: Fallback region

### 2. Tip Calculation

Configure tips based on network conditions:
```rust
let config = BundleConfig {
    default_tip_lamports: 10_000,  // For normal conditions
    max_tip_lamports: 100_000,      // For high congestion
    ..Default::default()
};
```

### 3. Fallback Strategy

Always provide RPC fallback:
- Bundler returns `is_available() == false` when Jito SDK not ready
- Engine automatically falls back to RPC broadcast
- No manual intervention required

## Production Deployment

### Requirements

1. **Jito SDK** - Add to Cargo.toml (not currently included)
2. **Endpoint URLs** - Configure production Jito endpoints
3. **Authentication** - Set up API keys if required
4. **Monitoring** - Track bundler metrics in production

### Example Production Configuration

```rust
// Production bundler configuration
let config = BundleConfig {
    endpoints: vec![
        BundleEndpoint {
            region: "ny".to_string(),
            url: env::var("JITO_NY_ENDPOINT")?,
            priority: 1,
        },
        BundleEndpoint {
            region: "ams".to_string(),
            url: env::var("JITO_AMS_ENDPOINT")?,
            priority: 2,
        },
    ],
    default_tip_lamports: 20_000,  // Higher for production
    max_tip_lamports: 200_000,
};

let bundler = Arc::new(JitoBundler::new(config, rpc_client));
let engine = BuyEngine::new_with_bundler(
    rpc,
    nonce_manager,
    candidate_rx,
    app_state,
    config,
    tx_builder,
    Some(bundler),
);
```

## Troubleshooting

### Bundler Not Being Used

Check:
1. Bundler is provided to `new_with_bundler`
2. `bundler.is_available()` returns `true`
3. JitoBundler has valid endpoints configured

### All Submissions Failing

Check:
1. Jito endpoints are reachable
2. API authentication is configured
3. Tip amounts are sufficient
4. Check `bundler_submission_failed` metric

### Performance Issues

Monitor:
1. `prepare_bundle_ms` histogram
2. Endpoint latencies
3. Tip calculation overhead

## Future Enhancements

Potential improvements:
1. **Jito SDK Integration** - Full production support
2. **Bundle Batching** - Multiple transactions in single bundle
3. **Adaptive Tips** - ML-based tip calculation
4. **Health Checks** - Periodic endpoint availability testing
5. **Circuit Breakers** - Automatic bundler disable on repeated failures

## See Also

- `src/tx_builder/bundle.rs` - Bundler trait and implementations
- `tests/bundler_integration_test.rs` - Integration tests
- `docs/docs_TX_BUILDER_SUPERCOMPONENT_PLAN.md` - Original task specification
