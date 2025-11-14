//! Position Tracker Module - Real-time position monitoring with P&L calculations
//!
//! This module provides lock-free position tracking for active trading positions.
//! It integrates seamlessly with the BuyEngine to record buy/sell operations
//! and provides real-time P&L calculations for GUI monitoring.
//!
//! ## Key Features
//!
//! - **Lock-free concurrent access**: Uses DashMap for zero-contention reads
//! - **Real-time P&L tracking**: Calculates profit/loss with current market prices
//! - **Partial sell support**: Tracks sold portions and remaining holdings
//! - **Automatic cleanup**: Removes fully sold positions automatically
//!
//! ## Usage Example
//!
//! ```no_run
//! use bot::position_tracker::PositionTracker;
//! use solana_sdk::pubkey::Pubkey;
//! use std::sync::Arc;
//!
//! let tracker = Arc::new(PositionTracker::new());
//!
//! // Record a buy
//! let mint = Pubkey::new_unique();
//! tracker.record_buy(mint, 1_000_000, 10_000_000); // 1M tokens for 0.01 SOL
//!
//! // Update price and calculate P&L
//! tracker.update_price(&mint, 0.02); // Price doubled
//! let positions = tracker.get_all_positions();
//! 
//! // Record a partial sell
//! tracker.record_sell(&mint, 500_000, 10_000_000); // Sell half for 0.01 SOL
//! ```

use dashmap::DashMap;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Active trading position with P&L tracking
///
/// Represents a single token position with entry details, current state,
/// and methods for calculating profit/loss.
#[derive(Clone, Debug)]
pub struct ActivePosition {
    /// Token mint address
    pub mint: Pubkey,
    
    /// Unix timestamp when position was opened (seconds)
    pub entry_timestamp: u64,
    
    /// Initial token amount purchased
    pub initial_token_amount: u64,
    
    /// Total SOL spent on purchase (in lamports)
    pub initial_sol_cost: u64,
    
    /// Total tokens sold so far
    pub sold_token_amount: u64,
    
    /// Total SOL received from sales (in lamports)
    pub total_sol_from_sales: u64,
    
    /// Last observed price in SOL per token
    pub last_seen_price: f64,
    
    /// Last update timestamp (monotonic)
    pub last_update: Instant,
}

impl ActivePosition {
    /// Get the remaining token amount (not yet sold)
    ///
    /// # Returns
    /// Number of tokens still held
    pub fn remaining_token_amount(&self) -> u64 {
        self.initial_token_amount.saturating_sub(self.sold_token_amount)
    }
    
    /// Calculate percentage of position that has been sold
    ///
    /// # Returns
    /// Percentage (0.0 to 100.0) of tokens sold
    pub fn sold_percent(&self) -> f64 {
        if self.initial_token_amount == 0 {
            return 0.0;
        }
        (self.sold_token_amount as f64 / self.initial_token_amount as f64) * 100.0
    }
    
    /// Calculate total profit/loss using current price
    ///
    /// This method accounts for both sold and remaining tokens:
    /// - Sold tokens: Use actual SOL received
    /// - Remaining tokens: Use current market price
    ///
    /// # Arguments
    /// * `current_price_sol` - Current market price per token in SOL
    ///
    /// # Returns
    /// Tuple of (P&L in SOL, P&L as percentage)
    ///
    /// # Example
    /// ```
    /// # use bot::position_tracker::ActivePosition;
    /// # use solana_sdk::pubkey::Pubkey;
    /// # use std::time::{Instant, SystemTime, UNIX_EPOCH};
    /// let position = ActivePosition {
    ///     mint: Pubkey::new_unique(),
    ///     entry_timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    ///     initial_token_amount: 1_000_000,
    ///     initial_sol_cost: 10_000_000, // 0.01 SOL
    ///     sold_token_amount: 0,
    ///     total_sol_from_sales: 0,
    ///     last_seen_price: 0.00000001, // 0.01 SOL per token
    ///     last_update: Instant::now(),
    /// };
    ///
    /// // Price doubles
    /// let (pnl_sol, pnl_percent) = position.calculate_pnl(0.00000002);
    /// assert!(pnl_sol > 0.0); // Profit
    /// assert!(pnl_percent > 90.0); // ~100% gain
    /// ```
    pub fn calculate_pnl(&self, current_price_sol: f64) -> (f64, f64) {
        let remaining = self.remaining_token_amount();
        
        // Current value of remaining tokens in lamports
        let current_value_lamports = remaining as f64 * current_price_sol * 1_000_000_000.0;
        
        // Total value = SOL from sales + current value of remaining tokens
        let total_value_lamports = 
            self.total_sol_from_sales as i128 + current_value_lamports as i128;
        
        // P&L = Total value - Initial cost
        let total_pnl_lamports = total_value_lamports - self.initial_sol_cost as i128;
        
        // Convert to SOL
        let pnl_sol = total_pnl_lamports as f64 / 1_000_000_000.0;
        
        // Calculate percentage
        let pnl_percent = if self.initial_sol_cost > 0 {
            (total_pnl_lamports as f64 / self.initial_sol_cost as f64) * 100.0
        } else {
            0.0
        };
        
        (pnl_sol, pnl_percent)
    }
    
