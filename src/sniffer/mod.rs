// Sniffer components
pub mod config;               // SnifferConfig, Domyślne wartości, parsowanie env/toml
pub mod integration;          // SnifferApi: start/stop/pause/resume, stats watch, health
pub mod core;                 // Geyser gRPC client wrapper + stream loop (hot-path receive)
pub mod prefilter;            // Zero-copy hot-path filters (program_id, account_includes, size)
pub mod extractor;            // Minimal extractor -> PremintCandidate (hot-path cheap checks)
pub mod handoff;              // bounded mpsc, batch send, backpressure policy, priority logic
pub mod analytics;            // accumulator (atomic) + EMA background updater + heuristics
pub mod telemetry;            // atomics counters, sampler, JSON snapshot / watch export
pub mod security;             // cheap inline sanity checks + async verifier pool
pub mod errors;               // SnifferError enum, Retry policies (ExponentialBackoff)
pub mod dataflow;             // Formal dataflow contracts, domain boundaries, event tracking
pub mod supervisor;           // Lifecycle management, pause/resume/stop, panic recovery

// Re-export commonly used types
pub use config::{SnifferConfig, DropPolicy, BatchSendMode};
pub use integration::{Sniffer, SnifferApi, EventCollector};
pub use extractor::{PremintCandidate, PriorityLevel};
pub use errors::{SnifferError, MintExtractError, AccountExtractError, ExponentialBackoff};
pub use telemetry::{SnifferMetrics, HandoffDiagnostics};
pub use analytics::PredictiveAnalytics;
pub use dataflow::{SnifferEvent, ValidatedCandidate, CandidateId};
pub use supervisor::{Supervisor, SnifferState, SupervisorCommand, WorkerHandle};
