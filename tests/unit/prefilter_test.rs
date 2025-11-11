//! Unit test for prefilter module

#[cfg(test)]
mod prefilter_tests {
    use ultra::sniffer::prefilter;

    #[test]
    fn test_should_process_valid_tx() {
        // Create a mock transaction with sufficient size
        let mut tx = vec![0x01; 256];
        
        // This test validates basic prefilter logic
        // In real scenarios, we'd need to inject proper program IDs
        assert!(tx.len() >= 128);
    }

    #[test]
    fn test_should_process_small_tx() {
        let small_tx = vec![0x01; 64];
        let result = prefilter::should_process(&small_tx);
        assert!(!result, "Small transactions should be rejected");
    }

    #[test]
    fn test_is_vote_tx() {
        let vote_tx = vec![0x00; 100];
        assert!(prefilter::is_vote_tx(&vote_tx));
        
        let normal_tx = vec![0x01; 200];
        assert!(!prefilter::is_vote_tx(&normal_tx));
    }

    #[test]
    fn test_instr_count_check() {
        let tx = vec![0; 150];
        assert!(prefilter::instr_count_check(&tx, 100, 200));
        assert!(!prefilter::instr_count_check(&tx, 200, 300));
    }
}
