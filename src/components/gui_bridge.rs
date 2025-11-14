//! GUI Bridge Module - Zero-copy state snapshots for GUI integration
//!
//! This module provides lock-free, zero-copy communication between the trading bot
//! and the GUI monitoring interface. It's designed to have zero impact on bot performance
//! while providing real-time position tracking and price updates.

use arc_swap::ArcSwap;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

// Import types from the types module to avoid duplication
use crate::types::{TradingMode, PortfolioConfig};

/// GUI state snapshot (zero-copy where possible)
///
/// This structure provides a complete snapshot of the bot's current state
/// for GUI rendering without requiring locks on the main bot state.
#[derive(Clone, Debug)]
pub struct GuiSnapshot {
    /// All active trading positions
    pub active_positions: Vec<PositionSnapshot>,

    /// Current bot state (Running, Stopped, Paused)
    pub bot_state: BotState,

    /// Timestamp when this snapshot was created
    pub timestamp: Instant,
}

/// Snapshot of a single trading position
///
/// Contains all necessary information to display a position in the GUI,
/// including entry price, current price, and P&L calculations.
#[derive(Clone, Debug)]
pub struct PositionSnapshot {
    /// Token mint address
    pub mint: Pubkey,

    /// Entry price in SOL
    pub entry_price_sol: f64,

    /// Current market price in SOL
    pub current_price_sol: f64,

    /// Amount of tokens held
    pub token_amount: u64,

    /// Initial SOL cost (in lamports)
    pub initial_sol_cost: u64,

    /// Current value in SOL
    pub current_value_sol: f64,

    /// Profit/Loss in SOL
    pub pnl_sol: f64,

    /// Profit/Loss as percentage
    pub pnl_percent: f64,
}

/// Bot operational state
///
/// Represents the current operational state of the trading bot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BotState {
    /// Bot is actively trading
    Running,

    /// Bot is stopped and not processing candidates
    Stopped,

    /// Bot is paused temporarily
    Paused,
}

/// Price update message for real-time GUI updates
///
/// Published by the bot whenever a price is observed or updated.
#[derive(Clone, Debug)]
pub struct PriceUpdate {
    /// Token mint address
    pub mint: Pubkey,

    /// Current price in SOL
    pub price_sol: f64,

    /// Current price in USD (if available)
    pub price_usd: Option<f64>,

    /// Timestamp of this update (Unix timestamp in seconds)
    pub timestamp: u64,
}

/// Lock-free snapshot provider for GUI integration
///
/// Uses ArcSwap for atomic snapshot updates without blocking the bot.
/// Provides a non-blocking interface for the GUI to read the latest state.
pub struct GuiSnapshotProvider {
    /// Latest snapshot available for GUI
    ///
    /// Uses ArcSwap to provide lock-free atomic updates and reads.
    /// The GUI can read this at any time without blocking the bot.
    latest_snapshot: Arc<ArcSwap<GuiSnapshot>>,

    /// Channel for publishing price updates
    ///
    /// Uses an mpsc channel to send price updates to the GUI.
    /// This is non-blocking for the sender (bot).
    price_tx: mpsc::Sender<PriceUpdate>,
}

impl GuiSnapshotProvider {
    /// Create a new snapshot provider
    ///
    /// # Arguments
    /// * `price_tx` - Channel sender for price updates
    ///
    /// # Returns
    /// A new GuiSnapshotProvider with an initial empty snapshot
    pub fn new(price_tx: mpsc::Sender<PriceUpdate>) -> Self {
        let initial_snapshot = GuiSnapshot {
            active_positions: Vec::new(),
            bot_state: BotState::Stopped,
            timestamp: Instant::now(),
        };

        Self {
            latest_snapshot: Arc::new(ArcSwap::from_pointee(initial_snapshot)),
            price_tx,
        }
    }

    /// Update the snapshot atomically (non-blocking)
    ///
    /// This method uses ArcSwap to update the snapshot without any locks.
    /// The old snapshot is automatically dropped when no GUI threads are using it.
    ///
    /// # Arguments
    /// * `snapshot` - The new snapshot to publish
    pub fn update_snapshot(&self, snapshot: GuiSnapshot) {
        self.latest_snapshot.store(Arc::new(snapshot));
    }

    /// Get the latest snapshot (non-blocking read)
    ///
    /// Returns a cloned Arc to the latest snapshot. This is very cheap
    /// (just incrementing a reference count) and completely lock-free.
    ///
    /// # Returns
    /// Arc pointing to the latest GuiSnapshot
    pub fn get_snapshot(&self) -> Arc<GuiSnapshot> {
        self.latest_snapshot.load_full()
    }

    /// Publish a price update (non-blocking)
    ///
    /// Attempts to send a price update through the channel.
    /// If the channel is full or closed, the update is silently dropped
    /// to avoid blocking the bot.
    ///
    /// # Arguments
    /// * `update` - The price update to publish
    ///
    /// # Returns
    /// `true` if the update was sent, `false` if it was dropped
    pub fn publish_price(&self, update: PriceUpdate) -> bool {
        // Use try_send to avoid blocking if GUI is slow
        self.price_tx.try_send(update).is_ok()
    }

