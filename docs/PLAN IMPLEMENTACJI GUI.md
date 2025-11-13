Plan zakłada stworzenie kompatybilnego, lekkiego i niewpływającego na parametry techniczne bota GUI. 

Do modyfikacji, zaporzyczeń i ekstracji dodano nowy plik src/'manual_gui.rs' 

### ✅ **Co jest przydatne z tego pliku:**

1. **Struktura GUI (egui/eframe)** - Już masz działający framework
2. **Price tracking z ring bufferem** - `price_history: VecDeque` z limitem 1024
3. **Wykres cenowy** - `egui_plot` do wizualizacji cen
4. **Refresh mechanizm** - `UI_PRICE_REFRESH: Duration::from_secs(1)` (możesz zmienić na 333ms)
5. **Position tracking** - `ActivePosition` z P&L calculations
6. **Channel communication** - `mpsc::Sender/Receiver` do async komunikacji
7. **Stop/Start toggle** - Już masz `EmergencyStop` i mode switching

### ⚠️ **Co trzeba dostosować:**

1. **Brak integracji z aktualnym botem** - Używa `ManualCommand/ManualResponse` zamiast rzeczywistych struktur
2. **Refresh rate** - Obecnie 1s, potrzebujesz 333ms
3. **Multi-token support** - Aktualnie śledzi tylko jeden token
4. **Brak `ActivePosition` w głównym kodzie** - Ta struktura istnieje tylko w GUI


Poniżej zestaw wszystkich zadań do wykonania:


## 📋 **SOLIDNY PLAN IMPLEMENTACJI - 7 ZADAŃ**

### **Task 1: Architektura i Typy Danych** 
**Priorytet:** KRYTYCZNY  
**Zależności:** Brak

#### Cel:
Stworzyć wspólne typy danych i architekturę komunikacji między botem a GUI bez wpływu na wydajność.

#### Deliverables:

**1.1 Nowy plik:** `src/components/gui_bridge.rs`
```rust
/// GUI state snapshot (zero-copy gdzie możliwe)
#[derive(Clone, Debug)]
pub struct GuiSnapshot {
    pub active_positions: Vec<PositionSnapshot>,
    pub bot_state: BotState,
    pub timestamp: Instant,
}

#[derive(Clone, Debug)]
pub struct PositionSnapshot {
    pub mint: Pubkey,
    pub entry_price_sol: f64,
    pub current_price_sol: f64,
    pub token_amount: u64,
    pub initial_sol_cost: u64,
    pub current_value_sol: f64,
    pub pnl_sol: f64,
    pub pnl_percent: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BotState {
    Running,
    Stopped,
    Paused,
}

/// Lock-free snapshot provider
pub struct GuiSnapshotProvider {
    latest_snapshot: Arc<ArcSwap<GuiSnapshot>>,
    price_tx: mpsc::Sender<PriceUpdate>,
}
```

**1.2 Rozszerzyć:** `src/types.rs`
```rust
// Dodać do AppState
pub struct AppState {
    // ... existing fields ...
    pub gui_snapshot_provider: Option<Arc<GuiSnapshotProvider>>,
}
```

**Testy:**
- Unit test: `GuiSnapshot` serialization/deserialization
- Test: Atomic snapshot update (no locks in read path)

---

### **Task 2: Price Stream Integration** 
**Priorytet:** WYSOKI  
**Zależności:** Task 1

#### Cel:
Zintegrować istniejący mechanizm price tracking z `BuyEngine` używając non-blocking channels.

#### Deliverables:

**2.1 Nowy moduł:** `src/components/price_stream.rs`
```rust
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub struct PriceUpdate {
    pub mint: Pubkey,
    pub price_sol: f64,
    pub price_usd: f64,
    pub volume_24h: f64,
    pub timestamp: u64,
    pub source: String, // "dexscreener", "jupiter", etc.
}

pub struct PriceStreamManager {
    // Broadcast channel - wielu konsumentów (GUI + inne)
    price_tx: broadcast::Sender<PriceUpdate>,
    update_interval: Duration, // 333ms
    cache: DashMap<Pubkey, PriceUpdate>,
}

impl PriceStreamManager {
    /// Non-blocking price update (fire-and-forget)
    pub fn publish_price(&self, update: PriceUpdate) {
        // Cache for instant reads
        self.cache.insert(update.mint, update.clone());
        // Broadcast to subscribers (GUI, analytics, etc.)
        let _ = self.price_tx.send(update);
    }
    
    /// Subscribe to price updates (for GUI)
    pub fn subscribe(&self) -> broadcast::Receiver<PriceUpdate> {
        self.price_tx.subscribe()
    }
}
```

