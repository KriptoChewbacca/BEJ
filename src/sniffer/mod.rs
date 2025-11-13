#![allow(unused_imports)] // Allow unused imports for re-exports

// Sniffer components
pub mod analytics; // accumulator (atomic) + EMA background updater + heuristics
pub mod config; // SnifferConfig, Domyślne wartości, parsowanie env/toml
pub mod core; // Geyser gRPC client wrapper + stream loop (hot-path receive)
pub mod dataflow; // Formal dataflow contracts, domain boundaries, event tracking
pub mod errors; // SnifferError enum, Retry policies (ExponentialBackoff)
pub mod extractor; // Minimal extractor -> PremintCandidate (hot-path cheap checks)
pub mod handoff; // bounded mpsc, batch send, backpressure policy, priority logic
pub mod integration; // SnifferApi: start/stop/pause/resume, stats watch, health
pub mod prefilter; // Zero-copy hot-path filters (program_id, account_includes, size)
pub mod security; // cheap inline sanity checks + async verifier pool
pub mod supervisor;
pub mod telemetry; // atomics counters, sampler, JSON snapshot / watch export // Lifecycle management, pause/resume/stop, panic recovery

// Re-export commonly used types
pub use extractor::PriorityLevel;
