# A4 Quick Reference

## Configuration Parameters (New in A4)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `send_max_retries` | `u8` | `3` | Max retries for HIGH priority sends |
| `send_retry_delay_us` | `u64` | `100` | Retry delay in microseconds |
| `stream_buffer_capacity` | `usize` | `2048` | Internal buffer capacity |
| `drop_policy` | `DropPolicy` | `DropNewest` | Drop strategy when full |
| `batch_send_mode` | `BatchSendMode` | `Sync` | Batch processing mode |
| `graceful_shutdown_timeout_ms` | `u64` | `5000` | Shutdown drain timeout |

## Enums

### DropPolicy
```rust
pub enum DropPolicy {
    DropOldest,  // Drop old items first
    DropNewest,  // Drop new items first (default)
    Block,       // Block until space (caution!)
}
```

### BatchSendMode
```rust
pub enum BatchSendMode {
    Sync,   // Sequential processing (default)
    Async,  // Parallel workers
}
```

## Metrics

### New in A4
- `stream_buffer_depth` - Current buffer size (0 to batch_size)

### Existing (Enhanced)
- `backpressure_events` - Channel full events
- `dropped_full_buffer` - Total drops

## Usage Examples

### Basic Configuration
```rust
let config = SnifferConfig::default();
let sniffer = Sniffer::new(config);
```

### High-Throughput Configuration
```rust
let config = SnifferConfig {
    batch_send_mode: BatchSendMode::Async,
    send_max_retries: 5,
    stream_buffer_capacity: 4096,
    ..Default::default()
};
```

### Graceful Shutdown
```rust
// Signal shutdown
sniffer.stop();

// Process loop will:
// 1. Exit main loop
// 2. Drain remaining batch
// 3. Timeout after graceful_shutdown_timeout_ms
```

## Performance Characteristics

| Mode | Throughput | Latency | Overhead |
|------|------------|---------|----------|
| Sync | 10k+ tx/s | 5-10μs | Minimal |
| Async | 15k+ tx/s | 10-20μs | Task spawn |

## Monitoring

### Prometheus Alerts
```yaml
# High backpressure
- alert: SnifferBackpressure
  expr: rate(sniffer_backpressure_events[1m]) > 100
  for: 5m

# Buffer saturation
- alert: SnifferBufferFull
  expr: sniffer_stream_buffer_depth / 2048 > 0.8
  for: 2m

# High drop rate
- alert: SnifferDropRate
  expr: rate(sniffer_dropped_full_buffer[5m]) / rate(sniffer_tx_seen[5m]) > 0.05
  for: 5m
```

## Migration from A3 to A4

### Code Changes
No breaking changes! A4 adds new optional parameters with sensible defaults.

### Config Changes
```rust
// A3 config (still works)
let config = SnifferConfig::default();

// A4 config (with new features)
let config = SnifferConfig {
    send_max_retries: 5,  // New
    batch_send_mode: BatchSendMode::Async,  // New
    ..Default::default()
};
```

## Testing

### Run A4 Tests
```bash
# Standalone test file
cargo test --test sniffer_a4_test

# Integrated tests in sniffer.rs
cargo test test_a4
```

### Test Scenarios Covered
1. ✅ Backpressure (20k tx/s producer, 20ms consumer)
2. ✅ Async mode ordering (parallel workers)
3. ✅ Graceful shutdown (100% delivery or bounded drops)
4. ✅ Sustained load (10k tx/s for 2s)
5. ✅ Config validation
6. ✅ Metrics tracking

## Troubleshooting

### High Backpressure
```rust
// Solution 1: Increase buffer
config.stream_buffer_capacity = 4096;

// Solution 2: Use async mode
config.batch_send_mode = BatchSendMode::Async;

// Solution 3: Adjust consumer
// Ensure buy_engine can keep up
```

### Slow Shutdown
```rust
// Increase timeout
config.graceful_shutdown_timeout_ms = 10000;

// Or accept some drops
// (check dropped_full_buffer metric)
```

### High Drop Rate
```rust
// Solution 1: More retries
config.send_max_retries = 5;

// Solution 2: Faster retries
config.send_retry_delay_us = 50;

// Solution 3: Larger channel
config.channel_capacity = 2048;
```

## Best Practices

1. **Start with Sync mode** - Simpler, lower overhead
2. **Monitor buffer depth** - Early warning for saturation
3. **Test graceful shutdown** - Ensure clean exits
4. **Tune for workload** - Adjust based on metrics
5. **Use Async for bursts** - Better handling of spikes

## Quick Checklist

- [ ] Configure `send_max_retries` for HIGH priority
- [ ] Set appropriate `graceful_shutdown_timeout_ms`
- [ ] Choose `batch_send_mode` (Sync vs Async)
- [ ] Set up monitoring for `stream_buffer_depth`
- [ ] Configure alerts for backpressure events
- [ ] Test graceful shutdown in staging
- [ ] Validate drop rate < 1% in production
