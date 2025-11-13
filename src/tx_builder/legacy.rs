//! Legacy API wrappers for backward compatibility
//!
//! This module provides backward-compatible wrappers around the new
//! modular transaction builder API. Legacy functions maintain the same
//! signatures but delegate to the new implementation.
//!
//! ## Compatibility
//! - Maintains existing function signatures
//! - Defaults to enforce_nonce=true for safety
//! - Logs deprecation warnings (once per session)
//! - Zero breaking changes for existing code
//!
//! ## Implementation Status
//! **TODO (Task 6)**: Implement legacy wrapper functions

// Placeholder for legacy API implementation
// This will be implemented in Task 6

// Legacy wrapper functions will be added in Task 6:
// pub async fn build_buy_transaction(...) -> Result<VersionedTransaction, TransactionBuilderError> { ... }
// pub async fn build_sell_transaction(...) -> Result<VersionedTransaction, TransactionBuilderError> { ... }
