//! Stress test: burst of 10k transactions

#[cfg(test)]
mod burst_tests {
    use ultra::sniffer::{Sniffer, SnifferApi, SnifferConfig};
    use tokio::time::Duration;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore] // Run only when explicitly requested
    async fn test_burst_10k_tx() {
        let mut config = SnifferConfig::default();
        config.channel_capacity = 10000;
        config.stream_buffer_capacity = 10000;
        
        let sniffer = Sniffer::new(config);
        let metrics = sniffer.get_metrics();
        
        let mut rx = sniffer.start().await.expect("Failed to start");
        
        // Let the sniffer run and process mock transactions
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        let tx_seen = metrics.tx_seen.load(std::sync::atomic::Ordering::Relaxed);
        println!("Processed {} transactions in burst test", tx_seen);
        
        // Should have processed many transactions
        assert!(tx_seen > 0);
        
        sniffer.stop();
        
        // Drain remaining candidates
        while rx.try_recv().is_ok() {}
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore] // Run only when explicitly requested
    async fn test_sustained_load() {
        let config = SnifferConfig::default();
        let sniffer = Sniffer::new(config);
        let metrics = sniffer.get_metrics();
        
        let mut rx = sniffer.start().await.expect("Failed to start");
        
        // Run for 10 seconds
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        let tx_seen = metrics.tx_seen.load(std::sync::atomic::Ordering::Relaxed);
        let tx_filtered = metrics.tx_filtered.load(std::sync::atomic::Ordering::Relaxed);
        
        println!("Sustained load: {} seen, {} filtered", tx_seen, tx_filtered);
        
        sniffer.stop();
        
        // Drain
        while rx.try_recv().is_ok() {}
    }
}
