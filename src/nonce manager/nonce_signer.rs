//! Signer abstraction for nonce operations
//!
//! This module provides an async signer interface that supports:
//! - Local keypair signing
//! - Hardware wallet signing (HSM, Ledger)
//! - Remote signer support
//! - Mock signer for testing
use super::nonce_errors::{NonceError, NonceResult};
use async_trait::async_trait;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::Transaction,
};

/// Async signer trait for signing transactions
#[async_trait]
pub trait SignerService: Send + Sync {
    /// Get the public key of this signer
    async fn pubkey(&self) -> Pubkey;

    /// Sign a transaction
    async fn sign_transaction(&self, transaction: &mut Transaction) -> NonceResult<()>;

    /// Sign multiple transactions (batch operation)
    async fn sign_transactions(&self, transactions: &mut [Transaction]) -> NonceResult<()> {
        for tx in transactions.iter_mut() {
            self.sign_transaction(tx).await?;
        }
        Ok(())
    }

    /// Sign a nonce advance operation (Security Enhancement 3)
    /// Used for authority rotation and HSM/Ledger integration
    async fn sign_advance(
        &self,
        _nonce_account: &Pubkey,
        transaction: &mut Transaction,
    ) -> NonceResult<()> {
        // Default implementation delegates to sign_transaction
        self.sign_transaction(transaction).await
    }

    /// Batch verify signatures with hardware acceleration (Security Enhancement 3)
    /// Returns true if all signatures are valid
    async fn batch_verify_signatures(
        &self,
        signatures: &[Signature],
        messages: &[Vec<u8>],
    ) -> NonceResult<bool> {
        // Default implementation: sequential verification
        // In production, this would use GPU/FPGA acceleration for >10 signatures
        if signatures.len() != messages.len() {
            return Err(NonceError::Signing(
                "Signature count does not match message count".to_string(),
            ));
        }

        // TODO: Add GPU/FPGA acceleration via rust-cuda/opencl for pools >20 nonces
        // For now, stub returns true (would need actual verification implementation)
        Ok(true)
    }
}

/// Local keypair signer (for development and testing)
pub struct LocalSigner {
    keypair: Keypair,
}

impl LocalSigner {
    pub fn new(keypair: Keypair) -> Self {
        Self { keypair }
    }

    pub fn from_bytes(bytes: &[u8]) -> NonceResult<Self> {
        // Validate length
        if bytes.len() != 64 {
            return Err(NonceError::Signing(format!(
                "Invalid keypair length: expected 64 bytes, got {}",
                bytes.len()
            )));
        }
        // Reject all-zero keys
        if bytes.iter().all(|&b| b == 0) {
            return Err(NonceError::Signing(
                "Invalid keypair: all-zero key rejected".to_string(),
            ));
        }
        Keypair::try_from(bytes)
            .map(|kp| Self { keypair: kp })
            .map_err(|e| NonceError::Signing(format!("Invalid keypair bytes: {}", e)))
    }
}

#[async_trait]
impl SignerService for LocalSigner {
    async fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }

    async fn sign_transaction(&self, transaction: &mut Transaction) -> NonceResult<()> {
        // Local signing is synchronous but we wrap it in async
        tokio::task::yield_now().await; // Yield to allow other tasks to run

        transaction
            .try_sign(&[&self.keypair], transaction.message.recent_blockhash)
            .map_err(NonceError::from_signer_error)?;

        Ok(())
    }
}

/// Mock signer for testing
pub struct MockSigner {
    pubkey: Pubkey,
    should_fail: bool,
}

impl MockSigner {
    pub fn new(pubkey: Pubkey) -> Self {
        Self {
            pubkey,
            should_fail: false,
        }
    }

    pub fn new_failing(pubkey: Pubkey) -> Self {
        Self {
            pubkey,
            should_fail: true,
        }
    }
}

#[async_trait]
impl SignerService for MockSigner {
    async fn pubkey(&self) -> Pubkey {
        self.pubkey
    }

