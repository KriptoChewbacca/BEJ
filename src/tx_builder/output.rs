//! Transaction build output with RAII nonce management
//!
//! This module provides TxBuildOutput, which holds the built transaction
//! along with an optional NonceLease. The lease is automatically released
//! when the output is dropped, ensuring no nonce leaks.
//!
//! ## Key Features
//! - Holds VersionedTransaction ready for broadcast
//! - Optional NonceLease with Drop guard semantics
//! - List of required signers for validation
//! - Explicit async release method for controlled cleanup
//!
//! ## Implementation Status
//! **COMPLETED (Task 2)**: TxBuildOutput with RAII nonce management

use crate::nonce_manager::NonceLease;
use crate::tx_builder::errors::TransactionBuilderError;
use solana_sdk::{pubkey::Pubkey, transaction::VersionedTransaction};
use tracing::warn;

/// Transaction build output with RAII nonce management
///
/// This struct holds a fully built transaction along with an optional
/// nonce lease. The lease is automatically released when the output is
/// dropped, ensuring no resource leaks.
///
/// # RAII Contract
///
/// This struct enforces the following guarantees:
///
/// 1. **Owned Data**: All fields are owned, no references held
/// 2. **Automatic Cleanup**: NonceLease Drop is triggered when TxBuildOutput is dropped
/// 3. **Explicit Release**: `release_nonce()` method for explicit cleanup
/// 4. **Idempotent**: Multiple release attempts are safe (handled by NonceLease)
/// 5. **No Async in Drop**: Drop only logs warnings, actual release is in NonceLease
/// 6. **Zero Leaks**: Nonce is guaranteed to be released (explicitly or on drop)
///
/// # Lifecycle
///
/// 1. Created with a transaction and optional nonce lease
/// 2. Held during transaction signing and broadcast
/// 3. Either:
///    - Explicitly released via `release_nonce()` after successful broadcast
///    - Automatically released on drop (with warning)
///
/// # Example
///
/// ```no_run
/// // Create output with nonce guard
/// let output = TxBuildOutput::new(tx, Some(nonce_lease));
///
/// // Sign and broadcast transaction
/// let sig = rpc.send_transaction(&output.tx).await?;
///
/// // Explicitly release after successful broadcast
/// output.release_nonce().await?;
/// ```
///
/// # Error Handling
///
/// ```no_run
/// async fn broadcast_with_cleanup(output: TxBuildOutput) -> Result<Signature, Error> {
///     match rpc.send_transaction(&output.tx).await {
///         Ok(sig) => {
///             // Success - explicitly release
///             output.release_nonce().await?;
///             Ok(sig)
///         }
///         Err(e) => {
///             // Failure - drop output (auto-release via RAII)
///             drop(output);
///             Err(e)
///         }
///     }
/// }
/// ```
pub struct TxBuildOutput {
    /// The built transaction ready for signing/broadcast
    pub tx: VersionedTransaction,

    /// Optional nonce lease guard (held until broadcast completes)
    /// Automatically released on drop via RAII pattern
    ///
    /// This field is owned data, not a reference. The lease will be automatically
    /// released when this struct is dropped, preventing resource leaks.
    pub nonce_guard: Option<NonceLease>,

    /// List of required signers for this transaction
    /// Extracted from message.header.num_required_signatures
    pub required_signers: Vec<Pubkey>,
}

impl TxBuildOutput {
    /// Create new TxBuildOutput with nonce guard
    ///
    /// Automatically extracts required signers from the transaction header
    /// based on num_required_signatures field using the compat layer.
    ///
    /// # Arguments
    ///
    /// * `tx` - The built transaction ready for broadcast
    /// * `nonce_guard` - Optional nonce lease to hold until broadcast completes
    ///
    /// # Returns
    ///
    /// A new TxBuildOutput with the transaction, nonce guard, and extracted signers
    ///
    /// # Example
    ///
    /// ```no_run
    /// let output = TxBuildOutput::new(tx, Some(nonce_lease));
    /// assert_eq!(output.required_signers.len(), 1);
    /// ```
    pub fn new(
        tx: VersionedTransaction,
        nonce_guard: Option<NonceLease>,
    ) -> Self {
        // Extract required signers using compat layer for unified API
        let required_signers = crate::compat::get_required_signers(&tx.message).to_vec();

        Self {
            tx,
            nonce_guard,
            required_signers,
        }
    }

    /// Get reference to the transaction
    ///
    /// # Returns
    ///
    /// A reference to the underlying VersionedTransaction
    pub fn tx_ref(&self) -> &VersionedTransaction {
        &self.tx
    }

    /// Consume self and extract the transaction
    ///
    /// This method extracts the transaction and drops the nonce guard.
    /// Used by legacy wrappers for backward compatibility.
    ///
    /// # Warning
    ///
    /// This releases the nonce guard early. Prefer holding the TxBuildOutput
    /// until after broadcast for proper RAII semantics.
    ///
    /// # Returns
    ///
    /// The owned VersionedTransaction (nonce guard is dropped)
    pub fn into_tx(mut self) -> VersionedTransaction {
        use std::mem;
        // Take ownership by replacing with a default value
        // The nonce_guard will be dropped when self is dropped
        mem::take(&mut self.tx)
    }

    /// Get slice of required signers
    ///
    /// # Returns
    ///
    /// A slice containing the public keys of all required signers
    pub fn required_signers(&self) -> &[Pubkey] {
        &self.required_signers
    }

    /// Explicitly release nonce guard (if held)
    ///
    /// This method should be called after successful transaction broadcast.
    /// Returns an error if the nonce release fails.
    ///
    /// # RAII Contract
    ///
    /// This method enforces RAII by:
    /// - Consuming `self` to prevent use-after-release
    /// - Idempotent: safe to call even if no nonce guard is held
    /// - Explicit cleanup: allows handling release errors
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the nonce was released successfully or no nonce was held
    /// - `Err(TransactionBuilderError)` if the release operation failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// let output = builder.build_buy_transaction_output(...).await?;
    /// let sig = rpc.send_transaction(output.tx.clone()).await?;
    ///
    /// // Explicitly release after successful broadcast
    /// output.release_nonce().await?;
    /// ```
    pub async fn release_nonce(mut self) -> Result<(), TransactionBuilderError> {
        if let Some(guard) = self.nonce_guard.take() {
            guard.release().await
                .map_err(|e| TransactionBuilderError::Internal(
                    format!("Failed to release nonce: {}", e)
                ))?;
        }
        Ok(())
    }
}

impl Drop for TxBuildOutput {
    /// RAII cleanup: Warn if nonce guard is being dropped without explicit release
    ///
    /// This implementation:
    /// - Does NOT perform async operations (RAII contract requirement)
    /// - Only logs a warning for diagnostic purposes
    /// - Relies on NonceLease's Drop for actual cleanup
    /// - Prevents resource leaks through automatic cleanup chain
    ///
    /// # Why Only Warn?
    ///
    /// Best practice is to explicitly call `release_nonce()` after successful
    /// broadcast. If Drop is triggered, it likely means:
    /// - An error occurred before broadcast
    /// - The developer forgot to call release_nonce()
    /// - The output was intentionally dropped early
    ///
    /// In all cases, the NonceLease Drop implementation will handle the
    /// actual cleanup, but we log a warning for diagnostic purposes.
    fn drop(&mut self) {
        if let Some(ref guard) = self.nonce_guard {
            warn!(
                nonce = %guard.nonce_pubkey(),
                drop_source = "TxBuildOutput",
                "TxBuildOutput dropped with active nonce guard - lease will be auto-released via NonceLease Drop"
            );
        }
    }
}
