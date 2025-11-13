//! Observability module for correlation and tracing

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Correlation ID for tracking requests across components
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CorrelationId(String);

impl CorrelationId {
    /// Create a new correlation ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from existing string
    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    /// Get the inner string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for CorrelationId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for CorrelationId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Task 5: TraceContext for distributed tracing
///
/// Provides trace_id, span_id, and correlation_id for tracking
/// transaction building operations across the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    /// Unique trace identifier for the entire operation
    pub trace_id: String,
    
    /// Unique span identifier for this specific operation
    pub span_id: String,
    
    /// Correlation ID for request tracking
    pub correlation_id: CorrelationId,
    
    /// Optional parent span ID
    pub parent_span_id: Option<String>,
    
    /// Operation name
    pub operation: String,
    
    /// Creation timestamp (Unix epoch seconds)
    pub timestamp: u64,
}

impl TraceContext {
    /// Create a new trace context for an operation
    pub fn new(operation: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string(),
            correlation_id: CorrelationId::new(),
            parent_span_id: None,
            operation: operation.to_string(),
            timestamp: now,
        }
    }
    
    /// Create a child span context
    pub fn child_span(&self, operation: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            trace_id: self.trace_id.clone(),
            span_id: Uuid::new_v4().to_string(),
            correlation_id: self.correlation_id.clone(),
            parent_span_id: Some(self.span_id.clone()),
            operation: operation.to_string(),
            timestamp: now,
        }
    }
    
    /// Get the trace ID
    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }
    
    /// Get the span ID
    pub fn span_id(&self) -> &str {
        &self.span_id
    }
    
    /// Get the correlation ID
    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new("default")
    }
}
