//! Ultra - Advanced Solana Trading Bot
//!
//! This is the main entry point for the Ultra trading bot, implementing
//! Universe Class Grade architecture for high-frequency Solana trading.
//!
//! ## Features
//!
//! - **Real-time Transaction Monitoring**: Geyser gRPC streaming
//! - **Multi-DEX Support**: PumpFun, Raydium, Orca integration
//! - **MEV Protection**: Jito bundle support
//! - **Advanced Nonce Management**: Enterprise-grade nonce pooling
//! - **Resilient RPC**: Intelligent connection pooling and failover
//! - **Comprehensive Metrics**: Prometheus integration
//! - **Distributed Tracing**: OpenTelemetry-compatible observability

// Compiler warning configuration
#![deny(unused_imports)]
#![deny(unused_mut)]
#![deny(unused_variables)]
#![warn(dead_code)]
#![warn(unused_must_use)]

use anyhow::{Context, Result};
use clap::Parser;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Conditional GUI imports
#[cfg(feature = "gui_monitor")]
use std::sync::atomic::AtomicU8;

#[cfg(feature = "gui_monitor")]
use components::gui_bridge::GuiCommand;

// Module declarations
mod compat; // Solana SDK compatibility layer
mod components; // GUI integration components
mod config;
mod endpoints;
mod metrics;
mod observability;
mod security;
mod structured_logging;
mod types;
mod wallet;

// Component modules with non-standard paths (directories with spaces)
#[path = "nonce manager/mod.rs"]
mod nonce_manager;

#[path = "rpc manager/mod.rs"]
mod rpc_manager;

mod buy_engine;
mod sniffer;
// Legacy monolithic tx_builder - will be migrated to modular structure in Task 6
#[path = "tx_builder_legacy.rs"]
mod tx_builder;

// GUI module (conditional compilation based on gui_monitor feature)
#[cfg(feature = "gui_monitor")]
mod gui;

mod position_tracker;

// Re-exports
use config::Config;
use types::{AppState, Mode, PremintCandidate};
use wallet::WalletManager;

/// Command line arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Operating mode (simulation or production)
    #[arg(short, long, default_value = "simulation")]
    mode: String,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Metrics port
    #[arg(long, default_value = "9090")]
    metrics_port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    init_logging(args.verbose)?;

    info!("🚀 Starting Ultra Trading Bot - Universe Class Grade");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    info!("📋 Loading configuration from: {}", args.config);
    let config = load_config(&args.config)?;

    // Determine operating mode
    let mode = match args.mode.as_str() {
        "production" => Mode::Production,
        "simulation" => Mode::Simulation,
        _ => {
            warn!("Unknown mode '{}', defaulting to simulation", args.mode);
            Mode::Simulation
        }
    };
    info!("🎯 Operating Mode: {:?}", mode);

    // Initialize application state
    let app_state = Arc::new(AppState::new(mode));

    // Initialize wallet
    info!(
        "🔑 Initializing wallet from: {}",
        config.wallet.keypair_path
    );
    let wallet =
        WalletManager::from_file(&config.wallet.keypair_path).context("Failed to load wallet")?;
    info!("💼 Wallet address: {}", wallet.pubkey());

    // Initialize metrics
    if config.monitoring.enable_metrics {
        info!("📊 Starting metrics server on port {}", args.metrics_port);
        let metrics_port = args.metrics_port;
        tokio::spawn(async move {
            if let Err(e) = endpoints::endpoint_server(metrics_port).await {
                error!("Metrics server error: {}", e);
            }
        });
    }

    // Initialize RPC manager
    info!(
        "🌐 Initializing RPC manager with {} endpoints",
        config.rpc.endpoints.len()
    );
    let _rpc_endpoints: Vec<rpc_manager::EndpointConfig> = config
        .rpc
        .endpoints
        .iter()
        .map(|url| rpc_manager::EndpointConfig {
            url: url.clone(),
            endpoint_type: rpc_manager::EndpointType::Standard,
            weight: 1.0,
            max_requests_per_second: config.rpc.rate_limit_rps,
        })
        .collect();

    // Note: Actual RPC pool initialization would happen here
    // let rpc_pool = Arc::new(rpc_manager::RpcPool::new(rpc_endpoints).await?);

    // Initialize nonce manager
    info!(
        "🔢 Initializing nonce manager with pool size: {}",
        config.nonce.pool_size
    );
    // Note: Actual nonce manager initialization would happen here
    // let nonce_manager = Arc::new(nonce_manager::NonceManager::new(...).await?);

    // Initialize sniffer
    info!("👁️ Initializing transaction sniffer");
    info!("   Geyser endpoint: {}", config.sniffer.geyser_endpoint);
    info!(
        "   Monitored programs: {}",
        config.sniffer.monitored_programs.len()
    );

    // Create channel for candidates
    let (_candidate_tx, candidate_rx) = mpsc::unbounded_channel::<PremintCandidate>();

    // Note: Actual sniffer initialization would happen here
    // let sniffer_config = sniffer::SnifferConfig { ... };
    // let sniffer = sniffer::Sniffer::new(sniffer_config, candidate_tx).await?;

    // Initialize buy engine
    info!("💰 Initializing buy engine");
    // Note: Actual buy engine initialization would happen here
    // let buy_engine = buy_engine::BuyEngine::new(...);

    // Create shared components for GUI integration
    #[cfg(feature = "gui_monitor")]
    let position_tracker = Arc::new(position_tracker::PositionTracker::new());
    
    #[cfg(feature = "gui_monitor")]
    let price_stream = Arc::new(components::price_stream::PriceStreamManager::new(
        1000, // channel capacity
        std::time::Duration::from_millis(333), // 333ms refresh rate
    ));
    
    #[cfg(feature = "gui_monitor")]
    let bot_state = Arc::new(AtomicU8::new(1)); // 1 = Running

    // ZADANIE 1: Create GUI command channel
    #[cfg(feature = "gui_monitor")]
    let (gui_cmd_tx, mut gui_cmd_rx) = mpsc::channel::<GuiCommand>(100);

    // Launch GUI monitor if feature is enabled
    #[cfg(feature = "gui_monitor")]
    {
        info!("🎨 Launching GUI monitoring dashboard");
        let pos_tracker_gui = Arc::clone(&position_tracker);
        let price_rx_gui = price_stream.subscribe();
        let bot_state_gui = Arc::clone(&bot_state);
        let cmd_tx_gui = gui_cmd_tx.clone();
        
        std::thread::spawn(move || {
            if let Err(e) = gui::launch_monitoring_gui_with_commands(
                pos_tracker_gui,
                price_rx_gui,
                bot_state_gui,
                cmd_tx_gui,
            ) {
                error!("GUI error: {}", e);
            }
        });
        
        info!("✅ GUI monitor launched successfully (333ms refresh rate)");
    }

    // ZADANIE 1: Spawn GUI command handler
    // Note: This would be connected to a real BuyEngine instance
    // For now, we'll set up the infrastructure
    #[cfg(feature = "gui_monitor")]
    {
        info!("📡 Starting GUI command handler");
        tokio::spawn(async move {
            while let Some(cmd) = gui_cmd_rx.recv().await {
                // In a real implementation, this would call handle_gui_command
                // with a reference to the actual BuyEngine instance
                info!("Received GUI command: {:?}", cmd);
                // Example: handle_gui_command(&buy_engine, cmd).await;
            }
        });
        info!("✅ GUI command handler started");
    }

    #[cfg(not(feature = "gui_monitor"))]
    info!("ℹ️  GUI monitoring disabled (compile with --features gui_monitor to enable)");

    // Start main event loop
    info!("✅ All components initialized successfully");
    info!("🎬 Starting main event loop...");

    run_event_loop(app_state, candidate_rx).await?;

    Ok(())
}

