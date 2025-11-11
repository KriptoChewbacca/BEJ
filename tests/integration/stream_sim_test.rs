//! Integration test for stream simulation

#[cfg(test)]
mod stream_sim_tests {
    use ultra::sniffer::{Sniffer, SnifferApi, SnifferConfig};
    use tokio::time::{timeout, Duration};

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_stream_start_stop() {
        let config = SnifferConfig::default();
        let sniffer = Sniffer::new(config);
        
        // Start the sniffer
        let rx = timeout(Duration::from_secs(2), sniffer.start())
            .await
            .expect("Timeout starting sniffer")
            .expect("Failed to start sniffer");
        
        assert!(sniffer.is_running());
        
        // Let it run briefly
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Stop the sniffer
        sniffer.stop();
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        assert!(!sniffer.is_running());
        
        drop(rx);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_pause_resume() {
        let config = SnifferConfig::default();
        let sniffer = Sniffer::new(config);
        
        let _rx = sniffer.start().await.expect("Failed to start");
        
        // Pause
        sniffer.pause();
        assert!(sniffer.is_paused());
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Resume
        sniffer.resume();
        assert!(!sniffer.is_paused());
        
        sniffer.stop();
    }
}
