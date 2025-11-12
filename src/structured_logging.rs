//! Structured logging and pipeline context

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Structured logger for pipeline events
#[derive(Debug, Clone)]
pub struct StructuredLogger {
    context_id: String,
}

impl StructuredLogger {
    pub fn new(context_id: String) -> Self {
        Self { context_id }
    }
    
    pub fn log_candidate_processed(&self, mint: &str, program: &str, success: bool) {
        tracing::debug!(
            context_id = %self.context_id,
            mint = %mint,
            program = %program,
            success = %success,
            "Candidate processed"
        );
    }
    
    pub fn log_buy_attempt(&self, mint: &str, tx_count: usize) {
        tracing::info!(
            context_id = %self.context_id,
            mint = %mint,
            tx_count = %tx_count,
            "Attempting buy transaction"
        );
    }
    
    pub fn log_buy_success(&self, mint: &str, sig: &str, latency_ms: u64) {
        tracing::info!(
            context_id = %self.context_id,
            mint = %mint,
            signature = %sig,
            latency_ms = %latency_ms,
            "Buy transaction successful"
        );
    }
    
    pub fn log_buy_failure(&self, mint: &str, error: &str, latency_ms: u64) {
        tracing::warn!(
            context_id = %self.context_id,
            mint = %mint,
            error = %error,
            latency_ms = %latency_ms,
            "Buy transaction failed"
        );
    }
    
    pub fn log_sell_operation(&self, mint: &str, sell_percent: f64, new_holdings_percent: f64) {
        tracing::info!(
            context_id = %self.context_id,
            mint = %mint,
            sell_percent = %sell_percent,
            new_holdings_percent = %new_holdings_percent,
            "Sell operation"
        );
    }
    
    pub fn log_nonce_operation(&self, operation: &str, index: Option<usize>, success: bool) {
        tracing::debug!(
            context_id = %self.context_id,
            operation = %operation,
            index = ?index,
            success = %success,
            "Nonce operation"
        );
    }
    
    pub fn warn(&self, message: &str) {
        tracing::warn!(
            context_id = %self.context_id,
            message = %message,
            "Warning"
        );
    }
    
    pub fn error(&self, message: &str) {
        tracing::error!(
            context_id = %self.context_id,
            message = %message,
            "Error"
        );
    }
}

/// Pipeline execution context for distributed tracing
#[derive(Debug, Clone)]
pub struct PipelineContext {
    /// Unique request ID
    pub request_id: String,
    
    /// Trace ID for distributed tracing
    pub trace_id: String,
    
    /// Span ID
    pub span_id: String,
    
    /// Parent span ID (if any)
    pub parent_span_id: Option<String>,
    
    /// Operation name
    pub operation: String,
    
    /// Timestamp
    pub timestamp: u64,
    
    /// Correlation ID (same as request_id for compatibility)
    pub correlation_id: String,
    
    /// Structured logger instance
    pub logger: StructuredLogger,
}

impl PipelineContext {
    /// Create a new pipeline context
    pub fn new(operation: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let request_id = Uuid::new_v4().to_string();
        let correlation_id = request_id.clone();
        
        Self {
            request_id: request_id.clone(),
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: None,
            operation: operation.to_string(),
            timestamp: now,
            correlation_id,
            logger: StructuredLogger::new(request_id),
        }
    }
    
    /// Create a child context
    pub fn child(&self, operation: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            request_id: self.request_id.clone(),
            trace_id: self.trace_id.clone(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: Some(self.span_id.clone()),
            operation: operation.to_string(),
            timestamp: now,
            correlation_id: self.correlation_id.clone(),
            logger: self.logger.clone(),
        }
    }
}

impl Default for PipelineContext {
    fn default() -> Self {
        Self::new("default")
    }
}