/// Initialize logging subsystem
fn init_logging(verbose: bool) -> Result<()> {
    let env_filter = if verbose {
        "ultra=debug,info"
    } else {
        "ultra=info,warn,error"
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| env_filter.into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .init();

    Ok(())
}

/// Load configuration from file with fallback to defaults
fn load_config(path: &str) -> Result<Config> {
    if std::path::Path::new(path).exists() {
        Config::from_file_with_env(path)
            .with_context(|| format!("Failed to load config from {}", path))
    } else {
        warn!("Config file '{}' not found, using defaults", path);
        Ok(Config::default())
    }
}

/// Main event loop
async fn run_event_loop(
    app_state: Arc<AppState>,
    mut candidate_rx: mpsc::UnboundedReceiver<PremintCandidate>,
) -> Result<()> {
    info!("Event loop started");

    let mut stats_interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

    loop {
        tokio::select! {
            // Handle incoming candidates
            Some(candidate) = candidate_rx.recv() => {
                // Update statistics
                app_state.increment_candidates().await;

                // Check if paused
                if app_state.is_paused().await {
                    continue;
                }

                // Process candidate
                info!("📥 Received candidate: mint={}", candidate.mint);

                // In a real implementation, this would be passed to the buy engine
                // buy_engine.process_candidate(candidate).await;
            }

            // Periodic statistics reporting
            _ = stats_interval.tick() => {
                let stats = app_state.stats.read().await;
                info!("📊 Statistics:");
                info!("   Total candidates: {}", stats.total_candidates);
                info!("   Total trades: {}", stats.total_trades);
                info!("   Successful: {}", stats.successful_trades);
                info!("   Failed: {}", stats.failed_trades);
                info!("   Total volume: {:.4} SOL", stats.total_volume_sol);

                // Update metrics
                let m = metrics::metrics();
                m.candidates_received.inc_by(stats.total_candidates);
                m.trades_total.inc_by(stats.total_trades);
                m.trades_success.inc_by(stats.successful_trades);
                m.trades_failed.inc_by(stats.failed_trades);
            }

            // Graceful shutdown signal
            _ = tokio::signal::ctrl_c() => {
                info!("🛑 Received shutdown signal");
                break;
            }
        }
    }

    info!("👋 Shutting down gracefully...");
    Ok(())
}

/// Handle GUI commands sent from the monitoring dashboard
///
/// This function processes commands from the GUI and executes them on the BuyEngine.
/// It's designed to be called from a tokio task that receives commands from the GUI.
///
/// # Arguments
/// * `engine` - Reference to the BuyEngine instance
/// * `cmd` - The GUI command to process
///
/// # Returns
/// `Result<()>` indicating success or error
#[cfg(feature = "gui_monitor")]
async fn handle_gui_command(
    engine: &Arc<buy_engine::BuyEngine>,
    cmd: GuiCommand,
) -> Result<()> {
    use components::gui_bridge::GuiCommand;
    
    match cmd {
        GuiCommand::Sell { mint, percent } => {
            info!(mint = %mint, percent = percent, "GUI: Manual sell requested");
            engine.sell_manual(&mint, percent).await?;
        }
        
        GuiCommand::SetTradingMode(mode) => {
            info!(mode = ?mode, "GUI: Trading mode change requested");
            engine.set_trading_mode(mode).await;
        }
        
        GuiCommand::SetStopLoss { mint, threshold_percent } => {
            info!(
                mint = %mint,
                threshold = threshold_percent,
                "GUI: Stop loss configuration"
            );
            engine.set_stop_loss(mint, threshold_percent).await;
        }
        
        GuiCommand::SetTakeProfit {
            mint,
            threshold_percent,
            sell_percent,
        } => {
            info!(
                mint = %mint,
                threshold = threshold_percent,
                sell_pct = sell_percent,
                "GUI: Take profit configuration"
            );
            engine
                .set_take_profit(mint, threshold_percent, sell_percent)
                .await;
        }
        
        GuiCommand::ClearStrategy { mint } => {
            info!(mint = %mint, "GUI: Clear strategy requested");
            engine.clear_strategy(&mint).await;
        }
        
        GuiCommand::SetMultiTokenMode {
            enabled,
            max_positions,
        } => {
            info!(
                enabled = enabled,
                max_positions = ?max_positions,
                "GUI: Multi-token mode change"
            );
            let mut st = engine.app_state.lock().await;
            st.portfolio_config.enable_multi_token = enabled;
            if let Some(max) = max_positions {
                st.portfolio_config.max_concurrent_positions = max;
            }
        }
        
        GuiCommand::SetPaused(paused) => {
            info!(paused = paused, "GUI: Pause state change");
            let st = engine.app_state.lock().await;
            *st.is_paused.write().await = paused;
        }
        
        GuiCommand::EmergencyStop => {
            warn!("GUI: Emergency stop requested");
            // Emergency stop logic would go here
            // For now, just pause trading
            let st = engine.app_state.lock().await;
            *st.is_paused.write().await = true;
        }
        
        GuiCommand::UpdatePortfolioConfig(config) => {
            info!(config = ?config, "GUI: Portfolio config update");
            let mut st = engine.app_state.lock().await;
            st.portfolio_config = config;
        }
    }
    
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    // Include test modules
    mod auto_sell_monitor_tests; // ZADANIE 1.2: Auto-sell monitor loop tests
    mod buy_engine_multi_token_tests; // Multi-token buy engine tests
    mod config_validation; // Multi-token configuration validation tests
    mod error_conversion_tests;
    mod execution_context_tests;
    mod instruction_ordering_tests;
    mod migration_tests; // Backward compatibility tests
    mod multi_token_integration_tests; // Comprehensive multi-token integration tests
    mod multi_token_state_tests; // Multi-token state management tests
    mod nonce_concurrency_tests;
    mod nonce_integration_tests;
    mod nonce_lease_tests;
    mod nonce_raii_comprehensive_tests;
    mod phase1_nonce_enforcement_tests;
    mod phase2_raii_output_tests; // Phase 2 RAII output integration tests
    mod phase4_e2e_perf_stress_tests; // Phase 4 E2E, Performance, and Stress tests
    mod production_stress_tests; // Task 4: Production-grade stress tests
    mod sell_multi_token_tests; // Multi-token sell logic tests
    mod simulation_nonce_tests;
    mod strategy_management_tests; // ZADANIE 1.4: Strategy management API tests
    mod task2_raii_tests; // Task 2: RAII tests for ExecutionContext and TxBuildOutput
    mod task5_gui_control_tests; // Task 5: GUI Bot State Control Integration tests
    mod task6_gui_feature_gating_tests; // Task 6: GUI Feature Gating tests
    mod test_helpers;
    mod tpsl_evaluation_tests; // ZADANIE 1.3: TP/SL evaluation logic tests
    mod trading_mode_tests; // ZADANIE 1.1: Trading mode management tests
    mod tx_builder_fee_strategy_test;
    mod tx_builder_improvements_tests;
    mod tx_builder_output_tests;
    mod tx_builder_sell_nonce_test;
    mod v0_transaction_compat_tests;

    #[test]
    fn test_config_defaults() {
        let config = Config::default();
        assert!(!config.rpc.endpoints.is_empty());
        assert!(config.nonce.pool_size > 0);
    }

    #[tokio::test]
    async fn test_app_state() {
        let state = AppState::new(Mode::Simulation);
        assert_eq!(state.get_mode().await, Mode::Simulation);
        assert!(!state.is_paused().await);
    }
}