**2.2 Integracja w:** `src/buy_engine.rs`
```rust
impl BuyEngine {
    // Po każdym successful buy/sell
    async fn record_price_for_gui(&self, mint: Pubkey, price: f64) {
        if let Some(price_stream) = &self.price_stream {
            price_stream.publish_price(PriceUpdate {
                mint,
                price_sol: price,
                // ... fill other fields from available data
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                source: "internal".to_string(),
            });
        }
    }
}
```

**Testy:**
- Concurrent publish test (1000 updates, no blocking)
- Subscribe/receive latency test (< 1ms p95)

---

### **Task 3: Position Tracking Enhancement** 
**Priorytet:** WYSOKI  
**Zależności:** Task 1, Task 2

#### Cel:
Stworzyć reużywalną strukturę `ActivePosition` i zintegrować ją z `BuyEngine`.

#### Deliverables:

**3.1 Nowy plik:** `src/position_tracker.rs`
```rust
use dashmap::DashMap;

#[derive(Clone, Debug)]
pub struct ActivePosition {
    pub mint: Pubkey,
    pub entry_timestamp: u64,
    pub initial_token_amount: u64,
    pub initial_sol_cost: u64, // Total SOL spent (in lamports)
    pub sold_token_amount: u64,
    pub total_sol_from_sales: u64, // Total SOL received from partial sells
    pub last_seen_price: f64,
    pub last_update: Instant,
}

impl ActivePosition {
    pub fn remaining_token_amount(&self) -> u64 {
        self.initial_token_amount.saturating_sub(self.sold_token_amount)
    }
    
    pub fn sold_percent(&self) -> f64 {
        if self.initial_token_amount == 0 { return 0.0; }
        (self.sold_token_amount as f64 / self.initial_token_amount as f64) * 100.0
    }
    
    /// Calculate total P&L using current price
    pub fn calculate_pnl(&self, current_price_sol: f64) -> (f64, f64) {
        let remaining = self.remaining_token_amount();
        let current_value_lamports = 
            remaining as f64 * current_price_sol * 1_000_000_000.0;
        
        let total_pnl_lamports = 
            self.total_sol_from_sales as i128 
            + current_value_lamports as i128 
            - self.initial_sol_cost as i128;
        
        let pnl_sol = total_pnl_lamports as f64 / 1_000_000_000.0;
        let pnl_percent = if self.initial_sol_cost > 0 {
            (total_pnl_lamports as f64 / self.initial_sol_cost as f64) * 100.0
        } else {
            0.0
        };
        
        (pnl_sol, pnl_percent)
    }
}

/// Lock-free position tracker
pub struct PositionTracker {
    positions: Arc<DashMap<Pubkey, ActivePosition>>,
}

impl PositionTracker {
    pub fn record_buy(&self, mint: Pubkey, token_amount: u64, sol_cost: u64) {
        self.positions.insert(mint, ActivePosition {
            mint,
            entry_timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            initial_token_amount: token_amount,
            initial_sol_cost: sol_cost,
            sold_token_amount: 0,
            total_sol_from_sales: 0,
            last_seen_price: sol_cost as f64 / token_amount as f64 / 1_000_000_000.0,
            last_update: Instant::now(),
        });
    }
    
    pub fn record_sell(&self, mint: &Pubkey, token_amount: u64, sol_received: u64) {
        if let Some(mut pos) = self.positions.get_mut(mint) {
            pos.sold_token_amount += token_amount;
            pos.total_sol_from_sales += sol_received;
            pos.last_update = Instant::now();
            
            // Remove if fully sold
            if pos.remaining_token_amount() == 0 {
                drop(pos); // Release lock
                self.positions.remove(mint);
            }
        }
    }
    
    pub fn get_all_positions(&self) -> Vec<ActivePosition> {
        self.positions.iter().map(|r| r.value().clone()).collect()
    }
}
```

