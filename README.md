# Bot - Advanced Solana Trading Bot

Bot is an advanced, high-performance trading bot for the Solana blockchain, implementing Universe Class Grade architecture for automated trading across multiple decentralized exchanges.

## Features

- **Real-time Transaction Monitoring**: Geyser gRPC streaming
- **Multi-DEX Support**: PumpFun, Raydium, Orca integration
- **MEV Protection**: Jito bundle support
- **Advanced Nonce Management**: Enterprise-grade nonce pooling
- **Resilient RPC**: Intelligent connection pooling and failover
- **Comprehensive Metrics**: Prometheus integration
- **Distributed Tracing**: OpenTelemetry-compatible observability

## Requirements

### Minimum Supported Rust Version (MSRV)

This project requires **Rust 1.83.0** or later.

See [MSRV.md](MSRV.md) for detailed information about the minimum supported Rust version.

### Installation

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# The project will automatically use Rust 1.83.0 via rust-toolchain.toml
rustup show

# Verify installation
rustc --version  # Should show 1.83.0 or later
```

## Building

### Quick Start

```bash
# Clone the repository
git clone https://github.com/CryptoRomanescu/Universe.git
cd Universe

# Build with default features
cargo build --release

# Build with all features
cargo build --release --all-features
```

### Feature Flags

The project supports several optional features:

- `pumpfun` - PumpFun DEX integration
- `orca` - Orca Whirlpools integration
- `raydium` - Raydium DEX integration (currently disabled)
- `zk_enabled` - Zero-knowledge proof support
- `mock-mode` - Mock trading mode for testing
- `test_utils` - Testing utilities
- `dex-all` - Enable all DEX integrations

See [BUILD_MATRIX.md](BUILD_MATRIX.md) for complete feature documentation and testing matrix.

### Building with Specific Features

```bash
# Build with PumpFun support only
cargo build --release --features pumpfun

# Build with multiple DEXs
cargo build --release --features "pumpfun,orca"

# Build with all DEXs and ZK proofs
cargo build --release --features "dex-all,zk_enabled"
```

## Testing

```bash
# Run all tests
cargo test

# Run tests with all features
cargo test --all-features

# Run tests for specific feature
cargo test --features pumpfun
```

### Build Matrix Testing

To test all feature combinations locally:

```bash
# Run the build matrix test script
./scripts/run_build_matrix.sh
```

This will test all documented feature combinations to ensure proper compilation.

## Development

### Code Quality

The project enforces strict compiler warnings:

- `deny(unused_imports)` - No unused imports allowed
- `deny(unused_mut)` - No unused mutable variables
- `deny(unused_variables)` - No unused variables
- `warn(dead_code)` - Warning for unused code
- `warn(unused_must_use)` - Warning for ignored must-use values

### Linting

```bash
# Run clippy for additional linting
cargo clippy --all-features -- -D warnings

# Check code formatting
cargo fmt -- --check

# Apply formatting
cargo fmt
```

### CI/CD

The project uses GitHub Actions for continuous integration. See `.github/workflows/build-matrix.yml` for the full CI configuration.

The CI pipeline includes:
- MSRV verification
- Build matrix testing across all feature combinations
- Clippy linting
- Code formatting checks

## Configuration

Create a `Config.toml` file based on your requirements. See the example configuration for details.

## Documentation

- [MSRV.md](MSRV.md) - Minimum Supported Rust Version details
- [BUILD_MATRIX.md](BUILD_MATRIX.md) - Build and feature matrix documentation

## Contributing

1. Ensure your code compiles with the MSRV (Rust 1.83.0)
2. Run `cargo fmt` before committing
3. Ensure `cargo clippy --all-features` passes
4. Test your changes with `./scripts/run_build_matrix.sh`
5. All CI checks must pass

## License

See LICENSE file for details.

## Security

For security concerns, please contact the maintainers directly.
