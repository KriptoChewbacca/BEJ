# WebSocket Streaming - Quick Reference

## Quick Start

### 1. Basic Setup (Native Solana - Free)

```bash
# .env configuration
STREAMING_MODE=websocket
SOLANA_WS_URL=wss://api.devnet.solana.com
```

```bash
# Build and run
cargo build --release --features ws-stream
cargo run --release
```

### 2. Enhanced Providers (Free Tier)

#### Helius (100k requests/day)
```bash
SOLANA_WS_URL=wss://devnet.helius-rpc.com/?api-key=YOUR_KEY
```

#### QuickNode (10M credits/month)
```bash
SOLANA_WS_URL=wss://your-endpoint.solana-devnet.quiknode.pro/YOUR_TOKEN/
```

#### Alchemy (300M compute units/month)
```bash
SOLANA_WS_URL=wss://solana-devnet.g.alchemy.com/v2/YOUR_KEY
```

## Code Examples

### Basic Connection
```rust
use bot::streaming::websocket_stream::WebSocketStream;

let stream = WebSocketStream::new("wss://api.devnet.solana.com".to_string());
let client = stream.connect().await?;
```

### Subscribe to Program
```rust
use tokio::sync::mpsc;

let (tx, mut rx) = mpsc::unbounded_channel();
stream.subscribe_program(client, &program_id, tx).await?;

// Receive updates
while let Some(update) = rx.recv().await {
    println!("Update: slot {}, data: {} bytes", 
             update.slot, update.account_data.len());
}
```

### Subscribe to Signature
```rust
let signature = Signature::from_str("...")?;
let (tx, mut rx) = mpsc::unbounded_channel();
stream.subscribe_signatures(client, &signature, tx).await?;
```

## Feature Flags

```bash
# WebSocket only (default)
cargo build --features ws-stream

# Geyser gRPC only
cargo build --features geyser-stream

# Both (runtime selection)
cargo build --features ws-stream,geyser-stream
```

## Configuration Files

### Config.toml
```toml
[streaming]
mode = "websocket"
websocket_url = "wss://api.devnet.solana.com"
commitment = "confirmed"
```

### .env
```bash
STREAMING_MODE=websocket
SOLANA_WS_URL=wss://api.devnet.solana.com
```

## Testing

```bash
# Run all tests
cargo test --features ws-stream

# Run WebSocket tests (network tests ignored by default)
cargo test --features ws-stream --test websocket_integration_test

# Run with network tests (requires internet)
cargo test --features ws-stream -- --ignored

# Run demo
cargo run --example websocket_demo --features ws-stream
```

## Common Issues

### Connection Fails
- Check internet connection
- Verify WebSocket URL is correct
- Try different provider
- Check API key if using enhanced provider

### Rate Limited
- Switch to provider with higher limits
- Reduce subscription frequency
- Upgrade to premium tier

### Compilation Errors
- Ensure `ws-stream` feature is enabled
- Check Rust version (>= 1.83.0)
- Run `cargo clean` and rebuild

## Architecture

```
┌─────────────────┐
│ WebSocketStream │
└────────┬────────┘
         │
         ├─ connect() → Arc<PubsubClient>
         │
         ├─ subscribe_program()
         │  └─ tokio::spawn(notifications loop)
         │
         └─ subscribe_signatures()
            └─ tokio::spawn(notifications loop)
```

## Performance Tips

1. **Commitment Level**: Use `confirmed` for balanced speed/finality
2. **Subscriptions**: Only subscribe to necessary programs
3. **Channel Handling**: Process updates quickly to avoid backpressure
4. **Connection Pooling**: Reuse clients for multiple subscriptions

## Upgrade Path

### From WebSocket to Geyser
1. Update Config.toml:
   ```toml
   mode = "geyser"
   geyser_endpoint = "https://your-endpoint:10000"
   ```
2. Rebuild with geyser feature:
   ```bash
   cargo build --features geyser-stream
   ```

## Resources

- 📖 [Full Setup Guide](docs/WEBSOCKET_SETUP.md)
- 🔒 [Security Summary](WEBSOCKET_SECURITY_SUMMARY.md)
- 💻 [Demo Example](examples/websocket_demo.rs)
- 🧪 [Integration Tests](tests/websocket_integration_test.rs)

## Support

For issues or questions:
- Check existing issues on GitHub
- Review Solana WebSocket docs
- Contact RPC provider support
- File new issue with logs and config
