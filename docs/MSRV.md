# Minimum Supported Rust Version (MSRV)

## Current Toolchain: Nightly (Temporary)

The Ultra trading bot currently requires **Rust nightly** due to transitive dependency requirements.

### ⚠️ Important: We DO NOT use nightly features

Our codebase uses **only stable Rust features** (edition 2021). The nightly requirement is **temporary** and caused by transitive dependencies, not our code.

## Why Nightly is Required (Temporary)

As of December 2024, several transitive dependencies in the Solana/cryptography ecosystem require **edition2024**, which is only available in nightly Rust:

- `base64ct` 1.8.0+: requires edition2024 (pulled by `ed25519-dalek`, `sha2`, and other crypto crates)
- `image` 0.25.8+: requires Rust 1.85+ with edition2024 support (pulled by `eframe`)
- `smithay-clipboard` 0.7.3+: requires Rust 1.85+ (pulled by `eframe`)

These are **indirect dependencies** - we don't directly depend on them, but they're pulled in by:
- Solana SDK (cryptography stack)
- eframe (GUI framework, even though we don't use GUI features)

### Our Code is Stable-Compatible

✅ **edition = "2021"** (NOT edition2024)  
✅ **No nightly features used** (no #![feature(...)])  
✅ **All code works on stable Rust 1.83.0+**  
✅ **Only dependencies require nightly**

## Migration Path to Stable

We will return to stable Rust when:

1. **Rust 1.85+ is released** (expected Q1 2025) and stabilizes edition2024, OR
2. **Dependencies are updated** to work without edition2024, OR
3. **We pin transitive dependencies** to older versions (may have security implications)

**Target stable version**: Rust 1.85.0 (when available)

## Historical MSRV (When Stable Returns)

When we can return to stable, the MSRV will be determined by:

- `solana-net-utils` (2.3.13): requires Rust 1.83.0
- `solana-program` (2.3.0): requires Rust 1.79.0
- `serde_with` (3.15.1): requires Rust 1.76
- `tokio` (1.48.0): requires Rust 1.71

## Installation

### Using rustup

```bash
# Install or update to the required version
rustup install 1.83.0
rustup default 1.83.0

# Or use the toolchain file (recommended)
# The rust-toolchain.toml file will automatically use 1.83.0
rustup show
```

### Verification

```bash
# Check your Rust version
rustc --version
# Should output: rustc 1.83.0 (...)

# Verify the project uses the correct version
cargo --version
```

## CI/CD Integration

The MSRV is enforced in continuous integration through:
1. The `rust-toolchain.toml` file (automatic toolchain selection)
2. The `rust-version` field in `Cargo.toml` (metadata and validation)

## Updating MSRV

When updating the MSRV:
1. Update the `rust-version` field in `Cargo.toml`
2. Update the `channel` field in `rust-toolchain.toml`
3. Update this documentation
4. Test all feature combinations with the new version
5. Update CI workflows if necessary

## Support Policy

- We support the current MSRV for at least 6 months after declaration
- MSRV updates are considered breaking changes and require a major or minor version bump
- Security-critical updates may require MSRV bumps with shorter notice
