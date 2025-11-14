//! Monitoring GUI Module - Real-time bot monitoring dashboard
//!
//! This module provides a lightweight GUI for monitoring the trading bot's
//! active positions, P&L, and price charts with a 333ms refresh rate.
//!
//! ## Key Features
//!
//! - **333ms Refresh Rate**: Smooth, real-time updates without overwhelming the UI
//! - **Zero Performance Impact**: Non-blocking reads from shared state
//! - **Position Tracking**: Live P&L calculations for all active positions
//! - **Price Charts**: Historical price visualization with egui_plot
//! - **Bot Control**: START/STOP/PAUSE controls
//!
//! ## Architecture
//!
//! The GUI runs in its own thread and communicates with the bot via:
//! - `PositionTracker`: Lock-free position data (DashMap)
//! - `PriceStream`: Broadcast channel for price updates
//! - `AtomicU8`: Shared bot state (Running/Stopped/Paused)

use crate::components::price_stream::PriceUpdate;
use crate::position_tracker::PositionTracker;
use eframe::egui::{self, Button, Color32, Ui};
use egui_plot::{Line, Plot, PlotPoints};
use solana_sdk::pubkey::Pubkey;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

/// GUI refresh interval (333ms for smooth updates)
const GUI_REFRESH_INTERVAL: Duration = Duration::from_millis(333);

/// Maximum price history points to store per token
const MAX_PRICE_HISTORY: usize = 1024;

/// Monitoring GUI application
///
/// Provides a real-time dashboard for monitoring the trading bot's
/// performance, positions, and price movements.
pub struct MonitoringGui {
    // Data sources (read-only from bot)
    /// Position tracker (shared with BuyEngine)
    position_tracker: Arc<PositionTracker>,

    /// Price update receiver (broadcast channel)
    price_rx: broadcast::Receiver<PriceUpdate>,

    /// Bot state (0=Stopped, 1=Running, 2=Paused)
    bot_state: Arc<AtomicU8>,

    // UI state (local to GUI)
    /// Price history for chart visualization
    /// Maps mint -> VecDeque of (timestamp, price)
    price_history: HashMap<Pubkey, VecDeque<(f64, f64)>>,

    /// Timestamp of last UI update
    last_update: Instant,

    /// Currently selected mint for detail view
    selected_mint: Option<Pubkey>,
}

impl MonitoringGui {
    /// Create a new monitoring GUI
    ///
    /// # Arguments
    /// * `position_tracker` - Shared position tracker from the bot
    /// * `price_rx` - Broadcast receiver for price updates
    /// * `bot_state` - Shared atomic bot state
    ///
    /// # Returns
    /// A new MonitoringGui instance
    pub fn new(
        position_tracker: Arc<PositionTracker>,
        price_rx: broadcast::Receiver<PriceUpdate>,
        bot_state: Arc<AtomicU8>,
    ) -> Self {
        Self {
            position_tracker,
            price_rx,
            bot_state,
            price_history: HashMap::new(),
            last_update: Instant::now(),
            selected_mint: None,
        }
    }

    /// Poll for price updates (non-blocking)
    ///
    /// Reads all available price updates from the broadcast channel
    /// and updates the price history for chart visualization.
    fn poll_price_updates(&mut self) {
        // Drain all available price updates
        while let Ok(price_update) = self.price_rx.try_recv() {
            self.update_price_history(price_update);
        }
    }

    /// Update price history with a new price point
    ///
    /// Maintains a ring buffer of price points for each token.
    fn update_price_history(&mut self, update: PriceUpdate) {
        let history = self
            .price_history
            .entry(update.mint)
            .or_insert_with(|| VecDeque::with_capacity(MAX_PRICE_HISTORY));

        history.push_back((update.timestamp as f64, update.price_sol));

        // Maintain ring buffer size
        if history.len() > MAX_PRICE_HISTORY {
            history.pop_front();
        }
    }