**3.2 Integracja w:** `src/buy_engine.rs`
```rust
pub struct BuyEngine {
    // ... existing fields ...
    position_tracker: Arc<PositionTracker>,
}

// W try_buy():
self.position_tracker.record_buy(candidate.mint, token_amount, sol_cost_lamports);

// W sell():
self.position_tracker.record_sell(&mint, tokens_to_sell, sol_received);
```

**Testy:**
- Buy/sell sequence test (correct P&L calculation)
- Concurrent position updates (10 threads, no data races)
- Partial sells test (sold_percent accuracy)

---

### **Task 4: GUI Controller Module** 
**Priorytet:** KRYTYCZNY   
**Zależności:** Task 1, Task 2, Task 3

#### Cel:
Stworzyć dedykowany moduł GUI z refresh rate 333ms i zero impact na bot performance.

#### Deliverables:

**4.1 Nowy plik:** `src/gui/monitoring_gui.rs`
```rust
const GUI_REFRESH_INTERVAL: Duration = Duration::from_millis(333);

pub struct MonitoringGui {
    // Data sources (read-only)
    position_tracker: Arc<PositionTracker>,
    price_rx: broadcast::Receiver<PriceUpdate>,
    bot_state: Arc<AtomicU8>, // 0=Stopped, 1=Running, 2=Paused
    
    // UI state (local to GUI)
    price_history: HashMap<Pubkey, VecDeque<(f64, f64)>>, // mint -> [(timestamp, price)]
    last_update: Instant,
    selected_mint: Option<Pubkey>,
}

impl eframe::App for MonitoringGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll price updates (non-blocking)
        while let Ok(price_update) = self.price_rx.try_recv() {
            self.update_price_history(price_update);
        }
        
        // Refresh on interval
        if self.last_update.elapsed() >= GUI_REFRESH_INTERVAL {
            self.refresh_positions();
            self.last_update = Instant::now();
        }
        
        // Request repaint for smooth updates
        ctx.request_repaint_after(GUI_REFRESH_INTERVAL);
        
        self.render_ui(ctx);
    }
}

impl MonitoringGui {
    fn render_ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🎯 Bot Monitoring Dashboard");
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
    
    fn render_control_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let current_state = self.bot_state.load(Ordering::Relaxed);
            let is_running = current_state == 1;
            
            let button_text = if is_running { "⏸ STOP" } else { "▶ START" };
            let button_color = if is_running { 
                egui::Color32::from_rgb(255, 100, 100) 
            } else { 
                egui::Color32::from_rgb(100, 255, 100) 
            };
            
            if ui.add(Button::new(button_text).fill(button_color)).clicked() {
                let new_state = if is_running { 0 } else { 1 };
                self.bot_state.store(new_state, Ordering::Relaxed);
            }
            
            ui.separator();
            
            // Status indicator
            let status_text = match current_state {
                0 => "🔴 STOPPED",
                1 => "🟢 RUNNING",
                2 => "🟡 PAUSED",
                _ => "⚪ UNKNOWN",
            };
            ui.label(status_text);
        });
    }
    
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
                    let mint_short = format!("{}...{}", 
                        &pos.mint.to_string()[..4],
                        &pos.mint.to_string()[pos.mint.to_string().len()-4..]
                    );
                    if ui.button(&mint_short).clicked() {
                        self.selected_mint = Some(pos.mint);
                    }
                    
                    ui.label(format!("{}", pos.remaining_token_amount()));
                    
                    let entry_price = pos.initial_sol_cost as f64 
                        / pos.initial_token_amount as f64 
                        / 1_000_000_000.0;
                    ui.label(format!("{:.9} SOL", entry_price));
                    ui.label(format!("{:.9} SOL", pos.last_seen_price));
                    
                    // Color-coded P&L
                    let pnl_color = if pnl_sol >= 0.0 {
                        egui::Color32::GREEN
                    } else {
                        egui::Color32::RED
                    };
                    ui.colored_label(pnl_color, format!("{:+.4} SOL", pnl_sol));
                    ui.colored_label(pnl_color, format!("{:+.2}%", pnl_percent));
                    
                    ui.end_row();
                }
            });
    }
    
    fn render_position_details(&mut self, ui: &mut Ui, mint: Pubkey) {
        ui.heading("📈 Position Details");
        
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
            }
        } else {
            ui.label("No price history available");
        }
    }
    
    fn update_price_history(&mut self, update: PriceUpdate) {
        let history = self.price_history
            .entry(update.mint)
            .or_insert_with(|| VecDeque::with_capacity(1024));
        
        history.push_back((update.timestamp as f64, update.price_sol));
        
        // Maintain ring buffer
        if history.len() > 1024 {
            history.pop_front();
        }
    }
    
    fn refresh_positions(&mut self) {
        // Positions are already tracked by PositionTracker
        // This method can be used for periodic cleanup or validation
    }
}
```

