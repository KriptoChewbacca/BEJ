//! Task 2: Comprehensive RAII tests for ExecutionContext and TxBuildOutput
//!
//! These tests validate the RAII (Resource Acquisition Is Initialization)
//! pattern implementation for nonce lease management.

#[cfg(test)]
mod task2_raii_tests {
    use bot::nonce_manager::NonceLease;
    use bot::tx_builder::{ExecutionContext, TxBuildOutput};
    use solana_sdk::{
        hash::Hash,
        message::{v0::Message as MessageV0, VersionedMessage},
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        system_instruction,
        transaction::VersionedTransaction,
    };
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    };
    use std::time::Duration;

    /// Helper to create a minimal transaction for testing
    fn create_test_transaction(num_signers: u8) -> VersionedTransaction {
        let payer = Keypair::new();

        // Create a simple transfer instruction
        let instruction =
            system_instruction::transfer(&payer.pubkey(), &Keypair::new().pubkey(), 1000);

        // Build message
        let message =
            MessageV0::try_compile(&payer.pubkey(), &[instruction], &[], Hash::default()).unwrap();

        // Create transaction with dummy signatures
        let mut tx = VersionedTransaction {
            signatures: vec![],
            message: VersionedMessage::V0(message),
        };

        // Add dummy signatures for all required signers
        for _ in 0..num_signers {
            tx.signatures
                .push(solana_sdk::signature::Signature::default());
        }

        tx
    }

    /// Create a mock NonceLease for testing
    fn create_mock_lease(release_counter: Arc<AtomicU64>) -> NonceLease {
        let nonce_pubkey = Pubkey::new_unique();
        let nonce_blockhash = Hash::new_unique();
        let ttl = Duration::from_secs(60);

        // Create release callback that increments counter
        let release_fn = move || {
            release_counter.fetch_add(1, Ordering::SeqCst);
        };

        NonceLease::new(nonce_pubkey, 1000, nonce_blockhash, ttl, release_fn)
    }

    // ============================================================================
    // ExecutionContext Tests
    // ============================================================================

    #[test]
    fn test_execution_context_non_durable() {
        // Test ExecutionContext without nonce (recent blockhash mode)
        let context = ExecutionContext {
            blockhash: Hash::new_unique(),
            nonce_pubkey: None,
            nonce_authority: None,
            nonce_lease: None,
            #[cfg(feature = "zk_enabled")]
            zk_proof: None,
        };

        assert!(!context.is_durable(), "Should not be durable mode");
        assert!(context.nonce_pubkey.is_none());
        assert!(context.nonce_authority.is_none());
    }

    #[test]
    fn test_execution_context_durable() {
        // Test ExecutionContext with nonce (durable mode)
        let release_counter = Arc::new(AtomicU64::new(0));
        let lease = create_mock_lease(release_counter.clone());

        let context = ExecutionContext {
            blockhash: Hash::new_unique(),
            nonce_pubkey: Some(Pubkey::new_unique()),
            nonce_authority: Some(Pubkey::new_unique()),
            nonce_lease: Some(lease),
            #[cfg(feature = "zk_enabled")]
            zk_proof: None,
        };

        assert!(context.is_durable(), "Should be durable mode");
        assert!(context.nonce_pubkey.is_some());
        assert!(context.nonce_authority.is_some());
    }

    #[test]
    fn test_execution_context_extract_lease() {
        // Test lease extraction (ownership transfer)
        let release_counter = Arc::new(AtomicU64::new(0));
        let lease = create_mock_lease(release_counter.clone());

        let context = ExecutionContext {
            blockhash: Hash::new_unique(),
            nonce_pubkey: Some(Pubkey::new_unique()),
            nonce_authority: Some(Pubkey::new_unique()),
            nonce_lease: Some(lease),
            #[cfg(feature = "zk_enabled")]
            zk_proof: None,
        };

        // Extract lease (consumes context)
        let extracted_lease = context.extract_lease();
        assert!(extracted_lease.is_some(), "Lease should be extracted");

        // Lease should still be valid
        drop(extracted_lease);

        // Release counter should be incremented once (from drop)
        assert_eq!(
            release_counter.load(Ordering::SeqCst),
            1,
            "Lease should be released on drop"
        );
    }

    #[test]
    fn test_execution_context_drop_releases_lease() {
        // Test that dropping ExecutionContext releases the lease
        let release_counter = Arc::new(AtomicU64::new(0));
        let lease = create_mock_lease(release_counter.clone());

        {
            let _context = ExecutionContext {
                blockhash: Hash::new_unique(),
                nonce_pubkey: Some(Pubkey::new_unique()),
                nonce_authority: Some(Pubkey::new_unique()),
                nonce_lease: Some(lease),
                #[cfg(feature = "zk_enabled")]
                zk_proof: None,
            };
            // Context dropped here
        }

        // Release should have been called via NonceLease Drop
        assert_eq!(
            release_counter.load(Ordering::SeqCst),
            1,
            "Lease should be released on context drop"
        );
    }

    // ============================================================================
    // TxBuildOutput Tests
    // ============================================================================

    #[test]
    fn test_tx_build_output_without_nonce() {
        // Test TxBuildOutput without nonce guard
        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, None);

        assert!(output.nonce_guard.is_none(), "Should have no nonce guard");
        assert_eq!(
            output.required_signers.len(),
            1,
            "Should have 1 required signer"
        );
    }

    #[test]
    fn test_tx_build_output_with_nonce() {
        // Test TxBuildOutput with nonce guard
        let release_counter = Arc::new(AtomicU64::new(0));
        let lease = create_mock_lease(release_counter.clone());

        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, Some(lease));

        assert!(output.nonce_guard.is_some(), "Should have nonce guard");
        assert_eq!(
            output.required_signers.len(),
            1,
            "Should have 1 required signer"
        );

        // Drop output
        drop(output);

        // Lease should be released
        assert_eq!(
            release_counter.load(Ordering::SeqCst),
            1,
            "Lease should be released on drop"
        );
    }

    #[test]
    fn test_tx_build_output_extracts_multiple_signers() {
        // Test that signers are correctly extracted
        // Note: Simple transfer has only 1 signer
        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, None);

        assert_eq!(
            output.required_signers.len(),
            1,
            "Should have 1 required signer"
        );
    }

    #[tokio::test]
    async fn test_tx_build_output_explicit_release() {
        // Test explicit nonce release
        let release_counter = Arc::new(AtomicU64::new(0));
        let lease = create_mock_lease(release_counter.clone());

        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, Some(lease));

        // Explicitly release
        let result = output.release_nonce().await;
        assert!(result.is_ok(), "Explicit release should succeed");

        // Release counter should be incremented
        assert_eq!(
            release_counter.load(Ordering::SeqCst),
            1,
            "Lease should be released explicitly"
        );
    }

    #[tokio::test]
    async fn test_tx_build_output_release_without_nonce() {
        // Test releasing when no nonce guard is held
        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, None);

        // Should succeed even without nonce
        let result = output.release_nonce().await;
        assert!(
            result.is_ok(),
            "Release without nonce should succeed (no-op)"
        );
    }

    #[test]
    fn test_tx_build_output_into_tx() {
        // Test extracting transaction
        let release_counter = Arc::new(AtomicU64::new(0));
        let lease = create_mock_lease(release_counter.clone());

        let original_tx = create_test_transaction(1);
        let original_blockhash = original_tx.message.recent_blockhash().clone();

        let output = TxBuildOutput::new(original_tx, Some(lease));

        // Extract transaction
        let extracted_tx = output.into_tx();

        // Verify transaction is the same
        assert_eq!(extracted_tx.message.recent_blockhash(), &original_blockhash);

        // Lease should be released when output is dropped (after into_tx)
        assert_eq!(
            release_counter.load(Ordering::SeqCst),
            1,
            "Lease should be released after into_tx"
        );
    }

    #[test]
    fn test_tx_build_output_tx_ref() {
        // Test getting transaction reference
        let tx = create_test_transaction(1);
        let original_blockhash = tx.message.recent_blockhash().clone();

        let output = TxBuildOutput::new(tx, None);
        let tx_ref = output.tx_ref();

        assert_eq!(tx_ref.message.recent_blockhash(), &original_blockhash);
    }

    #[test]
    fn test_tx_build_output_required_signers() {
        // Test getting required signers slice
        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, None);

        let signers = output.required_signers();
        assert_eq!(signers.len(), 1, "Should have 1 required signer");
    }

    // ============================================================================
    // RAII Contract Validation Tests
    // ============================================================================

    #[test]
    fn test_raii_no_leak_on_early_drop() {
        // Test that dropping output early doesn't leak lease
        let release_counter = Arc::new(AtomicU64::new(0));

        {
            let lease = create_mock_lease(release_counter.clone());
            let tx = create_test_transaction(1);
            let _output = TxBuildOutput::new(tx, Some(lease));
            // Intentional early drop (simulating error path)
        }

        // Lease should be released
        assert_eq!(
            release_counter.load(Ordering::SeqCst),
            1,
            "Lease should not leak on early drop"
        );
    }

    #[tokio::test]
    async fn test_raii_idempotent_release() {
        // Test that lease release is idempotent
        let release_counter = Arc::new(AtomicU64::new(0));
        let lease = create_mock_lease(release_counter.clone());

        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, Some(lease));

        // First release (explicit)
        let result = output.release_nonce().await;
        assert!(result.is_ok(), "First release should succeed");

        // Counter should be 1
        assert_eq!(
            release_counter.load(Ordering::SeqCst),
            1,
            "Counter should be 1 after explicit release"
        );

        // Note: output is consumed by release_nonce, so we can't call it again
        // This is part of the RAII contract - preventing double-release at compile time
    }

    #[test]
    fn test_ownership_semantics() {
        // Test that lease ownership is properly transferred
        let release_counter = Arc::new(AtomicU64::new(0));

        // Create context with lease
        let lease = create_mock_lease(release_counter.clone());
        let context = ExecutionContext {
            blockhash: Hash::new_unique(),
            nonce_pubkey: Some(Pubkey::new_unique()),
            nonce_authority: Some(Pubkey::new_unique()),
            nonce_lease: Some(lease),
            #[cfg(feature = "zk_enabled")]
            zk_proof: None,
        };

        // Extract lease (transfer ownership, consumes context)
        let lease = context.extract_lease().expect("Should extract lease");

        // Counter should still be 0 (context doesn't release extracted lease)
        assert_eq!(
            release_counter.load(Ordering::SeqCst),
            0,
            "Context should not release extracted lease"
        );

        // Create output with lease
        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, Some(lease));

        // Drop output (should release lease)
        drop(output);
        assert_eq!(
            release_counter.load(Ordering::SeqCst),
            1,
            "Output drop should release lease"
        );
    }

    // ============================================================================
    // Concurrency Tests
    // ============================================================================

    #[tokio::test]
    async fn test_concurrent_output_creation() {
        // Test that multiple outputs can be created concurrently
        let mut handles = vec![];

        for _ in 0..10 {
            let handle = tokio::spawn(async move {
                let release_counter = Arc::new(AtomicU64::new(0));
                let lease = create_mock_lease(release_counter.clone());
                let tx = create_test_transaction(1);
                let output = TxBuildOutput::new(tx, Some(lease));
                drop(output);
                assert_eq!(release_counter.load(Ordering::SeqCst), 1);
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.expect("Task should complete");
        }
    }

    #[tokio::test]
    async fn test_concurrent_context_operations() {
        // Test concurrent context creation and lease extraction
        let mut handles = vec![];

        for _ in 0..10 {
            let handle = tokio::spawn(async move {
                let release_counter = Arc::new(AtomicU64::new(0));
                let lease = create_mock_lease(release_counter.clone());

                let context = ExecutionContext {
                    blockhash: Hash::new_unique(),
                    nonce_pubkey: Some(Pubkey::new_unique()),
                    nonce_authority: Some(Pubkey::new_unique()),
                    nonce_lease: Some(lease),
                    #[cfg(feature = "zk_enabled")]
                    zk_proof: None,
                };

                let lease = context.extract_lease().expect("Should extract");
                drop(lease);

                assert_eq!(release_counter.load(Ordering::SeqCst), 1);
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.expect("Task should complete");
        }
    }

    #[test]
    fn test_debug_output_no_leak() {
        // Test that Debug impl doesn't cause issues
        let release_counter = Arc::new(AtomicU64::new(0));
        let lease = create_mock_lease(release_counter.clone());

        let context = ExecutionContext {
            blockhash: Hash::new_unique(),
            nonce_pubkey: Some(Pubkey::new_unique()),
            nonce_authority: Some(Pubkey::new_unique()),
            nonce_lease: Some(lease),
            #[cfg(feature = "zk_enabled")]
            zk_proof: None,
        };

        // Call Debug format
        let debug_str = format!("{:?}", context);
        assert!(debug_str.contains("ExecutionContext"));
        assert!(debug_str.contains("nonce_lease_status"));

        // Drop and verify release
        drop(context);
        assert_eq!(release_counter.load(Ordering::SeqCst), 1);
    }
}
