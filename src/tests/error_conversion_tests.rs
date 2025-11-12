#![allow(unused_imports)]
//! Test error type conversions and consolidation
//!
//! This test file verifies that the error handling consolidation is working correctly.

#[cfg(test)]
mod tests {
    use crate::nonce_manager::NonceError;
    use crate::rpc_manager::RpcManagerError;
    use crate::tx_builder::TransactionBuilderError;
    use solana_sdk::pubkey::Pubkey;
    use solana_client::client_error::{ClientError, ClientErrorKind};
    use solana_client::rpc_request::RpcError;
    use solana_sdk::signature::SignerError;

    #[test]
    fn test_nonce_error_to_transaction_builder_error() {
        // Test automatic conversion using #[from]
        let nonce_err = NonceError::NoLeaseAvailable;
        let tx_err: TransactionBuilderError = nonce_err.into();
        
        match tx_err {
            TransactionBuilderError::Nonce(err) => {
                assert!(matches!(err, NonceError::NoLeaseAvailable));
            }
            _ => panic!("Expected Nonce variant"),
        }
    }

    #[test]
    fn test_rpc_manager_error_to_transaction_builder_error() {
        // Test automatic conversion using #[from]
        let rpc_err = RpcManagerError::Timeout {
            endpoint: "https://test.com".to_string(),
            timeout_ms: 5000,
        };
        let tx_err: TransactionBuilderError = rpc_err.into();
        
        match tx_err {
            TransactionBuilderError::RpcManager(err) => {
                assert!(matches!(err, RpcManagerError::Timeout { .. }));
            }
            _ => panic!("Expected RpcManager variant"),
        }
    }

    #[test]
    fn test_client_error_to_nonce_error() {
        // Test automatic conversion from ClientError to NonceError
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
    fn test_signer_error_to_nonce_error() {
        // Test automatic conversion from SignerError to NonceError
        let signer_err = SignerError::InvalidInput("invalid key".to_string());
        let nonce_err: NonceError = signer_err.into();
        
        match nonce_err {
            NonceError::Signing(msg) => {
                assert!(msg.contains("invalid key"));
            }
            _ => panic!("Expected Signing error variant"),
        }
    }

    #[test]
    fn test_nonce_lease_error_variants() {
        // Verify all three lease error variants exist and are distinct
        let no_lease = NonceError::NoLeaseAvailable;
        let acquire_failed = NonceError::LeaseAcquireFailed("timeout".to_string());
        let release_failed = NonceError::LeaseReleaseFailed("already released".to_string());
        
        // Verify display messages
        assert!(no_lease.to_string().contains("No lease available"));
        assert!(acquire_failed.to_string().contains("Failed to acquire lease"));
        assert!(release_failed.to_string().contains("Failed to release lease"));
        
        // Verify transient classification
        assert!(no_lease.is_transient(), "NoLeaseAvailable should be transient");
        assert!(acquire_failed.is_transient(), "LeaseAcquireFailed should be transient");
        assert!(!release_failed.is_transient(), "LeaseReleaseFailed should not be transient");
    }

    #[test]
    fn test_all_error_fields_owned() {
        // Verify that all error fields are owned (no &str lifetimes)
        
        // NonceError variants with String fields
        let err1 = NonceError::LeaseAcquireFailed("test".to_string());
        let err2 = NonceError::LeaseReleaseFailed("test".to_string());
        let err3 = NonceError::Client("test".to_string());
        let err4 = NonceError::Rpc {
            endpoint: Some("test".to_string()),
            message: "msg".to_string(),
        };
        
        // TransactionBuilderError variants with String fields
        let err5 = TransactionBuilderError::ConfigValidation("test".to_string());
        let err6 = TransactionBuilderError::RpcConnection("test".to_string());
        let err7 = TransactionBuilderError::SigningFailed("test".to_string());
        
        // If this compiles, it means all fields are owned
        drop(err1);
        drop(err2);
        drop(err3);
        drop(err4);
        drop(err5);
        drop(err6);
        drop(err7);
    }

    #[test]
    fn test_error_clone_trait() {
        // Verify that all error types implement Clone
        let nonce_err = NonceError::NoLeaseAvailable;
        let _cloned = nonce_err.clone();
        
        let rpc_err = RpcManagerError::Timeout {
            endpoint: "test".to_string(),
            timeout_ms: 1000,
        };
        let _cloned = rpc_err.clone();
        
        let tx_err = TransactionBuilderError::ConfigValidation("test".to_string());
        let _cloned = tx_err.clone();
    }

    #[test]
    fn test_nested_error_conversion() {
        // Test that nested errors can be automatically converted
        let nonce_err = NonceError::LeaseAcquireFailed("pool exhausted".to_string());
        let tx_err: TransactionBuilderError = nonce_err.into();
        
        // Verify we can access the nested error
        match tx_err {
            TransactionBuilderError::Nonce(err) => {
                match err {
                    NonceError::LeaseAcquireFailed(msg) => {
                        assert_eq!(msg, "pool exhausted");
                    }
                    _ => panic!("Expected LeaseAcquireFailed variant"),
                }
            }
            _ => panic!("Expected Nonce variant"),
        }
    }

    #[test]
    fn test_error_partial_eq() {
        // Test that NonceError implements PartialEq
        let err1 = NonceError::NoLeaseAvailable;
        let err2 = NonceError::NoLeaseAvailable;
        let err3 = NonceError::LeaseAcquireFailed("test".to_string());
        
        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }
}
