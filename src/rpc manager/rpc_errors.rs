use std::fmt;
use solana_client::client_error::ClientError;
use thiserror::Error;

/// Comprehensive RPC Manager error types
#[derive(Debug, Clone, Error)]
pub enum RpcManagerError {
    /// Transport-level errors (network, connection)
    #[error("Transport error: {message} (endpoint: {endpoint})")]
    Transport {
        endpoint: String,
        message: String,
        #[source]
        source: Option<Box<RpcManagerError>>,
    },
    
    /// Timeout errors
    #[error("Timeout after {timeout_ms}ms (endpoint: {endpoint})")]
    Timeout {
        endpoint: String,
        timeout_ms: u64,
    },
    
    /// RPC response errors (from the RPC server)
    #[error("RPC response error: {message} (endpoint: {endpoint}, code: {code:?})")]
    RpcResponse {
        endpoint: String,
        message: String,
        code: Option<i64>,
    },
    
    /// Nonce pool exhausted
    #[error("Nonce pool exhausted (available: {available}, required: {required})")]
    NonceExhausted {
        available: usize,
        required: usize,
    },
    
    /// Fatal errors that should not be retried
    #[error("Fatal error: {0}")]
    Fatal(String),
    
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    /// Circuit breaker is open
    #[error("Circuit breaker open for {tier:?} (failures: {failure_count})")]
    CircuitBreakerOpen {
        tier: String,
        failure_count: u32,
    },
    
    /// Rate limit exceeded
    #[error("Rate limit exceeded (endpoint: {endpoint})")]
    RateLimitExceeded {
        endpoint: String,
    },
    
    /// No healthy endpoints available
    #[error("No healthy endpoints available (total: {total}, unhealthy: {unhealthy})")]
    NoHealthyEndpoints {
        total: usize,
        unhealthy: usize,
    },
    
    /// Specific Solana errors
    #[error("Blockhash not found (endpoint: {endpoint})")]
    BlockhashNotFound {
        endpoint: String,
    },
    
    #[error("Transaction expired (endpoint: {endpoint})")]
    TransactionExpired {
        endpoint: String,
    },
    
    #[error("Account not found: {account} (endpoint: {endpoint})")]
    AccountNotFound {
        account: String,
        endpoint: String,
    },
    
    #[error("Insufficient funds (endpoint: {endpoint})")]
    InsufficientFunds {
        endpoint: String,
    },
    
    /// Validation errors
    #[error("Validation error: {0}")]
    Validation(String),
    
    /// Internal errors
    #[error("Internal error: {0}")]
    Internal(String),
}

impl RpcManagerError {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            // Retryable errors
            RpcManagerError::Transport { .. } => true,
            RpcManagerError::Timeout { .. } => true,
            RpcManagerError::RateLimitExceeded { .. } => true,
            RpcManagerError::NoHealthyEndpoints { .. } => true,
            RpcManagerError::BlockhashNotFound { .. } => true,
            
            // Non-retryable errors
            RpcManagerError::Fatal(_) => false,
            RpcManagerError::Configuration(_) => false,
            RpcManagerError::CircuitBreakerOpen { .. } => false,
            RpcManagerError::TransactionExpired { .. } => false,
            RpcManagerError::AccountNotFound { .. } => false,
            RpcManagerError::InsufficientFunds { .. } => false,
            RpcManagerError::Validation(_) => false,
            RpcManagerError::NonceExhausted { .. } => false,
            
            // RPC response errors may or may not be retryable
            RpcManagerError::RpcResponse { code, .. } => {
                // Retry on server errors (5xx)
                if let Some(c) = code {
                    *c >= 500 && *c < 600
                } else {
                    false
                }
            }
            
