//! Unit tests for observability and metrics (Task 5)

#[cfg(test)]
mod tests {
    use bot::metrics::{metrics, MetricsExporter, Timer};
    use bot::observability::TraceContext;
    use std::time::Duration;

    #[test]
    fn test_trace_context_creation() {
        let ctx = TraceContext::new("test_operation");
        
        assert!(!ctx.trace_id().is_empty());
        assert!(!ctx.span_id().is_empty());
        assert!(!ctx.correlation_id().as_str().is_empty());
        assert_eq!(ctx.operation, "test_operation");
        assert!(ctx.parent_span_id.is_none());
    }

    #[test]
    fn test_trace_context_child_span() {
        let parent_ctx = TraceContext::new("parent_operation");
        let child_ctx = parent_ctx.child_span("child_operation");
        
        // Child should inherit trace_id and correlation_id
        assert_eq!(child_ctx.trace_id(), parent_ctx.trace_id());
        assert_eq!(
            child_ctx.correlation_id().as_str(),
            parent_ctx.correlation_id().as_str()
        );
        
        // Child should have different span_id
        assert_ne!(child_ctx.span_id(), parent_ctx.span_id());
        
        // Child should reference parent
        assert_eq!(
            child_ctx.parent_span_id.as_ref().unwrap(),
            parent_ctx.span_id()
        );
        
        // Child should have correct operation name
        assert_eq!(child_ctx.operation, "child_operation");
    }

    #[test]
    fn test_metrics_counters_exist() {
        let m = metrics();
        
        // Verify Task 5 counters are accessible
        let before = m.total_acquires.get();
        m.total_acquires.inc();
        assert_eq!(m.total_acquires.get(), before + 1);
        
        let before = m.total_releases.get();
        m.total_releases.inc();
        assert_eq!(m.total_releases.get(), before + 1);
        
        let before = m.total_refreshes.get();
        m.total_refreshes.inc();
        assert_eq!(m.total_refreshes.get(), before + 1);
        
        let before = m.total_failures.get();
        m.total_failures.inc();
        assert_eq!(m.total_failures.get(), before + 1);
    }

    #[test]
    fn test_metrics_histograms_record() {
        let m = metrics();
        
        // Test that histograms accept observations
        m.acquire_lease_ms.observe(5.0);
        m.prepare_bundle_ms.observe(10.0);
        m.build_to_land_ms.observe(50.0);
        
        // Verify sample counts increased (basic smoke test)
        assert!(m.acquire_lease_ms.get_sample_count() > 0);
        assert!(m.prepare_bundle_ms.get_sample_count() > 0);
        assert!(m.build_to_land_ms.get_sample_count() > 0);
    }

    #[test]
    fn test_timer_with_histogram() {
        // Test Timer integration with new histograms
        let timer = Timer::with_name("acquire_lease_ms");
        std::thread::sleep(Duration::from_millis(1));
        
        let before_count = metrics().acquire_lease_ms.get_sample_count();
        timer.finish();
        
        // Verify observation was recorded
        assert!(metrics().acquire_lease_ms.get_sample_count() > before_count);
    }

    #[test]
    fn test_metrics_exporter_json_format() {
        let exporter = MetricsExporter::default_interval();
        
        // Export should produce valid JSON
        let json_result = exporter.export_json();
        assert!(json_result.is_ok());
        
        let json_str = json_result.unwrap();
        
        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        
        // Verify structure contains expected fields
        assert!(parsed["timestamp"].is_number());
        assert!(parsed["metrics"]["counters"].is_object());
        assert!(parsed["metrics"]["gauges"].is_object());
        assert!(parsed["metrics"]["prometheus_format"].is_string());
        
        // Verify Task 5 counters are present
        assert!(parsed["metrics"]["counters"]["total_acquires"].is_number());
        assert!(parsed["metrics"]["counters"]["total_releases"].is_number());
        assert!(parsed["metrics"]["counters"]["total_refreshes"].is_number());
        assert!(parsed["metrics"]["counters"]["total_failures"].is_number());
    }

    #[test]
    fn test_metrics_exporter_custom_interval() {
        let exporter = MetricsExporter::new(Duration::from_secs(30));
        
        // Should be able to create with custom interval
        assert_eq!(exporter.interval, Duration::from_secs(30));
    }

    #[test]
    fn test_trace_context_serialization() {
        let ctx = TraceContext::new("test_op");
        
        // Verify it can be serialized to JSON
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(!json.is_empty());
        
        // Verify it can be deserialized
        let deserialized: TraceContext = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.trace_id, ctx.trace_id);
        assert_eq!(deserialized.span_id, ctx.span_id);
        assert_eq!(deserialized.operation, ctx.operation);
    }

    #[test]
    fn test_multiple_histogram_observations() {
        let m = metrics();
        
        // Record multiple observations
        for i in 1..=10 {
            m.acquire_lease_ms.observe(i as f64);
        }
        
        // Verify sample count
        assert!(m.acquire_lease_ms.get_sample_count() >= 10);
        
        // Verify sum is positive
        assert!(m.acquire_lease_ms.get_sample_sum() > 0.0);
    }
}
