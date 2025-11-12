use serde::{Deserialize, Serialize};

/// Configuration for an individual RPC endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcEndpointConfig {
    /// The RPC endpoint URL
    pub url: String,
    
    /// Weight for load balancing (higher = more requests)
    #[serde(default = "default_weight")]
    pub weight: f64,
    
    /// Maximum concurrent requests to this endpoint
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: u32,
    
    /// Optional API key or credentials
    #[serde(default)]
    pub credentials: Option<String>,
    
    /// Preferred nonce account (optional)
    #[serde(default)]
    pub preferred_nonce_account: Option<String>,
    
    /// Request timeout in milliseconds
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    
    /// Rate limit (requests per second)
    #[serde(default = "default_rate_limit")]
    pub rate_limit_rps: u32,
}

fn default_weight() -> f64 {
    1.0
}

fn default_max_concurrency() -> u32 {
    100
}

fn default_timeout_ms() -> u64 {
    5000
}

fn default_rate_limit() -> u32 {
    100
}

/// Global RPC manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcManagerConfig {
    /// List of RPC endpoints
    pub endpoints: Vec<RpcEndpointConfig>,
    
    /// Health check interval in seconds
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval_secs: u64,
    
    /// Circuit breaker failure threshold
    #[serde(default = "default_failure_threshold")]
    pub circuit_breaker_failure_threshold: u32,
    
    /// Circuit breaker timeout in seconds
    #[serde(default = "default_circuit_breaker_timeout")]
    pub circuit_breaker_timeout_secs: u64,
    
    /// Enable predictive failure detection
    #[serde(default = "default_true")]
    pub enable_predictive_failure: bool,
    
    /// Predictive failure probability threshold
    #[serde(default = "default_predictive_threshold")]
    pub predictive_failure_threshold: f64,
    
    /// Enable telemetry
    #[serde(default = "default_true")]
    pub enable_telemetry: bool,
    
    /// Telemetry export endpoint (optional)
    #[serde(default)]
    pub telemetry_endpoint: Option<String>,
}

fn default_health_check_interval() -> u64 {
    1
}

fn default_failure_threshold() -> u32 {
    5
}

fn default_circuit_breaker_timeout() -> u64 {
    60
}

fn default_true() -> bool {
    true
}

fn default_predictive_threshold() -> f64 {
    0.75
}

