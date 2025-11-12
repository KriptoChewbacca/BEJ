//! Configuration module for the Ultra trading bot
//!
//! This module handles all configuration loading from TOML files,
//! environment variables, and provides structured configuration types.

use serde::{Deserialize, Serialize};

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// RPC endpoints configuration
    pub rpc: RpcConfig,
    
    /// Wallet configuration
    pub wallet: WalletConfig,
    
    /// Trading configuration
    pub trading: TradingConfig,
    
    /// Nonce configuration
    pub nonce: NonceConfig,
    
    /// Sniffer configuration
    pub sniffer: SnifferConfig,
    
    /// Monitoring and metrics
    pub monitoring: MonitoringConfig,
    
    /// Number of nonce accounts to use per transaction (for parallel submission)
    #[serde(default = "default_nonce_count")]
    pub nonce_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    /// List of RPC endpoints
    pub endpoints: Vec<String>,
    
    /// Request timeout in seconds
    #[serde(default = "default_rpc_timeout")]
    pub timeout_secs: u64,
    
    /// Max retries per request
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    
    /// Rate limit (requests per second)
    #[serde(default = "default_rate_limit")]
    pub rate_limit_rps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    /// Path to keypair file
    pub keypair_path: String,
    
    /// Enable hardware wallet
    #[serde(default)]
    pub use_hardware_wallet: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    /// Maximum slippage tolerance (basis points)
    #[serde(default = "default_max_slippage")]
    pub max_slippage_bps: u16,
    
    /// Default buy amount in SOL
    pub buy_amount_sol: f64,
    
    /// Minimum liquidity required (in lamports)
    #[serde(default = "default_min_liquidity")]
    pub min_liquidity_lamports: u64,
    
    /// Enable MEV protection via Jito
    #[serde(default)]
    pub enable_jito: bool,
    
    /// Jito tip in lamports
    #[serde(default = "default_jito_tip")]
    pub jito_tip_lamports: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonceConfig {
    /// Number of nonce accounts to maintain
    #[serde(default = "default_nonce_pool_size")]
    pub pool_size: usize,
    
    /// Nonce refresh interval in seconds
    #[serde(default = "default_nonce_refresh_interval")]
    pub refresh_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnifferConfig {
    /// Geyser gRPC endpoint
    pub geyser_endpoint: String,
    
    /// Programs to monitor
    pub monitored_programs: Vec<String>,
    
    /// Buffer size for transaction stream
    #[serde(default = "default_stream_buffer_size")]
    pub stream_buffer_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable Prometheus metrics
    #[serde(default = "default_true")]
    pub enable_metrics: bool,
    
    /// Metrics port
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
    
    /// Enable tracing
    #[serde(default = "default_true")]
    pub enable_tracing: bool,
}

// Default value functions
fn default_rpc_timeout() -> u64 { 30 }
fn default_max_retries() -> u32 { 3 }
fn default_rate_limit() -> u32 { 100 }
fn default_max_slippage() -> u16 { 500 }
fn default_min_liquidity() -> u64 { 1_000_000_000 }
fn default_jito_tip() -> u64 { 10_000 }
fn default_nonce_pool_size() -> usize { 10 }
fn default_nonce_refresh_interval() -> u64 { 60 }
fn default_stream_buffer_size() -> usize { 4096 }
fn default_metrics_port() -> u16 { 9090 }
fn default_true() -> bool { true }
fn default_nonce_count() -> usize { 1 }

impl Config {
    /// Load configuration from TOML file
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
    
    /// Load configuration with environment variable overrides
    pub fn from_file_with_env(path: &str) -> anyhow::Result<Self> {
        dotenv::dotenv().ok();
        Self::from_file(path)
    }
    
    /// Create default configuration
    pub fn default() -> Self {
        Self {
            rpc: RpcConfig {
                endpoints: vec!["https://api.mainnet-beta.solana.com".to_string()],
                timeout_secs: default_rpc_timeout(),
                max_retries: default_max_retries(),
                rate_limit_rps: default_rate_limit(),
            },
            wallet: WalletConfig {
                keypair_path: "~/.config/solana/id.json".to_string(),
                use_hardware_wallet: false,
            },
            trading: TradingConfig {
                max_slippage_bps: default_max_slippage(),
                buy_amount_sol: 0.1,
                min_liquidity_lamports: default_min_liquidity(),
                enable_jito: false,
                jito_tip_lamports: default_jito_tip(),
            },
            nonce: NonceConfig {
                pool_size: default_nonce_pool_size(),
                refresh_interval_secs: default_nonce_refresh_interval(),
            },
            sniffer: SnifferConfig {
                geyser_endpoint: "http://localhost:10000".to_string(),
                monitored_programs: vec![],
                stream_buffer_size: default_stream_buffer_size(),
            },
            monitoring: MonitoringConfig {
                enable_metrics: default_true(),
                metrics_port: default_metrics_port(),
                enable_tracing: default_true(),
            },
            nonce_count: default_nonce_count(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::default()
    }
}
