//! Error types for the Transaction Builder supercomponent
//!
//! This module provides a comprehensive, Universe-grade error taxonomy for all
//! transaction building operations. Errors are designed to be:
//! - Informative: Rich context for debugging and monitoring
//! - Efficient: Minimal allocations in hot paths
//! - Composable: Easy to convert from underlying error types
//! - Observable: Integration with metrics and tracing

use thiserror::Error;

/// Comprehensive error type for all transaction builder operations
///
/// This error type covers the entire transaction building lifecycle:
/// - Nonce acquisition and management
/// - Instruction construction and validation
/// - Transaction simulation
/// - Signing operations
/// - Internal consistency checks
#[derive(Error, Debug)]
pub enum TransactionBuilderError {
    /// Failed to acquire a nonce lease from the NonceManager
    ///
    /// This typically indicates:
    /// - All available nonces are currently in use
    /// - The NonceManager is saturated
    /// - A deadlock or resource exhaustion condition
    #[error("Nonce acquisition failed: {0}")]
    NonceAcquisition(String),

    /// Failed to build an instruction for a specific program
    ///
    /// Contains the program ID and detailed reason for failure
    #[error("Instruction build error (program={program}): {reason}")]
    InstructionBuild {
        /// The program ID that failed to build an instruction
        program: String,
        /// Detailed reason for the failure
        reason: String,
    },

    /// Transaction simulation failed
    ///
    /// This can indicate:
    /// - Insufficient compute units
    /// - Invalid account state
    /// - Program execution failure
    /// - Slippage exceeded
    #[error("Simulation failed: {0}")]
    Simulation(String),

    /// Failed to sign the transaction
    ///
    /// This can indicate:
    /// - Wallet/keypair unavailable
    /// - Hardware wallet communication failure
    /// - Invalid transaction structure
    #[error("Signing failed: {0}")]
    Signing(String),

    /// Blockhash-related errors
    ///
    /// This includes:
    /// - Failed to fetch recent blockhash
    /// - Blockhash expired/stale
    /// - Quorum consensus failure
    #[error("Blockhash error: {0}")]
    Blockhash(String),

    /// Invalid instruction order or structure
    ///
    /// Specific to durable nonce transactions which require:
    /// 1. advance_nonce_account (first)
    /// 2. Compute budget instructions
    /// 3. DEX/program instructions
    #[error("Invalid instruction order: {0}")]
    InvalidInstructionOrder(String),

    /// Configuration or validation error
    ///
    /// This includes:
    /// - Invalid TransactionConfig values
    /// - Missing required fields
    /// - Constraint violations
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// RPC client error
    ///
    /// Wraps underlying RPC communication failures
    #[error("RPC error: {0}")]
    Rpc(String),

    /// Bundler-specific errors (Jito MEV)
    ///
    /// This includes:
    /// - Bundle preparation failures
    /// - Multi-region submission errors
    /// - Tip calculation errors
    #[error("Bundler error: {0}")]
    Bundler(String),

    /// Resource exhaustion or capacity limits
    ///
    /// This includes:
    /// - Worker pool saturation
    /// - Rate limiter throttling
    /// - Memory pressure
    #[error("Resource exhaustion: {0}")]
    ResourceExhaustion(String),

    /// Internal invariant violation or unexpected state
    ///
    /// These errors should be rare and typically indicate bugs
    /// or corrupted internal state
    #[error("Internal error: {0}")]
    Internal(String),

    /// Wrapped error from external crates
    ///
    /// Used for errors that don't fit into other categories
    #[error("External error: {0}")]
    External(#[from] anyhow::Error),
}

impl TransactionBuilderError {
    /// Check if this error is potentially retryable
    ///
    /// Returns `true` if retrying the operation might succeed,
    /// `false` if the error is fatal or non-retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            // Transient failures that may succeed on retry
            Self::NonceAcquisition(_) => true,
            Self::Blockhash(_) => true,
            Self::Rpc(_) => true,
            Self::ResourceExhaustion(_) => true,
            Self::Simulation(msg) => {
                // Some simulation failures are retryable (e.g., blockhash issues)
                // but others are not (e.g., insufficient balance)
                !msg.contains("insufficient") && !msg.contains("balance")
            }

