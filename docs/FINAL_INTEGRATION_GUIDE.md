# Final Integration - Implementation Guide

This document describes the complete implementation of the Final Integration Stage for the Sniffer module.

## Overview

The Final Integration Stage adds critical production-ready features to the Sniffer module:

1. **Event Emission & Telemetry** - Full pipeline observability
2. **Supervisor Integration** - Lifecycle management and panic recovery
3. **Handoff Diagnostics** - Adaptive backpressure and queue monitoring
4. **Config Reload** - Runtime parameter updates without restart
5. **CI Integration** - Automated performance testing and regression detection

## Architecture

### Event Flow with Telemetry

```
[Geyser Stream]
    ↓ (BytesReceived event)
[Prefilter]
    ↓ (PrefilterPassed/Rejected event)
[Extractor]
    ↓ (CandidateExtracted/Failed event)
[Security]
    ↓ (SecurityPassed/Rejected event)
[Handoff]
    ↓ (HandoffSent/Dropped event)
[Buy Engine]
```

Each stage emits telemetry events that are collected in the `EventCollector` for diagnostics and analysis.

### Supervisor Worker Management

```
Supervisor
├── process_loop (CRITICAL)
├── telemetry_loop
├── analytics_updater
├── threshold_updater
└── config_reload
```

The supervisor manages all async workers with:
- Coordinated start/stop/pause/resume
- Panic detection and recovery
- Graceful shutdown with timeout
- Health monitoring

## Components

### 1. EventCollector

**Purpose**: Collect and store pipeline events for diagnostics

**Location**: `sniffer/integration.rs`

**API**:
```rust
let collector = EventCollector::new(10000); // 10k event buffer
collector.collect(event);
let recent = collector.get_recent(100);
```

**Features**:
- Circular buffer (10k events)
- Thread-safe event collection
- Minimal performance overhead

### 2. HandoffDiagnostics

**Purpose**: Track queue performance and adapt backpressure policy

**Location**: `sniffer/telemetry.rs`

**API**:
```rust
let diagnostics = HandoffDiagnostics::new();
diagnostics.record_queue_wait(wait_us);
diagnostics.record_drop(is_high_priority);
let avg_wait = diagnostics.avg_queue_wait();
let histogram = diagnostics.get_histogram();
```

**Features**:
- Queue wait time tracking
- Histogram buckets: 0-10μs, 10-100μs, 100-1000μs, 1000+μs
- Drop tracking by priority level
- Adaptive policy recommendations

### 3. Adaptive Backpressure

**Purpose**: Automatically adjust drop policy based on queue conditions

**Location**: `sniffer/handoff.rs`

**Logic**:
```rust
if avg_queue_wait > 1000μs {
    // High congestion - drop aggressively
    policy = DropPolicy::DropNewest
} else if avg_queue_wait < 100μs {
    // Low congestion - can afford to block
    policy = DropPolicy::Block
} else {
    // Use configured policy
}
```

**Benefits**:
- Automatic adaptation to load conditions
- Prevents queue buildup
- Maximizes throughput under varying conditions

### 4. Config Reload

**Purpose**: Update parameters at runtime without restart

**Location**: `sniffer/integration.rs::config_reload_loop()`

**Watched Parameters**:
- `threshold_update_rate` - Analytics threshold adaptation rate
- `batch_size` - Candidate batch size
- `drop_policy` - Backpressure drop policy
- All other SnifferConfig fields

**Mechanism**:
- File modification watching (every 5s)
- Atomic parameter updates
- Validation before applying
- Logging of all changes

### 5. CI Performance Testing

**Purpose**: Detect performance regressions automatically

**Location**: `.github/workflows/sniffer-performance.yml`

**Jobs**:

#### Benchmark Job
- Runs prefilter, extractor, and analytics benchmarks
- Stores results as artifacts
- Compares against baselines

#### Stress Test Job
- 10k tx/s sustained load
- P99 latency measurement
- Drop rate validation
- Resource usage tracking

#### Performance Regression Check
- Downloads benchmark results
- Compares against baseline
- Fails PR if regression > 20%
- Posts results as PR comment

## Performance Targets

### Benchmarks
- **Prefilter**: < 1 μs per transaction
- **Extractor**: < 5 μs per extraction
- **Analytics**: < 100 μs per EMA update

### Stress Tests
- **Throughput**: ≥ 10,000 tx/s
- **Latency P99**: < 10 ms
- **Drop Rate**: < 5%
- **CPU**: < 20% (single core)
- **Memory**: < 100 MB

## Usage Examples

### Basic Usage with All Features

