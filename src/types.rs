//! Common types used throughout the application

use crate::components::gui_bridge::GuiSnapshotProvider;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};

/// Trading mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Simulation mode (no real transactions)
    Simulation,
    /// Production mode (real transactions)
    Production,
    /// Sniffing mode (only monitor, no trading)
    Sniffing,
    /// Passive token mode (hold and monitor a specific token)
    PassiveToken(Pubkey),
    /// Quantum manual mode (manual trading with quantum strategies)
    QuantumManual,
}

/// Premint candidate from sniffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremintCandidate {
    /// Mint address
    pub mint: Pubkey,

    /// Program ID that created this token (e.g., "pumpfun", "letsbonk")
    pub program: String,

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

/// Token position information for multi-token portfolio management
#[derive(Debug, Clone)]
pub struct TokenPosition {
    /// The premint candidate information
    pub candidate: PremintCandidate,
    
    /// Entry price when position was opened
    pub entry_price: f64,
    
    /// Current holdings percentage (0.0 - 1.0)
    pub holdings_percent: f64,
    
    /// Timestamp when position was entered (Unix timestamp in seconds)
    pub entry_timestamp: u64,
}

impl TokenPosition {
    /// Create a new token position
    pub fn new(candidate: PremintCandidate, entry_price: f64) -> Self {
        Self {
            candidate,
            entry_price,
            holdings_percent: 1.0,
            entry_timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Current operating mode
    pub mode: Arc<RwLock<Mode>>,

    /// Is the system paused
    pub is_paused: Arc<RwLock<bool>>,

    /// Statistics
    pub stats: Arc<RwLock<Stats>>,

    /// Active token positions (multi-token support)
    pub active_tokens: Arc<DashMap<Pubkey, TokenPosition>>,

    /// GUI snapshot provider for real-time monitoring (optional)
    pub gui_snapshot_provider: Option<Arc<GuiSnapshotProvider>>,

    /// Portfolio configuration for multi-token trading
    pub portfolio_config: PortfolioConfig,
    
    // ============================================================================
    // DEPRECATED FIELDS - Kept for backward compatibility, will be removed in future
    // ============================================================================
    // These fields are kept temporarily to avoid breaking existing code that may
    // still reference them. New code should use active_tokens instead.
    
    /// DEPRECATED: Use active_tokens instead
    /// Currently active token (if any)
    #[deprecated(since = "0.2.0", note = "Use active_tokens instead")]
    pub active_token: Option<PremintCandidate>,

    /// DEPRECATED: Use TokenPosition.entry_price in active_tokens instead
    /// Last buy price (if any)
    #[deprecated(since = "0.2.0", note = "Use TokenPosition.entry_price in active_tokens")]
    pub last_buy_price: Option<f64>,

    /// DEPRECATED: Use TokenPosition.holdings_percent in active_tokens instead
    /// Current holdings percentage (0.0 - 1.0)
    #[deprecated(since = "0.2.0", note = "Use TokenPosition.holdings_percent in active_tokens")]
    pub holdings_percent: f64,
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
            active_tokens: Arc::new(DashMap::new()),
            gui_snapshot_provider: None,
            portfolio_config: PortfolioConfig::default(),
            // Deprecated fields
            #[allow(deprecated)]
            active_token: None,
            #[allow(deprecated)]
            last_buy_price: None,
            #[allow(deprecated)]
            holdings_percent: 0.0,
        }
    }

    /// Create new application state with GUI support
    pub fn with_gui(mode: Mode, gui_provider: Arc<GuiSnapshotProvider>) -> Self {
        Self {
            mode: Arc::new(RwLock::new(mode)),
            is_paused: Arc::new(RwLock::new(false)),
            stats: Arc::new(RwLock::new(Stats::default())),
            active_tokens: Arc::new(DashMap::new()),
            gui_snapshot_provider: Some(gui_provider),
            portfolio_config: PortfolioConfig::default(),
            // Deprecated fields
            #[allow(deprecated)]
            active_token: None,
            #[allow(deprecated)]
            last_buy_price: None,
            #[allow(deprecated)]
            holdings_percent: 0.0,
        }
    }

