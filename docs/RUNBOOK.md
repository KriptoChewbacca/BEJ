# Sniffer Operations Runbook

## Table of Contents
1. [Restart Sniffer](#restart-sniffer)
2. [Diagnostics - High Drop Rate](#diagnostics-high-drop-rate)
3. [Scaling](#scaling)
4. [Threshold Regulation](#threshold-regulation)
5. [Common Errors and Responses](#common-errors-and-responses)

---

## Restart Sniffer

### Normal Restart (Graceful)
```rust
// In your application code:
sniffer.stop();  // Signals graceful shutdown
sleep(Duration::from_secs(2)).await;  // Wait for drain
let new_rx = sniffer.start_sniff().await?;
```

### Force Restart (Emergency)
```bash
# Kill the process
pkill -9 <sniffer_process>

# Restart with systemd (if configured)
systemctl restart sniffer.service
```

### Validation After Restart
```rust
// Check health status
assert!(sniffer.health());  // Should return true

// Check metrics
let metrics = sniffer.get_metrics();
println!("TX seen: {}", metrics.tx_seen.load(Ordering::Relaxed));
```

---

## Diagnostics - High Drop Rate

### Symptoms
- `dropped_full_buffer` metric increasing rapidly
- `backpressure_events` > 1000/min
- P99 latency > 20ms

### Diagnostic Steps

#### 1. Check Current Metrics
```rust
let metrics = sniffer.get_metrics();
let drop_rate = metrics.dropped_full_buffer.load(Ordering::Relaxed) as f64 
    / metrics.tx_seen.load(Ordering::Relaxed) as f64;
println!("Drop rate: {:.2}%", drop_rate * 100.0);

// Check priority breakdown
let high_dropped = metrics.high_priority_dropped.load(Ordering::Relaxed);
println!("High priority dropped: {}", high_dropped);
```

#### 2. Check Channel Depth
```bash
# Monitor queue depth via Prometheus
# Metric: sniffer_queue_depth
# Alert threshold: >80%
```

#### 3. Check Consumer Health (buy_engine)
```rust
// Verify buy_engine is consuming
// Check buy_engine metrics for processing rate
```

### Remediation

#### Option 1: Increase Channel Capacity
```rust
let mut config = SnifferConfig::default();
config.channel_capacity = 2048;  // Double from 1024
config.validate()?;
```

#### Option 2: Reduce Batch Size (Faster Drain)
```rust
config.batch_size = 5;  // Reduce from 10
config.batch_timeout_ms = 5;  // Faster timeout
```

#### Option 3: Pause and Resume
```rust
// Temporarily pause to let consumer catch up
sniffer.pause();
sleep(Duration::from_secs(5)).await;
sniffer.resume();
```

---

## Scaling

### Vertical Scaling (Single Instance)

#### CPU Optimization
- Ensure running on physical cores, not hyperthreads
- Set CPU affinity: `taskset -c 0-3 ./sniffer`
- Monitor with: `top -H -p <pid>`

#### Memory Optimization
```rust
// Reduce memory footprint
config.channel_capacity = 512;  // From 1024
config.stream_buffer_size = 2048;  // From 4096

// Clear latency samples periodically
sniffer.get_metrics().latency_samples.lock().clear();
```

### Horizontal Scaling (Multiple Instances)

#### Load Balancing by Program
```rust
// Instance 1: Pump.fun only
let config1 = SnifferConfig {
    grpc_endpoint: "http://endpoint1:10000".to_string(),
    ..Default::default()
};

// Instance 2: Raydium only
let config2 = SnifferConfig {
    grpc_endpoint: "http://endpoint2:10000".to_string(),
    ..Default::default()
};
```

#### Monitoring Multi-Instance Setup
- Use Prometheus federation
- Aggregate metrics: `sum(rate(sniffer_tx_seen[1m])) by (instance)`

---

## Threshold Regulation

### Understanding Thresholds

The sniffer uses dual-EMA for priority classification:
- **Short EMA**: α = 0.2 (reactive to spikes)
- **Long EMA**: α = 0.05 (baseline average)
- **Threshold**: Ratio > 1.5 → HIGH priority

### Adjusting Sensitivity

#### More Aggressive (Catch More HIGH)
```rust
config.ema_alpha_short = 0.3;  // More reactive
config.initial_threshold = 1.2;  // Lower threshold
```

#### More Conservative (Reduce FALSE POSITIVES)
```rust
config.ema_alpha_short = 0.1;  // Less reactive
config.initial_threshold = 2.0;  // Higher threshold
```

### Monitoring Threshold Effectiveness
```prometheus
# High vs Low ratio
rate(sniffer_high_priority_sent[1m]) / rate(sniffer_low_priority_sent[1m])

# High priority drop rate (should be <1%)
rate(sniffer_high_priority_dropped[1m]) / rate(sniffer_high_priority_sent[1m])
```

---

## Common Errors and Responses

### Error: "Stream ended, attempting reconnection"

**Cause**: gRPC connection lost  
**Action**: Automatic retry (up to 5 attempts)  
**Manual Override**:
```bash
# Check network
ping <grpc_endpoint>

# Check service
systemctl status geyser.service
```

### Error: "Failed to subscribe after 5 attempts"

**Cause**: Persistent connection failure  
**Action**: 
1. Verify gRPC endpoint is reachable
2. Check for firewall/network issues
3. Restart Geyser service
4. Update `grpc_endpoint` in config

```rust
let mut config = SnifferConfig::default();
config.grpc_endpoint = "http://backup-endpoint:10000".to_string();
```

### Error: "Candidate channel closed"

**Cause**: buy_engine stopped consuming  
**Action**:
```rust
// Restart buy_engine
buy_engine.restart().await?;

// Reconnect sniffer
let new_rx = sniffer.start_sniff().await?;
```

### High Security Drops

**Symptom**: `security_drop_count` increasing  
**Cause**: Malformed transactions or spam  
**Action**:
```rust
// Monitor trend
let security_rate = metrics.security_drop_count.load(Ordering::Relaxed) as f64
    / metrics.tx_seen.load(Ordering::Relaxed) as f64;

// If > 10%, investigate with DEBUG logging
std::env::set_var("RUST_LOG", "debug");
```

### Backpressure Alert

**Symptom**: `backpressure_events` > 100/sec  
**Root Causes**:
1. buy_engine processing too slow
2. Channel capacity too small
3. Burst traffic spike

**Triage**:
```bash
# Check buy_engine CPU/latency
# Prometheus: buy_engine_processing_latency_p99

# Check sniffer queue
# Prometheus: sniffer_queue_depth

# If queue >80%, increase capacity:
config.channel_capacity = 2048;
```

---

## Health Check Endpoints

### HTTP Health Endpoint (if integrated)
```bash
curl http://localhost:8080/health/sniffer
# Returns: {"status": "ok", "reconnects": 0}
```

### Programmatic Check
```rust
// Simple health check
if !sniffer.health() {
    error!("Sniffer unhealthy!");
    // Take action: alert, restart, etc.
}

// Detailed diagnostics
let metrics = sniffer.get_metrics();
println!("Health: {}", sniffer.health());
println!("Reconnects: {}", metrics.reconnect_count.load(Ordering::Relaxed));
println!("P99 latency: {:?}", metrics.get_percentile_latency(0.99));
```

---

## Performance Baselines

### Expected Metrics (Normal Operation)

| Metric | Baseline | Alert Threshold |
|--------|----------|----------------|
| TX Seen | 1000-5000/s | <100/s |
| Filter Rate | 90-95% | <85% |
| Drop Rate | <2% | >5% |
| P99 Latency | <10ms | >20ms |
| CPU Usage | 10-20% | >30% |
| Memory | 50-80MB | >150MB |
| Reconnects | 0 | >5/hour |

### Alerts Configuration (Prometheus)
```yaml
groups:
  - name: sniffer_alerts
    rules:
      - alert: SnifferHighDropRate
        expr: rate(sniffer_dropped_full_buffer[1m]) / rate(sniffer_tx_seen[1m]) > 0.05
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "Sniffer drop rate above 5%"
          
      - alert: SnifferHighLatency
        expr: sniffer_latency_p99_us > 20000
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Sniffer P99 latency above 20ms"
          
      - alert: SnifferUnhealthy
        expr: sniffer_health == 0
        for: 30s
        labels:
          severity: critical
        annotations:
          summary: "Sniffer health check failing"
```

---

## Configuration Tuning Guide

### Scenario: High Throughput (>10k tx/s)
```rust
SnifferConfig {
    channel_capacity: 2048,
    stream_buffer_size: 8192,
    batch_size: 20,
    batch_timeout_ms: 5,
    high_priority_max_retries: 3,
    ..Default::default()
}
```

### Scenario: Low Latency (<5ms P99)
```rust
SnifferConfig {
    channel_capacity: 512,
    stream_buffer_size: 1024,
    batch_size: 5,
    batch_timeout_ms: 2,
    high_priority_max_retries: 1,
    ..Default::default()
}
```

### Scenario: Resource Constrained
```rust
SnifferConfig {
    channel_capacity: 256,
    stream_buffer_size: 512,
    batch_size: 10,
    batch_timeout_ms: 20,
    telemetry_interval_secs: 10,  // Reduce telemetry frequency
    ..Default::default()
}
```

---

## Maintenance Windows

### Before Maintenance
```rust
// 1. Pause sniffer
sniffer.pause();

// 2. Drain channel
sleep(Duration::from_secs(5)).await;

// 3. Export metrics
let final_metrics = sniffer.get_metrics().snapshot();
save_to_file("pre_maintenance_metrics.json", &final_metrics);

// 4. Stop
sniffer.stop();
```

### After Maintenance
```rust
// 1. Verify config
config.validate()?;

// 2. Start sniffer
let rx = sniffer.start_sniff().await?;

// 3. Monitor for 5 minutes
for _ in 0..30 {
    sleep(Duration::from_secs(10)).await;
    assert!(sniffer.health());
    let metrics = sniffer.get_metrics();
    println!("TX seen: {}", metrics.tx_seen.load(Ordering::Relaxed));
}
```

---

## Troubleshooting Decision Tree

```
Is sniffer.health() == false?
├─ Yes → Check reconnect_count
│  ├─ >10 → gRPC endpoint issue
│  │  └─ Action: Verify endpoint, check network
│  └─ <10 → Channel closed
│     └─ Action: Restart buy_engine
└─ No → Check metrics
   ├─ drop_rate >5% → Backpressure issue
   │  ├─ queue_depth >80% → Increase capacity
   │  └─ buy_engine slow → Optimize consumer
   ├─ p99_latency >20ms → CPU overload
   │  └─ Action: Reduce batch_size, check CPU affinity
   └─ filter_rate <85% → Prefilter ineffective
      └─ Action: Review prefilter logic
```

---

## Contact & Escalation

**L1 Support**: Check metrics, restart sniffer  
**L2 Support**: Tune configuration, analyze logs  
**L3 Support**: Code changes, performance profiling

**Escalation Criteria**:
- Health check failing >5 minutes
- Drop rate >10% sustained
- Reconnects >20 in 1 hour
- Memory >150MB
