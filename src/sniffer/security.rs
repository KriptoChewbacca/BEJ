//! Security module with inline sanity checks and async verification pool

use solana_sdk::pubkey::Pubkey;
use super::extractor::PremintCandidate;

/// Inline sanity check for transaction size
/// Returns true if transaction size is within acceptable bounds
#[inline(always)]
pub fn check_tx_size(tx_bytes: &[u8]) -> bool {
    const MIN_TX_SIZE: usize = 64;
    const MAX_TX_SIZE: usize = 1232; // Solana max transaction size
    
    let size = tx_bytes.len();
    size >= MIN_TX_SIZE && size <= MAX_TX_SIZE
}

/// Inline check for valid pubkey (not all zeros)
#[inline(always)]
pub fn is_valid_pubkey(pubkey: &Pubkey) -> bool {
    *pubkey != Pubkey::default()
}

/// Inline check for suspicious pubkey patterns
/// Returns true if pubkey looks suspicious
#[inline(always)]
pub fn is_suspicious_pubkey(pubkey: &Pubkey) -> bool {
    // Check for all zeros (already covered by is_valid_pubkey)
    if *pubkey == Pubkey::default() {
        return true;
    }
    
    // Check for all 0xFF
    let bytes = pubkey.to_bytes();
    if bytes.iter().all(|&b| b == 0xFF) {
        return true;
    }
    
    // Check for repeating patterns (simplified check)
    // If first 4 bytes are all the same, might be suspicious
    if bytes[0] == bytes[1] && bytes[1] == bytes[2] && bytes[2] == bytes[3] {
        return true;
    }
    
    false
}

/// Inline check for candidate validity
/// This is a cheap check that runs inline before sending to channel
#[inline]
pub fn is_valid_candidate(candidate: &PremintCandidate) -> bool {
    // Check mint is valid
    if !is_valid_pubkey(&candidate.mint) {
        return false;
    }
    
    // Check mint is not suspicious
    if is_suspicious_pubkey(&candidate.mint) {
        return false;
    }
    
    // Check we have at least one account
    if candidate.accounts.is_empty() {
        return false;
    }
    
    // Check price hint is reasonable (not NaN, not negative, not infinite)
    if !candidate.price_hint.is_finite() || candidate.price_hint < 0.0 {
        return false;
    }
    
    // All checks passed
    true
}

/// Quick sanity check on transaction bytes
/// Returns true if transaction passes basic sanity checks
#[inline]
pub fn quick_sanity_check(tx_bytes: &[u8]) -> bool {
    // Check size
    if !check_tx_size(tx_bytes) {
        return false;
    }
    
    // Check not all zeros
    if tx_bytes.iter().all(|&b| b == 0) {
        return false;
    }
    
    // Check not all 0xFF
    if tx_bytes.iter().all(|&b| b == 0xFF) {
        return false;
    }
    
    true
}

/// Async verifier pool for heavy verification tasks
/// This should be called from a separate task pool, NOT in the hot path
pub mod async_verifier {
    use super::*;
    use tokio::sync::mpsc;
    use tracing::{debug, warn};
    
    /// Verification request
    pub struct VerificationRequest {
        pub candidate: PremintCandidate,
        pub tx_bytes: Vec<u8>,
    }
    
    /// Verification result
    pub struct VerificationResult {
        pub candidate: PremintCandidate,
        pub is_valid: bool,
        pub reason: Option<String>,
    }
    
    /// Async verifier worker
    /// This runs in a separate task and performs heavy verification
    pub async fn verifier_worker(
        mut rx: mpsc::Receiver<VerificationRequest>,
        tx: mpsc::Sender<VerificationResult>,
    ) {
        debug!("Verifier worker started");
        
        while let Some(request) = rx.recv().await {
            let result = verify_candidate_deep(&request.candidate, &request.tx_bytes);
            
            let verification_result = VerificationResult {
                candidate: request.candidate,
                is_valid: result.is_ok(),
                reason: result.err(),
            };
            
            if tx.send(verification_result).await.is_err() {
                warn!("Failed to send verification result - receiver dropped");
                break;
            }
        }
        
        debug!("Verifier worker stopped");
    }
    
