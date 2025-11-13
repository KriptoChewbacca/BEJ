use crate::bot::ActivePosition;
use crate::components::manual_controller::{ManualCommand, ManualResponse};
use crate::components::price_stream::PriceData;
use eframe::{egui, egui::Ui};
use egui::widgets::Button;
use egui_plot::{Line, Plot, PlotPoints};
use solana_sdk::pubkey::Pubkey;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const PRICE_HISTORY_CAP: usize = 1024;
const UI_PRICE_REFRESH: Duration = Duration::from_secs(1);

#[derive(PartialEq)]
pub enum UiMode {
    Auto,   // auto buy + auto sell
    Hybrid, // auto buy + manual sell (== SellStrategy::Manual po stronie kontrolera)
}

pub struct ManualGuiApp {
    command_tx: mpsc::Sender<ManualCommand>,
    response_rx: mpsc::Receiver<ManualResponse>,
    current_mode: UiMode,
    active_mint: String,
    tracked_mint: Option<Pubkey>,
    last_update: Instant,

    // Live data
    current_price: Option<PriceData>,
    position_info: Option<ActivePosition>,
    price_history: VecDeque<(f64, f64)>, // (timestamp, price)

    // SL/TP forms
    stop_loss_input: String,
    take_profit_input: String,

    // Metrics data
    current_metrics_tx_count: Option<u64>,
    current_metrics_holders: Option<u64>,

    // Status messages
    status_message: String,
    status_is_error: bool,
}

impl ManualGuiApp {
    pub fn new(
        command_tx: mpsc::Sender<ManualCommand>,
        response_rx: mpsc::Receiver<ManualResponse>,
    ) -> Self {
        Self {
            command_tx,
            response_rx,
            current_mode: UiMode::Hybrid,
            active_mint: String::new(),
            tracked_mint: None,
            last_update: Instant::now(),
            current_price: None,
            position_info: None,
            price_history: VecDeque::with_capacity(PRICE_HISTORY_CAP),
            stop_loss_input: String::new(),
            take_profit_input: String::new(),
            current_metrics_tx_count: None,
            current_metrics_holders: None,
            status_message: "Ready".to_string(),
            status_is_error: false,
        }
    }

