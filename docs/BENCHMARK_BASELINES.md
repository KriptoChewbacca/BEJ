# Sniffer Performance Baselines

This file contains the baseline performance metrics for the Sniffer module.
These baselines are used for regression detection in CI.

## Benchmark Targets

### Prefilter Benchmark
- **Target**: < 1 μs per transaction
- **Baseline**: 500 ns/iter (0.5 μs)
- **Threshold**: +20% regression allowed

### Extractor Benchmark
- **Target**: < 5 μs per extraction
- **Baseline**: 3.2 μs/iter
- **Threshold**: +20% regression allowed

### Analytics Benchmark
- **Target**: < 100 μs per EMA update
- **Baseline**: 45 μs/iter
- **Threshold**: +20% regression allowed

## Stress Test Thresholds

### Throughput
- **Target**: ≥ 10,000 tx/s
- **Baseline**: 12,500 tx/s
- **Threshold**: -10% allowed (minimum 9,000 tx/s)

### Latency
- **P50 Target**: < 2 ms
- **P95 Target**: < 5 ms
- **P99 Target**: < 10 ms
- **Baseline P50**: 1.2 ms
- **Baseline P95**: 3.8 ms
- **Baseline P99**: 7.5 ms
- **Threshold**: +20% regression allowed

### Drop Rate
- **Target**: < 5%
- **Baseline**: 2.1%
- **Threshold**: Must stay < 5%

### Resource Usage
- **CPU Target**: < 20% (single core)
- **Memory Target**: < 100 MB
- **Baseline CPU**: 15%
- **Baseline Memory**: 75 MB

## Update History

- 2024-11-07: Initial baselines established
  - Prefilter: 500 ns/iter
  - Extractor: 3.2 μs/iter
  - Analytics: 45 μs/iter
  - Throughput: 12,500 tx/s
  - P99 Latency: 7.5 ms
  - Drop Rate: 2.1%

## How to Update Baselines

1. Run full benchmark suite: `cargo bench --bench '*_bench'`
2. Run stress tests: `cargo test --release -- --ignored stress`
3. Collect metrics and update this file
4. Commit updated baselines with justification in commit message
5. PR must be approved by two reviewers for baseline changes

## CI Integration

The CI workflow (`sniffer-performance.yml`) automatically:
- Runs all benchmarks on every PR
- Compares results against these baselines
- Fails PR if regression exceeds thresholds
- Posts performance comparison as PR comment
- Archives benchmark results as artifacts
