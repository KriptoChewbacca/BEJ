# Sniffer Architecture Diagram

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          ULTRA SNIFFER v2.0                              │
│                    100% Optimized Architecture                           │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│  EXTERNAL INPUT                                                          │
│  ┌──────────────┐                                                        │
│  │  Geyser gRPC │ ◄── TLS/HTTPS Encrypted Stream                        │
│  │   Stream     │     (Solana Transactions)                             │
│  └──────┬───────┘                                                        │
│         │                                                                │
└─────────┼────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 1.5: CONNECTION LAYER (Retry & Reconnect)                         │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  stream_core::subscribe_with_retry()                             │   │
│  │  • Exponential Backoff + Jitter                                  │   │
│  │  • Max 5 attempts                                                │   │
│  │  • Auto-reconnect every 10s after exhaustion                     │   │
│  │  • Health status tracking (health_ok atomic)                     │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────┬───────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 2.1: STREAM BUFFER (Bounded, 4096 elements)                       │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  MockStreamReceiver::recv()                                      │   │
│  │  • Bounded capacity: 4096 (configurable)                         │   │
│  │  • select! for multiplexing                                      │   │
│  │  • Async loop, no RPC blocking                                   │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────┬───────────────────────────────────────────────────────────────┘
          │
          ▼ Bytes (transaction payload)
┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 2.2: HOT-PATH PREFILTER (Zero-Copy, SIMD-Style)                   │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  prefilter::should_process(&tx_bytes)                            │   │
│  │  ┌────────────────────────────────────────────────────────────┐  │   │
│  │  │ 1. Size check (min 128 bytes)                              │  │   │
│  │  │ 2. Vote transaction rejection                              │  │   │
│  │  │ 3. Pump.fun program ID detection (windows pattern)         │  │   │
│  │  │ 4. SPL Token program ID detection                          │  │   │
│  │  │ ► 90% rejection rate ◄                                     │  │   │
│  │  └────────────────────────────────────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│  Metrics: tx_filtered++                                                 │
└─────────┬───────────────────────────────────────────────────────────────┘
          │
          ▼ PASS (10% of transactions)
┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 2.3: DATA EXTRACTION (Offset-Based, Zero SDK)                     │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  prefilter::extract_mint(&tx_bytes)                              │   │
│  │  prefilter::extract_accounts(&tx_bytes)                          │   │
│  │  • BytesMut for zero-copy                                        │   │
│  │  • Offset-based pubkey extraction                                │   │
│  │  • Structural validation only (length checks)                    │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────┬───────────────────────────────────────────────────────────────┘
          │
          ▼ mint, accounts
┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 3.3: INLINE SECURITY CHECKS (Lightweight)                         │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  • Account count: 1-8 range                                      │   │
│  │  • Pubkey validity                                               │   │
│  │  • No heavy crypto in hot path                                   │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│  Metrics: security_drop_count++                                         │
└─────────┬───────────────────────────────────────────────────────────────┘
          │
          ▼ VALID
┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 2.5: PREDICTIVE HEURISTICS (EMA-Based)                            │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  PredictiveAnalytics::update(volume)                             │   │
│  │  ┌────────────────────────────────────────────────────────────┐  │   │
│  │  │ Short EMA (α=0.2)  ─┐                                      │  │   │
│  │  │ Long EMA (α=0.05)   ├─► Acceleration Ratio                 │  │   │
│  │  │ Threshold (1.5)    ─┘                                      │  │   │
│  │  │                                                            │  │   │
│  │  │ ratio > threshold → HIGH Priority                         │  │   │
│  │  │ ratio ≤ threshold → LOW Priority                          │  │   │
│  │  └────────────────────────────────────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────┬───────────────────────────────────────────────────────────────┘
          │
          ▼ priority, price_hint
┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 1.3: PREMINT CANDIDATE CREATION (Minimal, ~90 bytes)              │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  PremintCandidate::new(mint, accounts, price_hint, trace_id,     │   │
│  │                        priority)                                  │   │
│  │  ┌────────────────────────────────────────────────────────────┐  │   │
│  │  │ mint: Pubkey              (32 bytes)                       │  │   │
│  │  │ accounts: SmallVec        (~40 bytes, stack-allocated)     │  │   │
│  │  │ price_hint: f64           (8 bytes)                        │  │   │
│  │  │ trace_id: u64             (8 bytes)                        │  │   │
│  │  │ priority: PriorityLevel   (1 byte)                         │  │   │
│  │  │ TOTAL: ~90 bytes, zero heap allocations                    │  │   │
│  │  └────────────────────────────────────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────┬───────────────────────────────────────────────────────────────┘
          │
          ▼ Batch (10-20 candidates)
┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 4.4: BATCHING (Configurable Size & Timeout)                       │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  batch: Vec<PremintCandidate>                                    │   │
│  │  • Size: 10 (configurable)                                       │   │
│  │  • Timeout: 10ms (configurable)                                  │   │
│  │  • Send when: full OR timeout                                    │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────┬───────────────────────────────────────────────────────────────┘
          │
          ▼ Batch ready
┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 2.4 & 1.1: BOUNDED CHANNEL + DROP POLICY                          │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  send_batch() with try_send()                                    │   │
│  │  ┌────────────────────────────────────────────────────────────┐  │   │
│  │  │ mpsc::channel(capacity=1024)                               │  │   │
│  │  │                                                            │  │   │
│  │  │ For each candidate:                                        │  │   │
│  │  │   try_send()                                               │  │   │
│  │  │   ├─ OK → candidates_sent++                                │  │   │
│  │  │   └─ FULL                                                  │  │   │
│  │  │       ├─ HIGH Priority: retry 1-2x with 50μs sleep         │  │   │
│  │  │       └─ LOW Priority: drop immediately                    │  │   │
│  │  │                                                            │  │   │
│  │  │ No .await in hot path!                                     │  │   │
│  │  └────────────────────────────────────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│  Metrics: backpressure_events++, dropped_full_buffer++                 │
└─────────┬───────────────────────────────────────────────────────────────┘
          │
          ▼ Receiver<PremintCandidate>
┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 1.1 & 1.4: ONE-DIRECTIONAL API (No Callbacks)                     │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  BUY ENGINE (Consumer)                                           │   │
│  │  ┌────────────────────────────────────────────────────────────┐  │   │
│  │  │ while let Some(candidate) = rx.recv().await {              │  │   │
│  │  │     // Process candidate                                   │  │   │
│  │  │     // Build transaction                                   │  │   │
│  │  │     // Submit to chain                                     │  │   │
│  │  │ }                                                          │  │   │
│  │  └────────────────────────────────────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 2.6 & 3.1: TELEMETRY (Atomics Only, No Locks in Hot Path)         │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  SnifferMetrics (All AtomicU64, Relaxed ordering)               │   │
│  │  ┌────────────────────────────────────────────────────────────┐  │   │
│  │  │ tx_seen                    background_worker()              │  │   │
│  │  │ tx_filtered                     │                           │  │   │
│  │  │ candidates_sent                 ▼                           │  │   │
│  │  │ dropped_full_buffer        ┌─────────┐                      │  │   │
│  │  │ security_drop_count   ────►│ Every 5s│────► JSON snapshot   │  │   │
│  │  │ backpressure_events        └─────────┘      Prometheus      │  │   │
│  │  │ reconnect_count                                             │  │   │
│  │  │ high_priority_sent                                          │  │   │
│  │  │ low_priority_sent                                           │  │   │
│  │  │ high_priority_dropped                                       │  │   │
│  │  └────────────────────────────────────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 3.2: LATENCY TRACKING (Sampled, P50/P95/P99)                      │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  Every 100th transaction:                                        │   │
│  │    latency = processing_time                                     │   │
│  │    metrics.record_latency(latency_us)                            │   │
│  │                                                                   │   │
│  │  Background calculation:                                         │   │
│  │    sorted = samples.sort()                                       │   │
│  │    p50 = sorted[len * 0.50]                                      │   │
│  │    p95 = sorted[len * 0.95]                                      │   │
│  │    p99 = sorted[len * 0.99]                                      │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 3.4: HEALTH & CONTROL API                                         │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  Public Methods:                                                 │   │
│  │  • health() → bool          (gRPC + channel + reconnect check)  │   │
│  │  • pause()                  (stop producing candidates)          │   │
│  │  • resume()                 (restart producing)                  │   │
│  │  • is_running() → bool                                           │   │
│  │  • is_paused() → bool                                            │   │
│  │  • get_metrics() → Arc<SnifferMetrics>                           │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 4.6: GRACEFUL SHUTDOWN                                             │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  sniffer.stop()                                                  │   │
│  │    ↓                                                             │   │
│  │  running.store(false)                                            │   │
│  │    ↓                                                             │   │
│  │  Drain remaining batch                                           │   │
│  │    ↓                                                             │   │
│  │  Close telemetry exporters                                       │   │
│  │    ↓                                                             │   │
│  │  Signal buy_engine (channel close)                               │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│  TASK 4.5: MONITORING STACK (Docker + Prometheus + Grafana)             │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  ┌──────────┐    ┌────────────┐    ┌─────────┐                  │   │
│  │  │ Sniffer  │───►│ Prometheus │───►│ Grafana │                  │   │
│  │  │ :8080    │    │ :9090      │    │ :3000   │                  │   │
│  │  └──────────┘    └────────────┘    └─────────┘                  │   │
│  │       │                 │                                        │   │
│  │  /health           /metrics      Dashboards + Alerts            │   │
│  │  /metrics          /alerts       Drop rate >5%                  │   │
│  │                    /targets      Latency P99 >20ms              │   │
│  │                                  Queue depth >80%               │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘

═══════════════════════════════════════════════════════════════════════════
  PERFORMANCE CHARACTERISTICS
═══════════════════════════════════════════════════════════════════════════

  Throughput:        >10,000 tx/s
  Latency (P99):     <10ms
  Memory:            <150MB
  CPU:               <25% (i5)
  Drop Rate:         <5%
  Filter Rate:       >90%
  Architecture:      Zero deadlocks, bounded channels, atomics only
  
═══════════════════════════════════════════════════════════════════════════
```

## Data Flow Diagram

```
Transaction Bytes → Prefilter → Extract → Security → Heuristics → Candidate → Batch → Channel → Buy Engine
     (100%)          (10%)       (9%)      (8.5%)      (8.5%)      (8.5%)     (8%)     (7.6%)     (100% of sent)

Filter Rejection: 90%
Security Drops:   5% of passed
Drop Rate:        <5% of candidates due to backpressure
Success Rate:     >95% of candidates delivered
```

## Configuration Flow

```
sniffer_config.toml
        │
        ├─► SnifferConfig::from_file()
        │         │
        │         └─► validate()
        │                 │
        │                 └─► Sniffer::new(config)
        │                           │
        └───────────────────────────┘
                                    │
                                    ▼
                          start_sniff() → Runtime
```

## Monitoring Flow

```
Metrics (Atomics) → Background Worker (every 5s) → JSON Snapshot → Prometheus → Grafana
                                                                         ↓
                                                                   Alert Rules
                                                                         ↓
                                                                   Alertmanager
```