    /// Get a clone of the price sender for sharing
    ///
    /// This allows multiple components to publish price updates.
    ///
    /// # Returns
    /// A clone of the price update sender
    pub fn price_sender(&self) -> mpsc::Sender<PriceUpdate> {
        self.price_tx.clone()
    }
}

impl Default for BotState {
    fn default() -> Self {
        BotState::Stopped
    }
}

// =============================================================================
// GUI Commands (Future Feature)
// =============================================================================
// NOTE: These command types are placeholders for future GUI-to-bot communication.
// Currently unused, will be integrated when implementing manual control features.

/// Commands that can be sent from GUI to bot
///
/// Enables manual control and configuration changes from the GUI.
#[derive(Clone, Debug)]
pub enum GuiCommand {
    /// Manually trigger a sell for specific token
    Sell {
        mint: Pubkey,
        percent: f64, // 0.0 to 1.0
    },
    
    /// Change trading mode (Manual/Auto/Hybrid)
    SetTradingMode(TradingMode),
    
    /// Set stop loss for a specific token
    SetStopLoss {
        mint: Pubkey,
        threshold_percent: f64,
    },
    
    /// Set take profit for a specific token
    SetTakeProfit {
        mint: Pubkey,
        threshold_percent: f64,
        sell_percent: f64,
    },
    
    /// Clear all TP/SL strategies for a token
    ClearStrategy {
        mint: Pubkey,
    },
    
    /// Enable/disable multi-token mode
    SetMultiTokenMode {
        enabled: bool,
        max_positions: Option<usize>,
    },
    
    /// Pause/Resume trading
    SetPaused(bool),
    
    /// Emergency stop all trading and close positions
    EmergencyStop,
    
    /// Update portfolio configuration
    UpdatePortfolioConfig(PortfolioConfig),
}

/// Response from bot to GUI commands
///
/// Provides feedback on command execution.
/// Currently a placeholder for future functionality.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum GuiCommandResponse {
    /// Command executed successfully
    Success,
    
    /// Command failed with error message
    Error(String),
    
    /// Command acknowledged but pending
    Pending,
}



impl GuiSnapshot {
    /// Create a new empty snapshot
    ///
    /// # Arguments
    /// * `bot_state` - The current bot state
    ///
    /// # Returns
    /// A new GuiSnapshot with no active positions
    pub fn new(bot_state: BotState) -> Self {
        Self {
            active_positions: Vec::new(),
            bot_state,
            timestamp: Instant::now(),
        }
    }

    /// Create a snapshot with positions
    ///
    /// # Arguments
    /// * `bot_state` - The current bot state
    /// * `positions` - Vector of active positions
    ///
    /// # Returns
    /// A new GuiSnapshot with the specified positions
    pub fn with_positions(bot_state: BotState, positions: Vec<PositionSnapshot>) -> Self {
        Self {
            active_positions: positions,
            bot_state,
            timestamp: Instant::now(),
        }
    }
}