    /// Get the entry price (average price paid per token)
    ///
    /// # Returns
    /// Entry price in SOL per token
    pub fn entry_price(&self) -> f64 {
        if self.initial_token_amount == 0 {
            return 0.0;
        }
        self.initial_sol_cost as f64 / self.initial_token_amount as f64 / 1_000_000_000.0
    }
}

/// Lock-free position tracker
///
/// Thread-safe position tracking using DashMap for concurrent access.
/// Designed to be shared across the BuyEngine and GUI without locks.
pub struct PositionTracker {
    /// Active positions indexed by mint
    /// 
    /// Uses DashMap for lock-free concurrent access. Multiple threads can
    /// read and write simultaneously without contention.
    positions: Arc<DashMap<Pubkey, ActivePosition>>,
}

impl PositionTracker {
    /// Create a new position tracker
    ///
    /// # Returns
    /// A new PositionTracker with no active positions
    pub fn new() -> Self {
        Self {
            positions: Arc::new(DashMap::new()),
        }
    }
    
    /// Record a buy transaction
    ///
    /// Creates or updates a position when tokens are purchased.
    /// If a position already exists for this mint, it will be replaced
    /// (this assumes one active position per token).
    ///
    /// # Arguments
    /// * `mint` - Token mint address
    /// * `token_amount` - Number of tokens purchased
    /// * `sol_cost` - Total SOL spent (in lamports)
    ///
    /// # Example
    /// ```
    /// # use bot::position_tracker::PositionTracker;
    /// # use solana_sdk::pubkey::Pubkey;
    /// let tracker = PositionTracker::new();
    /// let mint = Pubkey::new_unique();
    /// 
    /// // Buy 1M tokens for 0.01 SOL (10M lamports)
    /// tracker.record_buy(mint, 1_000_000, 10_000_000);
    /// ```
    pub fn record_buy(&self, mint: Pubkey, token_amount: u64, sol_cost: u64) {
        let entry_price = if token_amount > 0 {
            sol_cost as f64 / token_amount as f64 / 1_000_000_000.0
        } else {
            0.0
        };
        
        self.positions.insert(mint, ActivePosition {
            mint,
            entry_timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            initial_token_amount: token_amount,
            initial_sol_cost: sol_cost,
            sold_token_amount: 0,
            total_sol_from_sales: 0,
            last_seen_price: entry_price,
            last_update: Instant::now(),
        });
    }
    
    /// Record a sell transaction
    ///
    /// Updates a position when tokens are sold. Tracks the amount sold
    /// and SOL received. Automatically removes the position if fully sold.
    ///
    /// # Arguments
    /// * `mint` - Token mint address
    /// * `token_amount` - Number of tokens sold
    /// * `sol_received` - SOL received from sale (in lamports)
    ///
    /// # Returns
    /// `true` if the position was found and updated, `false` if not found
    ///
    /// # Example
    /// ```
    /// # use bot::position_tracker::PositionTracker;
    /// # use solana_sdk::pubkey::Pubkey;
    /// let tracker = PositionTracker::new();
    /// let mint = Pubkey::new_unique();
    /// 
    /// tracker.record_buy(mint, 1_000_000, 10_000_000);
    /// 
    /// // Sell half for profit
    /// tracker.record_sell(&mint, 500_000, 15_000_000);
    /// ```
    pub fn record_sell(&self, mint: &Pubkey, token_amount: u64, sol_received: u64) -> bool {
        if let Some(mut pos) = self.positions.get_mut(mint) {
            pos.sold_token_amount += token_amount;
            pos.total_sol_from_sales += sol_received;
            pos.last_update = Instant::now();
            
            // Calculate new price from this sale
            if token_amount > 0 {
                pos.last_seen_price = sol_received as f64 / token_amount as f64 / 1_000_000_000.0;
            }
            
            // Check if fully sold
            let fully_sold = pos.remaining_token_amount() == 0;
            
            // Release the lock before potentially removing
            drop(pos);
            
            // Remove if fully sold
            if fully_sold {
                self.positions.remove(mint);
            }
            
            true
        } else {
            false
        }
    }
    
