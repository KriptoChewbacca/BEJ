//! Integration test for backpressure handling

#[cfg(test)]
mod backpressure_tests {
    use ultra::sniffer::handoff::{try_send_candidate, BatchSender};
    use ultra::sniffer::extractor::{PremintCandidate, PriorityLevel};
    use ultra::sniffer::telemetry::SnifferMetrics;
    use solana_sdk::pubkey::Pubkey;
    use smallvec::SmallVec;
    use tokio::sync::mpsc;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_channel_backpressure() {
        let (tx, mut rx) = mpsc::channel(10);
        let metrics = Arc::new(SnifferMetrics::new());
        
        // Fill the channel
        for i in 0..10 {
            let candidate = PremintCandidate::new(
                Pubkey::new_unique(),
                SmallVec::new(),
                1.0,
                i,
                PriorityLevel::Low,
            );
            try_send_candidate(&tx, candidate, &metrics);
        }
        
        // Next send should trigger backpressure
        let candidate = PremintCandidate::new(
            Pubkey::new_unique(),
            SmallVec::new(),
            1.0,
            99,
            PriorityLevel::Low,
        );
        try_send_candidate(&tx, candidate, &metrics);
        
        // Check that backpressure was detected
        assert!(metrics.backpressure_events.load(std::sync::atomic::Ordering::Relaxed) > 0);
        
        // Drain channel
        while rx.try_recv().is_ok() {}
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_batch_sender() {
        let (tx, mut rx) = mpsc::channel(100);
        let metrics = Arc::new(SnifferMetrics::new());
        let mut batch_sender = BatchSender::new(
            tx,
            3,
            Duration::from_millis(100),
            metrics,
        );
        
        // Add candidates
        for i in 0..5 {
            let candidate = PremintCandidate::new(
                Pubkey::new_unique(),
                SmallVec::new(),
                1.0,
                i,
                PriorityLevel::Low,
            );
            batch_sender.add(candidate);
        }
        
        // Should have sent 2 batches (3 + 2)
        let mut count = 0;
        while rx.try_recv().is_ok() {
            count += 1;
        }
        assert_eq!(count, 5);
    }
}