    fn send_command(&self, command: ManualCommand) {
        let tx = self.command_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = tx.send(command).await {
                eprintln!("Failed to send command: {:?}", e);
            }
        });
    }

    fn poll_responses(&mut self) {
        // Process pending responses non-blocking
        while let Ok(response) = self.response_rx.try_recv() {
            self.handle_response(response);
        }
    }

    fn handle_response(&mut self, response: ManualResponse) {
        match response {
            ManualResponse::Success { message } => {
                self.status_message = format!("✅ {}", message);
                self.status_is_error = false;
            }
            ManualResponse::Error { error } => {
                self.status_message = format!("❌ {}", error);
                self.status_is_error = true;
            }
            ManualResponse::PositionInfo {
                position,
                current_price,
            } => {
                self.position_info = position;
                if let Some(price) = current_price {
                    self.update_price_data(price);
                }
            }
            ManualResponse::PriceInfo { price } => {
                if let Some(price_data) = price {
                    self.update_price_data(price_data);
                }
            }
            ManualResponse::Metrics {
                mint: _,
                tx_count,
                holders,
            } => {
                // Store metrics data for display in the GUI
                self.current_metrics_tx_count = tx_count;
                self.current_metrics_holders = holders;

                let metrics_msg = format!(
                    "📊 Metrics - TX: {}, Holders: {}",
                    tx_count.map_or("N/A".to_string(), |c| c.to_string()),
                    holders.map_or("N/A".to_string(), |h| h.to_string())
                );
                self.status_message = metrics_msg;
                self.status_is_error = false;
            }
            ManualResponse::RiskCleared {
                mint: _,
                cleared_sl,
                cleared_tp,
            } => {
                let mut cleared_items = Vec::new();
                if cleared_sl {
                    cleared_items.push("SL");
                }
                if cleared_tp {
                    cleared_items.push("TP");
                }

                let message = if cleared_items.is_empty() {
                    "No risk rules to clear".to_string()
                } else {
                    format!("🧹 Cleared: {}", cleared_items.join(", "))
                };

                self.status_message = message;
                self.status_is_error = false;
            }
        }
    }

    fn update_price_data(&mut self, price_data: PriceData) {
        let timestamp = price_data.timestamp as f64;

        // Update current price
        self.current_price = Some(price_data.clone());

        // Add to price history
        self.price_history
            .push_back((timestamp, price_data.price_sol));

        // Maintain ring buffer size
        if self.price_history.len() > PRICE_HISTORY_CAP {
            self.price_history.pop_front();
        }
    }

    fn request_updates(&self) {
        if let Some(mint) = self.tracked_mint {
            // Request price update
            self.send_command(ManualCommand::GetPrice { mint });

            // Request position update
            self.send_command(ManualCommand::GetPosition { mint });
        }
    }

    fn clear_stale_ui_state(&mut self, new_mint: &Pubkey) {
        // Clear stale per-mint state as specified in the problem statement
        self.position_info = None;
        self.current_price = None;
        self.price_history.clear();
        self.current_metrics_tx_count = None;
        self.current_metrics_holders = None;

        // Reset timing to request fresh data immediately
        self.last_update = Instant::now() - UI_PRICE_REFRESH;

        // Add info message about state reset
        self.status_message = format!("🔄 State reset for new mint: {}", new_mint);
        self.status_is_error = false;
    }

    fn try_parse_mint(&mut self) -> bool {
        if let Ok(mint) = self.active_mint.parse::<Pubkey>() {
            // Check if this is a different mint than currently tracked
            let is_different_mint = self.tracked_mint.map_or(true, |current| current != mint);

            if is_different_mint {
                // Clear stale state for the new mint
                self.clear_stale_ui_state(&mint);
            }

            self.tracked_mint = Some(mint);
            self.request_updates();
            true
        } else {
            self.status_message = "Invalid mint address".to_string();
            self.status_is_error = true;
            false
        }
    }

    fn render_header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Active Mint:");

            let text_edit = ui.text_edit_singleline(&mut self.active_mint);

            // Handle Enter key press
            if text_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.try_parse_mint();
            }

            if ui.button("Track").clicked() {
                self.try_parse_mint();
            }
        });
    }

    fn render_mode_toggle(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Mode:");

            let auto_selected = ui
                .radio_value(&mut self.current_mode, UiMode::Auto, "Auto")
                .clicked();
            let hybrid_selected = ui
                .radio_value(&mut self.current_mode, UiMode::Hybrid, "Hybrid")
                .clicked();

            if auto_selected {
                self.send_command(ManualCommand::SwitchToAuto);
            } else if hybrid_selected {
                self.send_command(ManualCommand::SwitchToManual);
            }
        });
    }

    fn render_sell_buttons(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Sell:");

            if ui.button("10%").clicked() {
                if let Some(mint) = self.tracked_mint {
                    self.send_command(ManualCommand::Sell {
                        mint,
                        percentage: 0.10,
                    });
                }
            }

            if ui.button("25%").clicked() {
                if let Some(mint) = self.tracked_mint {
                    self.send_command(ManualCommand::Sell {
                        mint,
                        percentage: 0.25,
                    });
                }
            }

            if ui.button("50%").clicked() {
                if let Some(mint) = self.tracked_mint {
                    self.send_command(ManualCommand::Sell {
                        mint,
                        percentage: 0.50,
                    });
                }
            }

            if ui.button("100%").clicked() {
                if let Some(mint) = self.tracked_mint {
                    self.send_command(ManualCommand::Sell {
                        mint,
                        percentage: 1.00,
                    });
                }
            }
        });
    }

    fn render_sl_tp_forms(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Stop Loss:");
            ui.text_edit_singleline(&mut self.stop_loss_input);

            if ui.button("Set SL").clicked() {
                if let (Some(mint), Ok(price)) =
                    (self.tracked_mint, self.stop_loss_input.parse::<f64>())
                {
                    self.send_command(ManualCommand::SetStopLoss {
                        mint,
                        price_threshold: price,
                    });
                } else {
                    self.status_message = "Invalid stop loss price or no mint tracked".to_string();
                    self.status_is_error = true;
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Take Profit:");
            ui.text_edit_singleline(&mut self.take_profit_input);

            if ui.button("Set TP").clicked() {
                if let (Some(mint), Ok(price)) =
                    (self.tracked_mint, self.take_profit_input.parse::<f64>())
                {
                    self.send_command(ManualCommand::SetTakeProfit {
                        mint,
                        price_threshold: price,
                    });
                } else {
                    self.status_message =
                        "Invalid take profit price or no mint tracked".to_string();
                    self.status_is_error = true;
                }
            }
        });
    }

    fn render_metrics_and_risk(&mut self, ui: &mut Ui) {
        ui.group(|ui| {
            ui.label("📊 Token Metrics");

            if let Some(mint) = self.tracked_mint {
                ui.horizontal(|ui| {
                    if ui.button("🔄 Get Metrics").clicked() {
                        self.send_command(ManualCommand::GetMetrics { mint });
                    }

                    ui.separator();

                    // Display current metrics
                    match (
                        &self.current_metrics_tx_count,
                        &self.current_metrics_holders,
                    ) {
                        (Some(tx_count), Some(holders)) => {
                            ui.label(format!("TX: {}, Holders: {}", tx_count, holders));
                        }
                        (Some(tx_count), None) => {
                            ui.label(format!("TX: {}, Holders: N/A", tx_count));
                        }
                        (None, Some(holders)) => {
                            ui.label(format!("TX: N/A, Holders: {}", holders));
                        }
                        (None, None) => {
                            ui.label("TX: N/A, Holders: N/A");
                        }
                    }
                });

                ui.separator();

                ui.label("🧹 Risk Management");
                ui.horizontal(|ui| {
                    if ui
                        .add(Button::new("Clear SL").fill(egui::Color32::from_rgb(255, 165, 0)))
                        .clicked()
                    {
                        self.send_command(ManualCommand::ClearRisk {
                            mint,
                            clear_sl: true,
                            clear_tp: false,
                        });
                    }

                    if ui
                        .add(Button::new("Clear TP").fill(egui::Color32::from_rgb(255, 165, 0)))
                        .clicked()
                    {
                        self.send_command(ManualCommand::ClearRisk {
                            mint,
                            clear_sl: false,
                            clear_tp: true,
                        });
                    }

                    if ui
                        .add(Button::new("Clear Both").fill(egui::Color32::from_rgb(255, 100, 100)))
                        .clicked()
                    {
                        self.send_command(ManualCommand::ClearRisk {
                            mint,
                            clear_sl: true,
                            clear_tp: true,
                        });
                    }
                });
            } else {
                ui.label("⚠️ No mint tracked - enter and track a mint address first");
            }
        });
    }

    fn render_emergency_stop(&self, ui: &mut Ui) {
        if ui
            .add(Button::new("🚨 Emergency Stop").fill(egui::Color32::RED))
            .clicked()
        {
            self.send_command(ManualCommand::EmergencyStop);
        }
    }

    fn render_live_stats(&self, ui: &mut Ui) {
        ui.group(|ui| {
            ui.label("📊 Live Stats");

            if let Some(ref position) = self.position_info {
                ui.label(format!(
                    "Entry Price: {:.6} SOL",
                    position.initial_sol_amount as f64 / 1_000_000_000.0
                ));
                ui.label(format!(
                    "Remaining Tokens: {}",
                    position.remaining_token_amount()
                ));
                ui.label(format!("Sold Tokens: {}", position.sold_token_amount));
                ui.label(format!("Sold %: {:.2}%", position.sold_percent()));
            } else {
                ui.label("No active position");
            }

            if let Some(ref price) = self.current_price {
                ui.label(format!("Current Price: {:.6} SOL", price.price_sol));
                ui.label(format!("USD Price: ${:.4}", price.price_usd));
                ui.label(format!("24h Volume: ${:.2}", price.volume_24h));
                ui.label(format!("Source: {}", price.source));

                // Calculate correct total P&L using new formula
                if let Some(ref position) = self.position_info {
                    let remaining_tokens = position.remaining_token_amount();
                    let current_value_of_remaining_lamports =
                        remaining_tokens as f64 * price.price_sol * 1_000_000_000.0;
                    let total_pnl_lamports = position.total_sol_from_sales as i128
                        + current_value_of_remaining_lamports as i128
                        - position.total_initial_sol_cost as i128;
                    let total_pnl_sol = total_pnl_lamports as f64 / 1_000_000_000.0;

                    let pnl_percent = if position.total_initial_sol_cost > 0 {
                        total_pnl_lamports as f64 / position.total_initial_sol_cost as f64 * 100.0
                    } else {
                        0.0
                    };

                    let pnl_color = if total_pnl_lamports >= 0 {
                        egui::Color32::GREEN
                    } else {
                        egui::Color32::RED
                    };

                    ui.colored_label(
                        pnl_color,
                        format!("Total P&L: {:.4} SOL ({:.2}%)", total_pnl_sol, pnl_percent),
                    );

                    // Also show entry price per token for reference
                    let entry_price_per_token = if position.initial_token_amount > 0 {
                        position.total_initial_sol_cost as f64
                            / position.initial_token_amount as f64
                            / 1_000_000_000.0
                    } else {
                        0.0
                    };
                    ui.label(format!(
                        "Entry Price/Token: {:.9} SOL",
                        entry_price_per_token
                    ));
                }
            } else {
                ui.label("No price data");
            }
        });
    }

    fn render_price_chart(&self, ui: &mut Ui) {
        if self.price_history.is_empty() {
            ui.label("No price history");
            return;
        }

        let points: PlotPoints = self
            .price_history
            .iter()
            .enumerate()
            .map(|(i, (_, price))| [i as f64, *price])
            .collect();

        let line = Line::new(points);

        Plot::new("price_chart")
            .view_aspect(2.0)
            .height(200.0)
            .show(ui, |plot_ui| plot_ui.line(line));
    }
}

