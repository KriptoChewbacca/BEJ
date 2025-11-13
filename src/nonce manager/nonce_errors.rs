use solana_client::client_error::ClientError;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::SignerError;
use thiserror::Error;

/// Universe-level error classification for enhanced error handling
#[derive(Debug, Clone, PartialEq)]
pub enum UniverseErrorType {
    /// Base nonce error wrapped
    Base(Box<NonceError>),
    /// Validator is behind by specified slots
    ValidatorBehind { slots: i64 },
    /// Consensus failure detected
    ConsensusFailure,
    /// Geyser stream error
    GeyserStreamError,
    /// Shredstream timeout
    ShredstreamTimeout,
    /// Circuit breaker is open
    CircuitBreakerOpen,
    /// Predictive failure with probability
    PredictiveFailure { probability: f64 },
    /// Security violation detected
    SecurityViolation { reason: String },
    /// Quota exceeded
    QuotaExceeded,
    /// Cluster congestion detected
    ClusterCongestion { tps: u32 },
    /// Clustered anomaly detected by ML
    ClusteredAnomaly { cluster_id: u8, confidence: f64 },
}

impl std::fmt::Display for UniverseErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UniverseErrorType::Base(e) => write!(f, "Base error: {}", e),
            UniverseErrorType::ValidatorBehind { slots } => {
                write!(f, "Validator behind by {} slots", slots)
            }
            UniverseErrorType::ConsensusFailure => write!(f, "Consensus failure"),
            UniverseErrorType::GeyserStreamError => write!(f, "Geyser stream error"),
            UniverseErrorType::ShredstreamTimeout => write!(f, "Shredstream timeout"),
            UniverseErrorType::CircuitBreakerOpen => write!(f, "Circuit breaker open"),
            UniverseErrorType::PredictiveFailure { probability } => {
                write!(f, "Predictive failure (p={:.2})", probability)
            }
            UniverseErrorType::SecurityViolation { reason } => {
                write!(f, "Security violation: {}", reason)
            }
            UniverseErrorType::QuotaExceeded => write!(f, "Quota exceeded"),
            UniverseErrorType::ClusterCongestion { tps } => {
                write!(f, "Cluster congestion (tps={})", tps)
            }
            UniverseErrorType::ClusteredAnomaly {
                cluster_id,
                confidence,
            } => write!(
                f,
                "Clustered anomaly (cluster={}, conf={:.2})",
                cluster_id, confidence
            ),
        }
    }
}

/// Error classification result with confidence score
#[derive(Debug, Clone)]
pub struct ErrorClassification {
    pub error_type: UniverseErrorType,
    pub confidence: f64,
    pub is_transient: bool,
    pub should_taint: bool,
}

/// Nonce Manager specific errors
#[derive(Debug, Clone, Error, PartialEq)]
pub enum NonceError {
    /// RPC operation failed
    #[error("RPC error: {message} (endpoint: {endpoint:?})")]
    Rpc {
        endpoint: Option<String>,
        message: String,
    },

    /// Nonce account not found or invalid
    #[error("Nonce account error: {0}")]
    InvalidNonceAccount(String),

    /// Nonce is expired
    #[error("Nonce expired: account {account}, last_valid_slot {last_valid_slot}, current_slot {current_slot}")]
    NonceExpired {
        account: Pubkey,
        last_valid_slot: u64,
        current_slot: u64,
    },

    /// Nonce pool exhausted
    #[error("Nonce pool exhausted: {0} nonces requested, {1} available")]
    PoolExhausted(usize, usize),

    /// Nonce is locked by another operation
    #[error("Nonce is locked: {0}")]
    NonceLocked(Pubkey),

    /// Nonce is tainted and should not be used
    #[error("Nonce is tainted: {0}")]
    NonceTainted(Pubkey),

    /// Signing error
    #[error("Signing error: {0}")]
    Signing(String),

    /// Timeout waiting for operation
    #[error("Timeout after {0}ms")]
    Timeout(u64),

    /// Invalid configuration
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Lease expired
    #[error("Lease expired for nonce {0}")]
    LeaseExpired(Pubkey),

    /// No lease available (availability issue - pool exhausted or all leases in use)
    #[error("No lease available: all nonces are currently in use or pool is empty")]
    NoLeaseAvailable,

    /// Failed to acquire lease (operational issue)
    #[error("Failed to acquire lease: {0}")]
    LeaseAcquireFailed(String),

    /// Failed to release lease (operational issue)
    #[error("Failed to release lease: {0}")]
    LeaseReleaseFailed(String),

    /// Failed to advance nonce
    #[error("Failed to advance nonce {0}: {1}")]
    AdvanceFailed(Pubkey, String),

