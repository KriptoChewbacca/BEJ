//! Unit test for extractor module

#[cfg(test)]
mod extractor_tests {
    use ultra::sniffer::extractor::{PremintCandidate, PriorityLevel};
    use solana_sdk::pubkey::Pubkey;
    use smallvec::SmallVec;

    #[test]
    fn test_candidate_creation() {
        let mint = Pubkey::new_unique();
        let mut accounts = SmallVec::new();
        accounts.push(Pubkey::new_unique());
        accounts.push(Pubkey::new_unique());
        
        let candidate = PremintCandidate::new(
            mint,
            accounts.clone(),
            1.5,
            123,
            PriorityLevel::High,
        );
        
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
        
        let high_candidate = PremintCandidate::new(
            mint,
            accounts.clone(),
            1.0,
            1,
            PriorityLevel::High,
        );
        assert!(high_candidate.is_high_priority());
        
        let low_candidate = PremintCandidate::new(
            mint,
            accounts,
            1.0,
            2,
            PriorityLevel::Low,
        );
        assert!(!low_candidate.is_high_priority());
    }
}
