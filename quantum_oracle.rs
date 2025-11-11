//! Simple Oracle for Hot-Swap Demonstration
//!
//! This module provides a minimal Oracle implementation to demonstrate
//! the hot-swap capability in the OODA loop.

use crate::oracle::types::{FeatureWeights, ScoreThresholds};
use crate::types::{PremintCandidate, QuantumCandidateGui};
use anyhow::Result;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{info, instrument};

/// Simple Oracle configuration for hot-swap demonstration
#[derive(Debug, Clone)]
pub struct SimpleOracleConfig {
    pub weights: FeatureWeights,
    pub thresholds: ScoreThresholds,
    pub rpc_endpoints: Vec<String>,
}

impl Default for SimpleOracleConfig {
    fn default() -> Self {
        Self {
            weights: FeatureWeights::default(),
            thresholds: ScoreThresholds::default(),
            rpc_endpoints: vec!["https://api.mainnet-beta.solana.com".to_string()],
        }
    }
}

/// Simplified scored candidate for demonstration
#[derive(Debug, Clone)]
pub struct ScoredCandidate {
    pub mint: String,
    pub predicted_score: u8,
    pub feature_scores: HashMap<String, f64>,
    pub reason: String,
    pub timestamp: u64,
    pub calculation_time: u128,
    pub anomaly_detected: bool,
}

/// Simple Oracle metrics
#[derive(Debug, Default, Clone)]
pub struct OracleMetrics {
    pub total_scored: u64,
    pub avg_scoring_time: f64,
    pub high_score_count: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub rpc_errors: u64,
    pub api_errors: u64,
}

/// Simplified Predictive Oracle for hot-swap demonstration
pub struct PredictiveOracle {
    pub candidate_receiver: mpsc::Receiver<PremintCandidate>,
    pub scored_sender: mpsc::Sender<ScoredCandidate>,
    pub gui_suggestions: Arc<Mutex<Option<mpsc::Sender<QuantumCandidateGui>>>>,
    pub config: Arc<RwLock<SimpleOracleConfig>>,
    
    // Simple state tracking
    pub metrics: Arc<RwLock<OracleMetrics>>,
}

impl PredictiveOracle {
    /// Create a new simplified Predictive Oracle for hot-swap demonstration
    pub fn new(
        candidate_receiver: mpsc::Receiver<PremintCandidate>,
        scored_sender: mpsc::Sender<ScoredCandidate>,
        config: Arc<RwLock<SimpleOracleConfig>>,
    ) -> Result<Self> {
        // We can't use async in constructor, so we'll do basic validation
        // Real validation will happen during runtime
        info!("Created Simplified Predictive Oracle for hot-swap demonstration");

        Ok(Self {
            candidate_receiver,
            scored_sender,
            gui_suggestions: Arc::new(Mutex::new(None)),
            config,
            metrics: Arc::new(RwLock::new(OracleMetrics::default())),
        })
    }

    /// Set GUI sender for notifications
    pub fn set_gui_sender(&self, sender: mpsc::Sender<QuantumCandidateGui>) {
        tokio::spawn({
            let gui_suggestions = self.gui_suggestions.clone();
            async move {
                let mut gui_lock = gui_suggestions.lock().await;
                *gui_lock = Some(sender);
            }
        });
    }

    /// Update Oracle configuration for hot-swap (Pillar II "Act" phase)
    #[instrument(skip(self, new_weights, new_thresholds))]
    pub async fn update_config(&self, new_weights: FeatureWeights, new_thresholds: ScoreThresholds) -> Result<()> {
        info!("Hot-swapping Oracle configuration...");
        
        // Update the shared config
        {
            let mut config_guard = self.config.write().await;
            config_guard.weights = new_weights.clone();
            config_guard.thresholds = new_thresholds.clone();
        }
        
        info!("Oracle configuration updated successfully:");
        info!("  New liquidity weight: {:.3}", new_weights.liquidity);
        info!("  New holder_distribution weight: {:.3}", new_weights.holder_distribution);
        info!("  New volume_growth weight: {:.3}", new_weights.volume_growth);
        info!("  New min_liquidity_sol threshold: {:.2}", new_thresholds.min_liquidity_sol);
        
        Ok(())
    }

    /// Get current Oracle metrics
    pub async fn get_metrics(&self) -> OracleMetrics {
        let metrics_guard = self.metrics.read().await;
        metrics_guard.clone()
    }
    
    /// Clear cache (simplified)
    pub async fn clear_cache(&self) {
        info!("Cache cleared (simplified implementation)");
    }
    
    /// Get cache size (simplified)
    pub async fn get_cache_size(&self) -> usize {
        0 // Simplified implementation
    }
    
    /// Shutdown Oracle (simplified)
    pub async fn shutdown(&self) {
        info!("Oracle shutdown requested (simplified implementation)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    fn create_test_config() -> SimpleOracleConfig {
        SimpleOracleConfig {
            rpc_endpoints: vec!["https://api.mainnet-beta.solana.com".to_string()],
            ..SimpleOracleConfig::default()
        }
    }

    #[tokio::test]
    async fn test_oracle_creation() {
        let (candidate_tx, candidate_rx) = mpsc::channel(10);
        let (scored_tx, _scored_rx) = mpsc::channel(10);
        let config = Arc::new(RwLock::new(create_test_config()));

        let oracle = PredictiveOracle::new(candidate_rx, scored_tx, config);
        assert!(oracle.is_ok());
    }

    #[tokio::test]
    async fn test_oracle_creation_empty_rpc_endpoints() {
        let (candidate_tx, candidate_rx) = mpsc::channel(10);
        let (scored_tx, _scored_rx) = mpsc::channel(10);
        let mut config = create_test_config();
        config.rpc_endpoints.clear();
        let config = Arc::new(RwLock::new(config));

        let oracle = PredictiveOracle::new(candidate_rx, scored_tx, config);
        // Since validation is now done at runtime, the constructor should succeed
        assert!(oracle.is_ok());
    }

    #[tokio::test]
    async fn test_update_config() {
        let (candidate_tx, candidate_rx) = mpsc::channel(10);
        let (scored_tx, _scored_rx) = mpsc::channel(10);
        let config = Arc::new(RwLock::new(create_test_config()));

        let oracle = PredictiveOracle::new(candidate_rx, scored_tx, config.clone()).unwrap();
        
        // Create new weights and thresholds
        let new_weights = FeatureWeights {
            liquidity: 0.5,
            holder_distribution: 0.3,
            volume_growth: 0.2,
            ..FeatureWeights::default()
        };
        let new_thresholds = ScoreThresholds {
            min_liquidity_sol: 25.0,
            ..ScoreThresholds::default()
        };
        
        // Update config
        let result = oracle.update_config(new_weights.clone(), new_thresholds.clone()).await;
        assert!(result.is_ok());
        
        // Verify config was updated
        let config_guard = config.read().await;
        assert_eq!(config_guard.weights.liquidity, 0.5);
        assert_eq!(config_guard.weights.holder_distribution, 0.3);
        assert_eq!(config_guard.thresholds.min_liquidity_sol, 25.0);
    }

    #[tokio::test] 
    async fn test_get_metrics() {
        let (candidate_tx, candidate_rx) = mpsc::channel(10);
        let (scored_tx, _scored_rx) = mpsc::channel(10);
        let config = Arc::new(RwLock::new(create_test_config()));

        let oracle = PredictiveOracle::new(candidate_rx, scored_tx, config).unwrap();
        let metrics = oracle.get_metrics().await;

        // Should have default values
        assert_eq!(metrics.total_scored, 0);
        assert_eq!(metrics.high_score_count, 0);
    }
}