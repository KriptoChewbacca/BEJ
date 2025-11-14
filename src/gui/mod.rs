//! GUI Module - Monitoring dashboard for the trading bot
//!
//! This module provides a graphical user interface for monitoring the bot's
//! trading activity, positions, and P&L in real-time.
//!
//! ## Features
//!
//! - Real-time position tracking with P&L calculations
//! - Live price charts for active positions
//! - Bot control (START/STOP/PAUSE)
//! - 333ms refresh rate for smooth updates
//! - Zero performance impact on bot operations
//!
//! ## Usage
//!
//! The GUI is launched in a separate thread to avoid blocking the bot:
//!
//! ```no_run
//! use bot::gui::launch_monitoring_gui;
//! use bot::position_tracker::PositionTracker;
//! use bot::components::price_stream::PriceStreamManager;
//! use std::sync::Arc;
//! use std::sync::atomic::AtomicU8;
//! use std::time::Duration;
//!
//! # fn main() -> eframe::Result<()> {
//! let position_tracker = Arc::new(PositionTracker::new());
//! let price_stream = PriceStreamManager::new(1000, Duration::from_millis(333));
//! let bot_state = Arc::new(AtomicU8::new(1)); // Running
//!
//! // Launch in separate thread
//! std::thread::spawn(move || {
//!     let _ = launch_monitoring_gui(
//!         position_tracker,
//!         price_stream.subscribe(),
//!         bot_state,
//!     );
//! });
//! # Ok(())
//! # }
//! ```

pub mod monitoring_gui;

use crate::components::price_stream::PriceUpdate;
use crate::position_tracker::PositionTracker;
use monitoring_gui::MonitoringGui;
use std::sync::atomic::AtomicU8;
use std::sync::Arc;
use tokio::sync::broadcast;
use eframe::egui;

/// Launch the monitoring GUI
///
/// Creates and runs the monitoring GUI window. This function blocks until
/// the GUI window is closed, so it should be run in a separate thread.
///
/// # Arguments
/// * `position_tracker` - Shared position tracker from the bot
/// * `price_rx` - Broadcast receiver for price updates
/// * `bot_state` - Shared atomic bot state (0=Stopped, 1=Running, 2=Paused)
///
/// # Returns
/// `eframe::Result<()>` indicating success or error
///
/// # Example
/// ```no_run
/// use bot::gui::launch_monitoring_gui;
/// use bot::position_tracker::PositionTracker;
/// use bot::components::price_stream::PriceStreamManager;
/// use std::sync::Arc;
/// use std::sync::atomic::AtomicU8;
/// use std::time::Duration;
///
/// # fn main() {
/// let position_tracker = Arc::new(PositionTracker::new());
/// let price_stream = PriceStreamManager::new(1000, Duration::from_millis(333));
/// let bot_state = Arc::new(AtomicU8::new(1));
///
/// std::thread::spawn(move || {
///     let _ = launch_monitoring_gui(
///         position_tracker,
///         price_stream.subscribe(),
///         bot_state,
///     );
/// });
/// # }
/// ```
pub fn launch_monitoring_gui(
    position_tracker: Arc<PositionTracker>,
    price_rx: broadcast::Receiver<PriceUpdate>,
    bot_state: Arc<AtomicU8>,
) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Solana Sniper Bot - Monitoring Dashboard"),
        ..Default::default()
    };
    
    eframe::run_native(
        "Bot Monitor",
        options,
        Box::new(|_cc| {
            Ok(Box::new(MonitoringGui::new(
                position_tracker,
                price_rx,
                bot_state,
            )))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::price_stream::PriceStreamManager;
    use std::time::Duration;

    #[test]
    fn test_gui_module_compiles() {
        // Just verify the module compiles and types are correct
        let _tracker = Arc::new(PositionTracker::new());
        let _price_stream = PriceStreamManager::new(100, Duration::from_millis(333));
        let _bot_state = Arc::new(AtomicU8::new(0));
        
        // Can't actually launch GUI in tests, but we can verify the types
    }
}