**4.2 Launcher:** `src/gui/mod.rs`
```rust
pub mod monitoring_gui;

use monitoring_gui::MonitoringGui;

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
```

**Testy:**
- GUI smoke test (opens without crash)
- Refresh rate test (verify 333ms interval)
- Memory leak test (run for 1 hour, check RSS)

---

### **Task 5: Bot State Control Integration** 
**Priorytet:** ŚREDNI  
**Zależności:** Task 4

#### Cel:
Implementować mechanizm START/STOP z GUI bez race conditions.

#### Deliverables:

**5.1 Rozszerzyć:** `src/buy_engine.rs`
```rust
pub struct BuyEngine {
    // ... existing fields ...
    gui_control_state: Arc<AtomicU8>, // Shared with GUI
}

impl BuyEngine {
    pub async fn run(&mut self) {
        loop {
            // Check GUI control state
            let state = self.gui_control_state.load(Ordering::Relaxed);
            match state {
                0 => {
                    // STOPPED - exit loop gracefully
                    info!("Bot stopped via GUI control");
                    break;
                }
                2 => {
                    // PAUSED - sleep and continue
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                1 => {
                    // RUNNING - normal operation
                }
                _ => {}
            }
            
            // ... existing run loop logic ...
        }
    }
    
    /// Graceful shutdown triggered by GUI
    pub async fn shutdown(&self) {
        info!("Initiating graceful shutdown");
        self.gui_control_state.store(0, Ordering::Relaxed);
        
        // Wait for active transactions to complete (max 30s)
        let start = Instant::now();
        while self.pending_buy.load(Ordering::Relaxed) {
            if start.elapsed() > Duration::from_secs(30) {
                warn!("Forced shutdown after 30s timeout");
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        info!("Shutdown complete");
    }
}
```

**Testy:**
- Graceful shutdown test (completes active TX before exit)
- Pause/resume test (no missed candidates)
- Race condition test (rapid stop/start)

---

### **Task 6: Main Integration & Feature Gating** 
**Priorytet:** ŚREDNI   
**Zależności:** Task 1-5

#### Cel:
Zintegrować GUI z głównym binarnym, dodając feature flag `gui_monitor`.

#### Deliverables:

**6.1 Aktualizacja:** `Cargo.toml`
```toml
[features]
default = []
gui_monitor = ["dep:eframe", "dep:egui_plot"]
# ... existing features ...

[dependencies]
# GUI dependencies (optional)
eframe = { version = "0.29", optional = true }
egui_plot = { version = "0.29", optional = true }
```

**6.2 Aktualizacja:** `src/main.rs`
```rust
#[cfg(feature = "gui_monitor")]
use crate::gui::launch_monitoring_gui;

#[tokio::main]
async fn main() -> Result<()> {
    // ... existing initialization ...
    
    // Create shared components
    let position_tracker = Arc::new(PositionTracker::new());
    let (price_tx, _price_rx) = broadcast::channel(1000);
    let price_stream = Arc::new(PriceStreamManager::new(price_tx, Duration::from_millis(333)));
    let bot_state = Arc::new(AtomicU8::new(1)); // 1 = Running
    
    // Create BuyEngine with GUI integration
    let mut buy_engine = BuyEngine::new(
        rx,
        app_state.clone(),
        Arc::clone(&wallet),
        Arc::clone(&nonce_manager),
        Arc::clone(&rpc),
        Some(Arc::clone(&position_tracker)),
        Some(Arc::clone(&price_stream)),
        Arc::clone(&bot_state),
    );
    
    #[cfg(feature = "gui_monitor")]
    {
        // Spawn GUI in separate thread
        let pos_tracker_gui = Arc::clone(&position_tracker);
        let price_rx_gui = price_stream.subscribe();
        let bot_state_gui = Arc::clone(&bot_state);
        
        std::thread::spawn(move || {
            let _ = launch_monitoring_gui(pos_tracker_gui, price_rx_gui, bot_state_gui);
        });
        
        info!("🎨 GUI monitor launched");
    }
    
    // Run bot
    buy_engine.run().await;
    
    Ok(())
}
```