impl RpcManagerConfig {
    /// Load configuration from a TOML file
    pub fn from_toml_file(path: &str) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::IoError(format!("Failed to read config file {}: {}", path, e)))?;
        
        toml::from_str(&contents)
            .map_err(|e| ConfigError::ParseError(format!("Failed to parse TOML: {}", e)))
    }
    
    /// Load configuration from a JSON file
    pub fn from_json_file(path: &str) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::IoError(format!("Failed to read config file {}: {}", path, e)))?;
        
        serde_json::from_str(&contents)
            .map_err(|e| ConfigError::ParseError(format!("Failed to parse JSON: {}", e)))
    }
    
    /// Load configuration from environment variables
    /// Expected format: RPC_ENDPOINTS=url1,url2,url3
    pub fn from_env() -> Result<Self, ConfigError> {
        let endpoints_str = std::env::var("RPC_ENDPOINTS")
            .map_err(|_| ConfigError::MissingEnvVar("RPC_ENDPOINTS".to_string()))?;
        
        let urls: Vec<String> = endpoints_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        if urls.is_empty() {
            return Err(ConfigError::ValidationError("No RPC endpoints provided".to_string()));
        }
        
        let endpoints = urls
            .into_iter()
            .map(|url| RpcEndpointConfig {
                url,
                weight: default_weight(),
                max_concurrency: default_max_concurrency(),
                credentials: std::env::var("RPC_CREDENTIALS").ok(),
                preferred_nonce_account: None,
                timeout_ms: default_timeout_ms(),
                rate_limit_rps: default_rate_limit(),
            })
            .collect();
        
        Ok(Self {
            endpoints,
            health_check_interval_secs: default_health_check_interval(),
            circuit_breaker_failure_threshold: default_failure_threshold(),
            circuit_breaker_timeout_secs: default_circuit_breaker_timeout(),
            enable_predictive_failure: default_true(),
            predictive_failure_threshold: default_predictive_threshold(),
            enable_telemetry: std::env::var("ENABLE_TELEMETRY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            telemetry_endpoint: std::env::var("TELEMETRY_ENDPOINT").ok(),
        })
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Check for duplicate URLs
        let mut seen_urls = std::collections::HashSet::new();
        for endpoint in &self.endpoints {
            if !seen_urls.insert(&endpoint.url) {
                return Err(ConfigError::ValidationError(
                    format!("Duplicate RPC URL: {}", endpoint.url)
                ));
            }
            
            // Validate URL format
            if !endpoint.url.starts_with("http://") && !endpoint.url.starts_with("https://") {
                return Err(ConfigError::ValidationError(
                    format!("Invalid URL format: {}", endpoint.url)
                ));
            }
            
            // Validate weight
            if endpoint.weight <= 0.0 || !endpoint.weight.is_finite() {
                return Err(ConfigError::ValidationError(
                    format!("Invalid weight for {}: must be > 0", endpoint.url)
                ));
            }
            
            // Validate max_concurrency
            if endpoint.max_concurrency == 0 {
                return Err(ConfigError::ValidationError(
                    format!("Invalid max_concurrency for {}: must be > 0", endpoint.url)
                ));
            }
        }
        
        // Validate at least one endpoint
        if self.endpoints.is_empty() {
            return Err(ConfigError::ValidationError(
                "At least one RPC endpoint must be configured".to_string()
            ));
        }
        
        // Validate thresholds
        if self.predictive_failure_threshold < 0.0 || self.predictive_failure_threshold > 1.0 {
            return Err(ConfigError::ValidationError(
                "Predictive failure threshold must be between 0.0 and 1.0".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Create a default configuration from a list of URLs
    pub fn from_urls(urls: &[String]) -> Self {
        let endpoints = urls
            .iter()
            .map(|url| RpcEndpointConfig {
                url: url.clone(),
                weight: default_weight(),
                max_concurrency: default_max_concurrency(),
                credentials: None,
                preferred_nonce_account: None,
                timeout_ms: default_timeout_ms(),
                rate_limit_rps: default_rate_limit(),
            })
            .collect();
        
        Self {
            endpoints,
            health_check_interval_secs: default_health_check_interval(),
            circuit_breaker_failure_threshold: default_failure_threshold(),
            circuit_breaker_timeout_secs: default_circuit_breaker_timeout(),
            enable_predictive_failure: default_true(),
            predictive_failure_threshold: default_predictive_threshold(),
            enable_telemetry: default_true(),
            telemetry_endpoint: None,
        }
    }
}

/// Configuration-related errors
#[derive(Debug, Clone)]
pub enum ConfigError {
    IoError(String),
    ParseError(String),
    ValidationError(String),
    MissingEnvVar(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::IoError(msg) => write!(f, "IO error: {}", msg),
            ConfigError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ConfigError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            ConfigError::MissingEnvVar(var) => write!(f, "Missing environment variable: {}", var),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_from_urls() {
        let urls = vec![
            "https://api.mainnet-beta.solana.com".to_string(),
            "https://api.devnet.solana.com".to_string(),
        ];
        
        let config = RpcManagerConfig::from_urls(&urls);
        assert_eq!(config.endpoints.len(), 2);
        assert_eq!(config.endpoints[0].url, "https://api.mainnet-beta.solana.com");
    }
    
    #[test]
    fn test_config_validation() {
        // Valid config
        let valid_config = RpcManagerConfig::from_urls(&vec![
            "https://api.mainnet-beta.solana.com".to_string(),
        ]);
        assert!(valid_config.validate().is_ok());
        
        // Empty endpoints
        let mut invalid_config = valid_config.clone();
        invalid_config.endpoints.clear();
        assert!(invalid_config.validate().is_err());
        
        // Duplicate URLs
        let dup_config = RpcManagerConfig::from_urls(&vec![
            "https://api.mainnet-beta.solana.com".to_string(),
            "https://api.mainnet-beta.solana.com".to_string(),
        ]);
        assert!(dup_config.validate().is_err());
        
        // Invalid URL
        let mut bad_url_config = valid_config.clone();
        bad_url_config.endpoints[0].url = "not-a-url".to_string();
        assert!(bad_url_config.validate().is_err());
    }
    
    #[test]
    fn test_default_values() {
        let config = RpcManagerConfig::from_urls(&vec!["https://test.com".to_string()]);
        assert_eq!(config.health_check_interval_secs, 1);
        assert_eq!(config.circuit_breaker_failure_threshold, 5);
        assert_eq!(config.enable_telemetry, true);
        assert_eq!(config.predictive_failure_threshold, 0.75);
    }
}
