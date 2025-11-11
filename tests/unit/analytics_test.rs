//! Unit test for analytics module

#[cfg(test)]
mod analytics_tests {
    use ultra::sniffer::analytics::PredictiveAnalytics;

    #[test]
    fn test_analytics_creation() {
        let analytics = PredictiveAnalytics::new(0.2, 0.05, 1.5);
        
        let (short, long) = analytics.get_ema_values();
        assert_eq!(short, 0.0);
        assert_eq!(long, 0.0);
    }

    #[test]
    fn test_volume_accumulation() {
        let analytics = PredictiveAnalytics::new(0.2, 0.05, 1.5);
        
        // Accumulate some volume
        analytics.accumulate_volume(100.0);
        analytics.accumulate_volume(150.0);
        analytics.accumulate_volume(200.0);
        
        // Update EMAs
        analytics.update_ema();
        
        let (short, long) = analytics.get_ema_values();
        assert!(short > 0.0);
        assert!(long > 0.0);
    }

    #[test]
    fn test_priority_classification() {
        let analytics = PredictiveAnalytics::new(0.2, 0.05, 1.5);
        
        // Initialize with some baseline
        analytics.accumulate_volume(100.0);
        analytics.update_ema();
        
        // High volume should be high priority
        assert!(analytics.is_high_priority(200.0));
        
        // Low volume should be low priority
        assert!(!analytics.is_high_priority(50.0));
    }
}