            RpcManagerError::Internal(_) => false,
        }
    }
    
    /// Check if this endpoint should be blacklisted after this error
    pub fn should_blacklist(&self) -> bool {
        match self {
            // Blacklist on persistent transport/timeout errors
            RpcManagerError::Transport { .. } => false, // Will be handled by circuit breaker
            RpcManagerError::Timeout { .. } => false,
            
            // Don't blacklist on rate limiting - just back off
            RpcManagerError::RateLimitExceeded { .. } => false,
            
            // These are application-level errors, not endpoint issues
            RpcManagerError::TransactionExpired { .. } => false,
            RpcManagerError::AccountNotFound { .. } => false,
            RpcManagerError::InsufficientFunds { .. } => false,
            RpcManagerError::BlockhashNotFound { .. } => false,
            
            // Fatal and config errors indicate code problems
            RpcManagerError::Fatal(_) => false,
            RpcManagerError::Configuration(_) => false,
            
            // Circuit breaker already handles blacklisting
            RpcManagerError::CircuitBreakerOpen { .. } => false,
            
            // These shouldn't result in blacklisting
            RpcManagerError::NoHealthyEndpoints { .. } => false,
            RpcManagerError::NonceExhausted { .. } => false,
            RpcManagerError::Validation(_) => false,
            RpcManagerError::Internal(_) => false,
            
            RpcManagerError::RpcResponse { code, .. } => {
                // Blacklist on persistent server errors
                if let Some(c) = code {
                    *c >= 500 && *c < 600
                } else {
                    false
                }
            }
        }
    }
    
    /// Get the endpoint associated with this error, if any
    pub fn endpoint(&self) -> Option<&str> {
        match self {
            RpcManagerError::Transport { endpoint, .. } => Some(endpoint),
            RpcManagerError::Timeout { endpoint, .. } => Some(endpoint),
            RpcManagerError::RpcResponse { endpoint, .. } => Some(endpoint),
            RpcManagerError::RateLimitExceeded { endpoint } => Some(endpoint),
            RpcManagerError::BlockhashNotFound { endpoint } => Some(endpoint),
            RpcManagerError::TransactionExpired { endpoint } => Some(endpoint),
            RpcManagerError::AccountNotFound { endpoint, .. } => Some(endpoint),
            RpcManagerError::InsufficientFunds { endpoint } => Some(endpoint),
            _ => None,
        }
    }
    
    /// Create from ClientError with context
    pub fn from_client_error(err: ClientError, endpoint: &str) -> Self {
        let err_str = err.to_string().to_lowercase();
        
        // Classify based on error message
        if err_str.contains("blockhash not found") {
            RpcManagerError::BlockhashNotFound {
                endpoint: endpoint.to_string(),
            }
        } else if err_str.contains("transaction expired")
            || err_str.contains("block height exceeded")
        {
            RpcManagerError::TransactionExpired {
                endpoint: endpoint.to_string(),
            }
        } else if err_str.contains("account not found") {
            RpcManagerError::AccountNotFound {
                account: "unknown".to_string(),
                endpoint: endpoint.to_string(),
            }
        } else if err_str.contains("insufficient funds")
            || err_str.contains("insufficient lamports")
        {
            RpcManagerError::InsufficientFunds {
                endpoint: endpoint.to_string(),
            }
        } else if err_str.contains("rate limit")
            || err_str.contains("too many requests")
            || err_str.contains("429")
        {
            RpcManagerError::RateLimitExceeded {
                endpoint: endpoint.to_string(),
            }
        } else if err_str.contains("timeout") || err_str.contains("timed out") {
            RpcManagerError::Timeout {
                endpoint: endpoint.to_string(),
                timeout_ms: 5000,
            }
        } else {
            // Extract error code if available
            let code = err_str
                .split("code:")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse::<i64>().ok());
            
            RpcManagerError::RpcResponse {
                endpoint: endpoint.to_string(),
                message: err.to_string(),
                code,
            }
        }
    }
}