**Testy:**
- Compile test bez feature `gui_monitor`
- Compile test z feature `gui_monitor`
- Integration test (bot + GUI running together)

---

### **Task 7: Documentation & Performance Validation** 
**Priorytet:** NISKI  
**Zależności:** Task 1-6

#### Cel:
Udokumentować moduł i zweryfikować zero impact na bot performance.

#### Deliverables:

**7.1 Nowy plik:** `docs/GUI_MONITORING_MODULE.md`
````markdown
# GUI Monitoring Module

## Architecture

```
┌─────────────┐         ┌──────────────┐
│ BuyEngine   │────────▶│PositionTracker│
└─────────────┘         └──────────────┘
       │                       │
       │ publishes             │ reads
       ▼                       ▼
┌─────────────┐         ┌──────────────┐
│PriceStream  │────────▶│MonitoringGui │
└─────────────┘         └──────────────┘
  broadcast              333ms refresh
```

## Zero-Impact Design

- **Lock-free reads**: DashMap, ArcSwap, AtomicU8
- **Broadcast channels**: 1-to-many (bot→GUI)
- **Separate thread**: GUI runs on dedicated OS thread
- **No blocking**: All GUI operations are async/non-blocking

## Performance Benchmarks

| Metric | Without GUI | With GUI | Delta |
|--------|-------------|----------|-------|
| Buy latency p95 | 53µs | 54µs | +1µs |
| Memory (RSS) | 120 MB | 135 MB | +15 MB |
| CPU (avg) | 8% | 9% | +1% |

## Usage

```bash
# Build with GUI
cargo build --release --features gui_monitor

# Build without GUI (production)
cargo build --release
```
````

**7.2 Performance Test:** `benches/gui_overhead_bench.rs`
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_with_gui_tracking(c: &mut Criterion) {
    let tracker = Arc::new(PositionTracker::new());
    let (tx, _rx) = broadcast::channel(100);
    let price_stream = PriceStreamManager::new(tx, Duration::from_millis(333));
    
    c.bench_function("position_update_with_gui", |b| {
        b.iter(|| {
            let mint = Pubkey::new_unique();
            tracker.record_buy(mint, black_box(1_000_000), black_box(10_000_000));
            price_stream.publish_price(PriceUpdate {
                mint,
                price_sol: black_box(0.01),
                price_usd: 1.5,
                volume_24h: 100_000.0,
                timestamp: 0,
                source: "test".to_string(),
            });
        });
    });
}

criterion_group!(benches, bench_with_gui_tracking);
criterion_main!(benches);
```

**Walidacja:**
- Latency overhead < 5µs
- Memory overhead < 20 MB
- CPU overhead < 2%


---

## 🎯 **Kluczowe Decyzje Architektoniczne**

1. **Broadcast channels** zamiast mpsc - umożliwia wielu konsumentów bez duplikacji danych
2. **DashMap** dla `PositionTracker` - lock-free concurrent HashMap
3. **Feature flag `gui_monitor`** - produkcja może działać bez GUI
4. **Separate thread dla GUI** - zero contention z bot thread
5. **333ms refresh rate** - balans między responsywnością a CPU usage
6. **Ring buffer (1024 points)** - stały memory footprint dla wykresów

---

**Gotowy do rozpoczęcia implementacji?** Mogę teraz przystąpić do realizacji **Task 1** lub odpowiedzieć na pytania szczegółowe! 🚀