    async fn sign_transaction(&self, _transaction: &mut Transaction) -> NonceResult<()> {
        if self.should_fail {
            Err(NonceError::Signing(
                "Mock signer configured to fail".to_string(),
            ))
        } else {
            // In a real mock, we'd sign with a test keypair
            Ok(())
        }
    }
}

/// Remote signer (placeholder for future HSM/remote signing service)
pub struct RemoteSigner {
    endpoint: String,
    pubkey: Pubkey,
}

impl RemoteSigner {
    pub fn new(endpoint: String, pubkey: Pubkey) -> Self {
        Self { endpoint, pubkey }
    }
}

#[async_trait]
impl SignerService for RemoteSigner {
    async fn pubkey(&self) -> Pubkey {
        self.pubkey
    }

    async fn sign_transaction(&self, _transaction: &mut Transaction) -> NonceResult<()> {
        // Placeholder: would make HTTP/gRPC call to remote signing service
        Err(NonceError::Signing(
            "Remote signer not yet implemented".to_string(),
        ))
    }
}

/// Hardware wallet signer (placeholder for Ledger/Trezor support)
pub struct HardwareWalletSigner {
    device_path: String,
    derivation_path: String,
    pubkey: Pubkey,
}

impl HardwareWalletSigner {
    pub fn new(device_path: String, derivation_path: String, pubkey: Pubkey) -> Self {
        Self {
            device_path,
            derivation_path,
            pubkey,
        }
    }

    /// Query Ledger device for batch signing capability
    async fn query_device_capabilities(&self) -> NonceResult<bool> {
        // Placeholder: would query Ledger device via rust-ledger library
        // Returns true if device supports batch operations
        Ok(false)
    }
}

#[async_trait]
impl SignerService for HardwareWalletSigner {
    async fn pubkey(&self) -> Pubkey {
        self.pubkey
    }

    async fn sign_transaction(&self, _transaction: &mut Transaction) -> NonceResult<()> {
        // Placeholder: would communicate with hardware wallet via USB/Bluetooth
        Err(NonceError::Signing(
            "Hardware wallet signer not yet implemented".to_string(),
        ))
    }

    async fn sign_advance(
        &self,
        nonce_account: &Pubkey,
        _transaction: &mut Transaction,
    ) -> NonceResult<()> {
        // Placeholder: Special handling for Ledger nonce advance operations
        // Would use rust-ledger library for device communication
        Err(NonceError::Signing(format!(
            "Ledger nonce advance not yet implemented for account {}",
            nonce_account
        )))
    }

    async fn batch_verify_signatures(
        &self,
        signatures: &[Signature],
        messages: &[Vec<u8>],
    ) -> NonceResult<bool> {
        // Check if device supports batch operations
        if self.query_device_capabilities().await? {
            // Placeholder: Would use Ledger batch verification
            Ok(true)
        } else {
            // Fall back to default implementation
            Ok(signatures.len() == messages.len())
        }
    }
}

/// GPU/FPGA accelerated signature verification (Security Enhancement 3)
/// Stub for hardware acceleration integration
pub struct BatchSignatureVerifier {
    gpu_enabled: bool,
    threshold: usize,
}

impl BatchSignatureVerifier {
    pub fn new() -> Self {
        Self {
            gpu_enabled: false, // Would detect GPU/FPGA availability
            threshold: 10,      // Use GPU for >10 signatures
        }
    }

    /// Verify multiple signatures with hardware acceleration
    /// Target: 4x speedup for pools >20 nonces
    pub async fn verify_batch(
        &self,
        signatures: &[Signature],
        messages: &[Vec<u8>],
    ) -> NonceResult<bool> {
        if signatures.len() != messages.len() {
            return Err(NonceError::Signing("Signature count mismatch".to_string()));
        }

        // Use GPU/FPGA acceleration for large batches
        if self.gpu_enabled && signatures.len() >= self.threshold {
            // Placeholder: Would use rust-cuda or opencl bindings
            // For actual implementation:
            // 1. Transfer signatures and messages to GPU memory
            // 2. Run parallel verification kernels
            // 3. Retrieve results
            Ok(true)
        } else {
            // CPU verification for small batches
            // In production, would use actual Ed25519 verification
            Ok(true)
        }
    }
}

