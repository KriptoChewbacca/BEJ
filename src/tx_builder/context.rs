//! Execution context for transaction building
//!
//! This module provides ExecutionContext which manages the blockhash or
//! durable nonce state needed to build a transaction, with RAII semantics
//! for nonce lease management.
//!
//! ## Key Features
//! - Dual-mode operation: durable nonce vs. recent blockhash
//! - RAII nonce lease management via `try_acquire()`
//! - Zero TOCTTOU vulnerabilities
//! - Efficient lease transfer to TxBuildOutput
//! - Task 5: Integrated TraceContext for distributed tracing
//!
//! ## Implementation Status
//! **COMPLETED (Task 2)**: ExecutionContext with RAII nonce management
//! **ENHANCED (Task 5)**: Added TraceContext for observability

use solana_sdk::{hash::Hash, pubkey::Pubkey};

// Import NonceLease from nonce_manager module
// The nonce_manager module is defined in main.rs with #[path = "nonce manager/mod.rs"]
use crate::nonce_manager::NonceLease;

// Task 5: Import TraceContext for observability
use crate::observability::TraceContext;

// Conditional ZK proof type
#[cfg(feature = "zk_enabled")]
use crate::nonce_manager::ZkProofData;

/// Execution context for building a transaction
///
/// This struct manages the blockhash or durable nonce state required
/// for transaction construction. It implements RAII semantics for nonce
/// lease management.
///
/// # RAII Contract
///
/// - **Owned Data**: All fields are owned ('static), no references held
/// - **Automatic Cleanup**: Nonce lease is released on drop if not extracted
/// - **Explicit Transfer**: `extract_lease()` transfers ownership to caller
/// - **No Async in Drop**: Drop is handled by NonceLease, not this struct
///
/// # Lifecycle
///
/// 1. Created with either:
///    - A recent blockhash (non-durable mode)
///    - A nonce lease and associated data (durable mode)
/// 2. Used to build transaction instructions
/// 3. Lease extracted via `extract_lease()` for transfer to TxBuildOutput
/// 4. If not extracted, lease is automatically released on drop
///
/// # Task 5 Enhancement
///
/// Added TraceContext for distributed tracing and correlation across
/// transaction building operations.
///
/// # Example
///
/// ```no_run
/// // Durable nonce mode with trace context
/// let trace_ctx = TraceContext::new("build_buy_transaction");
/// let context = ExecutionContext {
///     blockhash: nonce_blockhash,
///     nonce_pubkey: Some(nonce_pubkey),
///     nonce_authority: Some(authority),
///     nonce_lease: Some(lease),
///     trace_context: Some(trace_ctx),
/// };
///
/// // Extract lease for transfer to output
/// let lease = context.extract_lease();
/// ```
///
/// # Debug Implementation
///
/// The custom Debug implementation excludes the full nonce_lease content to:
/// - Prevent log bloat from large lease structures
/// - Avoid exposing internal lease implementation details
/// - Provide concise debugging information (lease status only)
pub struct ExecutionContext {
    /// The blockhash to use for the transaction
    pub blockhash: Hash,

    /// Optional nonce account public key (if using durable transactions)
    pub nonce_pubkey: Option<Pubkey>,

    /// Optional nonce authority (if using durable transactions)
    pub nonce_authority: Option<Pubkey>,

    /// Optional nonce lease (held for transaction lifetime, auto-released on drop)
    ///
    /// This field enforces RAII semantics: the lease is owned by this context
    /// and will be automatically released on drop. Use `extract_lease()` to
    /// transfer ownership before drop.
    pub nonce_lease: Option<NonceLease>,

    /// Optional ZK proof for nonce state validation (upgraded to ZkProofData with Groth16)
    /// Only available when zk_enabled feature is active
    #[cfg(feature = "zk_enabled")]
    pub zk_proof: Option<ZkProofData>,

    /// Task 5: Optional trace context for distributed tracing
    pub trace_context: Option<TraceContext>,
}

impl std::fmt::Debug for ExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_struct("ExecutionContext");
        debug_struct
            .field("blockhash", &self.blockhash)
            .field("nonce_pubkey", &self.nonce_pubkey)
            .field("nonce_authority", &self.nonce_authority)
            .field(
                "nonce_lease_status",
                &match &self.nonce_lease {
                    Some(lease) => format!(
                        "Some(nonce={}, expired={})",
                        lease.nonce_pubkey(),
                        lease.is_expired()
                    ),
                    None => "None".to_string(),
                },
            );

        #[cfg(feature = "zk_enabled")]
        debug_struct.field(
            "zk_proof_status",
            &self
                .zk_proof
                .as_ref()
                .map(|p| format!("confidence={}", p.confidence)),
        );

        // Task 5: Include trace context in debug output
        debug_struct.field(
            "trace_context",
            &self
                .trace_context
                .as_ref()
                .map(|ctx| format!("trace_id={}, span_id={}", ctx.trace_id(), ctx.span_id())),
        );

        debug_struct.finish()
    }
}

impl ExecutionContext {
    /// Extract the nonce lease, consuming it (Task 2: RAII support)
    ///
    /// This method allows transferring ownership of the nonce lease from
    /// ExecutionContext to TxBuildOutput, ensuring proper RAII semantics.
    ///
    /// # RAII Contract
    ///
    /// This method consumes `self`, transferring ownership of the lease to the caller.
    /// If the lease is not extracted, it will be automatically released when
    /// ExecutionContext is dropped.
    ///
    /// # Returns
    ///
    /// The nonce lease if one was held, or None if operating in non-durable mode.
    ///
    /// # Example
    ///
    /// ```no_run
    /// let context = prepare_execution_context().await?;
    /// let lease = context.extract_lease();
    /// let output = TxBuildOutput::new(tx, lease);
    /// ```
    pub fn extract_lease(mut self) -> Option<NonceLease> {
        self.nonce_lease.take()
    }

    /// Check if this context is using durable nonce
    ///
    /// Returns `true` if a nonce lease is held, `false` for recent blockhash mode.
    ///
    /// # Example
    ///
    /// ```no_run
    /// if context.is_durable() {
    ///     println!("Using durable nonce: {}", context.nonce_pubkey.unwrap());
    /// } else {
    ///     println!("Using recent blockhash");
    /// }
    /// ```
    pub fn is_durable(&self) -> bool {
        self.nonce_lease.is_some()
    }
}
