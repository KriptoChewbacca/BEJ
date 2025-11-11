# Sniffer Quick Reference Guide

## 🚀 Quick Start

### Basic Usage
```rust
use sniffer::{Sniffer, SnifferConfig};

// Create with defaults
let sniffer = Sniffer::new(SnifferConfig::default());

// Or load from file
let config = SnifferConfig::from_file("sniffer_config.toml")?;
let sniffer = Sniffer::new(config);

// Start sniffing
let mut rx = sniffer.start_sniff().await?;

// Consume candidates
while let Some(candidate) = rx.recv().await {
    // Process candidate
}
```

## 📊 Key Metrics

### Performance Targets
| Metric | Target | How to Check |
|--------|--------|--------------|
| Throughput | >10k tx/s | `tx_seen` rate in Prometheus |
| Latency P99 | <10ms | `sniffer_latency_p99_us` |
| Drop Rate | <5% | `dropped_full_buffer / tx_seen` |
| Memory | <150MB | `process_resident_memory_bytes` |
| CPU | <25% | `process_cpu_seconds_total` |

### Check Metrics in Code
```rust
let metrics = sniffer.get_metrics();
println!("{}", metrics.snapshot());

// Get percentiles
if let Some(p99) = metrics.get_percentile_latency(0.99) {
    println!("P99 latency: {}us", p99);
}
```

## ⚙️ Configuration Cheat Sheet

### Common Tuning Scenarios

#### High Throughput (>10k tx/s)
```toml
channel_capacity = 2048
stream_buffer_size = 8192
batch_size = 20
batch_timeout_ms = 5
high_priority_max_retries = 3
```

#### Low Latency (<5ms)
```toml
channel_capacity = 512
stream_buffer_size = 1024
batch_size = 5
batch_timeout_ms = 2
high_priority_max_retries = 1
```

#### Memory Constrained
```toml
channel_capacity = 256
stream_buffer_size = 512
telemetry_interval_secs = 10
```

## 🔧 Common Operations

### Health Check
```rust
// Simple check
if !sniffer.health() {
    eprintln!("Unhealthy!");
}

// Detailed check
let is_running = sniffer.is_running();  // Use public method
let reconnects = metrics.reconnect_count.load(Ordering::Relaxed);
```

### Pause/Resume
```rust
// Pause during maintenance
sniffer.pause();
// ... do maintenance ...
sniffer.resume();
```

### Graceful Shutdown
```rust
// Signal shutdown
sniffer.stop();

// Wait for drain (in consumer loop)
while let Some(candidate) = rx.recv().await {
    // Process remaining candidates
}
```

## 🐛 Troubleshooting

### High Drop Rate (>5%)
1. Check: `dropped_full_buffer` metric
2. Increase: `channel_capacity`
3. Reduce: `batch_size` (faster drain)
4. Verify: buy_engine is consuming

### High Latency (>10ms)
1. Check: `sniffer_latency_p99_us` 
2. Reduce: `batch_size` and `batch_timeout_ms`
3. Check CPU usage
4. Verify no CPU throttling

### Frequent Reconnects
1. Check: `reconnect_count` metric
2. Verify: gRPC endpoint reachable
3. Check: Network stability
4. Review: Geyser service logs

### No Activity (tx_seen == 0)
1. Check: gRPC stream connected
2. Verify: Endpoint URL correct
3. Check: Geyser service running
4. Enable DEBUG logging

## 🔍 Debugging

### Enable Debug Logging
```bash
RUST_LOG=debug ./your_app
```

```rust
// In code
std::env::set_var("RUST_LOG", "debug");
```

### Check Specific Component
```bash
RUST_LOG=sniffer=debug ./your_app
```

### Trace Specific Transaction
```rust
// Use trace_id from candidate
debug!(trace_id = candidate.trace_id, "Processing candidate");
```

## 📈 Prometheus Queries

### Drop Rate
```promql
rate(sniffer_dropped_full_buffer[1m]) 
/ 
rate(sniffer_tx_seen[1m])
```