impl eframe::App for ManualGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll for responses
        self.poll_responses();

        // Periodic updates
        if self.last_update.elapsed() >= UI_PRICE_REFRESH {
            self.request_updates();
            self.last_update = Instant::now();
        }

        // Request repaint for next frame
        ctx.request_repaint();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🎮 Solana Sniper Manual Controller");

            ui.separator();

            // Header
            self.render_header(ui);

            ui.separator();

            // Mode toggle
            self.render_mode_toggle(ui);

            ui.separator();

            // Sell buttons
            self.render_sell_buttons(ui);

            ui.separator();

            // SL/TP forms
            self.render_sl_tp_forms(ui);

            ui.separator();

            // Metrics and Risk Management
            self.render_metrics_and_risk(ui);

            ui.separator();

            // Emergency stop
            self.render_emergency_stop(ui);

            ui.separator();

            // Live stats
            self.render_live_stats(ui);

            ui.separator();

            // Price chart
            ui.label("📈 Price Chart");
            self.render_price_chart(ui);

            ui.separator();

            // Status
            let status_color = if self.status_is_error {
                egui::Color32::RED
            } else {
                egui::Color32::GREEN
            };

            ui.colored_label(status_color, &self.status_message);
        });
    }
}

pub fn run_gui(
    command_tx: mpsc::Sender<ManualCommand>,
    response_rx: mpsc::Receiver<ManualResponse>,
) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Solana Sniper Manual Controller"),
        ..Default::default()
    };

    eframe::run_native(
        "Solana Sniper Manual Controller",
        options,
        Box::new(|_cc| Ok(Box::new(ManualGuiApp::new(command_tx, response_rx)))),
    )
}