    /// Transaction confirmation failed
    #[error("Transaction confirmation failed: {0}")]
    ConfirmationFailed(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Solana client error
    #[error("Solana client error: {0}")]
    Client(String),
}

impl NonceError {
    /// Check if this error is transient and retryable
    pub fn is_transient(&self) -> bool {
        match self {
            // Transient errors that should be retried
            NonceError::Rpc { .. } => true,
            NonceError::Timeout(_) => true,
            NonceError::AdvanceFailed(..) => true,
            NonceError::ConfirmationFailed(_) => true,
            NonceError::NoLeaseAvailable => true, // Can retry after some leases are released
            NonceError::LeaseAcquireFailed(_) => true,
            NonceError::Client(_) => true,

            // Permanent errors that should not be retried
            NonceError::InvalidNonceAccount(_) => false,
            NonceError::NonceExpired { .. } => false,
            NonceError::PoolExhausted(..) => false,
            NonceError::NonceLocked(_) => false,
            NonceError::NonceTainted(_) => false,
            NonceError::Signing(_) => false,
            NonceError::Configuration(_) => false,
            NonceError::LeaseExpired(_) => false,
            NonceError::LeaseReleaseFailed(_) => false,
            NonceError::Internal(_) => false,
        }
    }

    /// Convert from ClientError
    pub fn from_client_error(err: ClientError, endpoint: Option<String>) -> Self {
        let message = err.to_string();
        NonceError::Rpc { endpoint, message }
    }

    /// Convert from SignerError
    pub fn from_signer_error(err: SignerError) -> Self {
        NonceError::Signing(err.to_string())
    }
}

// Automatic conversion from ClientError
impl From<ClientError> for NonceError {
    fn from(err: ClientError) -> Self {
        NonceError::Client(err.to_string())
    }
}

// Automatic conversion from SignerError
impl From<SignerError> for NonceError {
    fn from(err: SignerError) -> Self {
        NonceError::Signing(err.to_string())
    }
}

/// Result type for nonce operations
pub type NonceResult<T> = Result<T, NonceError>;

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn test_nonce_error_display() {
        let err = NonceError::NoLeaseAvailable;
        assert_eq!(
            err.to_string(),
            "No lease available: all nonces are currently in use or pool is empty"
        );

        let err = NonceError::LeaseAcquireFailed("timeout".to_string());
        assert_eq!(err.to_string(), "Failed to acquire lease: timeout");

        let err = NonceError::LeaseReleaseFailed("nonce locked".to_string());
        assert_eq!(err.to_string(), "Failed to release lease: nonce locked");
    }

    #[test]
    fn test_nonce_error_transient_classification() {
        // Transient errors
        assert!(NonceError::NoLeaseAvailable.is_transient());
        assert!(NonceError::LeaseAcquireFailed("test".to_string()).is_transient());
        assert!(NonceError::Rpc {
            endpoint: Some("test".to_string()),
            message: "error".to_string()
        }
        .is_transient());
        assert!(NonceError::Timeout(1000).is_transient());
        assert!(NonceError::Client("test".to_string()).is_transient());

        // Permanent errors
        assert!(!NonceError::LeaseReleaseFailed("test".to_string()).is_transient());
        assert!(!NonceError::InvalidNonceAccount("test".to_string()).is_transient());
        assert!(!NonceError::PoolExhausted(5, 0).is_transient());
        assert!(!NonceError::Configuration("test".to_string()).is_transient());
    }

    #[test]
    fn test_client_error_conversion() {
        use solana_client::client_error::{ClientError, ClientErrorKind};
        use solana_client::rpc_request::RpcError;

        let rpc_err = RpcError::RpcResponseError {
            code: 500,
            message: "Internal server error".to_string(),
            data: solana_client::rpc_request::RpcResponseErrorData::Empty,
        };
        let client_err = ClientError::from(ClientErrorKind::RpcError(rpc_err));

        let nonce_err: NonceError = client_err.into();
        match nonce_err {
            NonceError::Client(msg) => {
                assert!(msg.contains("Internal server error") || msg.contains("RpcError"));
            }
            _ => panic!("Expected Client error variant"),
        }
    }

    #[test]
    fn test_signer_error_conversion() {
        let signer_err = SignerError::InvalidInput("test input".to_string());
        let nonce_err: NonceError = signer_err.into();

        match nonce_err {
            NonceError::Signing(msg) => {
                assert!(msg.contains("test input"));
            }
            _ => panic!("Expected Signing error variant"),
        }
    }

    #[test]
    fn test_lease_error_variants_distinct() {
        // Verify that the three lease error variants are distinct
        let no_lease = NonceError::NoLeaseAvailable;
        let acquire_failed = NonceError::LeaseAcquireFailed("reason".to_string());
        let release_failed = NonceError::LeaseReleaseFailed("reason".to_string());

        // Check they're different
        assert!(matches!(no_lease, NonceError::NoLeaseAvailable));
        assert!(matches!(acquire_failed, NonceError::LeaseAcquireFailed(_)));
        assert!(matches!(release_failed, NonceError::LeaseReleaseFailed(_)));

        // Check transient classification is correct
        assert!(no_lease.is_transient()); // Can retry later
        assert!(acquire_failed.is_transient()); // Can retry
        assert!(!release_failed.is_transient()); // Permanent issue
    }

    #[test]
    fn test_nonce_expired_error() {
        let pubkey = Pubkey::new_unique();
        let err = NonceError::NonceExpired {
            account: pubkey,
            last_valid_slot: 100,
            current_slot: 200,
        };

        let msg = err.to_string();
        assert!(msg.contains("Nonce expired"));
        assert!(msg.contains("100"));
        assert!(msg.contains("200"));
        assert!(!err.is_transient());
    }
}
