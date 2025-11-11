//! Common types used throughout the application

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Trading mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Simulation mode (no real transactions)
    Simulation,
    /// Production mode (real transactions)
    Production,
}

/// Premint candidate from sniffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremintCandidate {
    /// Mint address
    pub mint: Pubkey,
    
    /// Associated accounts
    pub accounts: Vec<Pubkey>,
    
    /// Priority level
    pub priority: PriorityLevel,
    
    /// Timestamp
    pub timestamp: u64,
    
    /// Estimated price hint
    pub price_hint: Option<f64>,
    
    /// Signature of the transaction that created this candidate
    pub signature: Option<String>,
}

/// Priority level for candidates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PriorityLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Receiver for candidates from sniffer
pub type CandidateReceiver = mpsc::UnboundedReceiver<PremintCandidate>;

/// Sender for candidates to buy engine
pub type CandidateSender = mpsc::UnboundedSender<PremintCandidate>;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Current operating mode
    pub mode: Arc<RwLock<Mode>>,
    
    /// Is the system paused
    pub is_paused: Arc<RwLock<bool>>,
    
    /// Statistics
    pub stats: Arc<RwLock<Stats>>,
}

/// Application statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    /// Total candidates received
    pub total_candidates: u64,
    
    /// Total trades executed
    pub total_trades: u64,
    
    /// Successful trades
    pub successful_trades: u64,
    
    /// Failed trades
    pub failed_trades: u64,
    
    /// Total volume in SOL
    pub total_volume_sol: f64,
}

impl AppState {
    /// Create new application state
    pub fn new(mode: Mode) -> Self {
        Self {
            mode: Arc::new(RwLock::new(mode)),
            is_paused: Arc::new(RwLock::new(false)),
            stats: Arc::new(RwLock::new(Stats::default())),
        }
    }
    
    /// Check if system is paused
    pub async fn is_paused(&self) -> bool {
        *self.is_paused.read().await
    }
    
    /// Get current mode
    pub async fn get_mode(&self) -> Mode {
        *self.mode.read().await
    }
    
    /// Update statistics
    pub async fn increment_candidates(&self) {
        let mut stats = self.stats.write().await;
        stats.total_candidates += 1;
    }
    
    pub async fn record_trade(&self, success: bool, volume_sol: f64) {
        let mut stats = self.stats.write().await;
        stats.total_trades += 1;
        if success {
            stats.successful_trades += 1;
        } else {
            stats.failed_trades += 1;
        }
        stats.total_volume_sol += volume_sol;
    }
}
