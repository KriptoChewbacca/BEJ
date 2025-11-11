//! Stress test: cold start latency measurement

#[cfg(test)]
mod cold_start_tests {
    use ultra::sniffer::{Sniffer, SnifferApi, SnifferConfig};
    use std::time::Instant;
    use tokio::time::Duration;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore] // Run only when explicitly requested
    async fn test_cold_start_latency() {
        let config = SnifferConfig::default();
        let sniffer = Sniffer::new(config);
        
        let start = Instant::now();
        let mut rx = sniffer.start().await.expect("Failed to start");
        let startup_latency = start.elapsed();
        
        println!("Cold start latency: {:?}", startup_latency);
        
        // Startup should be reasonably fast (< 1 second)
        assert!(startup_latency < Duration::from_secs(1));
        
        // Wait for first transaction
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let metrics = sniffer.get_metrics();
        let tx_seen = metrics.tx_seen.load(std::sync::atomic::Ordering::Relaxed);
        
        println!("Transactions seen after 200ms: {}", tx_seen);
        
        sniffer.stop();
        
        // Drain
        while rx.try_recv().is_ok() {}
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn test_multiple_restarts() {
        for i in 0..3 {
            let config = SnifferConfig::default();
            let sniffer = Sniffer::new(config);
            
            let start = Instant::now();
            let mut rx = sniffer.start().await.expect("Failed to start");
            let startup_latency = start.elapsed();
            
            println!("Restart {} latency: {:?}", i, startup_latency);
            
            // Let it run briefly
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            sniffer.stop();
            tokio::time::sleep(Duration::from_millis(50)).await;
            
            // Drain
            while rx.try_recv().is_ok() {}
        }
    }
}
