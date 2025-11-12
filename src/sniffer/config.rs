//! Configuration module for Sniffer with TOML and environment variable support

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::sync::watch;

/// Drop policy for when channel is full
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DropPolicy {
    /// Drop oldest items when channel is full
    DropOldest,
    /// Drop newest items when channel is full (default)
    DropNewest,
    /// Block until space is available (use with caution)
    Block,
}

/// Batch send mode configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchSendMode {
    /// Synchronous mode - simple await (current behavior)
    Sync,
    /// Asynchronous mode - spawn workers with try_send
    Async,
}

/// Sniffer configuration with magic constants and default values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnifferConfig {
    /// gRPC endpoint URL for Geyser stream
    pub grpc_endpoint: String,

    /// Channel capacity for handoff to buy_engine
    pub channel_capacity: usize,

    /// Stream buffer size for gRPC
    pub stream_buffer_size: usize,

    /// Maximum retry attempts for stream connection
    pub max_retry_attempts: u32,

    /// Initial backoff duration for retries (milliseconds)
    pub initial_backoff_ms: u64,

    /// Maximum backoff duration for retries (milliseconds)
    pub max_backoff_ms: u64,

    /// Telemetry export interval in seconds
    pub telemetry_interval_secs: u64,

    /// EMA alpha for short window (0.0-1.0)
    pub ema_alpha_short: f64,

    /// EMA alpha for long window (0.0-1.0)
    pub ema_alpha_long: f64,

    /// Initial threshold for priority classification
    pub initial_threshold: f64,

    /// Batch size for candidate processing
    pub batch_size: usize,

    /// Batch timeout in milliseconds
    pub batch_timeout_ms: u64,

    /// Maximum retries for HIGH priority candidates
    pub high_priority_max_retries: u8,

    /// EMA update interval in milliseconds for analytics_updater task
    pub ema_update_interval_ms: u64,

    /// Threshold update rate (0.0-1.0, how fast threshold adapts)
    pub threshold_update_rate: f64,

    /// Enable safe offset validation for mint/account extraction
    pub safe_offsets: bool,

    /// Maximum retries for send operations
    pub send_max_retries: u8,

    /// Retry delay in microseconds for send operations
    pub send_retry_delay_us: u64,

    /// Stream buffer capacity (internal buffer before handoff)
    pub stream_buffer_capacity: usize,

    /// Drop policy when channel is full
    pub drop_policy: DropPolicy,

    /// Batch send mode (sync or async)
    pub batch_send_mode: BatchSendMode,

    /// Graceful shutdown timeout in milliseconds
    pub graceful_shutdown_timeout_ms: u64,

    /// Configuration file path for hot reload
    pub config_file_path: String,

    /// Adaptive policy high congestion threshold (microseconds)
    pub adaptive_policy_high_threshold_us: f64,

    /// Adaptive policy low congestion threshold (microseconds)
    pub adaptive_policy_low_threshold_us: f64,
}

impl Default for SnifferConfig {
    fn default() -> Self {
        Self {
            grpc_endpoint: "http://127.0.0.1:10000".to_string(),
            channel_capacity: 1024,
            stream_buffer_size: 4096,
            max_retry_attempts: 5,
            initial_backoff_ms: 100,
            max_backoff_ms: 5000,
            telemetry_interval_secs: 5,
            ema_alpha_short: 0.2,
            ema_alpha_long: 0.05,
            initial_threshold: 1.5,
            batch_size: 10,
            batch_timeout_ms: 10,
            high_priority_max_retries: 2,
            ema_update_interval_ms: 200,
            threshold_update_rate: 0.1,
            safe_offsets: true,
            send_max_retries: 3,
            send_retry_delay_us: 100,
            stream_buffer_capacity: 2048,
            drop_policy: DropPolicy::DropNewest,
            batch_send_mode: BatchSendMode::Sync,
            graceful_shutdown_timeout_ms: 5000,
            config_file_path: "sniffer_config.toml".to_string(),
            adaptive_policy_high_threshold_us: 1000.0,
            adaptive_policy_low_threshold_us: 100.0,
        }
    }
}

impl SnifferConfig {
    /// Load configuration from TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = std::fs::read_to_string(path.as_ref())
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;

