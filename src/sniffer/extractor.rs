//! PremintCandidate extraction with fixed-size fields for hot-path performance

use super::errors::{AccountExtractError, MintExtractError};
use super::prefilter;
use smallvec::SmallVec;
use solana_sdk::pubkey::Pubkey;

/// Priority level for candidates
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityLevel {
    /// High priority - should be processed immediately
    High,
    /// Low priority - can be processed with normal queue
    Low,
}

/// Minimal candidate structure - only critical data with fixed-size fields
/// This structure is optimized for hot-path creation with minimal allocations
#[derive(Debug, Clone)]
pub struct PremintCandidate {
    /// Token mint address
    pub mint: Pubkey,
    /// Associated accounts (using SmallVec to avoid heap allocation for small counts)
    pub accounts: SmallVec<[Pubkey; 8]>,
    /// Price hint from heuristic analysis
    pub price_hint: f64,
    /// Unique trace ID for tracking
    pub trace_id: u64,
    /// Priority level
    pub priority: PriorityLevel,
}

impl PremintCandidate {
    /// Create a new candidate
    #[inline]
    pub fn new(
        mint: Pubkey,
        accounts: SmallVec<[Pubkey; 8]>,
        price_hint: f64,
        trace_id: u64,
        priority: PriorityLevel,
    ) -> Self {
        Self {
            mint,
            accounts,
            price_hint,
            trace_id,
            priority,
        }
    }

    /// Try to extract a candidate from transaction bytes
    /// This is the main hot-path extraction function
    ///
    /// Returns Ok(candidate) if extraction succeeds, Err otherwise
    /// Errors are logged but not propagated to avoid disrupting the hot path
    #[inline]
    pub fn try_extract_candidate(
        tx_bytes: &[u8],
        trace_id: u64,
        price_hint: f64,
        priority: PriorityLevel,
        safe_offsets: bool,
    ) -> Result<Self, ExtractError> {
        // Extract mint
        let mint =
            prefilter::extract_mint(tx_bytes, safe_offsets).map_err(ExtractError::MintExtract)?;

        // Extract accounts
        let accounts = prefilter::extract_accounts(tx_bytes, safe_offsets)
            .map_err(ExtractError::AccountExtract)?;

        Ok(Self::new(mint, accounts, price_hint, trace_id, priority))
    }

    /// Check if this is a high priority candidate
    #[inline(always)]
    pub fn is_high_priority(&self) -> bool {
        matches!(self.priority, PriorityLevel::High)
    }
}

/// Extraction error type
#[derive(Debug, Clone)]
pub enum ExtractError {
    /// Mint extraction failed
    MintExtract(MintExtractError),
    /// Account extraction failed
    AccountExtract(AccountExtractError),
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MintExtract(e) => write!(f, "Mint extraction error: {}", e),
            Self::AccountExtract(e) => write!(f, "Account extraction error: {}", e),
        }
    }
}

impl std::error::Error for ExtractError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidate_creation() {
        let mint = Pubkey::new_unique();
        let mut accounts = SmallVec::new();
        accounts.push(Pubkey::new_unique());
        accounts.push(Pubkey::new_unique());

        let candidate =
            PremintCandidate::new(mint, accounts.clone(), 1.5, 123, PriorityLevel::High);

        assert_eq!(candidate.mint, mint);
        assert_eq!(candidate.accounts.len(), 2);
        assert_eq!(candidate.price_hint, 1.5);
        assert_eq!(candidate.trace_id, 123);
        assert!(candidate.is_high_priority());
    }

    #[test]
    fn test_priority_level() {
        let mint = Pubkey::new_unique();
        let accounts = SmallVec::new();

        let high_candidate =
            PremintCandidate::new(mint, accounts.clone(), 1.0, 1, PriorityLevel::High);
        assert!(high_candidate.is_high_priority());

        let low_candidate = PremintCandidate::new(mint, accounts, 1.0, 2, PriorityLevel::Low);
        assert!(!low_candidate.is_high_priority());
    }
}
