//! Structured logging and pipeline context

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Pipeline execution context for distributed tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl PipelineContext {
    /// Create a new pipeline context
    pub fn new(operation: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            request_id: Uuid::new_v4().to_string(),
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: None,
            operation: operation.to_string(),
            timestamp: now,
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
        }
    }
}

impl Default for PipelineContext {
    fn default() -> Self {
        Self::new("default")
    }
}