    /// Update the last seen price for a position
    ///
    /// Updates the cached price without recording a transaction.
    /// Useful for real-time P&L calculations based on market prices.
    ///
    /// # Arguments
    /// * `mint` - Token mint address
    /// * `price_sol` - Current price in SOL per token
    ///
    /// # Returns
    /// `true` if the position was found and updated, `false` if not found
    pub fn update_price(&self, mint: &Pubkey, price_sol: f64) -> bool {
        if let Some(mut pos) = self.positions.get_mut(mint) {
            pos.last_seen_price = price_sol;
            pos.last_update = Instant::now();
            true
        } else {
            false
        }
    }
    
    /// Get all active positions
    ///
    /// Returns a snapshot of all currently active positions.
    /// This is a clone operation and may be expensive for large position counts.
    ///
    /// # Returns
    /// Vector of all active positions
    pub fn get_all_positions(&self) -> Vec<ActivePosition> {
        self.positions
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }
    
    /// Get a specific position by mint
    ///
    /// # Arguments
    /// * `mint` - Token mint address
    ///
    /// # Returns
    /// `Some(ActivePosition)` if found, `None` otherwise
    pub fn get_position(&self, mint: &Pubkey) -> Option<ActivePosition> {
        self.positions.get(mint).map(|entry| entry.value().clone())
    }
    
    /// Check if a position exists for a mint
    ///
    /// # Arguments
    /// * `mint` - Token mint address
    ///
    /// # Returns
    /// `true` if a position exists, `false` otherwise
    pub fn has_position(&self, mint: &Pubkey) -> bool {
        self.positions.contains_key(mint)
    }
    
    /// Get the number of active positions
    ///
    /// # Returns
    /// Count of active positions
    pub fn position_count(&self) -> usize {
        self.positions.len()
    }
    
    /// Remove a position manually
    ///
    /// Useful for emergency stops or manual position cleanup.
    ///
    /// # Arguments
    /// * `mint` - Token mint address
    ///
    /// # Returns
    /// The removed position, if it existed
    pub fn remove_position(&self, mint: &Pubkey) -> Option<ActivePosition> {
        self.positions.remove(mint).map(|(_, pos)| pos)
    }
    
    /// Clear all positions
    ///
    /// Removes all tracked positions. Use with caution.
    pub fn clear_all(&self) {
        self.positions.clear();
    }
}