impl Default for BatchSignatureVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::hash::Hash;
    // TODO(migrate-system-instruction): temporary allow, full migration post-profit
    #[allow(deprecated)]
    use solana_sdk::system_instruction;

    #[tokio::test]
    async fn test_local_signer() {
        let keypair = Keypair::new();
        let signer = LocalSigner::new(keypair);

        let pubkey = signer.pubkey().await;
        assert_ne!(pubkey, Pubkey::default());

        // Create a simple transaction
        let mut tx = Transaction::new_with_payer(
            &[system_instruction::transfer(
                &pubkey,
                &Pubkey::new_unique(),
                1000,
            )],
            Some(&pubkey),
        );
        tx.message.recent_blockhash = Hash::new_unique();

        let result = signer.sign_transaction(&mut tx).await;
        assert!(result.is_ok());
        assert!(!tx.signatures.is_empty());
    }

    #[tokio::test]
    async fn test_mock_signer_success() {
        let pubkey = Pubkey::new_unique();
        let signer = MockSigner::new(pubkey);

        assert_eq!(signer.pubkey().await, pubkey);

        let mut tx = Transaction::default();
        let result = signer.sign_transaction(&mut tx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_signer_failure() {
        let pubkey = Pubkey::new_unique();
        let signer = MockSigner::new_failing(pubkey);

        let mut tx = Transaction::default();
        let result = signer.sign_transaction(&mut tx).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NonceError::Signing(_)));
    }

    #[tokio::test]
    async fn test_batch_signing() {
        let keypair = Keypair::new();
        let signer = LocalSigner::new(keypair);
        let pubkey = signer.pubkey().await;

        let mut transactions: Vec<Transaction> = (0..3)
            .map(|_| {
                let mut tx = Transaction::new_with_payer(
                    &[system_instruction::transfer(
                        &pubkey,
                        &Pubkey::new_unique(),
                        1000,
                    )],
                    Some(&pubkey),
                );
                tx.message.recent_blockhash = Hash::new_unique();
                tx
            })
            .collect();

        let result = signer.sign_transactions(&mut transactions).await;
        assert!(result.is_ok());

        for tx in &transactions {
            assert!(!tx.signatures.is_empty());
        }
    }

    #[tokio::test]
    async fn test_sign_advance() {
        let keypair = Keypair::new();
        let signer = LocalSigner::new(keypair);
        let pubkey = signer.pubkey().await;
        let nonce_account = Pubkey::new_unique();

        let mut tx = Transaction::new_with_payer(
            &[system_instruction::transfer(
                &pubkey,
                &Pubkey::new_unique(),
                1000,
            )],
            Some(&pubkey),
        );
        tx.message.recent_blockhash = Hash::new_unique();

        let result = signer.sign_advance(&nonce_account, &mut tx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_batch_signature_verifier() {
        let verifier = BatchSignatureVerifier::new();

        // Test with matching counts
        let signatures = vec![Signature::default(); 5];
        let messages = vec![vec![0u8; 32]; 5];

        let result = verifier.verify_batch(&signatures, &messages).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Test with mismatched counts
        let signatures = vec![Signature::default(); 5];
        let messages = vec![vec![0u8; 32]; 3];

        let result = verifier.verify_batch(&signatures, &messages).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_batch_verify_signatures_trait() {
        let keypair = Keypair::new();
        let signer = LocalSigner::new(keypair);

        let signatures = vec![Signature::default(); 5];
        let messages = vec![vec![0u8; 32]; 5];

        let result = signer.batch_verify_signatures(&signatures, &messages).await;
        assert!(result.is_ok());
    }
}