### Throughput
```promql
rate(sniffer_tx_seen[1m])
```

### Queue Depth %
```promql
sniffer_queue_depth / sniffer_channel_capacity
```

### P99 Latency
```promql
sniffer_latency_p99_us
```

## 🎯 Priority System

### How It Works
- **EMA Calculation**: Short window (α=0.2) / Long window (α=0.05)
- **Threshold**: Default 1.5 (configurable)
- **HIGH Priority**: Acceleration ratio > threshold
- **LOW Priority**: Acceleration ratio ≤ threshold

### Tuning Priority
```toml
# More aggressive (more HIGH priority)
ema_alpha_short = 0.3
initial_threshold = 1.2

# More conservative (fewer HIGH priority)
ema_alpha_short = 0.1
initial_threshold = 2.0
```

## 🔐 Security Checks

### Inline Checks (Hot Path)
- Account count: 1-8 range
- Pubkey validity: byte length
- Transaction size: ≥128 bytes

### When Security Drops Are High (>10%)
1. Check `security_drop_count` metric
2. Enable DEBUG logging
3. Review transaction patterns
4. Verify not under attack

## 📦 Docker Commands

### Build and Run
```bash
docker-compose -f docker-compose.sniffer.yml up -d
```

### View Logs
```bash
docker logs ultra-sniffer -f
```

### Restart Service
```bash
docker-compose -f docker-compose.sniffer.yml restart sniffer
```

### Stop All
```bash
docker-compose -f docker-compose.sniffer.yml down
```

## 🎨 Grafana Dashboards

### Key Panels to Create
1. **Throughput**: `rate(sniffer_tx_seen[1m])`
2. **Drop Rate**: `rate(sniffer_dropped_full_buffer[1m]) / rate(sniffer_tx_seen[1m])`
3. **Latency P99**: `sniffer_latency_p99_us`
4. **Queue Depth**: `sniffer_queue_depth`
5. **Health**: `sniffer_health`
6. **Reconnects**: `rate(sniffer_reconnect_count[5m])`

## 🧪 Testing

### Run Unit Tests
```bash
cargo test
```

### Run Stress Tests
```bash
cargo test --test sniffer_stress_test -- --nocapture
```

### Specific Test
```bash
cargo test stress_test_10k_tps_30s -- --nocapture
```

## 📚 Reference Files

- **Implementation**: `sniffer.rs`
- **Configuration**: `sniffer_config.toml`
- **Operations**: `RUNBOOK.md`
- **Summary**: `SNIFFER_OPTIMIZATION_SUMMARY.md`
- **Tests**: `sniffer_stress_test.rs`
- **Deployment**: `docker-compose.sniffer.yml`
- **Alerts**: `prometheus-alerts.yml`

## 💡 Tips & Tricks

### Optimize for Bursts
```toml
# Larger buffer to absorb spikes
stream_buffer_size = 8192
channel_capacity = 2048
```

### Optimize for Consistency
```toml
# Smaller buffer for predictable performance
stream_buffer_size = 1024
channel_capacity = 512
```

### Reduce Memory Footprint
```rust
// Periodically clear latency samples
sniffer.get_metrics().latency_samples.lock().clear();
```

### CPU Affinity (Linux)
```bash
taskset -c 0-3 ./your_app
```

## 🚨 Alert Thresholds

| Alert | Threshold | Action |
|-------|-----------|--------|
| Drop Rate | >5% | Increase capacity |
| P99 Latency | >20ms | Reduce batch size |
| Queue Depth | >80% | Scale or optimize |
| Reconnects | >1/5min | Check network |
| Health | fail 1min | Immediate restart |

## 🔗 Quick Links

- **Metrics**: `http://localhost:9090`
- **Grafana**: `http://localhost:3000`
- **Health**: `http://localhost:8080/health`

---

**Remember**: When in doubt, check `RUNBOOK.md` for detailed procedures! 📖
