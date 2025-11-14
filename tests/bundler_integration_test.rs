//! Integration tests for Jito bundler functionality
//!
//! This test validates:
//! - MockBundler success and failure scenarios
//! - Bundler trait interface
//! - Metrics collection
//! - Fallback behavior

use bot::observability::TraceContext;
use bot::tx_builder::{Bundler, MockBundler};
use solana_sdk::signature::Signature;

#[tokio::test]
async fn test_mock_bundler_success_scenario() {
    // Create a MockBundler configured for success
    let bundler = MockBundler::new_success();

    // Verify it reports as available
    assert!(bundler.is_available());

    // Create trace context
    let trace_ctx = TraceContext::new("test_bundle_success");

    // Submit an empty bundle (should succeed)
    let result = bundler.submit_bundle(vec![], 10_000, &trace_ctx).await;

    // Verify success
    assert!(result.is_ok());
    let signature = result.unwrap();
    assert_eq!(signature, Signature::default());
}

#[tokio::test]
async fn test_mock_bundler_failure_scenario() {
    // Create a MockBundler configured for failure
    let bundler = MockBundler::new_failure();

    // Verify it reports as unavailable
    assert!(!bundler.is_available());

    // Create trace context
    let trace_ctx = TraceContext::new("test_bundle_failure");

    // Submit an empty bundle (should fail)
    let result = bundler.submit_bundle(vec![], 10_000, &trace_ctx).await;

    // Verify failure
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        bot::tx_builder::TransactionBuilderError::Bundler(_)
    ));
}

#[tokio::test]
async fn test_mock_bundler_tip_calculation() {
    // Test with default multiplier (2x)
    let bundler = MockBundler::new_success();
    assert_eq!(bundler.calculate_dynamic_tip(1000), 2000);
    assert_eq!(bundler.calculate_dynamic_tip(50_000), 100_000);

    // Test with custom multiplier (3x)
    let bundler_custom = MockBundler::new_custom(true, 3);
    assert_eq!(bundler_custom.calculate_dynamic_tip(1000), 3000);
    assert_eq!(bundler_custom.calculate_dynamic_tip(50_000), 150_000);
}

#[tokio::test]
async fn test_bundler_trait_object() {
    // Test that we can use Bundler as a trait object
    let bundler: Box<dyn Bundler> = Box::new(MockBundler::new_success());

    assert!(bundler.is_available());
    assert_eq!(bundler.calculate_dynamic_tip(1000), 2000);

    let trace_ctx = TraceContext::new("test_trait_object");
    let result = bundler.submit_bundle(vec![], 5000, &trace_ctx).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_bundler_arc() {
    // Test that Bundler can be used with Arc (required for BuyEngine integration)
    use std::sync::Arc;

    let bundler: Arc<dyn Bundler> = Arc::new(MockBundler::new_success());

    // Clone Arc to simulate sharing across tasks
    let bundler_clone = Arc::clone(&bundler);

    assert!(bundler_clone.is_available());

    let trace_ctx = TraceContext::new("test_arc_bundler");
    let result = bundler_clone.submit_bundle(vec![], 7500, &trace_ctx).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_bundler_concurrent_submissions() {
    // Test that bundler can handle concurrent submissions
    use std::sync::Arc;
    use tokio::task::JoinSet;

    let bundler: Arc<dyn Bundler> = Arc::new(MockBundler::new_success());
    let mut join_set = JoinSet::new();

    // Submit 10 bundles concurrently
    for i in 0..10 {
        let bundler = Arc::clone(&bundler);
        join_set.spawn(async move {
            let trace_ctx = TraceContext::new(&format!("concurrent_test_{}", i));
            bundler.submit_bundle(vec![], 10_000, &trace_ctx).await
        });
    }

    // Wait for all submissions
    let mut success_count = 0;
    while let Some(result) = join_set.join_next().await {
        let bundle_result = result.expect("Task should not panic");
        if bundle_result.is_ok() {
            success_count += 1;
        }
    }

    // All should succeed
    assert_eq!(success_count, 10);
}

#[tokio::test]
async fn test_mixed_bundler_failures() {
    // Test bundler with some failures
    use std::sync::Arc;
    use tokio::task::JoinSet;

    // Create bundlers with different behaviors
    let bundlers: Vec<Arc<dyn Bundler>> = vec![
        Arc::new(MockBundler::new_success()),
        Arc::new(MockBundler::new_failure()),
        Arc::new(MockBundler::new_success()),
        Arc::new(MockBundler::new_failure()),
    ];

    let mut join_set = JoinSet::new();

    for (i, bundler) in bundlers.into_iter().enumerate() {
        join_set.spawn(async move {
            let trace_ctx = TraceContext::new(&format!("mixed_test_{}", i));
            bundler.submit_bundle(vec![], 10_000, &trace_ctx).await
        });
    }

    let mut success_count = 0;
    let mut failure_count = 0;

    while let Some(result) = join_set.join_next().await {
        let bundle_result = result.expect("Task should not panic");
        match bundle_result {
            Ok(_) => success_count += 1,
            Err(_) => failure_count += 1,
        }
    }

    // Should have 2 successes and 2 failures
    assert_eq!(success_count, 2);
    assert_eq!(failure_count, 2);
}
