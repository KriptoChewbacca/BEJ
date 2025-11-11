//! Error types and retry policies for the Sniffer module

use std::fmt;
use std::time::Duration;

/// Main error type for Sniffer operations
#[derive(Debug, Clone)]
pub enum SnifferError {
    /// Configuration validation error
    ConfigValidation(String),
    /// Stream connection failed
    StreamConnection(String),
    /// Stream disconnected unexpectedly
    StreamDisconnected,
    /// Retry limit exceeded
    RetryLimitExceeded(u32),
    /// Channel send error
    ChannelSend(String),
    /// Shutdown requested
    ShutdownRequested,
    /// Invalid transaction data
    InvalidTransaction(String),
    /// gRPC error
    GrpcError(String),
    /// Timeout error
    Timeout(String),
}

impl fmt::Display for SnifferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigValidation(msg) => write!(f, "Configuration validation error: {}", msg),
            Self::StreamConnection(msg) => write!(f, "Stream connection error: {}", msg),
            Self::StreamDisconnected => write!(f, "Stream disconnected"),
            Self::RetryLimitExceeded(attempts) => write!(f, "Retry limit exceeded after {} attempts", attempts),
            Self::ChannelSend(msg) => write!(f, "Channel send error: {}", msg),
            Self::ShutdownRequested => write!(f, "Shutdown requested"),
            Self::InvalidTransaction(msg) => write!(f, "Invalid transaction: {}", msg),
            Self::GrpcError(msg) => write!(f, "gRPC error: {}", msg),
            Self::Timeout(msg) => write!(f, "Timeout: {}", msg),
        }
    }
}

impl std::error::Error for SnifferError {}

/// Error type for mint extraction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MintExtractError {
    /// Transaction too small to contain mint data
    TooSmall,
    /// Invalid mint pubkey (all zeros / default)
    InvalidMint,
    /// Extraction offset out of bounds
    OutOfBounds,
    /// Deserialization failed
    DeserializationFailed,
}

impl fmt::Display for MintExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooSmall => write!(f, "Transaction too small"),
            Self::InvalidMint => write!(f, "Invalid mint pubkey"),
            Self::OutOfBounds => write!(f, "Extraction offset out of bounds"),
            Self::DeserializationFailed => write!(f, "Deserialization failed"),
        }
    }
}

impl std::error::Error for MintExtractError {}

/// Error type for account extraction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountExtractError {
    /// Transaction too small to contain account data
    TooSmall,
    /// Invalid account pubkey (all zeros / default)
    InvalidAccount,
    /// Extraction offset out of bounds
    OutOfBounds,
    /// Deserialization failed
    DeserializationFailed,
}

impl fmt::Display for AccountExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooSmall => write!(f, "Transaction too small"),
            Self::InvalidAccount => write!(f, "Invalid account pubkey"),
            Self::OutOfBounds => write!(f, "Extraction offset out of bounds"),
            Self::DeserializationFailed => write!(f, "Deserialization failed"),
        }
    }
}

impl std::error::Error for AccountExtractError {}

/// Exponential backoff with jitter for retry logic
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    current_attempt: u32,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
}

impl ExponentialBackoff {
    /// Create a new exponential backoff strategy
    pub fn new(initial_backoff_ms: u64, max_backoff_ms: u64) -> Self {
        Self {
            current_attempt: 0,
            initial_backoff_ms,
            max_backoff_ms,
        }
    }

    /// Get the next backoff duration with jitter
    pub fn next_backoff(&mut self) -> Duration {
        let backoff_ms = (self.initial_backoff_ms * 2_u64.pow(self.current_attempt))
            .min(self.max_backoff_ms);
        
        self.current_attempt += 1;

        // Add jitter (±20%)
        let jitter = (backoff_ms / 5) as i64;
        let jitter_amount = (rand::random::<i64>() % (2 * jitter)) - jitter;
        let final_backoff = (backoff_ms as i64 + jitter_amount).max(0) as u64;

        Duration::from_millis(final_backoff)
    }

    /// Reset the backoff to initial state
    pub fn reset(&mut self) {
        self.current_attempt = 0;
    }

    /// Get current attempt number
    pub fn attempt(&self) -> u32 {
        self.current_attempt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff() {
        let mut backoff = ExponentialBackoff::new(100, 5000);
        
        // First attempt
        let delay1 = backoff.next_backoff();
        assert!(delay1.as_millis() >= 80 && delay1.as_millis() <= 120); // 100ms ± 20%
        
        // Second attempt should be roughly 2x
        let delay2 = backoff.next_backoff();
        assert!(delay2.as_millis() >= 160 && delay2.as_millis() <= 240); // 200ms ± 20%
        
        // Reset should go back to initial
        backoff.reset();
        let delay3 = backoff.next_backoff();
        assert!(delay3.as_millis() >= 80 && delay3.as_millis() <= 120);
    }

    #[test]
    fn test_backoff_max_limit() {
        let mut backoff = ExponentialBackoff::new(1000, 5000);
        
        // After many attempts, should not exceed max
        for _ in 0..10 {
            let delay = backoff.next_backoff();
            assert!(delay.as_millis() <= 6000); // 5000ms + jitter
        }
    }
}