    /// Refresh positions from the position tracker
    ///
    /// This is called periodically to ensure the UI reflects the latest state.
    /// Since PositionTracker uses DashMap, this is a lock-free operation.
    fn refresh_positions(&mut self) {
        // Positions are already tracked by PositionTracker
        // This method can be used for periodic cleanup or validation

        // Clean up price history for positions that no longer exist
        let active_mints: Vec<Pubkey> = self
            .position_tracker
            .get_all_positions()
            .iter()
            .map(|pos| pos.mint)
            .collect();

        self.price_history
            .retain(|mint, _| active_mints.contains(mint));

        // Clear selected mint if position is gone
        if let Some(mint) = self.selected_mint {
            if !self.position_tracker.has_position(&mint) {
                self.selected_mint = None;
            }
        }
    }

    /// Render the main UI
    fn render_ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🎯 Solana Sniper Bot - Monitoring Dashboard");
            ui.separator();

            // Control Panel
            self.render_control_panel(ui);
            ui.separator();

            // Position List
            self.render_position_list(ui);
            ui.separator();

            // Selected Position Details + Chart
            if let Some(mint) = self.selected_mint {
                self.render_position_details(ui, mint);
            }
        });
    }

    /// Render the bot control panel
    ///
    /// Shows bot status and START/STOP/PAUSE controls
    fn render_control_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let current_state = self.bot_state.load(Ordering::Relaxed);
            let is_running = current_state == 1;

            // START/STOP button
            let button_text = if is_running { "⏸ STOP" } else { "▶ START" };
            let button_color = if is_running {
                Color32::from_rgb(255, 100, 100) // Red when running (to stop)
            } else {
                Color32::from_rgb(100, 255, 100) // Green when stopped (to start)
            };

            if ui
                .add(Button::new(button_text).fill(button_color))
                .clicked()
            {
                let new_state = if is_running { 0 } else { 1 };
                self.bot_state.store(new_state, Ordering::Relaxed);
            }

            ui.separator();

            // Status indicator
            let (status_text, status_color) = match current_state {
                0 => ("🔴 STOPPED", Color32::from_rgb(255, 100, 100)),
                1 => ("🟢 RUNNING", Color32::from_rgb(100, 255, 100)),
                2 => ("🟡 PAUSED", Color32::from_rgb(255, 255, 100)),
                _ => ("⚪ UNKNOWN", Color32::GRAY),
            };
            ui.colored_label(status_color, status_text);

            ui.separator();

            // Position count
            let position_count = self.position_tracker.position_count();
            ui.label(format!("📊 Active Positions: {}", position_count));
        });
    }

    /// Render the list of active positions
    ///
    /// Shows a table with key metrics for each position
    fn render_position_list(&mut self, ui: &mut Ui) {
        ui.heading("Active Positions");

        let positions = self.position_tracker.get_all_positions();

        if positions.is_empty() {
            ui.label("No active positions");
            return;
        }

        egui::Grid::new("position_grid")
            .num_columns(6)
            .striped(true)
            .show(ui, |ui| {
                // Header
                ui.label("Token");
                ui.label("Amount");
                ui.label("Entry Price");
                ui.label("Current Price");
                ui.label("P&L SOL");
                ui.label("P&L %");
                ui.end_row();

                // Rows
                for pos in &positions {
                    let (pnl_sol, pnl_percent) = pos.calculate_pnl(pos.last_seen_price);

                    // Clickable mint (for selection)
                    let mint_str = pos.mint.to_string();
                    let mint_short = if mint_str.len() >= 8 {
                        format!("{}...{}", &mint_str[..4], &mint_str[mint_str.len() - 4..])
                    } else {
                        mint_str.clone()
                    };

                    if ui.button(&mint_short).clicked() {
                        self.selected_mint = Some(pos.mint);
                    }

                    ui.label(format!("{}", pos.remaining_token_amount()));

                    let entry_price = pos.entry_price();
                    ui.label(format!("{:.9} SOL", entry_price));
                    ui.label(format!("{:.9} SOL", pos.last_seen_price));

                    // Color-coded P&L
                    let pnl_color = if pnl_sol >= 0.0 {
                        Color32::GREEN
                    } else {
                        Color32::RED
                    };
                    ui.colored_label(pnl_color, format!("{:+.4} SOL", pnl_sol));
                    ui.colored_label(pnl_color, format!("{:+.2}%", pnl_percent));

                    ui.end_row();
                }
            });
    }

    /// Render detailed view for a selected position
    ///
    /// Shows price chart and detailed metrics
    fn render_position_details(&mut self, ui: &mut Ui, mint: Pubkey) {
        ui.heading("📈 Position Details");

        // Get position data
        if let Some(pos) = self.position_tracker.get_position(&mint) {
            // Position info
            ui.horizontal(|ui| {
                ui.label(format!("Token: {}", mint));
                ui.separator();
                ui.label(format!("Entry: {:.9} SOL", pos.entry_price()));
                ui.separator();
                ui.label(format!("Current: {:.9} SOL", pos.last_seen_price));
                ui.separator();
                ui.label(format!("Sold: {:.1}%", pos.sold_percent()));
            });

            ui.separator();

            // Price chart
            if let Some(history) = self.price_history.get(&mint) {
                if !history.is_empty() {
                    let points: PlotPoints = history
                        .iter()
                        .enumerate()
                        .map(|(i, (_, price))| [i as f64, *price])
                        .collect();

                    Plot::new("price_chart")
                        .view_aspect(2.0)
                        .height(200.0)
                        .show(ui, |plot_ui| {
                            plot_ui.line(Line::new(points));
                        });
                } else {
                    ui.label("No price history available");
                }
            } else {
                ui.label("No price history available");
            }
        } else {
            ui.label("Position no longer active");
            self.selected_mint = None;
        }
    }
}