impl Default for PositionTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::task;

    #[test]
    fn test_active_position_remaining_amount() {
        let mut pos = ActivePosition {
            mint: Pubkey::new_unique(),
            entry_timestamp: 0,
            initial_token_amount: 1_000_000,
            initial_sol_cost: 10_000_000,
            sold_token_amount: 300_000,
            total_sol_from_sales: 5_000_000,
            last_seen_price: 0.00000001,
            last_update: Instant::now(),
        };
        
        assert_eq!(pos.remaining_token_amount(), 700_000);
        
        pos.sold_token_amount = 1_000_000;
        assert_eq!(pos.remaining_token_amount(), 0);
        
        // Test overflow protection
        pos.sold_token_amount = 1_500_000;
        assert_eq!(pos.remaining_token_amount(), 0);
    }

    #[test]
    fn test_active_position_sold_percent() {
        let pos = ActivePosition {
            mint: Pubkey::new_unique(),
            entry_timestamp: 0,
            initial_token_amount: 1_000_000,
            initial_sol_cost: 10_000_000,
            sold_token_amount: 250_000,
            total_sol_from_sales: 0,
            last_seen_price: 0.0,
            last_update: Instant::now(),
        };
        
        assert!((pos.sold_percent() - 25.0).abs() < 0.01);
        
        // Test zero initial amount
        let pos_zero = ActivePosition {
            mint: Pubkey::new_unique(),
            entry_timestamp: 0,
            initial_token_amount: 0,
            initial_sol_cost: 0,
            sold_token_amount: 0,
            total_sol_from_sales: 0,
            last_seen_price: 0.0,
            last_update: Instant::now(),
        };
        
        assert_eq!(pos_zero.sold_percent(), 0.0);
    }

    #[test]
    fn test_active_position_pnl_no_sells() {
        let pos = ActivePosition {
            mint: Pubkey::new_unique(),
            entry_timestamp: 0,
            initial_token_amount: 1_000_000,
            initial_sol_cost: 10_000_000, // 0.01 SOL
            sold_token_amount: 0,
            total_sol_from_sales: 0,
            last_seen_price: 0.00000001, // Entry price
            last_update: Instant::now(),
        };
        
        // Price doubles
        let (pnl_sol, pnl_percent) = pos.calculate_pnl(0.00000002);
        
        // Should have ~0.01 SOL profit (100% gain)
        assert!((pnl_sol - 0.01).abs() < 0.0001);
        assert!((pnl_percent - 100.0).abs() < 1.0);
        
        // Price halves
        let (pnl_sol, pnl_percent) = pos.calculate_pnl(0.000000005);
        
        // Should have ~-0.005 SOL loss (50% loss)
        assert!((pnl_sol + 0.005).abs() < 0.0001);
        assert!((pnl_percent + 50.0).abs() < 1.0);
    }

    #[test]
    fn test_active_position_pnl_with_partial_sells() {
        let pos = ActivePosition {
            mint: Pubkey::new_unique(),
            entry_timestamp: 0,
            initial_token_amount: 1_000_000,
            initial_sol_cost: 10_000_000, // 0.01 SOL for 1M tokens
            sold_token_amount: 500_000,   // Sold half
            total_sol_from_sales: 15_000_000, // Got 0.015 SOL back
            last_seen_price: 0.00000002, // Current price for remaining
            last_update: Instant::now(),
        };
        
        // Remaining 500k tokens at 0.00000002 = 0.01 SOL
        // Already got 0.015 SOL from sales
        // Total value = 0.015 + 0.01 = 0.025 SOL
        // Initial cost = 0.01 SOL
        // P&L = 0.025 - 0.01 = 0.015 SOL (150% gain)
        
        let (pnl_sol, pnl_percent) = pos.calculate_pnl(0.00000002);
        
        assert!((pnl_sol - 0.015).abs() < 0.0001);
        assert!((pnl_percent - 150.0).abs() < 1.0);
    }

    #[test]
    fn test_active_position_entry_price() {
        let pos = ActivePosition {
            mint: Pubkey::new_unique(),
            entry_timestamp: 0,
            initial_token_amount: 1_000_000,
            initial_sol_cost: 10_000_000, // 0.01 SOL
            sold_token_amount: 0,
            total_sol_from_sales: 0,
            last_seen_price: 0.0,
            last_update: Instant::now(),
        };
        
        let entry_price = pos.entry_price();
        assert!((entry_price - 0.00000001).abs() < 0.000000001);
    }

    #[test]
    fn test_position_tracker_new() {
        let tracker = PositionTracker::new();
        assert_eq!(tracker.position_count(), 0);
    }

    #[test]
    fn test_position_tracker_record_buy() {
        let tracker = PositionTracker::new();
        let mint = Pubkey::new_unique();
        
        tracker.record_buy(mint, 1_000_000, 10_000_000);
        
        assert_eq!(tracker.position_count(), 1);
        assert!(tracker.has_position(&mint));
        
        let pos = tracker.get_position(&mint).unwrap();
        assert_eq!(pos.mint, mint);
        assert_eq!(pos.initial_token_amount, 1_000_000);
        assert_eq!(pos.initial_sol_cost, 10_000_000);
        assert_eq!(pos.sold_token_amount, 0);
    }

    #[test]
    fn test_position_tracker_record_sell() {
        let tracker = PositionTracker::new();
        let mint = Pubkey::new_unique();
        
        tracker.record_buy(mint, 1_000_000, 10_000_000);
        
        let result = tracker.record_sell(&mint, 300_000, 5_000_000);
        assert!(result);
        
        let pos = tracker.get_position(&mint).unwrap();
        assert_eq!(pos.sold_token_amount, 300_000);
        assert_eq!(pos.total_sol_from_sales, 5_000_000);
        assert_eq!(pos.remaining_token_amount(), 700_000);
    }

    #[test]
    fn test_position_tracker_full_sell() {
        let tracker = PositionTracker::new();
        let mint = Pubkey::new_unique();
        
        tracker.record_buy(mint, 1_000_000, 10_000_000);
        
        // Sell everything
        tracker.record_sell(&mint, 1_000_000, 20_000_000);
        
        // Position should be removed
        assert_eq!(tracker.position_count(), 0);
        assert!(!tracker.has_position(&mint));
    }

    #[test]
    fn test_position_tracker_sell_nonexistent() {
        let tracker = PositionTracker::new();
        let mint = Pubkey::new_unique();
        
        let result = tracker.record_sell(&mint, 100_000, 1_000_000);
        assert!(!result);
    }

    #[test]
    fn test_position_tracker_update_price() {
        let tracker = PositionTracker::new();
        let mint = Pubkey::new_unique();
        
        tracker.record_buy(mint, 1_000_000, 10_000_000);
        
        let result = tracker.update_price(&mint, 0.00000002);
        assert!(result);
        
        let pos = tracker.get_position(&mint).unwrap();
        assert_eq!(pos.last_seen_price, 0.00000002);
    }

    #[test]
    fn test_position_tracker_multiple_positions() {
        let tracker = PositionTracker::new();
        
        for i in 0..10 {
            let mint = Pubkey::new_unique();
            tracker.record_buy(mint, 1_000_000 * (i + 1), 10_000_000 * (i + 1));
        }
        
        assert_eq!(tracker.position_count(), 10);
        
        let positions = tracker.get_all_positions();
        assert_eq!(positions.len(), 10);
    }

    #[test]
    fn test_position_tracker_remove_position() {
        let tracker = PositionTracker::new();
        let mint = Pubkey::new_unique();
        
        tracker.record_buy(mint, 1_000_000, 10_000_000);
        
        let removed = tracker.remove_position(&mint);
        assert!(removed.is_some());
        assert_eq!(tracker.position_count(), 0);
    }

    #[test]
    fn test_position_tracker_clear_all() {
        let tracker = PositionTracker::new();
        
        for _ in 0..5 {
            let mint = Pubkey::new_unique();
            tracker.record_buy(mint, 1_000_000, 10_000_000);
        }
        
        assert_eq!(tracker.position_count(), 5);
        
        tracker.clear_all();
        assert_eq!(tracker.position_count(), 0);
    }

    #[tokio::test]
    async fn test_concurrent_position_updates() {
        let tracker = Arc::new(PositionTracker::new());
        let mut handles = vec![];
        
        // Create 10 positions
        let mints: Vec<Pubkey> = (0..10).map(|_| Pubkey::new_unique()).collect();
        
        for mint in &mints {
            tracker.record_buy(*mint, 1_000_000, 10_000_000);
        }
        
        // Spawn 10 tasks, each updating all positions
        for _ in 0..10 {
            let tracker_clone = Arc::clone(&tracker);
            let mints_clone = mints.clone();
            
            let handle = task::spawn(async move {
                for mint in mints_clone {
                    // Update price
                    tracker_clone.update_price(&mint, 0.00000002);
                    
                    // Record a small sell
                    tracker_clone.record_sell(&mint, 1000, 20_000);
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }
        
        // All positions should still exist (not fully sold)
        assert_eq!(tracker.position_count(), 10);
        
        // Verify all positions were updated
        for mint in &mints {
            let pos = tracker.get_position(mint).unwrap();
            assert_eq!(pos.sold_token_amount, 10_000); // 10 tasks * 1000
            assert_eq!(pos.total_sol_from_sales, 200_000); // 10 tasks * 20_000
        }
    }

    #[tokio::test]
    async fn test_concurrent_buy_sell_operations() {
        let tracker = Arc::new(PositionTracker::new());
        let mint = Pubkey::new_unique();
        
        // Initial position
        tracker.record_buy(mint, 10_000_000, 100_000_000);
        
        let mut handles = vec![];
        
        // Spawn 20 tasks alternating between small sells and price updates
        for i in 0..20 {
            let tracker_clone = Arc::clone(&tracker);
            
            let handle = task::spawn(async move {
                if i % 2 == 0 {
                    // Even tasks: sell
                    tracker_clone.record_sell(&mint, 100_000, 1_000_000);
                } else {
                    // Odd tasks: update price
                    tracker_clone.update_price(&mint, 0.00000001 * (i as f64 + 1.0));
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Position should still exist (10M initial - 10*100k sold = 9M remaining)
        assert!(tracker.has_position(&mint));
        
        let pos = tracker.get_position(&mint).unwrap();
        assert_eq!(pos.sold_token_amount, 1_000_000); // 10 sell tasks * 100k
    }

    #[test]
    fn test_position_tracker_default() {
        let tracker: PositionTracker = Default::default();
        assert_eq!(tracker.position_count(), 0);
    }
}
