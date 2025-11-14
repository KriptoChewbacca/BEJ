# WebSocket Streaming Setup Guide

## Overview

This guide explains how to set up WebSocket streaming for the Solana trading bot as a free alternative to Geyser gRPC. WebSocket streaming enables real-time transaction monitoring on devnet/testnet without requiring paid services.

## Free Tier Options for Testing

### 1. Native Solana WebSocket (Completely Free)

- **Endpoint**: `wss://api.devnet.solana.com`
- **Limits**: Rate limited, basic features
- **Best for**: Initial development, basic testing
- **Setup**: No registration required

```bash
# In .env file
STREAMING_MODE=websocket
SOLANA_WS_URL=wss://api.devnet.solana.com
```

### 2. Helius Free Tier (Recommended)

- **Endpoint**: `wss://devnet.helius-rpc.com/?api-key=YOUR_KEY`
- **Limits**: 100,000 requests/day
- **Sign up**: https://www.helius.dev
- **Best for**: Development and extended testing
- **Features**: Enhanced APIs, better reliability

```bash
# In .env file
STREAMING_MODE=websocket
SOLANA_WS_URL=wss://devnet.helius-rpc.com/?api-key=YOUR_API_KEY
```

### 3. QuickNode Free Tier

- **Endpoint**: Custom endpoint with token
- **Limits**: 10M credits/month
- **Sign up**: https://www.quicknode.com
- **Best for**: High-performance testing
- **Features**: Low latency, high throughput

```bash
# In .env file
STREAMING_MODE=websocket
SOLANA_WS_URL=wss://your-endpoint.solana-devnet.quiknode.pro/YOUR_TOKEN/
```

### 4. Alchemy Free Tier

- **Endpoint**: `wss://solana-devnet.g.alchemy.com/v2/YOUR_KEY`
- **Limits**: 300M compute units/month
- **Sign up**: https://www.alchemy.com
- **Best for**: Enterprise-grade testing
- **Features**: Advanced monitoring, webhooks

```bash
# In .env file
STREAMING_MODE=websocket
SOLANA_WS_URL=wss://solana-devnet.g.alchemy.com/v2/YOUR_API_KEY
```

## Configuration

### Method 1: Using .env file

1. Copy `.env.example` to `.env`:
   ```bash
   cp .env.example .env
   ```

2. Edit `.env` and set your preferred provider:
   ```bash
   STREAMING_MODE=websocket
   SOLANA_WS_URL=wss://api.devnet.solana.com
   ```

3. Run the bot:
   ```bash
   cargo run --release --features ws-stream
   ```

### Method 2: Using Config.toml

1. Edit `Config.toml`:
   ```toml
   [streaming]
   mode = "websocket"
   websocket_url = "wss://api.devnet.solana.com"
   commitment = "confirmed"
   ```

2. Run the bot:
   ```bash
   cargo run --release --features ws-stream
   ```

## Testing

### Build with WebSocket support

```bash
# Build with default features (includes ws-stream)
cargo build --release

# Explicitly enable ws-stream feature
cargo build --release --features ws-stream
```

### Run integration tests

```bash
# Run all tests with WebSocket support
cargo test --features ws-stream

# Run specific WebSocket tests
cargo test --features ws-stream websocket
```

### Run the bot with WebSocket streaming

```bash
# Using environment variable
STREAMING_MODE=websocket cargo run --release --features ws-stream

# Or configure in .env file
cargo run --release --features ws-stream
```

## Feature Flags

The bot supports feature flags to enable different streaming backends:

- **`ws-stream`**: WebSocket streaming (default, free tier)
- **`geyser-stream`**: Geyser gRPC streaming (premium, production)

```bash
# Build with WebSocket only
cargo build --features ws-stream

# Build with Geyser only
cargo build --features geyser-stream

# Build with both (runtime selection via config)
cargo build --features ws-stream,geyser-stream
```

## Network Selection

### Devnet (Testing)

```bash
SOLANA_WS_URL=wss://api.devnet.solana.com
```

### Testnet (Pre-production)

```bash
SOLANA_WS_URL=wss://api.testnet.solana.com
```

### Mainnet (Production - Requires Premium)

For mainnet, it's recommended to use premium providers or Geyser gRPC for better reliability and performance:

```bash
# Helius Mainnet
SOLANA_WS_URL=wss://mainnet.helius-rpc.com/?api-key=YOUR_KEY

# Or switch to Geyser
STREAMING_MODE=geyser
GRPC_ENDPOINT=https://your-geyser-endpoint:10000
```

## Commitment Levels

Configure commitment level in `Config.toml`:

```toml
[streaming]
commitment = "confirmed"  # Options: processed, confirmed, finalized
```

- **`processed`**: Fastest, may be rolled back
- **`confirmed`**: Balanced (recommended)
- **`finalized`**: Slowest, permanent

## Troubleshooting

### WebSocket connection fails

1. Check your internet connection
2. Verify the WebSocket URL is correct
3. For free tiers, check if you've exceeded rate limits
4. Try a different provider

### Rate limiting issues

If you encounter rate limiting:

1. Switch to a provider with higher limits (Helius, QuickNode, Alchemy)
2. Sign up for a free API key
3. Reduce the frequency of subscriptions

### Performance issues

For better performance:

1. Use `commitment = "confirmed"` instead of "finalized"
2. Subscribe only to necessary programs/accounts
3. Consider upgrading to Geyser gRPC for production

## Migration to Production

When ready for production:

1. Switch to Geyser gRPC for better reliability:
   ```toml
   [streaming]
   mode = "geyser"
   geyser_endpoint = "https://your-geyser-endpoint:10000"
   ```

2. Build with geyser-stream feature:
   ```bash
   cargo build --release --features geyser-stream
   ```

## Support

For issues or questions:
- Check the repository issues
- Review Solana WebSocket documentation
- Contact your RPC provider support
