# Build and Feature Matrix

This document describes the build matrix for the Ultra trading bot, including all feature combinations that should be tested.

**Status**: ✅ Fully implemented with CI automation and local tooling

## Feature Flags

The project supports the following feature flags:

### Core Features

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `default` | Base functionality (empty by default) | None |
| `mock-mode` | Enable mock trading mode for testing | None |
| `test_utils` | Testing utilities and helpers | None |

### DEX Integration Features

| Feature | Description | Dependencies | Status |
|---------|-------------|--------------|--------|
| `pumpfun` | PumpFun DEX integration | `pumpfun` crate | Active |
| `orca` | Orca Whirlpools integration | `orca_whirlpools` crate | Active |
| `raydium` | Raydium DEX integration | (temporarily disabled) | Disabled |
| `dex-all` | Meta-feature enabling all active DEXs | `pumpfun`, `orca` | Active |

### Advanced Features

| Feature | Description | Dependencies | Status |
|---------|-------------|--------------|--------|
| `zk_enabled` | Zero-knowledge proof support | `solana-zk-sdk` | Active |

## Build Matrix

### Recommended Test Combinations

The following combinations should be tested in CI:

```bash
# 1. Base build (no features)
cargo check

# 2. Individual DEX features
cargo check --features pumpfun
cargo check --features orca
cargo check --features raydium

# 3. All DEXs together
cargo check --features dex-all

# 4. With ZK proofs
cargo check --features zk_enabled

# 5. Mock mode
cargo check --features mock-mode

# 6. Testing utilities
cargo check --features test_utils

# 7. Common production combinations
cargo check --features "pumpfun,orca"
cargo check --features "pumpfun,orca,zk_enabled"
cargo check --features "dex-all,zk_enabled"

# 8. All features (comprehensive test)
cargo check --all-features
```

### Full Matrix (for reference)

The complete feature matrix includes 2^N combinations where N is the number of independent features. For this project:

- 3 DEX features (pumpfun, orca, raydium)
- 3 utility features (mock-mode, test_utils, zk_enabled)
- 1 meta-feature (dex-all)

Total independent features: 6
Theoretical combinations: 64

**Practical testing strategy**: Test the 11 combinations listed above, which cover:
- No features (baseline)
- Each feature individually
- Common production combinations
- All features together

## CI Integration

### GitHub Actions Workflow

The project uses a comprehensive GitHub Actions workflow (`.github/workflows/build-matrix.yml`) that includes:

1. **MSRV Check** - Verifies toolchain configuration
2. **Check Matrix** - Tests all 11 feature combinations with `cargo check`
3. **Test Matrix** - Runs tests with key feature combinations
4. **Clippy Check** - Linting with all features enabled
5. **Format Check** - Code formatting verification

**Important**: All jobs use `dtolnay/rust-toolchain@master` with explicit `toolchain: nightly` parameter.

The workflow tests these combinations:
- No features (baseline)
- Individual features: `pumpfun`, `orca`, `raydium`, `zk_enabled`, `mock-mode`, `test_utils`
- Meta-feature: `dex-all`
- Production combinations: `pumpfun,orca,zk_enabled`, `dex-all,zk_enabled`
- All features: `--all-features`

All checks run in parallel with independent caching for optimal CI performance.

## Running the Matrix Locally

### Quick Checks with Cargo Aliases

The project includes a `.cargo/config.toml` with convenient aliases:

```bash
# Individual feature checks
cargo check-default         # No features
cargo check-pumpfun        # PumpFun DEX
cargo check-orca           # Orca Whirlpools
cargo check-raydium        # Raydium
cargo check-zk             # ZK proofs
cargo check-mock           # Mock mode
cargo check-test-utils     # Test utilities
cargo check-dex-all        # All DEXs

# Production combinations
cargo check-prod           # pumpfun,orca,zk_enabled
cargo check-dex-all-zk     # dex-all,zk_enabled

# Comprehensive
cargo check-all            # All features
cargo test-all             # Test all features

# Full matrix
cargo test-matrix          # Run scripts/run_build_matrix.sh
```

### Full Matrix Script

Run the complete build matrix using the provided script:

```bash
./scripts/run_build_matrix.sh
```

This script:
- Verifies toolchain matches `rust-toolchain.toml`
- Tests all 11 feature combinations
- Provides colored output with pass/fail status
- Exits with error code on any failure
- Shows summary with total passed/failed

## Feature Flag Isolation

The codebase uses precise `#[cfg(feature = "...")]` guards to ensure feature-specific code doesn't leak between configurations:

### DEX Features

All DEX-specific imports and implementations are guarded:
- **pumpfun**: `use pumpfun::*` imports, `build_pumpfun_instruction()`, helper functions
- **orca**: `use orca_whirlpools::*` imports, `build_orca_instruction()`
- **raydium**: `use raydium_sdk_v2::*` imports, `build_raydium_instruction()`

Methods return `FeatureNotEnabled` error when called without the required feature.

### ZK Proof Feature

The `zk_enabled` feature controls:
- **Export**: `ZkProofData` type only exported from `nonce_manager` module when feature is active
- **Field**: `ExecutionContext.zk_proof` field only exists when feature is active
- **Imports**: `use solana_zk_sdk` statements are feature-guarded
- **Implementations**: ZK proof generation and verification code is conditionally compiled

### Test Utilities

Test utilities are isolated with `#[cfg(any(test, feature = "test_utils"))]` to ensure they're only available:
- In test builds (`cargo test`)
- When explicitly enabled (`cargo build --features test_utils`)

This prevents accidental usage of test code in production builds.

## Notes

- The `raydium` feature is currently disabled due to version conflicts but the flag is kept for future re-enabling
- The `dex-all` meta-feature currently includes only `pumpfun` and `orca`
- Some feature combinations may be redundant (e.g., enabling `dex-all` already enables `pumpfun` and `orca`)
- Testing without features is important to ensure the base system compiles standalone
- **Toolchain**: Project requires `nightly` toolchain (see `rust-toolchain.toml` and `MSRV.md` for details)

## Maintenance

When adding new features:
1. Document them in this file and in the feature table above
2. Add them to the CI matrix in `.github/workflows/build-matrix.yml`
3. Add convenient cargo alias in `.cargo/config.toml`
4. Test locally with `./scripts/run_build_matrix.sh` or individual aliases
5. Update the test script if needed
6. Add precise `#[cfg(feature = "...")]` guards to imports and feature-specific code
7. Ensure feature-specific types are conditionally exported from modules