            // Non-retryable failures
            Self::InstructionBuild { .. } => false,
            Self::Signing(_) => false,
            Self::InvalidInstructionOrder(_) => false,
            Self::Configuration(_) => false,
            Self::Bundler(_) => false,
            Self::Internal(_) => false,
            Self::External(_) => false,
        }
    }

    /// Get the error category for metrics and observability
    pub fn category(&self) -> &'static str {
        match self {
            Self::NonceAcquisition(_) => "nonce",
            Self::InstructionBuild { .. } => "instruction",
            Self::Simulation(_) => "simulation",
            Self::Signing(_) => "signing",
            Self::Blockhash(_) => "blockhash",
            Self::InvalidInstructionOrder(_) => "validation",
            Self::Configuration(_) => "config",
            Self::Rpc(_) => "rpc",
            Self::Bundler(_) => "bundler",
            Self::ResourceExhaustion(_) => "resource",
            Self::Internal(_) => "internal",
            Self::External(_) => "external",
        }
    }
}

// Convenience constructors for common error scenarios
impl TransactionBuilderError {
    /// Create a nonce acquisition error
    pub fn nonce_unavailable() -> Self {
        Self::NonceAcquisition("All nonces currently in use".to_string())
    }

    /// Create a nonce acquisition timeout error
    pub fn nonce_timeout() -> Self {
        Self::NonceAcquisition("Nonce acquisition timed out".to_string())
    }

    /// Create an instruction build error for a specific program
    pub fn instruction_failed(program: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InstructionBuild {
            program: program.into(),
            reason: reason.into(),
        }
    }

    /// Create a simulation failure error
    pub fn simulation_failed(reason: impl Into<String>) -> Self {
        Self::Simulation(reason.into())
    }

    /// Create a blockhash error
    pub fn blockhash_unavailable(reason: impl Into<String>) -> Self {
        Self::Blockhash(reason.into())
    }

    /// Create an invalid instruction order error
    pub fn invalid_order(reason: impl Into<String>) -> Self {
        Self::InvalidInstructionOrder(reason.into())
    }

    /// Create an internal error
    pub fn internal(reason: impl Into<String>) -> Self {
        Self::Internal(reason.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = TransactionBuilderError::NonceAcquisition("test".to_string());
        assert_eq!(err.to_string(), "Nonce acquisition failed: test");

        let err = TransactionBuilderError::InstructionBuild {
            program: "pump_program".to_string(),
            reason: "invalid accounts".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Instruction build error (program=pump_program): invalid accounts"
        );
    }

    #[test]
    fn test_error_retryability() {
        assert!(TransactionBuilderError::NonceAcquisition("test".to_string()).is_retryable());
        assert!(TransactionBuilderError::Blockhash("test".to_string()).is_retryable());
        assert!(TransactionBuilderError::Rpc("test".to_string()).is_retryable());

        assert!(!TransactionBuilderError::Signing("test".to_string()).is_retryable());
        assert!(!TransactionBuilderError::Configuration("test".to_string()).is_retryable());
        assert!(!TransactionBuilderError::Internal("test".to_string()).is_retryable());
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(
            TransactionBuilderError::NonceAcquisition("test".to_string()).category(),
            "nonce"
        );
        assert_eq!(
            TransactionBuilderError::Simulation("test".to_string()).category(),
            "simulation"
        );
        assert_eq!(
            TransactionBuilderError::Internal("test".to_string()).category(),
            "internal"
        );
    }

    #[test]
    fn test_convenience_constructors() {
        let err = TransactionBuilderError::nonce_unavailable();
        assert!(matches!(err, TransactionBuilderError::NonceAcquisition(_)));

        let err = TransactionBuilderError::instruction_failed("program", "reason");
        assert!(matches!(err, TransactionBuilderError::InstructionBuild { .. }));

        let err = TransactionBuilderError::simulation_failed("test");
        assert!(matches!(err, TransactionBuilderError::Simulation(_)));
    }
}
