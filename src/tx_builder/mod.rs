//! Universe-grade Transaction Builder Supercomponent
//!
//! This module provides a modular, high-performance transaction building system
//! for Solana with the following key features:
//!
//! ## Architecture
//!
//! The supercomponent is split into focused modules:
//! - **errors**: Comprehensive error taxonomy with observability hooks
//! - **context**: Execution context management with RAII nonce leases
//! - **output**: Transaction output with automatic resource cleanup
//! - **instructions**: Instruction planning and validation
//! - **simulate**: Durable-nonce-aware simulation
//! - **builder**: Core transaction building logic
//! - **legacy**: Backward-compatible wrapper API
//! - **bundle**: Jito MEV bundler integration
//!
//! ## Key Features
//!
//! ### RAII Nonce Management
//! - Deterministic nonce acquisition with `try_acquire()` semantics
//! - Automatic lease release via Drop guards
//! - Zero TOCTTOU vulnerabilities
//!
//! ### Durable Nonce Support
//! - Correct instruction ordering (advance_nonce → compute_budget → dex)
//! - Simulation-aware path that skips nonce consumption
//! - Dual-mode operation: durable vs. recent blockhash
//!
//! ### MEV Protection
//! - Optional Jito bundle integration
//! - Dynamic tip calculation based on network conditions
//! - Multi-region bundle submission
//! - Backrun protection markers
//!
//! ### Performance
//! - Zero blocking in async paths
//! - Minimal allocations in hot paths
//! - Lock-free where possible
//! - Target: <5ms p95 overhead, 1000+ tx/s throughput
//!
//! ### Observability
//! - Rich error context for debugging
//! - Metrics integration (latency histograms, counters)
//! - Distributed tracing support
//! - Correlation IDs for request tracking
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use tx_builder::{TxBuilder, TransactionBuilderError};
//!
//! # async fn example() -> Result<(), TransactionBuilderError> {
//! // Initialize builder with nonce manager and RPC access
//! // let builder = TxBuilder::new(...);
//!
//! // Build a transaction with automatic nonce management
//! // let output = builder.build_buy_transaction_output(
//! //     &candidate,
//! //     &config,
//! //     true,  // sign
//! //     true,  // enforce_nonce
//! // ).await?;
//!
//! // Transaction is ready to broadcast
//! // Nonce lease is held by output and automatically released on drop
//! // let tx = output.tx;
//!
//! # Ok(())
//! # }
//! ```

// Public API - Error types
pub mod errors;
pub use errors::TransactionBuilderError;

// Internal modules (not yet implemented - placeholders for Task 2+)
mod builder;
mod bundle;
mod context;
mod instructions;
mod legacy;
mod output;
mod simulate;

// Re-export key types for convenience
// Task 2: Export ExecutionContext and TxBuildOutput
pub use context::ExecutionContext;
pub use output::TxBuildOutput;

// Task 3: Export instruction planning types and functions
pub use instructions::{plan_buy_instructions, sanity_check_ix_order, InstructionPlan};

// Future exports (will be populated in later tasks)
// pub use builder::TxBuilder;
// pub use simulate::{strip_nonce_for_simulation, build_sim_tx_like};
// pub use bundle::{Bundler, BundleCandidate, JitoBundler};
// pub use legacy::*;

// Type aliases for backward compatibility
// These will be properly implemented in Task 6 (Legacy API)
// pub type Result<T> = std::result::Result<T, TransactionBuilderError>;
