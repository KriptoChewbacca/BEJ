# Examples

This directory contains example applications demonstrating various features of the trading bot.

## Running Examples

### Standard Examples (No Features Required)

These examples work with default features:

```bash
cargo run --example complete_example
cargo run --example final_integration_stage
cargo run --example tx_build_output_demo
cargo run --example universe_features_demo
```

### WebSocket Streaming Example

The WebSocket example uses the `ws-stream` feature (enabled by default):

```bash
# With default features:
cargo run --example websocket_demo

# Or explicitly:
cargo run --example websocket_demo --features ws-stream
```

### GUI Monitoring Example

The GUI monitoring dashboard requires the `gui_monitor` feature:

```bash
cargo run --example gui_monitoring --features gui_monitor
```

This example demonstrates:
- Real-time position tracking
- Price chart visualization
- Bot control (START/STOP)
- P&L calculations

## Feature Flags

- `ws-stream` (default): WebSocket streaming support
- `gui_monitor`: GUI monitoring dashboard with eframe/egui
- `geyser-stream`: Geyser gRPC streaming (not yet implemented, stub only)
- `test_utils`: Testing utilities (for benchmarks)
- `pumpfun`: Pump.fun DEX integration
- `zk_enabled`: ZK-SNARKs support
- `perf`: Performance profiling
- `prod_parse`: Production parsing optimizations
- `multi_token`: Multi-token portfolio support

## Example Descriptions

- **complete_example.rs**: Minimal bot setup and initialization
- **final_integration_stage.rs**: Integration of all major components
- **tx_build_output_demo.rs**: Transaction builder demonstration
- **universe_features_demo.rs**: Universe-grade features (Multi-Agent RL, Provenance Graph, Quantum Pruner)
- **websocket_demo.rs**: WebSocket streaming connection and subscription
- **gui_monitoring.rs**: GUI monitoring dashboard with live price updates
