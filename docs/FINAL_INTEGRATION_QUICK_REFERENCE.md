# Final Integration Stage - Quick Reference

Quick guide to using the new FINAL INTEGRATION STAGE features.

## 1. Dataflow Events

```rust
use ultra::sniffer::{SnifferEvent, CandidateId};

// Events are automatically emitted at each stage
// Access via telemetry or event collector

let trace_id: CandidateId = 12345;
```

## 2. Lifecycle Supervisor

```rust
use ultra::sniffer::{Supervisor, SupervisorCommand, WorkerHandle};

// Create supervisor
let supervisor = Supervisor::new();

// Register workers
let handle = tokio::spawn(async { /* worker code */ });
supervisor.register_worker(WorkerHandle::new("worker_name", handle, false));

// Lifecycle operations
supervisor.start().await?;
supervisor.pause();
supervisor.resume();
supervisor.stop(Duration::from_secs(5)).await?;

// Check state
if supervisor.is_healthy() {
    println!("Supervisor running normally");
}
```

## 3. Latency Correlation

```rust
use ultra::sniffer::SnifferMetrics;

let metrics = Arc::new(SnifferMetrics::new());

// Record correlation samples
metrics.record_correlation(latency_us, confidence, was_dropped);

// Analyze
let correlation = metrics.get_latency_confidence_correlation();
let avg_latency = metrics.get_avg_latency_high_confidence(0.8);
let drop_rate = metrics.get_drop_rate_high_latency(1000);

println!("Latency-Confidence correlation: {:?}", correlation);
```

## 4. Hot Config Reload

```rust
use ultra::sniffer::SnifferConfig;

// Start watching config file
let (tx, mut rx) = SnifferConfig::watch_config("config.toml".to_string());

// Listen for updates
tokio::spawn(async move {
    while let Ok(()) = rx.changed().await {
        let new_config = rx.borrow().clone();
        // Apply new config
    }
});
```

## 5. Backpressure Diagnostics

```rust
use ultra::sniffer::telemetry::HandoffDiagnostics;

let diagnostics = Arc::new(HandoffDiagnostics::new());

// Record events
diagnostics.record_drop(is_high_priority);
diagnostics.record_queue_wait(wait_us);

// Analyze
let avg_wait = diagnostics.avg_queue_wait();
let histogram = diagnostics.get_histogram();
println!("Queue wait histogram: {:?}", histogram);
```

## 6. Testing

```bash
# Run unit tests
cargo test --test prefilter_test
cargo test --test extractor_test
cargo test --test analytics_test
cargo test --test security_test

# Run integration tests
cargo test --test stream_sim_test -- --test-threads=4
cargo test --test backpressure_test -- --test-threads=4

# Run stress tests (ignored by default)
cargo test --test burst_10k_tx -- --ignored --nocapture
cargo test --test pause_resume -- --ignored --nocapture
cargo test --test cold_start_latency -- --ignored --nocapture
```

## 7. Benchmarking

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench prefilter_bench
cargo bench --bench extractor_bench
cargo bench --bench analytics_bench

# Save baseline
cargo bench -- --save-baseline main

# Compare to baseline
cargo bench -- --baseline main
```

## 8. Deterministic Shutdown

```rust
// Pattern used in integration.rs
loop {
    tokio::select! {
        biased;
        
        // Highest priority
        _ = shutdown_rx.recv() => break,
        
        // Medium priority
        _ = async {}, if paused => continue,
        
        // Lowest priority
        tx = stream.recv() => process(tx),
    }
}
```

## Common Patterns

### Full Integration Example

```rust
use ultra::sniffer::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Setup config with hot reload
    let (config_tx, mut config_rx) = SnifferConfig::watch_config("config.toml".to_string());
    
    // 2. Create supervisor
    let supervisor = Supervisor::new();
    
    // 3. Create sniffer
    let config = SnifferConfig::default();
    let sniffer = Sniffer::new(config);
    let metrics = sniffer.get_metrics();
    
    // 4. Start with supervision
    supervisor.start().await?;
    let rx = sniffer.start().await?;
    
    // 5. Monitor metrics
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            // Latency analysis
            if let Some(correlation) = metrics.get_latency_confidence_correlation() {
                println!("Latency-confidence correlation: {:.3}", correlation);
            }
            
            // Performance metrics
            if let Some(p99) = metrics.get_percentile_latency(0.99) {
                println!("P99 latency: {}μs", p99);
            }
        }
    });
    
    // 6. Handle config updates
    tokio::spawn(async move {
        while let Ok(()) = config_rx.changed().await {
            let new_config = config_rx.borrow();
            println!("Config updated: {:?}", new_config);
        }
    });
    
    // 7. Graceful shutdown
    tokio::signal::ctrl_c().await?;
    supervisor.stop(Duration::from_secs(5)).await?;
    sniffer.stop();
    
    Ok(())
}
```

## Performance Tips

1. **Event Emission**: Only emit events when telemetry is enabled
2. **Correlation Sampling**: Use 1% sampling for high-throughput scenarios
3. **Config Reload**: Check interval should be ≥5 seconds
4. **Supervisor**: Register only critical workers as `critical: true`
5. **Benchmarks**: Run with `--release` for accurate results

## Troubleshooting

### High Memory Usage
- Reduce `latency_samples` capacity
- Reduce `queue_wait_samples` capacity
- Increase sampling interval

### Slow Shutdown
- Check `graceful_shutdown_timeout_ms` setting
- Verify workers are respecting shutdown signal
- Look for blocking operations in workers

### Config Not Reloading
- Verify file permissions
- Check file modification timestamp updates
- Ensure config validation passes

### Test Failures
- Use `--test-threads=1` for debugging
- Add `--nocapture` to see println output
- Check for timing-dependent race conditions

## Further Reading

- `FINAL_INTEGRATION_STAGE.md` - Complete implementation details
- `tests/README.md` - Test infrastructure documentation
- `examples/final_integration_stage.rs` - Full working example
