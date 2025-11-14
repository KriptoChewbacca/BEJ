//! GUI Monitoring Example
//!
//! This example demonstrates how to launch the monitoring GUI for the trading bot.
//! It creates a minimal setup with:
//! - Position tracker for tracking buy/sell operations
//! - Price stream for real-time price updates
//! - Shared bot state for START/STOP control
//!
//! The GUI will display:
//! - Active positions with P&L calculations
//! - Price charts for selected positions
//! - Bot status and control buttons
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example gui_monitoring
//! ```
//!
//! ## Note
//!
//! This is a standalone example. In production, the GUI would be integrated
//! with the main bot application by passing the actual position tracker,
//! price stream, and bot state from the BuyEngine.

use bot::components::price_stream::{PriceStreamManager, PriceUpdate};
use bot::gui::launch_monitoring_gui;
use bot::position_tracker::PositionTracker;
use solana_sdk::pubkey::Pubkey;
use std::sync::atomic::AtomicU8;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result<()> {
    // Initialize components
    let position_tracker = Arc::new(PositionTracker::new());
    let price_stream = Arc::new(PriceStreamManager::new(1000, Duration::from_millis(333)));
    let bot_state = Arc::new(AtomicU8::new(1)); // Start in Running state

    // Add some demo positions
    let demo_mint1 = Pubkey::new_unique();
    let demo_mint2 = Pubkey::new_unique();

    position_tracker.record_buy(demo_mint1, 1_000_000, 10_000_000); // 1M tokens for 0.01 SOL
    position_tracker.record_buy(demo_mint2, 2_000_000, 30_000_000); // 2M tokens for 0.03 SOL

    // Simulate price updates in a background thread
    let price_stream_clone = Arc::clone(&price_stream);
    let position_tracker_clone = Arc::clone(&position_tracker);
    std::thread::spawn(move || {
        let mut counter = 0;
        loop {
            std::thread::sleep(Duration::from_millis(500));
            counter += 1;

            // Simulate price fluctuation
            let price1 = 0.00000001 + (counter as f64 * 0.000000001);
            let price2 = 0.000000015 + (counter as f64 * 0.0000000005);

            // Update prices
            position_tracker_clone.update_price(&demo_mint1, price1);
            position_tracker_clone.update_price(&demo_mint2, price2);

            // Publish price updates
            price_stream_clone.publish_price(PriceUpdate {
                mint: demo_mint1,
                price_sol: price1,
                price_usd: price1 * 150.0,
                volume_24h: 100_000.0,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                source: "demo".to_string(),
            });

            price_stream_clone.publish_price(PriceUpdate {
                mint: demo_mint2,
                price_sol: price2,
                price_usd: price2 * 150.0,
                volume_24h: 200_000.0,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                source: "demo".to_string(),
            });

            // Simulate a partial sell after 10 seconds
            if counter == 20 {
                println!("Simulating partial sell of demo_mint1...");
                position_tracker_clone.record_sell(&demo_mint1, 500_000, 15_000_000);
            }
        }
    });

    // Launch GUI (blocks until window is closed)
    println!("Launching monitoring GUI...");
    println!("The GUI will show 2 demo positions with simulated price updates.");
    println!("After 10 seconds, a partial sell will be simulated.");
    println!("Close the window to exit.");

    launch_monitoring_gui(position_tracker, price_stream.subscribe(), bot_state)
}