impl eframe::App for MonitoringGui {
    /// Update the GUI
    ///
    /// Called by eframe on every frame. This method:
    /// 1. Polls for price updates (non-blocking)
    /// 2. Refreshes position data at 333ms intervals
    /// 3. Renders the UI
    /// 4. Requests repaint for smooth updates
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll price updates (non-blocking)
        self.poll_price_updates();

        // Refresh on interval
        if self.last_update.elapsed() >= GUI_REFRESH_INTERVAL {
            self.refresh_positions();
            self.last_update = Instant::now();
        }

        // Request repaint for smooth updates
        ctx.request_repaint_after(GUI_REFRESH_INTERVAL);

        // Render UI
        self.render_ui(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_test_price_update(mint: Pubkey, price: f64) -> PriceUpdate {
        PriceUpdate {
            mint,
            price_sol: price,
            price_usd: price * 150.0,
            volume_24h: 100_000.0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            source: "test".to_string(),
        }
    }

    #[test]
    fn test_monitoring_gui_creation() {
        let tracker = Arc::new(PositionTracker::new());
        let (_tx, rx) = broadcast::channel(100);
        let bot_state = Arc::new(AtomicU8::new(0));

        let _gui = MonitoringGui::new(tracker, rx, bot_state);
        // Should create without panic
    }

    #[test]
    fn test_update_price_history() {
        let tracker = Arc::new(PositionTracker::new());
        let (_tx, rx) = broadcast::channel(100);
        let bot_state = Arc::new(AtomicU8::new(0));

        let mut gui = MonitoringGui::new(tracker, rx, bot_state);

        let mint = Pubkey::new_unique();
        let update = create_test_price_update(mint, 0.01);

        gui.update_price_history(update);

        assert!(gui.price_history.contains_key(&mint));
        assert_eq!(gui.price_history.get(&mint).unwrap().len(), 1);
    }

    #[test]
    fn test_price_history_ring_buffer() {
        let tracker = Arc::new(PositionTracker::new());
        let (_tx, rx) = broadcast::channel(100);
        let bot_state = Arc::new(AtomicU8::new(0));

        let mut gui = MonitoringGui::new(tracker, rx, bot_state);

        let mint = Pubkey::new_unique();

        // Add more than MAX_PRICE_HISTORY updates
        for i in 0..(MAX_PRICE_HISTORY + 100) {
            let update = create_test_price_update(mint, 0.01 * (i as f64 + 1.0));
            gui.update_price_history(update);
        }

        // Should maintain ring buffer size
        let history = gui.price_history.get(&mint).unwrap();
        assert_eq!(history.len(), MAX_PRICE_HISTORY);

        // Should have the latest values
        let last_price = history.back().unwrap().1;
        assert!(last_price > 10.0); // Should be from the later updates
    }

    #[test]
    fn test_bot_state_control() {
        let tracker = Arc::new(PositionTracker::new());
        let (_tx, rx) = broadcast::channel(100);
        let bot_state = Arc::new(AtomicU8::new(0)); // Stopped

        let _gui = MonitoringGui::new(tracker, rx, Arc::clone(&bot_state));

        // Change state to running
        bot_state.store(1, Ordering::Relaxed);
        assert_eq!(bot_state.load(Ordering::Relaxed), 1);

        // Change state to paused
        bot_state.store(2, Ordering::Relaxed);
        assert_eq!(bot_state.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_refresh_positions_cleanup() {
        let tracker = Arc::new(PositionTracker::new());
        let (_tx, rx) = broadcast::channel(100);
        let bot_state = Arc::new(AtomicU8::new(0));

        let mut gui = MonitoringGui::new(tracker.clone(), rx, bot_state);

        // Add price history for some mints
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();

        gui.update_price_history(create_test_price_update(mint1, 0.01));
        gui.update_price_history(create_test_price_update(mint2, 0.02));

        assert_eq!(gui.price_history.len(), 2);

        // Add only mint1 to position tracker
        tracker.record_buy(mint1, 1_000_000, 10_000_000);

        // Refresh should clean up mint2 from price history
        gui.refresh_positions();

        assert_eq!(gui.price_history.len(), 1);
        assert!(gui.price_history.contains_key(&mint1));
        assert!(!gui.price_history.contains_key(&mint2));
    }

    #[test]
    fn test_selected_mint_cleanup() {
        let tracker = Arc::new(PositionTracker::new());
        let (_tx, rx) = broadcast::channel(100);
        let bot_state = Arc::new(AtomicU8::new(0));

        let mut gui = MonitoringGui::new(tracker.clone(), rx, bot_state);

        let mint = Pubkey::new_unique();
        tracker.record_buy(mint, 1_000_000, 10_000_000);

        gui.selected_mint = Some(mint);

        // Sell the entire position
        tracker.record_sell(&mint, 1_000_000, 20_000_000);

        // Refresh should clear selected mint
        gui.refresh_positions();

        assert!(gui.selected_mint.is_none());
    }

    #[tokio::test]
    async fn test_poll_price_updates() {
        let tracker = Arc::new(PositionTracker::new());
        let (tx, rx) = broadcast::channel(100);
        let bot_state = Arc::new(AtomicU8::new(0));

        let mut gui = MonitoringGui::new(tracker, rx, bot_state);

        // Send some price updates
        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();

        let _ = tx.send(create_test_price_update(mint1, 0.01));
        let _ = tx.send(create_test_price_update(mint2, 0.02));

        // Give broadcast time to propagate
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Poll updates
        gui.poll_price_updates();

        // Should have received both updates
        assert_eq!(gui.price_history.len(), 2);
        assert!(gui.price_history.contains_key(&mint1));
        assert!(gui.price_history.contains_key(&mint2));
    }
}