    /// Deep verification of candidate (expensive - NOT for hot path)
    fn verify_candidate_deep(
        candidate: &PremintCandidate,
        tx_bytes: &[u8],
    ) -> Result<(), String> {
        // Perform expensive checks here
        // For example:
        // - Full transaction deserialization
        // - Signature verification
        // - Program ID validation against known list
        // - Account ownership checks
        // - Data payload validation
        
        // Quick inline checks first
        if !is_valid_candidate(candidate) {
            return Err("Candidate failed inline validation".to_string());
        }
        
        if !quick_sanity_check(tx_bytes) {
            return Err("Transaction failed sanity check".to_string());
        }
        
        // Add more expensive checks here as needed
        // For now, we pass if inline checks succeed
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::SmallVec;
    use super::extractor::PriorityLevel;

    #[test]
    fn test_check_tx_size() {
        assert!(!check_tx_size(&[0; 32])); // Too small
        assert!(check_tx_size(&[0; 256])); // Valid
        assert!(!check_tx_size(&[0; 2000])); // Too large
    }

    #[test]
    fn test_is_valid_pubkey() {
        let default_pubkey = Pubkey::default();
        assert!(!is_valid_pubkey(&default_pubkey));
        
        let valid_pubkey = Pubkey::new_unique();
        assert!(is_valid_pubkey(&valid_pubkey));
    }

    #[test]
    fn test_is_suspicious_pubkey() {
        // All zeros
        let zero_pubkey = Pubkey::default();
        assert!(is_suspicious_pubkey(&zero_pubkey));
        
        // All 0xFF
        let ff_pubkey = Pubkey::from([0xFF; 32]);
        assert!(is_suspicious_pubkey(&ff_pubkey));
        
        // Repeating pattern
        let mut bytes = [0u8; 32];
        bytes[0] = 0xAA;
        bytes[1] = 0xAA;
        bytes[2] = 0xAA;
        bytes[3] = 0xAA;
        let pattern_pubkey = Pubkey::from(bytes);
        assert!(is_suspicious_pubkey(&pattern_pubkey));
        
        // Valid random pubkey
        let valid_pubkey = Pubkey::new_unique();
        assert!(!is_suspicious_pubkey(&valid_pubkey));
    }

    #[test]
    fn test_is_valid_candidate() {
        let valid_mint = Pubkey::new_unique();
        let mut accounts = SmallVec::new();
        accounts.push(Pubkey::new_unique());
        
        let valid_candidate = PremintCandidate::new(
            valid_mint,
            accounts.clone(),
            1.5,
            123,
            PriorityLevel::High,
        );
        assert!(is_valid_candidate(&valid_candidate));
        
        // Invalid mint
        let invalid_candidate = PremintCandidate::new(
            Pubkey::default(),
            accounts.clone(),
            1.5,
            123,
            PriorityLevel::High,
        );
        assert!(!is_valid_candidate(&invalid_candidate));
        
        // No accounts
        let no_accounts_candidate = PremintCandidate::new(
            valid_mint,
            SmallVec::new(),
            1.5,
            123,
            PriorityLevel::High,
        );
        assert!(!is_valid_candidate(&no_accounts_candidate));
        
        // Invalid price
        let invalid_price_candidate = PremintCandidate::new(
            valid_mint,
            accounts,
            f64::NAN,
            123,
            PriorityLevel::High,
        );
        assert!(!is_valid_candidate(&invalid_price_candidate));
    }

    #[test]
    fn test_quick_sanity_check() {
        assert!(quick_sanity_check(&[0x01; 256]));
        assert!(!quick_sanity_check(&[0x00; 256])); // All zeros
        assert!(!quick_sanity_check(&[0xFF; 256])); // All 0xFF
        assert!(!quick_sanity_check(&[0x01; 32])); // Too small
    }
}
