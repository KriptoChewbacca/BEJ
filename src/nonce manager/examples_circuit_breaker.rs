//! Example usage of enhanced error handling with circuit breakers
//! 
//! This example demonstrates:
//! - Creating and using a CircuitBreaker
//! - Error classification with ErrorClassifier
//! - Enhanced retry logic with retry_with_backoff_enhanced
//! - Global circuit breaker coordination

use super::nonce_retry::{
    CircuitBreaker, GlobalCircuitBreaker, ErrorClassifier,
    RetryConfig, retry_with_backoff_enhanced,
};
use super::nonce_errors::{NonceError, NonceResult};

/// Example 1: Basic circuit breaker usage
pub async fn example_circuit_breaker() {
    // Create circuit breaker with 3 failure threshold, 2 success threshold, 30s timeout
    let breaker = CircuitBreaker::new(3, 2, std::time::Duration::from_secs(30));
    
    // Check if we can execute
    if breaker.can_execute().await {
        // Perform operation
        match perform_operation().await {
            Ok(_) => breaker.record_success().await,
            Err(_) => breaker.record_failure().await,
        }
    } else {
        println!("Circuit breaker is open, skipping operation");
    }
}

/// Example 2: Error classification
pub async fn example_error_classification() {
    let classifier = ErrorClassifier::new(100, 5);
    
    // Classify an error
    let error = NonceError::Timeout(1000);
    let classification = classifier.classify_error(&error).await;
    
    println!("Error type: {:?}", classification.error_type);
    println!("Confidence: {:.2}", classification.confidence);
    println!("Is transient: {}", classification.is_transient);
    println!("Should taint: {}", classification.should_taint);
}

/// Example 3: Enhanced retry with circuit breaker
pub async fn example_enhanced_retry() {
    let config = RetryConfig::default();
    let breaker = CircuitBreaker::default_thresholds();
    let classifier = ErrorClassifier::new(100, 5);
    
    let result = retry_with_backoff_enhanced(
        "my_operation",
        &config,
        Some(&breaker),
        Some(&classifier),
        || async {
            // Your operation here
            perform_operation().await
        }
    ).await;
    
    match result {
        Ok(_) => println!("Operation succeeded"),
        Err(e) => println!("Operation failed: {}", e),
    }
}

/// Example 4: Global circuit breaker coordination
pub async fn example_global_breaker() {
    let global = GlobalCircuitBreaker::new();
    
    // Get breaker for an endpoint
    let breaker1 = global.get_breaker("endpoint1").await;
    let breaker2 = global.get_breaker("endpoint2").await;
    
    // Record failures
    for _ in 0..3 {
        breaker1.record_failure().await;
        breaker2.record_failure().await;
    }
    
    // Check if global threshold exceeded (>50% endpoints open)
    if global.should_trip_global().await {
        println!("Global circuit breaker triggered - system-wide issue detected");
    }
    
    // Mark endpoint as tainted for security violations
    global.mark_tainted("endpoint1").await;
    
    if global.is_tainted("endpoint1").await {
        println!("Endpoint is tainted, avoiding it");
    }
}

// Helper function for examples
async fn perform_operation() -> NonceResult<()> {
    // Simulate some operation
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_examples_compile() {
        // Just ensure examples compile and can be called
        // Actual functionality is tested in nonce_retry tests
        example_circuit_breaker().await;
        example_error_classification().await;
        example_enhanced_retry().await;
        example_global_breaker().await;
    }
}