impl PositionSnapshot {
    /// Create a new position snapshot
    ///
    /// # Arguments
    /// * `mint` - Token mint address
    /// * `entry_price_sol` - Entry price in SOL
    /// * `current_price_sol` - Current price in SOL
    /// * `token_amount` - Amount of tokens held
    /// * `initial_sol_cost` - Initial SOL cost in lamports
    ///
    /// # Returns
    /// A new PositionSnapshot with calculated P&L values
    pub fn new(
        mint: Pubkey,
        entry_price_sol: f64,
        current_price_sol: f64,
        token_amount: u64,
        initial_sol_cost: u64,
    ) -> Self {
        // Calculate current value in SOL
        let current_value_sol = (token_amount as f64) * current_price_sol;

        // Calculate P&L in SOL
        let initial_sol = initial_sol_cost as f64 / 1_000_000_000.0;
        let pnl_sol = current_value_sol - initial_sol;

        // Calculate P&L percentage
        let pnl_percent = if initial_sol > 0.0 {
            (pnl_sol / initial_sol) * 100.0
        } else {
            0.0
        };

        Self {
            mint,
            entry_price_sol,
            current_price_sol,
            token_amount,
            initial_sol_cost,
            current_value_sol,
            pnl_sol,
            pnl_percent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_snapshot_creation() {
        let mint = Pubkey::new_unique();
        let entry_price = 0.01;
        let current_price = 0.02;
        let token_amount = 1_000_000;
        let initial_sol_cost = 10_000_000; // 0.01 SOL in lamports

        let snapshot = PositionSnapshot::new(
            mint,
            entry_price,
            current_price,
            token_amount,
            initial_sol_cost,
        );

        assert_eq!(snapshot.mint, mint);
        assert_eq!(snapshot.entry_price_sol, entry_price);
        assert_eq!(snapshot.current_price_sol, current_price);
        assert_eq!(snapshot.token_amount, token_amount);
        assert_eq!(snapshot.initial_sol_cost, initial_sol_cost);

        // Current value should be 1M tokens * 0.02 SOL = 20,000 SOL
        assert!((snapshot.current_value_sol - 20_000.0).abs() < 0.001);

        // P&L should be 20,000 - 0.01 = 19,999.99 SOL
        assert!(snapshot.pnl_sol > 19_999.0);

        // P&L percent should be positive and very large (100x gain)
        assert!(snapshot.pnl_percent > 100_000.0);
    }

    #[test]
    fn test_position_snapshot_zero_price() {
        let mint = Pubkey::new_unique();
        let snapshot = PositionSnapshot::new(mint, 0.01, 0.0, 1_000_000, 10_000_000);

        // Should handle zero current price without panicking
        assert_eq!(snapshot.current_value_sol, 0.0);
        assert!(snapshot.pnl_sol < 0.0); // Loss
    }

    #[test]
    fn test_gui_snapshot_creation() {
        let snapshot = GuiSnapshot::new(BotState::Running);

        assert_eq!(snapshot.bot_state, BotState::Running);
        assert!(snapshot.active_positions.is_empty());
    }

    #[test]
    fn test_gui_snapshot_with_positions() {
        let mint = Pubkey::new_unique();
        let position = PositionSnapshot::new(mint, 0.01, 0.02, 1_000_000, 10_000_000);

        let snapshot = GuiSnapshot::with_positions(BotState::Running, vec![position.clone()]);

        assert_eq!(snapshot.bot_state, BotState::Running);
        assert_eq!(snapshot.active_positions.len(), 1);
        assert_eq!(snapshot.active_positions[0].mint, mint);
    }

    #[tokio::test]
    async fn test_snapshot_provider_update_and_read() {
        let (tx, _rx) = mpsc::channel(100);
        let provider = GuiSnapshotProvider::new(tx);

        // Create a snapshot
        let mint = Pubkey::new_unique();
        let position = PositionSnapshot::new(mint, 0.01, 0.02, 1_000_000, 10_000_000);
        let snapshot = GuiSnapshot::with_positions(BotState::Running, vec![position]);

        // Update the snapshot
        provider.update_snapshot(snapshot.clone());

        // Read it back
        let read_snapshot = provider.get_snapshot();

        assert_eq!(read_snapshot.bot_state, BotState::Running);
        assert_eq!(read_snapshot.active_positions.len(), 1);
        assert_eq!(read_snapshot.active_positions[0].mint, mint);
    }

    #[tokio::test]
    async fn test_price_update_publish() {
        let (tx, mut rx) = mpsc::channel(100);
        let provider = GuiSnapshotProvider::new(tx);

        let mint = Pubkey::new_unique();
        let price_update = PriceUpdate {
            mint,
            price_sol: 0.01,
            price_usd: Some(1.5),
            timestamp: 1234567890,
        };

        // Publish price update
        let result = provider.publish_price(price_update.clone());
        assert!(result, "Price update should be published successfully");

        // Receive it
        let received = rx.recv().await.unwrap();
        assert_eq!(received.mint, mint);
        assert_eq!(received.price_sol, 0.01);
        assert_eq!(received.price_usd, Some(1.5));
    }

    #[tokio::test]
    async fn test_price_update_channel_full() {
        let (tx, _rx) = mpsc::channel(1); // Very small channel
        let provider = GuiSnapshotProvider::new(tx);

        let mint = Pubkey::new_unique();

        // First update should succeed
        let result1 = provider.publish_price(PriceUpdate {
            mint,
            price_sol: 0.01,
            price_usd: None,
            timestamp: 1,
        });
        assert!(result1);

        // Second update should fail (channel full, no receiver)
        let result2 = provider.publish_price(PriceUpdate {
            mint,
            price_sol: 0.02,
            price_usd: None,
            timestamp: 2,
        });
        assert!(!result2, "Should fail when channel is full");
    }

    #[test]
    fn test_bot_state_default() {
        let state: BotState = Default::default();
        assert_eq!(state, BotState::Stopped);
    }

    #[tokio::test]
    async fn test_concurrent_snapshot_updates() {
        use std::sync::Arc;
        use tokio::task;

        let (tx, _rx) = mpsc::channel(1000);
        let provider = Arc::new(GuiSnapshotProvider::new(tx));

        let mut handles = vec![];

        // Spawn multiple tasks updating the snapshot
        for i in 0..10 {
            let provider_clone = Arc::clone(&provider);
            let handle = task::spawn(async move {
                for j in 0..100 {
                    let state = if (i + j) % 2 == 0 {
                        BotState::Running
                    } else {
                        BotState::Paused
                    };
                    let snapshot = GuiSnapshot::new(state);
                    provider_clone.update_snapshot(snapshot);
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Should be able to read the final snapshot
        let final_snapshot = provider.get_snapshot();
        assert!(
            final_snapshot.bot_state == BotState::Running
                || final_snapshot.bot_state == BotState::Paused
        );
    }
}