        let config: SnifferConfig =
            toml::from_str(&contents).map_err(|e| anyhow!("Failed to parse TOML config: {}", e))?;

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from environment variables
    /// Environment variables override TOML values
    pub fn from_env(&mut self) -> Result<()> {
        if let Ok(endpoint) = std::env::var("SNIFFER_GRPC_ENDPOINT") {
            self.grpc_endpoint = endpoint;
        }

        if let Ok(capacity) = std::env::var("SNIFFER_CHANNEL_CAPACITY") {
            self.channel_capacity = capacity
                .parse()
                .map_err(|e| anyhow!("Invalid SNIFFER_CHANNEL_CAPACITY: {}", e))?;
        }

        if let Ok(size) = std::env::var("SNIFFER_STREAM_BUFFER_SIZE") {
            self.stream_buffer_size = size
                .parse()
                .map_err(|e| anyhow!("Invalid SNIFFER_STREAM_BUFFER_SIZE: {}", e))?;
        }

        if let Ok(retries) = std::env::var("SNIFFER_MAX_RETRY_ATTEMPTS") {
            self.max_retry_attempts = retries
                .parse()
                .map_err(|e| anyhow!("Invalid SNIFFER_MAX_RETRY_ATTEMPTS: {}", e))?;
        }

        if let Ok(alpha_short) = std::env::var("SNIFFER_EMA_ALPHA_SHORT") {
            self.ema_alpha_short = alpha_short
                .parse()
                .map_err(|e| anyhow!("Invalid SNIFFER_EMA_ALPHA_SHORT: {}", e))?;
        }

        if let Ok(alpha_long) = std::env::var("SNIFFER_EMA_ALPHA_LONG") {
            self.ema_alpha_long = alpha_long
                .parse()
                .map_err(|e| anyhow!("Invalid SNIFFER_EMA_ALPHA_LONG: {}", e))?;
        }

        self.validate()?;
        Ok(())
    }

    /// Validate configuration parameters
    pub fn validate(&self) -> Result<()> {
        if self.channel_capacity == 0 {
            return Err(anyhow!("channel_capacity must be > 0"));
        }
        if self.stream_buffer_size == 0 {
            return Err(anyhow!("stream_buffer_size must be > 0"));
        }
        if self.ema_alpha_short < 0.0 || self.ema_alpha_short > 1.0 {
            return Err(anyhow!("ema_alpha_short must be in range [0.0, 1.0]"));
        }
        if self.ema_alpha_long < 0.0 || self.ema_alpha_long > 1.0 {
            return Err(anyhow!("ema_alpha_long must be in range [0.0, 1.0]"));
        }
        if self.batch_size == 0 {
            return Err(anyhow!("batch_size must be > 0"));
        }
        if self.ema_update_interval_ms == 0 {
            return Err(anyhow!("ema_update_interval_ms must be > 0"));
        }
        if self.threshold_update_rate < 0.0 || self.threshold_update_rate > 1.0 {
            return Err(anyhow!("threshold_update_rate must be in range [0.0, 1.0]"));
        }
        if self.stream_buffer_capacity == 0 {
            return Err(anyhow!("stream_buffer_capacity must be > 0"));
        }
        if self.graceful_shutdown_timeout_ms == 0 {
            return Err(anyhow!("graceful_shutdown_timeout_ms must be > 0"));
        }
        if self.adaptive_policy_high_threshold_us <= 0.0 {
            return Err(anyhow!("adaptive_policy_high_threshold_us must be > 0"));
        }
        if self.adaptive_policy_low_threshold_us <= 0.0 {
            return Err(anyhow!("adaptive_policy_low_threshold_us must be > 0"));
        }
        if self.adaptive_policy_low_threshold_us >= self.adaptive_policy_high_threshold_us {
            return Err(anyhow!(
                "adaptive_policy_low_threshold_us must be < adaptive_policy_high_threshold_us"
            ));
        }
        Ok(())
    }

    /// Create configuration from default with environment overrides
    pub fn with_env_overrides() -> Result<Self> {
        let mut config = Self::default();
        config.from_env()?;
        Ok(config)
    }

    /// Watch configuration file for changes and send updates
    /// Returns a receiver that will get notified when config changes
    pub fn watch_config(
        path: String,
    ) -> (watch::Sender<SnifferConfig>, watch::Receiver<SnifferConfig>) {
        let initial_config = Self::from_file(&path).unwrap_or_default();
        let (tx, rx) = watch::channel(initial_config.clone());
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            use tokio::time::{interval, Duration};
            let mut check_interval = interval(Duration::from_secs(5));
            let mut last_modified = std::fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok());

            loop {
                check_interval.tick().await;

                // Check if file was modified
                if let Ok(metadata) = std::fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        if last_modified.map_or(true, |last| modified > last) {
                            last_modified = Some(modified);

                            // Try to reload config
                            if let Ok(new_config) = Self::from_file(&path) {
                                tracing::info!("Configuration reloaded from {}", path);
                                let _ = tx_clone.send(new_config);
                            } else {
                                tracing::warn!("Failed to reload configuration from {}", path);
                            }
                        }
                    }
                }
            }
        });

        (tx, rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_valid() {
        let config = SnifferConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_channel_capacity() {
        let mut config = SnifferConfig::default();
        config.channel_capacity = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_ema_alpha() {
        let mut config = SnifferConfig::default();
        config.ema_alpha_short = 1.5;
        assert!(config.validate().is_err());

        config.ema_alpha_short = -0.1;
        assert!(config.validate().is_err());
    }
}