```rust
use sniffer::{Sniffer, SnifferConfig, SnifferApi};

#[tokio::main]
async fn main() -> Result<()> {
    // Create sniffer with default config
    let config = SnifferConfig::default();
    let sniffer = Sniffer::new(config);
    
    // Access components
    let metrics = sniffer.get_metrics();
    let analytics = sniffer.get_analytics();
    let event_collector = sniffer.get_event_collector();
    let diagnostics = sniffer.get_handoff_diagnostics();
    let supervisor = sniffer.get_supervisor();
    
    // Start sniffer
    let mut rx = sniffer.start().await?;
    
    // Receive candidates
    while let Some(candidate) = rx.recv().await {
        println!("Received candidate: {:?}", candidate);
    }
    
    // Check diagnostics
    if let Some(avg_wait) = diagnostics.avg_queue_wait() {
        println!("Average queue wait: {:.2}μs", avg_wait);
    }
    
    // Get recent events
    let events = event_collector.get_recent(10);
    for event in events {
        println!("Event: {:?}", event);
    }
    
    // Graceful shutdown
    sniffer.stop();
    
    Ok(())
}
```

### Monitoring Queue Performance

```rust
use sniffer::telemetry::HandoffDiagnostics;
use std::sync::Arc;

async fn monitor_queue(diagnostics: Arc<HandoffDiagnostics>) {
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        if let Some(avg_wait) = diagnostics.avg_queue_wait() {
            println!("Average queue wait: {:.2}μs", avg_wait);
        }
        
        let histogram = diagnostics.get_histogram();
        println!("Wait time histogram:");
        println!("  0-10μs:      {}", histogram[0]);
        println!("  10-100μs:    {}", histogram[1]);
        println!("  100-1000μs:  {}", histogram[2]);
        println!("  >1000μs:     {}", histogram[3]);
        
        let high_drops = diagnostics.dropped_high_priority.load(Ordering::Relaxed);
        let low_drops = diagnostics.dropped_low_priority.load(Ordering::Relaxed);
        println!("Drops: high={}, low={}", high_drops, low_drops);
    }
}
```

### Event Analysis

```rust
use sniffer::{EventCollector, SnifferEvent};
use std::sync::Arc;

fn analyze_events(collector: Arc<EventCollector>) {
    let events = collector.get_recent(1000);
    
    let mut by_type = std::collections::HashMap::new();
    for event in events {
        *by_type.entry(event.event_type()).or_insert(0) += 1;
    }
    
    println!("Event distribution:");
    for (event_type, count) in by_type {
        println!("  {}: {}", event_type, count);
    }
}
```

## Testing

### Unit Tests
```bash
# Test event collector
cargo test --lib -- integration::tests::test_event_collector

# Test handoff diagnostics
cargo test --lib -- telemetry::tests

# Test supervisor
cargo test --lib -- supervisor::tests
```

### Integration Tests
```bash
# Run all integration tests
cargo test final_integration_tests
```

### Benchmarks
```bash
# Run all benchmarks
cargo bench --bench '*_bench'

# Run specific benchmark
cargo bench --bench prefilter_bench
```

### Stress Tests
```bash
# Run stress tests (normally ignored)
cargo test --release -- --ignored stress
```

## CI Integration

The CI workflow automatically runs on:
- Push to `main` or `develop` branches
- Pull requests targeting `main` or `develop`
- Changes to `sniffer/**` or `benches/**`

Results are:
- Stored as artifacts
- Compared against baselines
- Posted to PR comments
- Failed if regression > threshold

## Troubleshooting

### High Queue Wait Times
- Check `avg_queue_wait()` - should be < 1000μs
- Review histogram for distribution
- Consider increasing `channel_capacity`
- Check if consumer is keeping up

### High Drop Rates
- Check `dropped_high_priority` metric
- Review adaptive policy behavior
- Consider adjusting `drop_policy`
- Increase `batch_size` for better throughput

### Event Buffer Overflow
- EventCollector has 10k event limit
- Older events are dropped automatically
- Increase buffer size if needed
- Export events to external system

### Config Reload Not Working
- Verify file path is correct
- Check file permissions
- Review logs for reload messages
- File modification timestamp must change

## Future Enhancements

1. **Event Export** - Stream events to external monitoring systems
2. **Advanced Analytics** - ML-based threshold tuning
3. **Dynamic Worker Scaling** - Add/remove workers based on load
4. **Distributed Tracing** - OpenTelemetry integration
5. **Custom Metrics** - User-defined metric collection

## References

- [Supervisor Documentation](sniffer/supervisor.rs)
- [Telemetry Documentation](sniffer/telemetry.rs)
- [Dataflow Documentation](sniffer/dataflow.rs)
- [CI Workflow](.github/workflows/sniffer-performance.yml)
- [Benchmark Baselines](BENCHMARK_BASELINES.md)