/// Retry policy for RPC operations
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    
    /// Base delay in milliseconds
    pub base_delay_ms: u64,
    
    /// Maximum delay in milliseconds
    pub max_delay_ms: u64,
    
    /// Jitter factor (0.0 - 1.0)
    pub jitter_factor: f64,
    
    /// Multiplier for exponential backoff
    pub multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
            jitter_factor: 0.1,
            multiplier: 2.0,
        }
    }
}

impl RetryPolicy {
    /// Calculate delay for a given attempt number
    pub fn calculate_delay(&self, attempt: u32) -> Option<std::time::Duration> {
        if attempt >= self.max_attempts {
            return None;
        }
        
        // Exponential backoff
        let delay_ms = self.base_delay_ms as f64
            * self.multiplier.powi(attempt as i32);
        let delay_ms = delay_ms.min(self.max_delay_ms as f64);
        
        // Add jitter to prevent thundering herd
        let jitter = (rand::random::<f64>() - 0.5) * 2.0 * self.jitter_factor;
        let jittered_delay = (delay_ms * (1.0 + jitter)).max(0.0) as u64;
        
        Some(std::time::Duration::from_millis(jittered_delay))
    }
    
    /// Create a retry policy for aggressive retries
    pub fn aggressive() -> Self {
        Self {
            max_attempts: 5,
            base_delay_ms: 50,
            max_delay_ms: 2000,
            jitter_factor: 0.15,
            multiplier: 1.5,
        }
    }
    
    /// Create a retry policy for conservative retries
    pub fn conservative() -> Self {
        Self {
            max_attempts: 2,
            base_delay_ms: 200,
            max_delay_ms: 10000,
            jitter_factor: 0.05,
            multiplier: 3.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_is_retryable() {
        assert!(RpcManagerError::Transport {
            endpoint: "test".to_string(),
            message: "connection failed".to_string(),
            source: None,
        }.is_retryable());
        
        assert!(RpcManagerError::Timeout {
            endpoint: "test".to_string(),
            timeout_ms: 5000,
        }.is_retryable());
        
        assert!(!RpcManagerError::Fatal("test".to_string()).is_retryable());
        assert!(!RpcManagerError::TransactionExpired {
            endpoint: "test".to_string(),
        }.is_retryable());
    }
    
    #[test]
    fn test_error_should_blacklist() {
        // Most errors should not result in immediate blacklisting
        assert!(!RpcManagerError::RateLimitExceeded {
            endpoint: "test".to_string(),
        }.should_blacklist());
        
        assert!(!RpcManagerError::TransactionExpired {
            endpoint: "test".to_string(),
        }.should_blacklist());
    }
    
    #[test]
    fn test_error_endpoint() {
        let err = RpcManagerError::Timeout {
            endpoint: "https://test.com".to_string(),
            timeout_ms: 5000,
        };
        
        assert_eq!(err.endpoint(), Some("https://test.com"));
        
        let fatal_err = RpcManagerError::Fatal("test".to_string());
        assert_eq!(fatal_err.endpoint(), None);
    }
    
    #[test]
    fn test_retry_policy_delay() {
        let policy = RetryPolicy::default();
        
        // First attempt
        let delay1 = policy.calculate_delay(0);
        assert!(delay1.is_some());
        
        // Second attempt should be longer
        let delay2 = policy.calculate_delay(1);
        assert!(delay2.is_some());
        assert!(delay2.unwrap() >= delay1.unwrap());
        
        // Beyond max attempts
        let delay_none = policy.calculate_delay(10);
        assert!(delay_none.is_none());
    }
    
    #[test]
    fn test_retry_policy_variants() {
        let aggressive = RetryPolicy::aggressive();
        assert_eq!(aggressive.max_attempts, 5);
        assert_eq!(aggressive.base_delay_ms, 50);
        
        let conservative = RetryPolicy::conservative();
        assert_eq!(conservative.max_attempts, 2);
        assert_eq!(conservative.base_delay_ms, 200);
    }
}