    /// Create new application state with custom portfolio configuration
    pub fn with_config(mode: Mode, portfolio_config: PortfolioConfig) -> Self {
        Self {
            mode: Arc::new(RwLock::new(mode)),
            is_paused: Arc::new(RwLock::new(false)),
            stats: Arc::new(RwLock::new(Stats::default())),
            active_tokens: Arc::new(DashMap::new()),
            gui_snapshot_provider: None,
            portfolio_config,
            // Deprecated fields
            #[allow(deprecated)]
            active_token: None,
            #[allow(deprecated)]
            last_buy_price: None,
            #[allow(deprecated)]
            holdings_percent: 0.0,
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

    /// Check if in sniffing mode
    pub async fn is_sniffing(&self) -> bool {
        matches!(*self.mode.read().await, Mode::Sniffing)
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

    // ============================================================================
    // Multi-Token Portfolio Management Methods
    // ============================================================================

    /// Check if bot can buy a new token (respects multi-token configuration)
    ///
    /// Returns true if:
    /// - Multi-token mode is disabled and no active positions exist, OR
    /// - Multi-token mode is enabled and positions < max_concurrent_positions
    pub fn can_buy(&self) -> bool {
        if !self.portfolio_config.enable_multi_token {
            // Single-token mode: only allow buy if no active positions
            return self.active_tokens.is_empty();
        }
        // Multi-token mode: check against position limit
        self.active_tokens.len() < self.portfolio_config.max_concurrent_positions
    }

    /// Get position for a specific token
    ///
    /// Returns a cloned TokenPosition if it exists, None otherwise
    pub fn get_position(&self, mint: &Pubkey) -> Option<TokenPosition> {
        self.active_tokens.get(mint).map(|r| r.clone())
    }

    /// Get all active positions
    ///
    /// Returns a vector of (Pubkey, TokenPosition) tuples
    pub fn get_all_positions(&self) -> Vec<(Pubkey, TokenPosition)> {
        self.active_tokens
            .iter()
            .map(|r| (*r.key(), r.value().clone()))
            .collect()
    }

    /// Add or update a token position
    ///
    /// Returns the previous position if one existed
    pub fn set_position(&self, mint: Pubkey, position: TokenPosition) -> Option<TokenPosition> {
        self.active_tokens.insert(mint, position)
    }

    /// Remove a token position
    ///
    /// Returns the removed position if it existed
    pub fn remove_position(&self, mint: &Pubkey) -> Option<(Pubkey, TokenPosition)> {
        self.active_tokens.remove(mint)
    }

    /// Get the number of active positions
    pub fn position_count(&self) -> usize {
        self.active_tokens.len()
    }
}

// =============================================================================
// Multi-Token Portfolio Configuration Types (Future Feature)
// =============================================================================
// NOTE: These types are placeholders for future multi-token support.
// They are not yet integrated into the main trading logic.
// Use with `#[cfg(feature = "multi_token")]` when implementing.

/// Portfolio configuration for multi-token trading
///
/// Controls how the bot manages multiple concurrent positions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioConfig {
    /// Enable multi-token portfolio management
    /// When false, bot operates in single-token mode (default, safer)
    pub enable_multi_token: bool,

    /// Maximum number of concurrent positions
    /// Only applies when enable_multi_token = true
    pub max_concurrent_positions: usize,

    /// Maximum total exposure in SOL across all positions
    /// Prevents over-leveraging the portfolio
    pub max_total_exposure_sol: f64,
}

impl Default for PortfolioConfig {
    fn default() -> Self {
        Self {
            enable_multi_token: false,
            max_concurrent_positions: 1,
            max_total_exposure_sol: 10.0,
        }
    }
}

/// Trading mode for portfolio management
///
/// Defines how the bot handles multiple token opportunities.
/// Currently a placeholder for future functionality.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradingMode {
    /// Single token at a time (default, safest)
    Single,
    
    /// Multiple tokens simultaneously
    /// Requires enable_multi_token = true
    Multi,
    
    /// Adaptive based on market conditions (experimental)
    /// Switches between Single and Multi based on volatility
    Hybrid,
}

impl Default for TradingMode {
    fn default() -> Self {
        TradingMode::Single
    }
}

/// Sell strategy configuration
///
/// Defines when and how to sell positions.
/// Currently a placeholder for future functionality.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SellStrategy {
    /// Stop loss configuration
    pub stop_loss: Option<StopLossConfig>,
    
    /// Take profit configuration
    pub take_profit: Option<TakeProfitConfig>,
    
    /// Trailing stop configuration
    pub trailing_stop: Option<TrailingStopConfig>,
}

impl Default for SellStrategy {
    fn default() -> Self {
        Self {
            stop_loss: None,
            take_profit: None,
            trailing_stop: None,
        }
    }
}

/// Stop loss configuration
///
/// Defines automatic sell triggers to limit losses.
/// Currently a placeholder for future functionality.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopLossConfig {
    /// Stop loss percentage (e.g., 0.10 for -10%)
    pub percentage: f64,
    
    /// Enable time-based stop loss (sell after duration regardless of price)
    pub time_based: bool,
    
    /// Time limit in seconds (only if time_based = true)
    pub time_limit_seconds: Option<u64>,
}

impl Default for StopLossConfig {
    fn default() -> Self {
        Self {
            percentage: 0.10, // -10% stop loss
            time_based: false,
            time_limit_seconds: None,
        }
    }
}

/// Take profit configuration
///
/// Defines automatic sell triggers to lock in profits.
/// Currently a placeholder for future functionality.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeProfitConfig {
    /// Take profit percentage (e.g., 0.50 for +50%)
    pub percentage: f64,
    
    /// Partial take profit levels (optional)
    /// Array of (percentage_gain, percentage_to_sell) tuples
    pub partial_levels: Vec<(f64, f64)>,
}

impl Default for TakeProfitConfig {
    fn default() -> Self {
        Self {
            percentage: 0.50, // +50% take profit
            partial_levels: vec![],
        }
    }
}

/// Trailing stop configuration
///
/// Defines dynamic stop loss that follows price upward.
/// Currently a placeholder for future functionality.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrailingStopConfig {
    /// Trailing stop percentage (e.g., 0.05 for -5% from peak)
    pub percentage: f64,
    
    /// Activation threshold (profit % needed to activate trailing stop)
    pub activation_threshold: f64,
}

impl Default for TrailingStopConfig {
    fn default() -> Self {
        Self {
            percentage: 0.05,        // -5% from peak
            activation_threshold: 0.20, // Activate at +20%
        }
    }
}

