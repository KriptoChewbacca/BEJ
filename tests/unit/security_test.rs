//! Unit test for security module

#[cfg(test)]
mod security_tests {
    use ultra::sniffer::security;
    use ultra::sniffer::extractor::{PremintCandidate, PriorityLevel};
    use solana_sdk::pubkey::Pubkey;
    use smallvec::SmallVec;

    #[test]
    fn test_check_tx_size() {
        assert!(!security::check_tx_size(&[0; 32])); // Too small
        assert!(security::check_tx_size(&[0; 256])); // Valid
        assert!(!security::check_tx_size(&[0; 2000])); // Too large
    }

    #[test]
    fn test_is_valid_pubkey() {
        let default_pubkey = Pubkey::default();
        assert!(!security::is_valid_pubkey(&default_pubkey));
        
        let valid_pubkey = Pubkey::new_unique();
        assert!(security::is_valid_pubkey(&valid_pubkey));
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
        assert!(security::is_valid_candidate(&valid_candidate));
        
        // Invalid mint
        let invalid_candidate = PremintCandidate::new(
            Pubkey::default(),
            accounts,
            1.5,
            123,
            PriorityLevel::High,
        );
        assert!(!security::is_valid_candidate(&invalid_candidate));
    }

    #[test]
    fn test_quick_sanity_check() {
        assert!(security::quick_sanity_check(&[0x01; 256]));
        assert!(!security::quick_sanity_check(&[0x00; 256])); // All zeros
        assert!(!security::quick_sanity_check(&[0xFF; 256])); // All 0xFF
        assert!(!security::quick_sanity_check(&[0x01; 32])); // Too small
    }
}
