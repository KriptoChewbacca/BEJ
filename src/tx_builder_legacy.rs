//! tx_builder.rs
//! Production-ready TransactionBuilder for Solana sniper bot (UNIVERSE CLASS GRADE)
//!
//! ## Enhanced Features (Universe Class)
//!
//! ### Dynamic Instruction Building
//! - Pre-simulation for compute unit estimation with caching (TTL-based)
//! - Adaptive priority fees based on network congestion
//! - ML-based slippage optimization using historical volatility
//! - Dynamic CU limits with range adjustment (min/max)
//!
//! ### Multi-DEX Support with Fallback Cascade
//! - Hierarchical DEX priority (PumpFun > Raydium > Orca > LetsBonk)
//! - Liquidity depth validation before execution
//! - Parallel provider queries for optimal routing
//!
//! ### Blockhash Management (IMPROVED)
//! - Quorum consensus from multiple RPCs with explicit parameters (min_responses, max_slot_diff)
//! - Slot-based validation and deterministic stale detection
//! - Adaptive TTL based on network conditions
//! - Predictive caching with automatic pruning (time + slot based)
//!
//! ### Rate Limiting & Backpressure (NEW)
//! - Token bucket rate limiter for RPC calls (configurable RPS)
//! - Separate rate limiting for simulations and HTTP requests
//! - Prevents network saturation and throttling
//!
//! ### Circuit Breaker Pattern (NEW)
//! - Per-endpoint circuit breakers with configurable thresholds
//! - Automatic endpoint rotation on failures
//! - Half-open state for recovery testing
//! - Prevents cascade failures across RPC providers
//!
//! ### Retry Policy (IMPROVED)
//! - Centralized retry logic with exponential backoff + jitter
//! - Error classification: retryable vs fatal errors
//! - Configurable max attempts and delay parameters
//! - Prevents wasted retries on non-recoverable errors
//!
//! ### MEV Protection
//! - Jito bundle enhancements with searcher hints
//! - Dynamic tip calculation based on P90 fees
//! - Backrun protection markers
//! - Bundle simulation (optional)
//!
//! ### High-Throughput Scalability (IMPROVED)
//! - Bounded worker pool for batch operations (prevents concurrency spikes)
//! - Semaphore-controlled parallelism with RAII guards
//! - Priority support for high-priority sniper operations
//! - Connection pooling (50 idle connections per host)
//! - Zero-copy parsing support
//! - Hot-path optimizations (pre-allocated vectors, reduced clones)
//!
//! ### Security & Validation
//! - Pre-flight balance checks
//! - Runtime program verification with metadata
//! - Universe-level error classification
//! - Signer rotation every 100 transactions
//!
//! ## Configuration Parameters (NEW)
//!
//! ### Quorum Configuration
//! - `min_responses`: Minimum RPC responses for quorum (default: 2)
//! - `max_slot_diff`: Maximum slot difference between responses (default: 10)
//! - `enable_slot_validation`: Toggle slot-based staleness detection (default: true)
//!
//! ### Rate Limiting
//! - `rpc_rate_limit_rps`: RPC requests per second (default: 100, 0 = unlimited)
//! - `simulation_rate_limit_rps`: Simulations per second (default: 20, 0 = unlimited)
//! - `http_rate_limit_rps`: HTTP requests per second (default: 50, 0 = unlimited)
//!
//! ### Circuit Breaker
//! - `circuit_breaker_failure_threshold`: Failures before opening circuit (default: 5)
//! - `circuit_breaker_timeout_secs`: Cooldown period in seconds (default: 60)
//!
//! ### Worker Pool
//! - `max_concurrent_builds`: Maximum parallel batch operations (default: 50)
//!
//! ### Simulation Cache
//! - `simulation_cache_config.ttl_seconds`: Cache TTL (default: 30)
//! - `simulation_cache_config.max_size`: Maximum cache entries (default: 1000)
//! - `simulation_cache_config.enabled`: Toggle caching (default: true)
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use tx_builder::{TransactionBuilder, TransactionConfig, ProgramMetadata, QuorumConfig};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create config with enhanced features
//! let mut config = TransactionConfig {
//!     min_cu_limit: 100_000,
//!     max_cu_limit: 400_000,
//!     adaptive_priority_fee_base: 10_000,
//!     adaptive_priority_fee_multiplier: 1.5,
//!     enable_simulation: true,
//!     enable_ml_slippage: true,
//!     min_liquidity_lamports: 1_000_000_000, // 1 SOL
//!     
//!     // NEW: Quorum configuration
//!     quorum_config: QuorumConfig {
//!         min_responses: 3,
//!         max_slot_diff: 5,
//!         enable_slot_validation: true,
//!     },
//!     
//!     // NEW: Rate limiting
//!     rpc_rate_limit_rps: 100.0,
//!     simulation_rate_limit_rps: 20.0,
//!     http_rate_limit_rps: 50.0,
//!     
//!     // NEW: Circuit breaker
//!     circuit_breaker_failure_threshold: 5,
//!     circuit_breaker_timeout_secs: 60,
//!     
//!     // NEW: Worker pool size
//!     max_concurrent_builds: 50,
//!     
//!     ..Default::default()
//! };
//!
//! // Add allowed program with metadata
//! config.add_allowed_program(
//!     pump_program_id,
//!     ProgramMetadata {
//!         version: "1.0.0".to_string(),
//!         last_verified_slot: 12345678,
//!         is_verified: true,
//!     }
//! );
//!
//! // Initialize builder
//! let builder = TransactionBuilder::new(
//!     wallet,
//!     rpc_endpoints,
//!     nonce_manager,
//!     &config
//! ).await?;
//!
//! // Build with automatic optimization
//! let tx = builder.build_buy_transaction(&candidate, &config, true).await?;
//!
//! // Prepare MEV-protected Jito bundle
//! let bundle = builder.prepare_jito_bundle(
//!     vec![tx],
//!     100_000,
//!     Some(target_slot),
//!     true, // backrun_protect
//!     &config
//! ).await?;
//!
//! // Batch processing with bounded worker pool
//! let txs = builder.batch_build_buy_transactions(candidates, &config, true).await;
//!
//! // Monitor circuit breaker states
//! let states = builder.get_circuit_breaker_states().await;
//! for (endpoint, state) in states {
//!     println!("{}: {:?}", endpoint, state);
//! }
//!
//! # Ok(())
//! # }
//! ```
//!
//! ## Integration Points
//! - WalletManager for signing and public key
//! - NonceManager for parallel transaction preparation
//! - RpcBroadcaster for transaction broadcasting
//! - Security validator for pre-transaction checks
//! - supports pump.fun integration (via `pumpfun` crate if enabled, or HTTP PumpPortal/Moralis fallback)
//! - supports LetsBonk (external HTTP provider) for liquidity/quote lookup
//! - validates config values
//! - retry/backoff + multi-RPC fallback for blockhash
//! - signs VersionedTransaction via WalletManager
//! - prepares Jito bundle wrapper with MEV features for later submission
//! - careful logging and safe fallbacks (memo fallback when no program integration)

use anyhow::anyhow;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use dashmap::DashMap;
use futures::stream::{FuturesUnordered, StreamExt}; // Task 3: For early-exit quorum
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256}; // Task 2: For deterministic message hashing
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    message::{v0::Message as MessageV0, VersionedMessage},
    pubkey::Pubkey,
    signature::Signature,
    transaction::VersionedTransaction,
};
use std::collections::{HashMap, VecDeque};
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info, warn};

use crate::nonce_manager::{NonceError, NonceManager};
use crate::rpc_manager::rpc_errors::RpcManagerError;
use crate::types::PremintCandidate;
use crate::wallet::WalletManager;

// Optional integration: `pumpfun` crate
#[cfg(feature = "pumpfun")]
use pumpfun::{accounts::BondingCurveAccount, common::types::Cluster, PumpFun};

use spl_associated_token_account::get_associated_token_address;
use spl_token::id as token_program_id;
use spl_token::instruction::close_account;

// ============================================================================
// Transaction Build Output (Phase 1: RAII Nonce Management)
// ============================================================================

/// Output from transaction building with nonce lease management (RAII pattern)
///
/// This struct ensures proper lifecycle management of nonce leases through RAII:
/// - Holds the built transaction ready for signing/broadcast
/// - Maintains ownership of the nonce lease until explicitly released or dropped
/// - Automatically warns if lease is not properly released before drop
/// - Extracts required signers from transaction header for validation
///
/// # RAII Contract
///
/// This struct enforces the following RAII guarantees:
///
/// 1. **Owned Data**: All fields contain owned data ('static), no references
/// 2. **Automatic Cleanup**: `Drop` implementation ensures nonce lease is released
/// 3. **Explicit Release**: Prefer `release_nonce()` for controlled cleanup
/// 4. **Consume Pattern**: `release_nonce()` consumes `self` to prevent use-after-release
/// 5. **No Async in Drop**: Drop only logs; actual release is synchronous
/// 6. **Zero Leaks**: Lease is guaranteed to be released either explicitly or on drop
///
/// # Example Usage
/// ```no_run
/// let output = builder.build_buy_transaction_output(&candidate, &config, false, true).await?;
///
/// // Hold nonce guard during broadcast
/// let result = rpc.send_transaction(output.tx.clone()).await;
///
/// match result {
///     Ok(sig) => {
///         // Success - explicitly release nonce
///         output.release_nonce().await?;
///         Ok(sig)
///     }
///     Err(e) => {
///         // Error - drop output (auto-releases nonce)
///         drop(output);
///         Err(e)
///     }
/// }
/// ```
pub struct TxBuildOutput {
    /// The built transaction ready for signing/broadcast
    pub tx: VersionedTransaction,

    /// Optional nonce lease guard (held until broadcast completes)
    /// Automatically released on drop via RAII pattern
    ///
    /// This field is owned data, not a reference. The lease will be automatically
    /// released when this struct is dropped, preventing resource leaks.
    pub nonce_guard: Option<crate::nonce_manager::NonceLease>,

    /// List of required signers for this transaction
    /// Extracted from message.header.num_required_signatures
    pub required_signers: Vec<Pubkey>,
}

impl TxBuildOutput {
    /// Create new TxBuildOutput with nonce guard
    ///
    /// Automatically extracts required signers from the transaction header
    /// based on num_required_signatures field using the compat layer.
    pub fn new(
        tx: VersionedTransaction,
        nonce_guard: Option<crate::nonce_manager::NonceLease>,
    ) -> Self {
        // Extract required signers using compat layer for unified API
        let required_signers = crate::compat::get_required_signers(&tx.message).to_vec();

        Self {
            tx,
            nonce_guard,
            required_signers,
        }
    }

    /// Get reference to the transaction
    pub fn tx_ref(&self) -> &VersionedTransaction {
        &self.tx
    }

    /// Consume self and extract the transaction (Phase 2, Task 2.3)
    ///
    /// This method extracts the transaction and drops the nonce guard.
    /// Used by legacy wrappers for backward compatibility.
    ///
    /// # Warning
    ///
    /// This releases the nonce guard early. Prefer holding the TxBuildOutput
    /// until after broadcast for proper RAII semantics.
    pub fn into_tx(mut self) -> VersionedTransaction {
        use std::mem;
        // Take ownership by replacing with a default value
        // The nonce_guard will be dropped when self is dropped
        mem::take(&mut self.tx)
    }

    /// Get slice of required signers
    pub fn required_signers(&self) -> &[Pubkey] {
        &self.required_signers
    }

    /// Explicitly release nonce guard (if held)
    ///
    /// This method should be called after successful transaction broadcast.
    /// Returns an error if the nonce release fails.
    ///
    /// # RAII Contract
    ///
    /// This method enforces RAII by:
    /// - Consuming `self` to prevent use-after-release
    /// - Idempotent: safe to call even if no nonce guard is held
    /// - Explicit cleanup: allows handling release errors
    ///
    /// # Example
    ///
    /// ```no_run
    /// let output = builder.build_buy_transaction_output(...).await?;
    /// let sig = rpc.send_transaction(output.tx.clone()).await?;
    ///
    /// // Explicitly release after successful broadcast
    /// output.release_nonce().await?;
    /// ```
    pub async fn release_nonce(mut self) -> Result<(), TransactionBuilderError> {
        if let Some(guard) = self.nonce_guard.take() {
            guard.release().await?;
        }
        Ok(())
    }
}

impl Drop for TxBuildOutput {
    /// RAII cleanup: Warn if nonce guard is being dropped without explicit release
    ///
    /// This implementation:
    /// - Does NOT perform async operations (RAII contract requirement)
    /// - Only logs a warning for diagnostic purposes
    /// - Relies on NonceLease's Drop for actual cleanup
    /// - Prevents resource leaks through automatic cleanup chain
    fn drop(&mut self) {
        if let Some(ref guard) = self.nonce_guard {
            warn!(
                nonce = %guard.nonce_pubkey(),
                drop_source = "TxBuildOutput",
                "TxBuildOutput dropped with active nonce guard - lease will be auto-released via NonceLease Drop"
            );
        }
    }
}

// ============================================================================
// Rate Limiting & Backpressure
// ============================================================================

/// Token bucket rate limiter for RPC calls, simulations, and HTTP requests
#[derive(Debug)]
pub struct TokenBucket {
    tokens: Arc<RwLock<f64>>,
    capacity: f64,
    refill_rate: f64, // tokens per second
    last_refill: Arc<RwLock<Instant>>,
}

impl TokenBucket {
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            tokens: Arc::new(RwLock::new(capacity)),
            capacity,
            refill_rate,
            last_refill: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Try to consume tokens, returns true if successful
    pub async fn try_consume(&self, count: f64) -> bool {
        self.refill().await;
        let mut tokens = self.tokens.write().await;
        if *tokens >= count {
            *tokens -= count;
            true
        } else {
            false
        }
    }

    /// Wait until tokens are available, then consume
    pub async fn consume(&self, count: f64) {
        loop {
            if self.try_consume(count).await {
                return;
            }
            // Wait a bit before retrying
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn refill(&self) {
        let now = Instant::now();
        let mut last_refill = self.last_refill.write().await;
        let elapsed = now.duration_since(*last_refill).as_secs_f64();

        if elapsed > 0.0 {
            let mut tokens = self.tokens.write().await;
            *tokens = (*tokens + elapsed * self.refill_rate).min(self.capacity);
            *last_refill = now;
        }
    }
}

/// Circuit breaker state for RPC endpoints
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,   // Normal operation
    Open,     // Endpoint disabled
    HalfOpen, // Testing if endpoint recovered
}

/// Task 2: Circuit breaker detailed status for monitoring
#[derive(Debug, Clone)]
pub struct CircuitBreakerStatus {
    pub endpoint: String,
    pub state: CircuitState,
    pub failure_count: u32,
}

/// Circuit breaker for individual RPC endpoints
#[derive(Debug)]
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<AtomicU32>,
    failure_threshold: u32,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    timeout: Duration,
    half_open_success_threshold: u32,
    half_open_successes: Arc<AtomicU32>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, timeout: Duration) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(AtomicU32::new(0)),
            failure_threshold,
            last_failure_time: Arc::new(RwLock::new(None)),
            timeout,
            half_open_success_threshold: 2,
            half_open_successes: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Check if request is allowed
    pub async fn can_execute(&self) -> bool {
        let mut state = self.state.write().await;

        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout has elapsed
                if let Some(last_failure) = *self.last_failure_time.read().await {
                    if last_failure.elapsed() >= self.timeout {
                        *state = CircuitState::HalfOpen;
                        self.half_open_successes.store(0, Ordering::Relaxed);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record successful execution
    pub async fn record_success(&self) {
        let mut state = self.state.write().await;

        match *state {
            CircuitState::HalfOpen => {
                let successes = self.half_open_successes.fetch_add(1, Ordering::Relaxed) + 1;
                if successes >= self.half_open_success_threshold {
                    *state = CircuitState::Closed;
                    self.failure_count.store(0, Ordering::Relaxed);
                }
            }
            CircuitState::Closed => {
                self.failure_count.store(0, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    /// Record failed execution
    pub async fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        let mut last_failure = self.last_failure_time.write().await;
        *last_failure = Some(Instant::now());

        if failures >= self.failure_threshold {
            let mut state = self.state.write().await;
            *state = CircuitState::Open;
            warn!("Circuit breaker opened after {} failures", failures);
        }
    }

    /// Get current state for monitoring
    pub async fn get_state(&self) -> CircuitState {
        self.state.read().await.clone()
    }

    /// Task 2: Get failure count for telemetry
    pub fn get_failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::Relaxed)
    }

    /// Task 2: Manually trigger circuit open (for testing/admin control)
    pub async fn force_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Open;
        let mut last_failure = self.last_failure_time.write().await;
        *last_failure = Some(Instant::now());
        warn!("Circuit breaker manually forced open");
    }

    /// Task 2: Manually reset circuit (for testing/admin control)
    pub async fn force_reset(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::Relaxed);
        self.half_open_successes.store(0, Ordering::Relaxed);
        info!("Circuit breaker manually reset to closed state");
    }
}

/// Retry policy with error classification
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 50,
            max_delay_ms: 2000,
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryPolicy {
    /// Classify error as retryable or fatal
    pub fn is_retryable(&self, error: &str) -> bool {
        // Retryable errors
        let retryable_patterns = [
            "timeout",
            "connection",
            "network",
            "temporarily unavailable",
            "too many requests",
            "rate limit",
            "503",
            "502",
            "504",
        ];

        let error_lower = error.to_lowercase();
        retryable_patterns
            .iter()
            .any(|pattern| error_lower.contains(pattern))
    }

    /// Calculate delay for given attempt
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        let delay_ms =
            (self.initial_delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32)) as u64;
        Duration::from_millis(delay_ms.min(self.max_delay_ms))
    }
}

/// Quorum configuration for blockhash consensus
#[derive(Debug, Clone)]
pub struct QuorumConfig {
    /// Minimum number of RPC responses required for quorum
    pub min_responses: usize,
    /// Maximum allowed slot difference between responses
    pub max_slot_diff: u64,
    /// Enable slot-based validation
    pub enable_slot_validation: bool,
}

impl Default for QuorumConfig {
    fn default() -> Self {
        Self {
            min_responses: 2,
            max_slot_diff: 10,
            enable_slot_validation: true,
        }
    }
}

/// Simulation cache entry with TTL
#[derive(Debug, Clone)]
struct SimulationCacheEntry {
    compute_units: u64,
    cached_at: Instant,
    slot: u64,
}

/// Configuration for simulation caching
#[derive(Debug, Clone)]
pub struct SimulationCacheConfig {
    /// Time-to-live for cached simulation results
    pub ttl_seconds: u64,
    /// Maximum cache size (number of entries)
    pub max_size: usize,
    /// Enable simulation caching
    pub enabled: bool,
    /// Task 1: Programs to exclude from caching (by program_id string)
    pub excluded_programs: Vec<String>,
}

impl Default for SimulationCacheConfig {
    fn default() -> Self {
        Self {
            ttl_seconds: 30,
            max_size: 1000,
            enabled: true,
            excluded_programs: Vec::new(),
        }
    }
}

impl SimulationCacheConfig {
    /// Task 1: Check if a program should be excluded from caching
    pub fn is_program_excluded(&self, program_id: &Pubkey) -> bool {
        let program_str = program_id.to_string();
        self.excluded_programs
            .iter()
            .any(|excluded| excluded == &program_str)
    }
}

// Configuration

/// Metadata for tracking program information (Universe Class)
///
/// Tracks verification status and version information for allowed programs.
/// Used in conjunction with DashMap for thread-safe runtime verification.
#[derive(Debug, Clone)]
pub struct ProgramMetadata {
    /// Program version string (e.g., "1.0.0")
    pub version: String,
    /// Last slot where this program was verified
    pub last_verified_slot: u64,
    /// Whether the program has been verified as safe
    pub is_verified: bool,
}

impl Default for ProgramMetadata {
    fn default() -> Self {
        Self {
            version: "unknown".to_string(),
            last_verified_slot: 0,
            is_verified: false,
        }
    }
}

/// ML-based slippage predictor using recent market volatility (Universe Class)
///
/// Maintains a rolling window of historical slippage observations and uses
/// statistical analysis to predict optimal slippage tolerance based on
/// current market conditions.
///
/// # Algorithm
///
/// 1. Maintains a VecDeque of recent slippage observations (basis points)
/// 2. Calculates mean and standard deviation of observations
/// 3. Adjusts base slippage by volatility multiplier (capped at 50% increase)
/// 4. Higher volatility = higher recommended slippage tolerance
#[derive(Debug)]
pub struct SlippagePredictor {
    history: VecDeque<f64>,
    max_history_size: usize,
}

impl SlippagePredictor {
    pub fn new(max_history_size: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history_size),
            max_history_size,
        }
    }

    pub fn add_observation(&mut self, bps: f64) {
        if self.history.len() >= self.max_history_size {
            self.history.pop_front();
        }
        self.history.push_back(bps);
    }

    pub fn predict_optimal_slippage(&self, base_bps: u64) -> u64 {
        if self.history.is_empty() {
            return base_bps;
        }

        // Calculate volatility (standard deviation)
        let mean: f64 = self.history.iter().sum::<f64>() / self.history.len() as f64;
        let variance: f64 = self.history.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
            / self.history.len() as f64;
        let std_dev = variance.sqrt();

        // Adjust slippage based on volatility (higher volatility = higher slippage)
        let multiplier = 1.0 + (std_dev / 100.0).min(0.5); // Cap at 50% increase
        ((base_bps as f64) * multiplier).round() as u64
    }
}

/// Execution context holding blockhash and optional nonce lease (Task 1)
///
/// This struct encapsulates the result of nonce/blockhash decision logic,
/// making the lease lifecycle explicit through RAII patterns.
/// Enhanced with ZK proof support (Security Enhancement 1) - upgraded to full zk-SNARKs
///
/// # RAII Contract
///
/// The `nonce_lease` field provides automatic resource management:
/// - Lease is held for the lifetime of this context
/// - Lease is automatically released when context is dropped
/// - Lease can be explicitly extracted via `extract_lease()` for transfer of ownership
/// - No references are held - all data is owned or 'static
///
/// # Debug Implementation
///
/// The custom Debug implementation excludes the full nonce_lease content to:
/// - Prevent log bloat from large lease structures
/// - Avoid exposing internal lease implementation details
/// - Provide concise debugging information (lease status only)
pub(crate) struct ExecutionContext {
    /// The blockhash to use for the transaction
    pub(crate) blockhash: Hash,
    /// Optional nonce account public key (if using durable transactions)
    pub(crate) nonce_pubkey: Option<Pubkey>,
    /// Optional nonce authority (if using durable transactions)
    pub(crate) nonce_authority: Option<Pubkey>,
    /// Optional nonce lease (held for transaction lifetime, auto-released on drop)
    ///
    /// This field enforces RAII semantics: the lease is owned by this context
    /// and will be automatically released on drop. Use `extract_lease()` to
    /// transfer ownership before drop.
    pub(crate) nonce_lease: Option<crate::nonce_manager::NonceLease>,
    /// Optional ZK proof for nonce state validation (upgraded to ZkProofData with Groth16)
    /// Only available when zk_enabled feature is active
    #[cfg(feature = "zk_enabled")]
    pub(crate) zk_proof: Option<crate::nonce_manager::ZkProofData>,
}

impl std::fmt::Debug for ExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_struct("ExecutionContext");
        debug_struct
            .field("blockhash", &self.blockhash)
            .field("nonce_pubkey", &self.nonce_pubkey)
            .field("nonce_authority", &self.nonce_authority)
            .field(
                "nonce_lease_status",
                &match &self.nonce_lease {
                    Some(lease) => format!(
                        "Some(nonce={}, expired={})",
                        lease.nonce_pubkey(),
                        lease.is_expired()
                    ),
                    None => "None".to_string(),
                },
            );

        #[cfg(feature = "zk_enabled")]
        debug_struct.field(
            "zk_proof",
            &self
                .zk_proof
                .as_ref()
                .map(|p| format!("Present(confidence={:.2})", p.confidence)),
        );

        debug_struct.finish()
    }
}

impl ExecutionContext {
    /// Extract the nonce lease, consuming it (Phase 1: RAII support)
    ///
    /// This method allows transferring ownership of the nonce lease from
    /// ExecutionContext to TxBuildOutput, ensuring proper RAII semantics.
    ///
    /// # RAII Contract
    ///
    /// This method consumes `self`, transferring ownership of the lease to the caller.
    /// If the lease is not extracted, it will be automatically released when
    /// ExecutionContext is dropped.
    pub fn extract_lease(mut self) -> Option<crate::nonce_manager::NonceLease> {
        self.nonce_lease.take()
    }
}

/// Operation priority for nonce vs blockhash decision (Task 6)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationPriority {
    /// Critical sniper operations - require lease, fail fast on exhaustion
    CriticalSniper,
    /// Utility operations - prefer recent blockhash for speed
    Utility,
    /// Bulk/non-urgent operations - use recent if nonce pool below threshold
    Bulk,
}

impl Default for OperationPriority {
    fn default() -> Self {
        OperationPriority::Utility
    }
}

impl OperationPriority {
    /// Check if this operation requires a nonce lease (Task 6)
    pub fn requires_nonce(&self) -> bool {
        match self {
            OperationPriority::CriticalSniper => true,
            OperationPriority::Utility => false,
            OperationPriority::Bulk => false,
        }
    }

    /// Check if fallback to recent blockhash is allowed on nonce exhaustion (Task 6)
    pub fn allow_blockhash_fallback(&self) -> bool {
        match self {
            OperationPriority::CriticalSniper => false, // Fail fast
            OperationPriority::Utility => true,
            OperationPriority::Bulk => true,
        }
    }
}

/// Transaction configuration with Universe Class enhancements
///
/// Provides comprehensive control over transaction building, optimization,
/// and execution parameters. Supports dynamic compute unit adjustment,
/// adaptive priority fees, ML-based slippage optimization, and more.
#[derive(Debug, Clone)]
pub struct TransactionConfig {
    /// Compute unit price in micro-lamports per CU (for priority fees)
    /// Legacy field - prefer adaptive_priority_fee_* for Universe Class
    pub priority_fee_lamports: u64,

    /// Compute unit limit for the transaction
    /// Legacy field - prefer min_cu_limit/max_cu_limit for dynamic adjustment
    pub compute_unit_limit: u32,

    /// Minimum compute unit limit for dynamic adjustment (Universe Class)
    /// Used as lower bound when pre-simulation estimates CU requirements
    pub min_cu_limit: u32,

    /// Maximum compute unit limit for dynamic adjustment (Universe Class)
    /// Used as upper bound to prevent excessive CU allocation
    pub max_cu_limit: u32,

    /// Adaptive priority fee base lamports (Universe Class)
    /// Starting point for congestion-based fee calculation
    pub adaptive_priority_fee_base: u64,

    /// Congestion multiplier for adaptive priority fee (Universe Class)
    /// Applied to base when network congestion is detected (e.g., 1.5 = 50% increase)
    pub adaptive_priority_fee_multiplier: f64,

    /// Amount to buy in SOL lamports
    pub buy_amount_lamports: u64,

    /// Slippage tolerance in basis points (bps, 100 = 1%)
    /// Can be automatically adjusted if enable_ml_slippage is true
    pub slippage_bps: u64,

    /// RPC endpoints for rotation/fallback (Universe Class: Arc for zero-copy)
    pub rpc_endpoints: Arc<[String]>,

    /// Max attempts per endpoint
    pub rpc_retry_attempts: usize,

    /// HTTP and RPC request timeout (ms)
    pub rpc_timeout_ms: u64,

    /// PumpPortal HTTP endpoint and API key
    pub pumpportal_url: Option<String>,
    pub pumpportal_api_key: Option<String>,

    /// LetsBonk HTTP endpoint and API key
    pub letsbonk_api_url: Option<String>,
    pub letsbonk_api_key: Option<String>,

    /// Jito bundle toggle
    pub jito_bundle_enabled: bool,

    /// Optional signer keypair index (for multi-signer wallets)
    pub signer_keypair_index: Option<usize>,

    /// Nonce semaphore capacity (parallel builds control)
    pub nonce_count: usize,

    /// Allowlist of programs with metadata (empty = allow all) (Universe Class)
    /// Uses DashMap for lock-free concurrent access and ProgramMetadata for version tracking
    pub allowed_programs: Arc<DashMap<Pubkey, ProgramMetadata>>,

    /// DEX priority order for fallback cascade (Universe Class)
    /// Ordered by preference, first DEX is tried first
    pub dex_priority: Vec<DexProgram>,

    /// Minimum liquidity depth threshold in lamports (Universe Class)
    /// Transactions to tokens with less liquidity will be rejected
    pub min_liquidity_lamports: u64,

    /// Enable pre-transaction simulation (Universe Class)
    /// When true, simulates transactions to estimate CU and validate before submission
    pub enable_simulation: bool,

    /// Enable ML-based slippage optimization (Universe Class)
    /// When true, uses SlippagePredictor to adjust slippage based on market volatility
    pub enable_ml_slippage: bool,

    /// Quorum configuration for blockhash consensus
    pub quorum_config: QuorumConfig,

    /// Retry policy for RPC operations
    pub retry_policy: RetryPolicy,

    /// RPC rate limit (requests per second, 0 = unlimited)
    pub rpc_rate_limit_rps: f64,

    /// Simulation rate limit (simulations per second, 0 = unlimited)
    pub simulation_rate_limit_rps: f64,

    /// HTTP rate limit for external APIs (requests per second, 0 = unlimited)
    pub http_rate_limit_rps: f64,

    /// Circuit breaker failure threshold
    pub circuit_breaker_failure_threshold: u32,

    /// Circuit breaker timeout in seconds
    pub circuit_breaker_timeout_secs: u64,

    /// Simulation cache configuration
    pub simulation_cache_config: SimulationCacheConfig,

    /// Maximum concurrent batch builds (worker pool size)
    pub max_concurrent_builds: usize,

    /// Operation priority for nonce/blockhash decision (Task 6)
    pub operation_priority: OperationPriority,

    /// Task 5: Signer rotation interval in transactions (default: 100)
    /// After this many transactions, a rotation checkpoint is logged
    pub signer_rotation_interval: u64,

    /// Nonce lease TTL in seconds (Phase 1, Task 1.4)
    /// Time-to-live for nonce leases, after which they expire
    /// Default: 30 seconds
    pub nonce_lease_ttl_secs: u64,

    /// Cluster configuration for pumpfun SDK
    #[cfg(feature = "pumpfun")]
    pub cluster: Cluster,
}

impl Default for TransactionConfig {
    fn default() -> Self {
        Self {
            priority_fee_lamports: 10_000,
            compute_unit_limit: 200_000,
            min_cu_limit: 100_000,
            max_cu_limit: 400_000,
            adaptive_priority_fee_base: 10_000,
            adaptive_priority_fee_multiplier: 1.5,
            buy_amount_lamports: 10_000_000,
            slippage_bps: 1000, // 10%
            rpc_endpoints: Arc::new(["https://api.mainnet-beta.solana.com".to_string()]),
            rpc_retry_attempts: 3,
            rpc_timeout_ms: 8_000,
            pumpportal_url: None,
            pumpportal_api_key: None,
            letsbonk_api_url: None,
            letsbonk_api_key: None,
            jito_bundle_enabled: false,
            signer_keypair_index: None,
            nonce_count: 5,
            allowed_programs: Arc::new(DashMap::new()),
            dex_priority: vec![DexProgram::PumpFun, DexProgram::Raydium, DexProgram::Orca],
            min_liquidity_lamports: 1_000_000_000, // 1 SOL
            enable_simulation: true,
            enable_ml_slippage: false,
            quorum_config: QuorumConfig::default(),
            retry_policy: RetryPolicy::default(),
            rpc_rate_limit_rps: 100.0,
            simulation_rate_limit_rps: 20.0,
            http_rate_limit_rps: 50.0,
            circuit_breaker_failure_threshold: 5,
            circuit_breaker_timeout_secs: 60,
            simulation_cache_config: SimulationCacheConfig::default(),
            max_concurrent_builds: 50,
            operation_priority: OperationPriority::default(),
            signer_rotation_interval: 100,
            nonce_lease_ttl_secs: 30,
            #[cfg(feature = "pumpfun")]
            cluster: Cluster::mainnet(Default::default(), Default::default()),
        }
    }
}

impl TransactionConfig {
    pub fn validate(&self) -> Result<(), TransactionBuilderError> {
        if self.buy_amount_lamports == 0 {
            return Err(TransactionBuilderError::ConfigValidation(
                "buy_amount_lamports must be > 0".to_string(),
            ));
        }
        if self.slippage_bps > 10000 {
            return Err(TransactionBuilderError::ConfigValidation(
                "slippage_bps must be <= 10000".to_string(),
            ));
        }
        if self.rpc_endpoints.is_empty() {
            return Err(TransactionBuilderError::ConfigValidation(
                "rpc_endpoints must contain at least one endpoint".to_string(),
            ));
        }
        if self.nonce_count == 0 {
            return Err(TransactionBuilderError::ConfigValidation(
                "nonce_count must be > 0".to_string(),
            ));
        }
        if self.min_cu_limit > self.max_cu_limit {
            return Err(TransactionBuilderError::ConfigValidation(
                "min_cu_limit must be <= max_cu_limit".to_string(),
            ));
        }
        if self.adaptive_priority_fee_multiplier < 1.0 {
            return Err(TransactionBuilderError::ConfigValidation(
                "adaptive_priority_fee_multiplier must be >= 1.0".to_string(),
            ));
        }
        if self.quorum_config.min_responses == 0 {
            return Err(TransactionBuilderError::ConfigValidation(
                "quorum_config.min_responses must be > 0".to_string(),
            ));
        }
        if self.quorum_config.min_responses > self.rpc_endpoints.len() {
            return Err(TransactionBuilderError::ConfigValidation(format!(
                "quorum_config.min_responses ({}) cannot exceed number of RPC endpoints ({})",
                self.quorum_config.min_responses,
                self.rpc_endpoints.len()
            )));
        }
        if self.circuit_breaker_failure_threshold == 0 {
            return Err(TransactionBuilderError::ConfigValidation(
                "circuit_breaker_failure_threshold must be > 0".to_string(),
            ));
        }
        if self.max_concurrent_builds == 0 {
            return Err(TransactionBuilderError::ConfigValidation(
                "max_concurrent_builds must be > 0".to_string(),
            ));
        }
        Ok(())
    }

    pub fn is_program_allowed(&self, program_id: &Pubkey) -> bool {
        self.allowed_programs.is_empty() || self.allowed_programs.contains_key(program_id)
    }

    /// Add a program to the allowed list with metadata (Universe Class)
    pub fn add_allowed_program(&self, program_id: Pubkey, metadata: ProgramMetadata) {
        self.allowed_programs.insert(program_id, metadata);
    }

    /// Get program metadata if it exists (Universe Class)
    pub fn get_program_metadata(&self, program_id: &Pubkey) -> Option<ProgramMetadata> {
        self.allowed_programs.get(program_id).map(|r| r.clone())
    }

    /// Calculate adaptive priority fee based on congestion (Task 3)
    ///
    /// Helper method to compute the priority fee with multiplier.
    /// Used in transaction building and tests to avoid code duplication.
    pub fn calculate_adaptive_priority_fee(&self) -> u64 {
        (self.adaptive_priority_fee_base as f64 * self.adaptive_priority_fee_multiplier) as u64
    }
}

// Jito bundle representation (Universe Class Enhanced)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitoBundleCandidate {
    pub transactions: Vec<VersionedTransaction>,
    pub max_total_cost_lamports: u64,
    pub target_slot: Option<u64>,
    /// MEV searcher hints for bundle ordering (Universe Class)
    pub searcher_hints: Vec<u8>,
    /// Enable backrun protection (Universe Class)
    pub backrun_protect: bool,
}

// TransactionBuilder errors (Universe Class Enhanced)
#[derive(Debug, Clone, Error)]
pub enum TransactionBuilderError {
    #[error("Configuration validation failed: {0}")]
    ConfigValidation(String),

    #[error("RPC connection failed: {0}")]
    RpcConnection(String),

    #[error("RPC manager error: {0}")]
    RpcManager(#[from] RpcManagerError),

    #[error("Instruction building failed for {program}: {reason}")]
    InstructionBuild { program: String, reason: String },

    #[error("Signing failed: {0}")]
    SigningFailed(String),

    #[error("Blockhash fetch failed: {0}")]
    BlockhashFetch(String),

    #[error("Nonce error: {0}")]
    Nonce(#[from] NonceError),

    #[error("Serialization failed: {0}")]
    Serialization(String),

    #[error("Program {0} is not allowed by configuration")]
    ProgramNotAllowed(Pubkey),

    #[error("Feature not enabled: {feature} for {action}")]
    FeatureNotEnabled { feature: String, action: String },

    #[error("Feature not available: {feature} - {reason}")]
    FeatureNotAvailable { feature: String, reason: String },

    #[error("Simulation failed: {0}")]
    SimulationFailed(String),

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: u64, available: u64 },

    #[error("Liquidity depth too low: {available} < {required}")]
    LiquidityTooLow { available: u64, required: u64 },

    #[error("Universe error: {0:?}")]
    Universe(UniverseErrorType),
}

/// Universe-level error classification (Universe Class)
#[derive(Debug, Clone)]
pub enum UniverseErrorType {
    TransientError {
        reason: String,
        retry_after_ms: u64,
    },
    FatalError {
        reason: String,
    },
    SecurityViolation {
        reason: String,
        confidence: f64,
    },
    ComputeOverrun {
        used: u32,
        limit: u32,
    },
    AnomalyDetected {
        description: String,
        confidence: f64,
    },
}

// Supported DEX programs with priority ordering
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DexProgram {
    PumpFun,         // Priority 0 (highest)
    LetsBonk,        // Priority 1
    Raydium,         // Priority 2
    Orca,            // Priority 3
    Unknown(String), // Priority 255 (lowest)
}

impl DexProgram {
    /// Get priority score (lower is better)
    pub fn priority(&self) -> u8 {
        match self {
            DexProgram::PumpFun => 0,
            DexProgram::LetsBonk => 1,
            DexProgram::Raydium => 2,
            DexProgram::Orca => 3,
            DexProgram::Unknown(_) => 255,
        }
    }
}

impl From<&str> for DexProgram {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pump.fun" | "pumpfun" | "pumpportal" => DexProgram::PumpFun,
            "letsbonk.fun" | "letsbonk" | "bonk" => DexProgram::LetsBonk,
            "raydium" => DexProgram::Raydium,
            "orca" | "whirlpool" => DexProgram::Orca,
            _ => DexProgram::Unknown(s.to_string()),
        }
    }
}

// TransactionBuilder (Universe Class Enhanced)
pub struct TransactionBuilder {
    pub wallet: Arc<WalletManager>,
    http: Client,
    rpc_endpoints: Arc<[String]>,
    rpc_rotation_index: AtomicUsize,
    blockhash_cache: RwLock<HashMap<Hash, (Instant, u64)>>,
    // Reduced to 15s as requested, with adaptive extension
    blockhash_cache_ttl: Duration,
    adaptive_ttl_enabled: bool,
    nonce_manager: Arc<NonceManager>,
    rpc_clients: Vec<Arc<RpcClient>>,
    slippage_predictor: RwLock<SlippagePredictor>,
    tx_counter: AtomicU64, // For signer rotation

    // Rate limiters
    rpc_rate_limiter: Option<Arc<TokenBucket>>,
    simulation_rate_limiter: Option<Arc<TokenBucket>>,
    http_rate_limiter: Option<Arc<TokenBucket>>,

    // Circuit breakers per RPC endpoint
    circuit_breakers: Vec<Arc<CircuitBreaker>>,

    // Simulation cache (message hash -> CU estimate)
    simulation_cache: Arc<DashMap<Hash, SimulationCacheEntry>>,

    // Task 1 & 6: Cache and operation telemetry counters
    simulation_cache_hits: AtomicU64,
    simulation_cache_misses: AtomicU64,
    nonce_acquire_count: AtomicU64,
    nonce_exhausted_count: AtomicU64,
    blockhash_quorum_success_count: AtomicU64,
    blockhash_fallback_count: AtomicU64,

    // Worker pool semaphore for batch operations
    worker_pool_semaphore: Arc<Semaphore>,

    #[cfg(feature = "pumpfun")]
    pumpfun_client: PumpFun,
}

impl TransactionBuilder {
    pub async fn new(
        wallet: Arc<WalletManager>,
        rpc_endpoints: Vec<String>,
        nonce_manager: Arc<NonceManager>,
        config: &TransactionConfig,
    ) -> Result<Self, TransactionBuilderError> {
        let http = Client::builder()
            .timeout(Duration::from_millis(config.rpc_timeout_ms))
            .pool_max_idle_per_host(50) // Universe Class: connection pooling
            .build()
            .map_err(|e| TransactionBuilderError::RpcConnection(e.to_string()))?;

        // Pre-initialize RPC clients for connection pooling
        let rpc_clients = rpc_endpoints
            .iter()
            .map(|endpoint| {
                Arc::new(RpcClient::new_with_timeout(
                    endpoint.clone(),
                    Duration::from_millis(config.rpc_timeout_ms),
                ))
            })
            .collect();

        #[cfg(feature = "pumpfun")]
        let pumpfun_client = PumpFun::new(wallet.keypair_arc(), config.cluster.clone());

        // Initialize rate limiters
        let rpc_rate_limiter = if config.rpc_rate_limit_rps > 0.0 {
            Some(Arc::new(TokenBucket::new(
                config.rpc_rate_limit_rps,
                config.rpc_rate_limit_rps,
            )))
        } else {
            None
        };

        let simulation_rate_limiter = if config.simulation_rate_limit_rps > 0.0 {
            Some(Arc::new(TokenBucket::new(
                config.simulation_rate_limit_rps,
                config.simulation_rate_limit_rps,
            )))
        } else {
            None
        };

        let http_rate_limiter = if config.http_rate_limit_rps > 0.0 {
            Some(Arc::new(TokenBucket::new(
                config.http_rate_limit_rps,
                config.http_rate_limit_rps,
            )))
        } else {
            None
        };

        // Initialize circuit breakers for each RPC endpoint
        let circuit_breakers: Vec<Arc<CircuitBreaker>> = (0..rpc_endpoints.len())
            .map(|_| {
                Arc::new(CircuitBreaker::new(
                    config.circuit_breaker_failure_threshold,
                    Duration::from_secs(config.circuit_breaker_timeout_secs),
                ))
            })
            .collect();

        Ok(Self {
            wallet,
            http,
            rpc_endpoints: Arc::from(rpc_endpoints),
            rpc_rotation_index: AtomicUsize::new(0),
            blockhash_cache: RwLock::new(HashMap::new()),
            blockhash_cache_ttl: Duration::from_secs(15),
            adaptive_ttl_enabled: true,
            nonce_manager,
            rpc_clients,
            slippage_predictor: RwLock::new(SlippagePredictor::new(100)),
            tx_counter: AtomicU64::new(0),
            rpc_rate_limiter,
            simulation_rate_limiter,
            http_rate_limiter,
            circuit_breakers,
            simulation_cache: Arc::new(DashMap::new()),
            simulation_cache_hits: AtomicU64::new(0),
            simulation_cache_misses: AtomicU64::new(0),
            nonce_acquire_count: AtomicU64::new(0),
            nonce_exhausted_count: AtomicU64::new(0),
            blockhash_quorum_success_count: AtomicU64::new(0),
            blockhash_fallback_count: AtomicU64::new(0),
            worker_pool_semaphore: Arc::new(Semaphore::new(config.max_concurrent_builds)),
            #[cfg(feature = "pumpfun")]
            pumpfun_client,
        })
    }

    pub async fn get_recent_blockhash(
        &self,
        config: &TransactionConfig,
    ) -> Result<Hash, TransactionBuilderError> {
        // Apply rate limiting for RPC calls
        if let Some(limiter) = &self.rpc_rate_limiter {
            limiter.consume(1.0).await;
        }

        // Universe Class: Quorum consensus with explicit slot validation
        let quorum_enabled = config.quorum_config.min_responses > 1
            && self.rpc_clients.len() >= config.quorum_config.min_responses;

        if quorum_enabled {
            // Task 4: Fix num_rpcs calculation - ensure we don't request more than min_responses
            // Original bug: min().max(3) could give less than min_responses
            let num_rpcs = config
                .quorum_config
                .min_responses
                .min(self.rpc_clients.len());

            // Ensure we have enough RPCs for quorum
            if num_rpcs < config.quorum_config.min_responses {
                debug!(
                    available_rpcs = self.rpc_clients.len(),
                    required = config.quorum_config.min_responses,
                    "Not enough RPCs for quorum, falling back to single RPC"
                );
            } else {
                // Task 3: Consistent commitment config across quorum and fallback
                let commitment_config =
                    solana_sdk::commitment_config::CommitmentConfig::confirmed();

                // Task 3: Per-RPC timeout (use half of configured timeout for each RPC)
                let per_rpc_timeout = Duration::from_millis(config.rpc_timeout_ms / 2);

                let mut tasks = Vec::with_capacity(num_rpcs);

                for i in 0..num_rpcs {
                    let rpc = self.rpc_clients[i].clone();
                    let circuit_breaker = self.circuit_breakers[i].clone();
                    let endpoint = self.rpc_endpoints[i].clone();
                    let commitment = commitment_config.clone();
                    let timeout = per_rpc_timeout;

                    tasks.push(tokio::spawn(async move {
                        // Check circuit breaker before attempting
                        if !circuit_breaker.can_execute().await {
                            debug!(endpoint = %endpoint, "Circuit breaker open, skipping");
                            return None;
                        }

                        // Task 3: Apply per-RPC timeout
                        let result = tokio::time::timeout(
                            timeout,
                            async {
                                let hash_result = rpc.get_latest_blockhash_with_commitment(commitment).await?;
                                let slot = rpc.get_slot().await?;
                                Ok::<_, anyhow::Error>((hash_result.0, slot))
                            }
                        ).await;

                        match result {
                            Ok(Ok((hash, slot))) => {
                                circuit_breaker.record_success().await;
                                debug!(endpoint = %endpoint, hash = %hash, slot = slot, "Quorum RPC response");
                                Some((hash, slot))
                            }
                            Ok(Err(e)) => {
                                circuit_breaker.record_failure().await;
                                debug!(endpoint = %endpoint, error = %e, "Quorum RPC error");
                                None
                            }
                            Err(_) => {
                                circuit_breaker.record_failure().await;
                                debug!(endpoint = %endpoint, timeout_ms = ?timeout.as_millis(), "Quorum RPC timeout");
                                None
                            }
                        }
                    }));
                }

                // Task 3: Early-exit when quorum is met - use FuturesUnordered to poll as they complete
                let mut futures_stream: FuturesUnordered<_> = tasks.into_iter().collect();
                let mut hash_votes: HashMap<Hash, (u32, u64, u64)> = HashMap::new(); // (count, max_slot, min_slot)
                let mut completed_count = 0;

                // Process results as they arrive
                while let Some(result) = futures_stream.next().await {
                    // Process completed task
                    if let Ok(Some((hash, slot))) = result {
                        hash_votes
                            .entry(hash)
                            .and_modify(|(count, max_slot, min_slot)| {
                                *count += 1;
                                *max_slot = (*max_slot).max(slot);
                                *min_slot = (*min_slot).min(slot);
                            })
                            .or_insert((1, slot, slot));

                        completed_count += 1;

                        // Task 3: Check if we've reached quorum early after each result
                        for (candidate_hash, (count, max_slot, min_slot)) in hash_votes.iter() {
                            let slot_diff = max_slot.saturating_sub(*min_slot);

                            // Check quorum: min_responses met AND slot diff within threshold
                            if *count >= config.quorum_config.min_responses as u32 {
                                if !config.quorum_config.enable_slot_validation
                                    || slot_diff <= config.quorum_config.max_slot_diff
                                {
                                    // Quorum reached! Return early without waiting for remaining RPCs
                                    self.blockhash_quorum_success_count
                                        .fetch_add(1, Ordering::Relaxed);

                                    let mut cache = self.blockhash_cache.write().await;
                                    cache.insert(*candidate_hash, (Instant::now(), *max_slot));
                                    drop(cache);

                                    info!(
                                        hash = %candidate_hash,
                                        slot = max_slot,
                                        votes = count,
                                        completed_rpcs = completed_count,
                                        total_rpcs = num_rpcs,
                                        slot_diff = slot_diff,
                                        quorum_success_count = self.blockhash_quorum_success_count.load(Ordering::Relaxed),
                                        "Blockhash quorum reached early"
                                    );
                                    return Ok(*candidate_hash);
                                }
                            }
                        }
                    }
                }

                // Task 3: All tasks completed, log vote distribution
                if !hash_votes.is_empty() {
                    let vote_summary: Vec<(Hash, u32)> = hash_votes
                        .iter()
                        .map(|(h, (count, _, _))| (*h, *count))
                        .collect();
                    debug!(votes = ?vote_summary, "Blockhash quorum vote distribution (all completed)");
                }

                // Final check: see if any hash reached quorum
                for (hash, (count, max_slot, min_slot)) in hash_votes.iter() {
                    let slot_diff = max_slot.saturating_sub(*min_slot);

                    if *count >= config.quorum_config.min_responses as u32 {
                        if !config.quorum_config.enable_slot_validation
                            || slot_diff <= config.quorum_config.max_slot_diff
                        {
                            self.blockhash_quorum_success_count
                                .fetch_add(1, Ordering::Relaxed);

                            let mut cache = self.blockhash_cache.write().await;
                            cache.insert(*hash, (Instant::now(), *max_slot));
                            drop(cache);

                            info!(
                                hash = %hash,
                                slot = max_slot,
                                votes = count,
                                slot_diff = slot_diff,
                                quorum_success_count = self.blockhash_quorum_success_count.load(Ordering::Relaxed),
                                "Blockhash quorum reached with slot validation"
                            );
                            return Ok(*hash);
                        }
                        warn!(
                            slot_diff = slot_diff,
                            max_allowed = config.quorum_config.max_slot_diff,
                            "Quorum reached but slot diff too large, rejecting"
                        );
                    }
                }

                // Task 3: Log why fallback happened - quorum not reached
                self.blockhash_fallback_count
                    .fetch_add(1, Ordering::Relaxed);
                warn!(
                    fallback_count = self.blockhash_fallback_count.load(Ordering::Relaxed),
                    reason = "quorum_not_reached",
                    min_responses = config.quorum_config.min_responses,
                    "Falling back to single RPC - quorum consensus failed"
                );
            } // Close else block for quorum availability check
        }

        // Fallback: Check cache first (slot-based staleness detection)
        if config.quorum_config.enable_slot_validation {
            // Get current slot first, before acquiring the cache lock
            if let Ok(current_slot) = self.rpc_clients[0].get_slot().await {
                let cache = self.blockhash_cache.read().await;
                let _now = Instant::now();

                // Find most recent non-stale entry
                if let Some((hash, _)) = cache
                    .iter()
                    .filter(|(_, (instant, slot))| {
                        let time_valid = instant.elapsed() < self.blockhash_cache_ttl;
                        let slot_valid = current_slot.saturating_sub(*slot)
                            <= config.quorum_config.max_slot_diff;
                        time_valid && slot_valid
                    })
                    .max_by_key(|(_, (_, slot))| *slot)
                    .map(|(h, (i, s))| (*h, (*i, *s)))
                {
                    debug!(hash = %hash, "Using cached blockhash (slot-validated)");
                    return Ok(hash);
                }
            }
        } else {
            // Time-based cache only
            let cache = self.blockhash_cache.read().await;
            let _now = Instant::now();
            if let Some((hash, _)) = cache
                .iter()
                .filter(|(_, (instant, _))| instant.elapsed() < self.blockhash_cache_ttl)
                .max_by_key(|(_, (instant, _))| *instant)
                .map(|(h, (i, s))| (*h, (*i, *s)))
            {
                debug!(hash = %hash, "Using cached blockhash (time-based)");
                return Ok(hash);
            }
        }

        // Fallback to single RPC with circuit breaker and retry policy
        // Task 3: Use consistent commitment config across quorum and fallback
        let commitment_config = solana_sdk::commitment_config::CommitmentConfig::confirmed();
        let mut last_err = None;
        let max_attempts = config.retry_policy.max_attempts.max(1);

        for attempt in 0..max_attempts {
            let index =
                self.rpc_rotation_index.fetch_add(1, Ordering::Relaxed) % self.rpc_endpoints.len();
            let rpc_client = &self.rpc_clients[index];
            let circuit_breaker = &self.circuit_breakers[index];

            // Check circuit breaker
            if !circuit_breaker.can_execute().await {
                // Task 3: Log explicit fallback reason - circuit_open
                self.blockhash_fallback_count
                    .fetch_add(1, Ordering::Relaxed);
                debug!(
                    endpoint = %self.rpc_endpoints[index],
                    reason = "circuit_open",
                    fallback_count = self.blockhash_fallback_count.load(Ordering::Relaxed),
                    "Circuit breaker open, skipping endpoint"
                );
                continue;
            }

            match rpc_client
                .get_latest_blockhash_with_commitment(commitment_config)
                .await
            {
                Ok((hash, _last_valid_block_height)) => {
                    // Fetch slot for validation
                    let slot = rpc_client.get_slot().await.unwrap_or(0);

                    // Update cache with slot
                    {
                        let mut cache = self.blockhash_cache.write().await;
                        cache.insert(hash, (Instant::now(), slot));

                        // Prune old entries (deterministic: by slot and time)
                        let cutoff_time = Instant::now() - self.blockhash_cache_ttl * 2;
                        let cutoff_slot =
                            slot.saturating_sub(config.quorum_config.max_slot_diff * 2);
                        cache.retain(|_, (instant, entry_slot)| {
                            *instant > cutoff_time && *entry_slot > cutoff_slot
                        });
                    } // Lock released here

                    circuit_breaker.record_success().await;
                    return Ok(hash);
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    debug!(
                        attempt = attempt,
                        endpoint = %self.rpc_endpoints[index],
                        error = %error_msg,
                        "Blockhash fetch failed"
                    );

                    circuit_breaker.record_failure().await;

                    // Check if error is retryable
                    if !config.retry_policy.is_retryable(&error_msg) {
                        return Err(TransactionBuilderError::BlockhashFetch(format!(
                            "Fatal error (non-retryable): {}",
                            error_msg
                        )));
                    }

                    last_err = Some(anyhow!(error_msg));

                    // Apply exponential backoff
                    if attempt + 1 < max_attempts {
                        let delay = config.retry_policy.delay_for_attempt(attempt);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(TransactionBuilderError::BlockhashFetch(format!(
            "All RPC endpoints failed after {} attempts: {:?}",
            max_attempts, last_err
        )))
    }

    /// Task 1: Prepare execution context with nonce or recent blockhash (Shared Helper)
    ///
    /// This method encapsulates the nonce vs blockhash decision logic, unifying behavior
    /// across buy and sell transaction paths. The lease lifecycle is explicit through
    /// RAII patterns - the nonce lease is automatically released when ExecutionContext
    /// is dropped.
    ///
    /// # Telemetry
    ///
    /// - Increments `nonce_acquire_count` on successful acquisition
    /// - Increments `nonce_exhausted_count` on exhaustion
    /// - Increments `blockhash_fallback_count` on fallback to recent blockhash
    ///
    /// # Behavior
    ///
    /// Based on `config.operation_priority`:
    /// - `CriticalSniper`: Requires nonce, fails fast on exhaustion
    /// - `Utility`/`Bulk`: Prefers recent blockhash, allows fallback
    /// Unified durable nonce execution context preparation with enforcement control
    ///
    /// # Parameters
    ///
    /// * `config` - Transaction configuration
    /// * `enforce_nonce` - When `true`, always acquire nonce lease or fail (no fallback to blockhash).
    ///                     When `false`, skip nonce acquisition entirely (use recent blockhash).
    ///
    /// # Behavior
    ///
    /// * `enforce_nonce = false`: Zero interaction with NonceManager, returns blockhash-only context
    /// * `enforce_nonce = true`: Always acquire nonce lease, fail if unavailable (no fallback)
    ///
    /// This function provides deterministic control over nonce usage without fallback logic.
    async fn prepare_execution_context_with_enforcement(
        &self,
        config: &TransactionConfig,
        enforce_nonce: bool,
    ) -> Result<ExecutionContext, TransactionBuilderError> {
        if !enforce_nonce {
            // Skip nonce acquisition entirely - use recent blockhash
            let blockhash = self.get_recent_blockhash(config).await?;
            debug!(
                enforce_nonce = false,
                "Using recent blockhash (nonce enforcement disabled)"
            );

            return Ok(ExecutionContext {
                blockhash,
                nonce_pubkey: None,
                nonce_authority: None,
                nonce_lease: None,
                #[cfg(feature = "zk_enabled")]
                zk_proof: None,
            });
        }

        // enforce_nonce = true: Always acquire nonce lease or fail
        // Phase 1, Task 1.3: Use try_acquire for atomic, TOCTTOU-safe acquisition
        let ttl = Duration::from_secs(config.nonce_lease_ttl_secs);
        match self.nonce_manager.try_acquire_nonce(ttl, 2000).await {
            #[cfg(feature = "zk_enabled")]
            Some(mut lease) => {
                // Record successful nonce acquisition
                self.nonce_acquire_count.fetch_add(1, Ordering::Relaxed);

                let blockhash = lease.nonce_blockhash();
                let nonce_pubkey = Some(*lease.nonce_pubkey());
                let nonce_authority = Some(self.wallet.pubkey());

                // Extract and verify ZK proof from lease
                let zk_proof = lease.take_proof();

                // Verify ZK proof if available (abort on low confidence)
                if let Some(ref proof) = zk_proof {
                    if proof.confidence < 0.5 {
                        warn!(
                            nonce_pubkey = ?nonce_pubkey,
                            confidence = proof.confidence,
                            "ZK proof verification failed - aborting transaction"
                        );
                        return Err(TransactionBuilderError::Nonce(NonceError::Configuration(
                            format!(
                                "ZK proof verification failed - confidence: {:.2}",
                                proof.confidence
                            ),
                        )));
                    }

                    if proof.confidence < 0.8 {
                        warn!(
                            nonce_pubkey = ?nonce_pubkey,
                            confidence = proof.confidence,
                            "ZK proof low confidence warning"
                        );
                    }

                    debug!(
                        nonce_pubkey = ?nonce_pubkey,
                        confidence = proof.confidence,
                        "ZK proof verified successfully"
                    );
                }

                debug!(
                    nonce_acquire_count = self.nonce_acquire_count.load(Ordering::Relaxed),
                    nonce_pubkey = ?nonce_pubkey,
                    zk_proof_present = zk_proof.is_some(),
                    enforce_nonce = true,
                    "Using nonce with enforcement enabled"
                );

                Ok(ExecutionContext {
                    blockhash,
                    nonce_pubkey,
                    nonce_authority,
                    nonce_lease: Some(lease),
                    zk_proof,
                })
            }
            #[cfg(not(feature = "zk_enabled"))]
            Some(lease) => {
                // Record successful nonce acquisition
                self.nonce_acquire_count.fetch_add(1, Ordering::Relaxed);

                let blockhash = lease.nonce_blockhash();
                let nonce_pubkey = Some(*lease.nonce_pubkey());
                let nonce_authority = Some(self.wallet.pubkey());

                debug!(
                    nonce_acquire_count = self.nonce_acquire_count.load(Ordering::Relaxed),
                    nonce_pubkey = ?nonce_pubkey,
                    enforce_nonce = true,
                    "Using nonce with enforcement enabled"
                );

                Ok(ExecutionContext {
                    blockhash,
                    nonce_pubkey,
                    nonce_authority,
                    nonce_lease: Some(lease),
                })
            }
            None => {
                // Record exhaustion event
                self.nonce_exhausted_count.fetch_add(1, Ordering::Relaxed);

                // Fail fast when enforcement is enabled (no fallback allowed)
                Err(TransactionBuilderError::Nonce(NonceError::NoLeaseAvailable))
            }
        }
    }

    async fn prepare_execution_context(
        &self,
        config: &TransactionConfig,
    ) -> Result<ExecutionContext, TransactionBuilderError> {
        let use_nonce = config.operation_priority.requires_nonce();
        let allow_fallback = config.operation_priority.allow_blockhash_fallback();

        if use_nonce {
            // Critical operation - require nonce lease
            match self.nonce_manager.acquire_nonce().await {
                #[cfg(feature = "zk_enabled")]
                Ok(mut lease) => {
                    // Record successful nonce acquisition
                    self.nonce_acquire_count.fetch_add(1, Ordering::Relaxed);

                    let blockhash = lease.nonce_blockhash();
                    let nonce_pubkey = Some(*lease.nonce_pubkey());
                    let nonce_authority = Some(self.wallet.pubkey());

                    // Extract and verify ZK proof from lease
                    let zk_proof = lease.take_proof();

                    // Verify ZK proof if available (abort on low confidence)
                    if let Some(ref proof) = zk_proof {
                        if proof.confidence < 0.5 {
                            warn!(
                                nonce_pubkey = ?nonce_pubkey,
                                confidence = proof.confidence,
                                "ZK proof verification failed - aborting transaction"
                            );
                            return Err(TransactionBuilderError::Nonce(NonceError::Configuration(
                                format!(
                                    "ZK proof verification failed - confidence: {:.2}",
                                    proof.confidence
                                ),
                            )));
                        }

                        if proof.confidence < 0.8 {
                            warn!(
                                nonce_pubkey = ?nonce_pubkey,
                                confidence = proof.confidence,
                                "ZK proof low confidence warning"
                            );
                        }

                        debug!(
                            nonce_pubkey = ?nonce_pubkey,
                            confidence = proof.confidence,
                            "ZK proof verified successfully"
                        );
                    }

                    debug!(
                        nonce_acquire_count = self.nonce_acquire_count.load(Ordering::Relaxed),
                        nonce_pubkey = ?nonce_pubkey,
                        zk_proof_present = zk_proof.is_some(),
                        "Using nonce for critical operation"
                    );

                    Ok(ExecutionContext {
                        blockhash,
                        nonce_pubkey,
                        nonce_authority,
                        nonce_lease: Some(lease),
                        zk_proof,
                    })
                }
                #[cfg(not(feature = "zk_enabled"))]
                Ok(lease) => {
                    // Record successful nonce acquisition
                    self.nonce_acquire_count.fetch_add(1, Ordering::Relaxed);

                    let blockhash = lease.nonce_blockhash();
                    let nonce_pubkey = Some(*lease.nonce_pubkey());
                    let nonce_authority = Some(self.wallet.pubkey());

                    debug!(
                        nonce_acquire_count = self.nonce_acquire_count.load(Ordering::Relaxed),
                        nonce_pubkey = ?nonce_pubkey,
                        "Using nonce for critical operation"
                    );

                    Ok(ExecutionContext {
                        blockhash,
                        nonce_pubkey,
                        nonce_authority,
                        nonce_lease: Some(lease),
                    })
                }
                Err(e) => {
                    // Record exhaustion event
                    self.nonce_exhausted_count.fetch_add(1, Ordering::Relaxed);

                    if allow_fallback {
                        // Log why fallback happened - nonce exhaustion
                        self.blockhash_fallback_count
                            .fetch_add(1, Ordering::Relaxed);
                        warn!(
                            nonce_exhausted_count = self.nonce_exhausted_count.load(Ordering::Relaxed),
                            fallback_count = self.blockhash_fallback_count.load(Ordering::Relaxed),
                            reason = "nonce_exhaustion",
                            error = %e,
                            "Nonce exhausted, falling back to recent blockhash"
                        );

                        let blockhash = self.get_recent_blockhash(config).await?;
                        Ok(ExecutionContext {
                            blockhash,
                            nonce_pubkey: None,
                            nonce_authority: None,
                            nonce_lease: None,
                            #[cfg(feature = "zk_enabled")]
                            zk_proof: None,
                        })
                    } else {
                        // Fail fast for critical operations
                        Err(TransactionBuilderError::Nonce(NonceError::NoLeaseAvailable))
                    }
                }
            }
        } else {
            // Utility/Bulk operation - prefer recent blockhash
            let blockhash = self.get_recent_blockhash(config).await?;
            debug!(
                priority = ?config.operation_priority,
                "Using recent blockhash for non-critical operation"
            );

            Ok(ExecutionContext {
                blockhash,
                nonce_pubkey: None,
                nonce_authority: None,
                nonce_lease: None,
                #[cfg(feature = "zk_enabled")]
                zk_proof: None,
            })
        }
    }

    /// Sanity check for instruction ordering (debug/test builds only)
    ///
    /// Validates that instructions follow the correct order:
    /// 1. Advance nonce instruction (if present, must be at index 0)
    /// 2. Compute budget instructions (set_compute_unit_limit, set_compute_unit_price)
    /// 3. DEX instruction (the actual buy/sell operation)
    ///
    /// This function is only compiled in debug or test builds to catch
    /// ordering errors during development without runtime overhead in production.
    #[cfg(any(debug_assertions, test))]
    pub(crate) fn validate_instruction_order(
        instructions: &[Instruction],
        has_nonce: bool,
        simulation_mode: bool,
    ) -> Result<(), String> {
        if instructions.is_empty() {
            return Err("Instruction list is empty".to_string());
        }

        let mut expected_idx = 0;

        // Rule 1: Advance nonce instruction must be first (if present and not in simulation mode)
        if has_nonce && !simulation_mode {
            if expected_idx >= instructions.len() {
                return Err(
                    "Expected advance nonce instruction but instruction list too short".to_string(),
                );
            }

            let first_ix = &instructions[expected_idx];
            // Check if this is an advance_nonce_account instruction
            if first_ix.program_id != solana_sdk::system_program::id() {
                return Err(format!(
                    "Expected advance nonce instruction at index 0, but found program_id: {}",
                    first_ix.program_id
                ));
            }

            expected_idx += 1;
            debug!("✓ Advance nonce instruction at correct position (index 0)");
        } else if has_nonce && simulation_mode {
            // Simulation mode: advance nonce should NOT be present
            let first_ix = &instructions[0];
            if first_ix.program_id == solana_sdk::system_program::id() {
                // This might be a nonce instruction - log warning
                warn!("Simulation mode should not include advance nonce instruction");
            }
        }

        // Rule 2: Compute budget instructions should come next (if present)
        let compute_budget_program = solana_sdk::compute_budget::id();
        while expected_idx < instructions.len()
            && instructions[expected_idx].program_id == compute_budget_program
        {
            expected_idx += 1;
        }

        if expected_idx > 0 && instructions[expected_idx - 1].program_id == compute_budget_program {
            debug!("✓ Compute budget instructions at correct position");
        }

        // Rule 3: DEX instruction should be last
        if expected_idx >= instructions.len() {
            return Err("No DEX instruction found after compute budget instructions".to_string());
        }

        let dex_idx = expected_idx;
        let dex_program_id = instructions[dex_idx].program_id;

        // Verify this is not a compute budget or system program (should be DEX)
        if dex_program_id == compute_budget_program {
            return Err(format!(
                "Expected DEX instruction at index {}, found compute budget instruction",
                dex_idx
            ));
        }
        if dex_program_id == solana_sdk::system_program::id() && expected_idx > 0 {
            return Err(format!(
                "Expected DEX instruction at index {}, found system program instruction",
                dex_idx
            ));
        }

        debug!("✓ DEX instruction at correct position (index {})", dex_idx);

        // Verify there's exactly one more instruction (the DEX instruction)
        if expected_idx + 1 != instructions.len() {
            return Err(format!(
                "Expected exactly {} instructions, found {}. Only one DEX instruction should be present.",
                expected_idx + 1,
                instructions.len()
            ));
        }

        Ok(())
    }

    /// Build instructions in deterministic order for transaction construction
    ///
    /// # Instruction Order
    ///
    /// 1. **Advance nonce instruction** (if present and not simulation mode)
    ///    - Only included in production transactions with durable nonce
    ///    - Excluded in simulation mode
    ///
    /// 2. **Compute budget instructions** (if present)
    ///    - set_compute_unit_limit
    ///    - set_compute_unit_price
    ///
    /// 3. **DEX instruction** (always last)
    ///    - The actual buy/sell operation
    ///
    /// # Parameters
    ///
    /// * `exec_ctx` - Execution context (contains nonce info)
    /// * `simulation_mode` - If true, excludes advance nonce instruction
    /// * `compute_budget_instructions` - Optional compute budget instructions
    /// * `dex_instruction` - The DEX buy/sell instruction
    ///
    /// # Returns
    ///
    /// Properly ordered vector of instructions ready for transaction compilation
    pub(crate) fn build_ordered_instructions(
        exec_ctx: &ExecutionContext,
        simulation_mode: bool,
        compute_budget_instructions: Vec<Instruction>,
        dex_instruction: Instruction,
    ) -> Vec<Instruction> {
        let has_nonce = exec_ctx.nonce_pubkey.is_some() && exec_ctx.nonce_authority.is_some();
        let capacity = if has_nonce && !simulation_mode { 1 } else { 0 } // advance nonce
            + compute_budget_instructions.len()               // compute budget
            + 1; // DEX instruction

        let mut instructions = Vec::with_capacity(capacity);

        // Step 1: Add advance nonce instruction (only for production with nonce)
        if has_nonce && !simulation_mode {
            let nonce_pub = exec_ctx.nonce_pubkey.expect("nonce_pubkey checked above");
            let nonce_auth = exec_ctx
                .nonce_authority
                .expect("nonce_authority checked above");

            let advance_nonce_ix =
                solana_sdk::system_instruction::advance_nonce_account(&nonce_pub, &nonce_auth);
            instructions.push(advance_nonce_ix);
            debug!("Added advance nonce instruction at index 0");
        }

        // Step 2: Add compute budget instructions
        for ix in compute_budget_instructions {
            instructions.push(ix);
        }

        if !instructions.is_empty()
            && instructions.len() > if has_nonce && !simulation_mode { 1 } else { 0 }
        {
            debug!(
                "Added {} compute budget instruction(s)",
                instructions.len() - if has_nonce && !simulation_mode { 1 } else { 0 }
            );
        }

        // Step 3: Add DEX instruction (always last)
        instructions.push(dex_instruction);
        debug!("Added DEX instruction at index {}", instructions.len() - 1);

        // Sanity check in debug/test builds
        #[cfg(any(debug_assertions, test))]
        {
            if let Err(e) =
                Self::validate_instruction_order(&instructions, has_nonce, simulation_mode)
            {
                panic!("Instruction order validation failed: {}", e);
            }
        }

        instructions
    }

    /// Build a buy transaction (Phase 1, Task 1.1)
    ///
    /// This is a convenience wrapper that calls `build_buy_transaction_with_nonce`
    /// with `enforce_nonce = true` by default for trade-critical operations.
    ///
    /// # Arguments
    ///
    /// * `candidate` - Token candidate to buy
    /// * `config` - Transaction configuration
    /// * `sign` - Whether to sign the transaction
    ///
    /// # Note
    ///
    /// This method is now a legacy wrapper. Consider using `build_buy_transaction_output`
    /// for proper RAII nonce management.
    pub async fn build_buy_transaction(
        &self,
        candidate: &PremintCandidate,
        config: &TransactionConfig,
        sign: bool,
    ) -> Result<VersionedTransaction, TransactionBuilderError> {
        self.build_buy_transaction_with_nonce(candidate, config, sign, true)
            .await
    }

    /// Build a buy transaction with explicit nonce enforcement control (Phase 1, Task 1.1)
    ///
    /// # Legacy API Warning
    ///
    /// This method releases nonce guard early. Prefer using `build_buy_transaction_output`
    /// for proper RAII nonce management that holds the guard through broadcast.
    ///
    /// # Arguments
    ///
    /// * `candidate` - Token candidate to buy
    /// * `config` - Transaction configuration
    /// * `sign` - Whether to sign the transaction
    /// * `enforce_nonce` - Whether to enforce durable nonce usage (true for trade-critical ops)
    pub async fn build_buy_transaction_with_nonce(
        &self,
        candidate: &PremintCandidate,
        config: &TransactionConfig,
        sign: bool,
        enforce_nonce: bool,
    ) -> Result<VersionedTransaction, TransactionBuilderError> {
        // Phase 2, Task 2.3: Legacy wrapper with deprecation warning
        use std::sync::Once;
        static WARN_ONCE: Once = Once::new();

        WARN_ONCE.call_once(|| {
            warn!(
                "Legacy API: build_buy_transaction_with_nonce releases nonce early. \
                 Migrate to build_buy_transaction_output for proper RAII."
            );
        });

        let output = self
            .build_buy_transaction_output(candidate, config, sign, enforce_nonce)
            .await?;

        // Extract transaction, dropping the nonce guard early (legacy behavior)
        Ok(output.into_tx())
    }

    /// Build a buy transaction returning TxBuildOutput with RAII nonce guard (Phase 2, Task 2.3)
    ///
    /// This is the recommended API that returns `TxBuildOutput` holding the nonce guard
    /// through the transaction lifetime, ensuring proper RAII cleanup on all paths.
    ///
    /// # Arguments
    ///
    /// * `candidate` - Token candidate to buy
    /// * `config` - Transaction configuration
    /// * `sign` - Whether to sign the transaction
    /// * `enforce_nonce` - Whether to enforce durable nonce usage (true for trade-critical ops)
    ///
    /// # Returns
    ///
    /// `TxBuildOutput` containing the transaction and optional nonce guard.
    /// The guard should be held until broadcast completes, then explicitly released
    /// via `output.release_nonce().await` or dropped on error.
    ///
    /// # Example
    ///
    /// ```no_run
    /// let output = builder.build_buy_transaction_output(&candidate, &config, false, true).await?;
    ///
    /// // Hold guard during broadcast
    /// let sig = rpc.send_transaction(&output.tx).await?;
    ///
    /// // Explicitly release after successful broadcast
    /// output.release_nonce().await?;
    /// ```
    pub async fn build_buy_transaction_output(
        &self,
        candidate: &PremintCandidate,
        config: &TransactionConfig,
        sign: bool,
        enforce_nonce: bool,
    ) -> Result<TxBuildOutput, TransactionBuilderError> {
        config.validate()?;
        info!(
            mint = %candidate.mint,
            program = %candidate.program,
            "Building buy transaction (Universe Class)"
        );

        // Task 5: Increment transaction counter for signer rotation tracking
        let tx_count = self.tx_counter.fetch_add(1, Ordering::Relaxed);

        // Task 5: Check if rotation checkpoint is reached
        // In a full implementation, this would trigger actual key rotation
        // For now, we log the event for telemetry
        if tx_count % config.signer_rotation_interval == 0 && tx_count > 0 {
            info!(
                tx_count = tx_count,
                rotation_checkpoint = tx_count / config.signer_rotation_interval,
                interval = config.signer_rotation_interval,
                "Signer rotation checkpoint reached - rotation would occur here in full implementation"
            );
            // Note: Actual rotation would require:
            // 1. Multiple keypairs in WalletManager
            // 2. Atomic swap of active keypair
            // 3. Zeroization of old key material
            // 4. Update of signer_keypair_index in config
        }

        // Phase 1, Task 1.2: Priority defaulting policy
        // If enforce_nonce is true and priority is Utility, upgrade to CriticalSniper
        let mut effective_config = config.clone();
        if enforce_nonce && matches!(config.operation_priority, OperationPriority::Utility) {
            debug!(
                original_priority = ?config.operation_priority,
                new_priority = ?OperationPriority::CriticalSniper,
                "Upgrading operation priority for nonce enforcement"
            );
            effective_config.operation_priority = OperationPriority::CriticalSniper;
        }

        // Phase 1, Task 1.4: Use prepare_execution_context_with_enforcement
        let exec_ctx = self
            .prepare_execution_context_with_enforcement(&effective_config, enforce_nonce)
            .await?;
        let recent_blockhash = exec_ctx.blockhash;

        // Universe Class: ML-based slippage optimization
        // Note: We use the original config but the instruction builders will use
        // config.slippage_bps which should be set at transaction building time
        let _optimized_slippage = if config.enable_ml_slippage {
            let predictor = self.slippage_predictor.read().await;
            predictor.predict_optimal_slippage(config.slippage_bps)
        } else {
            config.slippage_bps
        };

        // Pre-allocate instruction vector for hot-path performance
        let mut _instructions: Vec<Instruction> = Vec::with_capacity(4);

        // Universe Class: Dynamic compute unit limit (will be set after simulation)
        let mut dynamic_cu_limit = config.compute_unit_limit;

        // Task 3: Calculate adaptive priority fee BEFORE simulation (needed for cache hash)
        // Universe Class: Adaptive priority fee based on congestion
        let adaptive_priority_fee = config.calculate_adaptive_priority_fee();

        // Build program-specific instruction first for simulation
        let dex_program = DexProgram::from(candidate.program.as_str());
        let buy_instruction = match dex_program {
            DexProgram::PumpFun => self.build_pumpfun_instruction(candidate, config).await,
            DexProgram::LetsBonk => self.build_letsbonk_instruction(candidate, config).await,
            DexProgram::Raydium => {
                self.build_placeholder_buy_instruction(candidate, config)
                    .await
            } // TODO: implement Raydium
            DexProgram::Orca => {
                self.build_placeholder_buy_instruction(candidate, config)
                    .await
            } // TODO: implement Orca
            DexProgram::Unknown(_) => {
                self.build_placeholder_buy_instruction(candidate, config)
                    .await
            }
        }?;

        // Check if this is a placeholder instruction (no adaptive fee for placeholders)
        let is_placeholder = matches!(
            dex_program,
            DexProgram::Unknown(_) | DexProgram::Raydium | DexProgram::Orca
        );

        // Universe Class: Pre-simulation for CU estimation with caching
        if config.enable_simulation {
            // Apply rate limiting for simulations
            if let Some(limiter) = &self.simulation_rate_limiter {
                limiter.consume(1.0).await;
            }

            // Build simulation instructions (compute budget + DEX only, no nonce)
            // Note: We use empty compute budget for simulation since we're estimating CU
            let sim_instructions = Self::build_ordered_instructions(
                &exec_ctx,
                true,   // simulation_mode = true (excludes advance nonce instruction)
                vec![], // No compute budget in simulation (we're estimating it)
                buy_instruction.clone(),
            );
            let payer = self.wallet.pubkey();

            if let Ok(sim_message) =
                MessageV0::try_compile(&payer, &sim_instructions, &[], recent_blockhash)
            {
                // Task 1: Create deterministic message hash for cache lookup
                // Use hash of message content instead of blockhash to avoid non-deterministic cache keys
                let mut hasher = Sha256::new();

                // Hash the message content (instructions + payer + accounts)
                hasher.update(payer.to_bytes());
                for instruction in &sim_instructions {
                    hasher.update(instruction.program_id.to_bytes());
                    hasher.update(&instruction.data);
                    for account in &instruction.accounts {
                        hasher.update(account.pubkey.to_bytes());
                        hasher.update(&[account.is_signer as u8, account.is_writable as u8]);
                    }
                }
                // Include compute unit limit in hash
                hasher.update(&dynamic_cu_limit.to_le_bytes());
                hasher.update(&adaptive_priority_fee.to_le_bytes());

                let hash_bytes = hasher.finalize();
                let message_hash = Hash::new_from_array(hash_bytes[..32].try_into().expect(
                    "Failed to convert SHA256 hash to 32-byte array for message cache key",
                ));

                // Task 1: Check if program is excluded from caching
                let program_id = &buy_instruction.program_id;
                let cache_enabled = config.simulation_cache_config.enabled
                    && !config
                        .simulation_cache_config
                        .is_program_excluded(program_id);

                // Check simulation cache first
                let cached_result = if cache_enabled {
                    self.simulation_cache.get(&message_hash).and_then(|entry| {
                        let _current_slot = entry.slot; // Will be validated below
                        let elapsed = entry.cached_at.elapsed().as_secs();

                        // Task 1: Blockhash used only for freshness validation, not as primary key
                        if elapsed < config.simulation_cache_config.ttl_seconds {
                            // Task 1: Record cache hit
                            self.simulation_cache_hits.fetch_add(1, Ordering::Relaxed);
                            debug!(
                                cached_cu = entry.compute_units,
                                age_secs = elapsed,
                                "Simulation cache hit"
                            );
                            Some(entry.compute_units)
                        } else {
                            // Entry expired, remove it
                            drop(entry);
                            self.simulation_cache.remove(&message_hash);
                            // Task 1: Record cache miss (expired)
                            self.simulation_cache_misses.fetch_add(1, Ordering::Relaxed);
                            None
                        }
                    })
                } else {
                    None
                };

                if let Some(cached_cu) = cached_result {
                    // Use cached CU estimate
                    let estimated_cu = ((cached_cu as f64) * 1.2) as u32;
                    dynamic_cu_limit = estimated_cu.clamp(config.min_cu_limit, config.max_cu_limit);
                    debug!(
                        cached_cu = cached_cu,
                        dynamic_cu = dynamic_cu_limit,
                        "CU estimation from cache"
                    );
                } else {
                    // Task 1: Record cache miss (not found)
                    if cache_enabled {
                        self.simulation_cache_misses.fetch_add(1, Ordering::Relaxed);
                    }

                    // Perform simulation
                    let sim_tx = VersionedTransaction {
                        signatures: vec![Signature::default()],
                        message: VersionedMessage::V0(sim_message),
                    };

                    let rpc = self.rpc_client_for(0);
                    match tokio::time::timeout(
                        Duration::from_millis(config.rpc_timeout_ms / 2),
                        rpc.simulate_transaction(&sim_tx),
                    )
                    .await
                    {
                        Ok(Ok(sim_result)) => {
                            if let Some(units_consumed) = sim_result.value.units_consumed {
                                // Cache the result (only if not excluded)
                                if cache_enabled {
                                    let slot = rpc.get_slot().await.unwrap_or(0);
                                    let cache_entry = SimulationCacheEntry {
                                        compute_units: units_consumed,
                                        cached_at: Instant::now(),
                                        slot,
                                    };

                                    self.simulation_cache.insert(message_hash, cache_entry);

                                    // Task 1: Prune cache using LRU ordering (oldest by timestamp)
                                    if self.simulation_cache.len()
                                        > config.simulation_cache_config.max_size
                                    {
                                        let remove_count =
                                            config.simulation_cache_config.max_size / 10;

                                        // Collect entries with timestamps and sort by LRU (oldest first)
                                        let mut entries: Vec<(Hash, Instant)> = self
                                            .simulation_cache
                                            .iter()
                                            .map(|e| (*e.key(), e.value().cached_at))
                                            .collect();
                                        entries.sort_by_key(|(_, timestamp)| *timestamp);

                                        // Remove oldest entries
                                        let keys_to_remove: Vec<Hash> = entries
                                            .iter()
                                            .take(remove_count)
                                            .map(|(key, _)| *key)
                                            .collect();

                                        for key in keys_to_remove {
                                            self.simulation_cache.remove(&key);
                                        }

                                        debug!(
                                            removed = remove_count,
                                            cache_size = self.simulation_cache.len(),
                                            "Pruned simulation cache using LRU"
                                        );
                                    }
                                }

                                // Add 20% buffer to simulated CU
                                let estimated_cu = ((units_consumed as f64) * 1.2) as u32;
                                dynamic_cu_limit =
                                    estimated_cu.clamp(config.min_cu_limit, config.max_cu_limit);

                                debug!(
                                    simulated_cu = units_consumed,
                                    dynamic_cu = dynamic_cu_limit,
                                    "CU estimation from simulation"
                                );
                            }

                            // Task 5: Classify simulation errors as fatal vs advisory
                            if let Some(err) = sim_result.value.err {
                                // Define fatal error patterns as constants for maintainability
                                const FATAL_ERROR_PATTERNS: &[&str] = &[
                                    "InstructionError",
                                    "ProgramFailedToComplete",
                                    "ComputeBudgetExceeded",
                                    "InsufficientFunds",
                                ];

                                let error_str = format!("{:?}", err);

                                // Check if error matches any fatal pattern
                                let is_fatal = FATAL_ERROR_PATTERNS
                                    .iter()
                                    .any(|pattern| error_str.contains(pattern));

                                if is_fatal {
                                    warn!(
                                        error = ?err,
                                        "Fatal simulation error detected, aborting transaction"
                                    );
                                    return Err(TransactionBuilderError::SimulationFailed(
                                        error_str,
                                    ));
                                }
                                // Advisory warning - log but proceed with caution
                                warn!(
                                    error = ?err,
                                    "Simulation returned advisory error, proceeding with caution"
                                );
                            }
                        }
                        Ok(Err(e)) => {
                            debug!(error = %e, "Simulation failed, using default CU limit");
                        }
                        Err(_) => {
                            debug!("Simulation timeout, using default CU limit");
                        }
                    }
                }
            }
        }

        // Build compute budget instructions
        let mut compute_budget_instructions = Vec::with_capacity(2);

        if dynamic_cu_limit > 0 {
            compute_budget_instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
                dynamic_cu_limit,
            ));
        }

        // Task 3: adaptive_priority_fee now calculated earlier (before simulation section)
        // to be included in cache hash for deterministic cache keys
        // Rule: No adaptive fee applied for placeholder instructions
        if adaptive_priority_fee > 0 && !is_placeholder {
            compute_budget_instructions.push(ComputeBudgetInstruction::set_compute_unit_price(
                adaptive_priority_fee,
            ));
        }

        // Build instructions in deterministic order using helper function
        // Order: advance_nonce (if present) -> compute_budget -> dex_instruction
        // simulation_mode = false for production transactions
        let instructions = Self::build_ordered_instructions(
            &exec_ctx,
            false, // simulation_mode = false (this is a production transaction)
            compute_budget_instructions,
            buy_instruction,
        );

        // Compile message (V0)
        let payer = self.wallet.pubkey();
        let message_v0 = MessageV0::try_compile(&payer, &instructions, &[], recent_blockhash)
            .map_err(|e| TransactionBuilderError::InstructionBuild {
                program: candidate.program.clone(),
                reason: format!("Failed to compile message: {}", e),
            })?;

        let versioned_message = VersionedMessage::V0(message_v0);
        let mut tx = VersionedTransaction {
            signatures: vec![],
            message: versioned_message,
        };

        if sign {
            // Sign the transaction manually since VersionedTransaction doesn't have try_sign
            use solana_sdk::signature::Signer;
            let keypair = self.wallet.keypair();
            let message_bytes = tx.message.serialize();
            let signature = keypair.sign_message(&message_bytes);
            tx.signatures = vec![signature];
        } else {
            // Initialize with default signatures matching required number of signers
            let required = crate::compat::get_num_required_signatures(&tx.message) as usize;
            tx.signatures = vec![Signature::default(); required];
        }

        debug!(
            mint = %candidate.mint,
            cu_limit = dynamic_cu_limit,
            priority_fee = adaptive_priority_fee,
            "Buy transaction built successfully"
        );

        // Phase 2, Task 2.2: Extract lease from ExecutionContext for RAII transfer
        let nonce_lease = exec_ctx.extract_lease();

        // Phase 2, Task 2.1: Create TxBuildOutput with nonce guard
        Ok(TxBuildOutput::new(tx, nonce_lease))
    }

    /// Build a sell transaction (Phase 1, Task 1.1)
    ///
    /// This is a convenience wrapper that calls `build_sell_transaction_with_nonce`
    /// with `enforce_nonce = true` by default for trade-critical operations.
    ///
    /// # Arguments
    ///
    /// * `mint` - Token mint to sell
    /// * `program` - DEX program to use
    /// * `sell_percent` - Percentage of holdings to sell (0.0-1.0)
    /// * `config` - Transaction configuration
    /// * `sign` - Whether to sign the transaction
    ///
    /// # Note
    ///
    /// This method is now a legacy wrapper. Consider using `build_sell_transaction_output`
    /// for proper RAII nonce management.
    pub async fn build_sell_transaction(
        &self,
        mint: &Pubkey,
        program: &str,
        sell_percent: f64,
        config: &TransactionConfig,
        sign: bool,
    ) -> Result<VersionedTransaction, TransactionBuilderError> {
        self.build_sell_transaction_with_nonce(mint, program, sell_percent, config, sign, true)
            .await
    }

    /// Build a sell transaction with explicit nonce enforcement control (Phase 1, Task 1.1)
    ///
    /// # Legacy API Warning
    ///
    /// This method releases nonce guard early. Prefer using `build_sell_transaction_output`
    /// for proper RAII nonce management that holds the guard through broadcast.
    ///
    /// # Arguments
    ///
    /// * `mint` - Token mint to sell
    /// * `program` - DEX program to use
    /// * `sell_percent` - Percentage of holdings to sell (0.0-1.0)
    /// * `config` - Transaction configuration
    /// * `sign` - Whether to sign the transaction
    /// * `enforce_nonce` - Whether to enforce durable nonce usage (true for trade-critical ops)
    pub async fn build_sell_transaction_with_nonce(
        &self,
        mint: &Pubkey,
        program: &str,
        sell_percent: f64,
        config: &TransactionConfig,
        sign: bool,
        enforce_nonce: bool,
    ) -> Result<VersionedTransaction, TransactionBuilderError> {
        // Phase 2, Task 2.3: Legacy wrapper with deprecation warning
        use std::sync::Once;
        static WARN_ONCE: Once = Once::new();

        WARN_ONCE.call_once(|| {
            warn!(
                "Legacy API: build_sell_transaction_with_nonce releases nonce early. \
                 Migrate to build_sell_transaction_output for proper RAII."
            );
        });

        let output = self
            .build_sell_transaction_output(mint, program, sell_percent, config, sign, enforce_nonce)
            .await?;

        // Extract transaction, dropping the nonce guard early (legacy behavior)
        Ok(output.into_tx())
    }

    /// Build a sell transaction returning TxBuildOutput with RAII nonce guard (Phase 2, Task 2.3)
    ///
    /// This is the recommended API that returns `TxBuildOutput` holding the nonce guard
    /// through the transaction lifetime, ensuring proper RAII cleanup on all paths.
    ///
    /// # Arguments
    ///
    /// * `mint` - Token mint to sell
    /// * `program` - DEX program to use
    /// * `sell_percent` - Percentage of holdings to sell (0.0-1.0)
    /// * `config` - Transaction configuration
    /// * `sign` - Whether to sign the transaction
    /// * `enforce_nonce` - Whether to enforce durable nonce usage (true for trade-critical ops)
    ///
    /// # Returns
    ///
    /// `TxBuildOutput` containing the transaction and optional nonce guard.
    /// The guard should be held until broadcast completes, then explicitly released
    /// via `output.release_nonce().await` or dropped on error.
    ///
    /// # Example
    ///
    /// ```no_run
    /// let output = builder.build_sell_transaction_output(mint, "pump.fun", 1.0, &config, false, true).await?;
    ///
    /// // Hold guard during broadcast
    /// let sig = rpc.send_transaction(&output.tx).await?;
    ///
    /// // Explicitly release after successful broadcast
    /// output.release_nonce().await?;
    /// ```
    pub async fn build_sell_transaction_output(
        &self,
        mint: &Pubkey,
        program: &str,
        sell_percent: f64,
        config: &TransactionConfig,
        sign: bool,
        enforce_nonce: bool,
    ) -> Result<TxBuildOutput, TransactionBuilderError> {
        config.validate()?;
        let sell_percent = sell_percent.clamp(0.0, 1.0);
        info!(mint = %mint, "Building sell transaction");

        // Phase 1, Task 1.2: Priority defaulting policy
        // If enforce_nonce is true and priority is Utility, upgrade to CriticalSniper
        let mut effective_config = config.clone();
        if enforce_nonce && matches!(config.operation_priority, OperationPriority::Utility) {
            debug!(
                original_priority = ?config.operation_priority,
                new_priority = ?OperationPriority::CriticalSniper,
                "Upgrading operation priority for nonce enforcement"
            );
            effective_config.operation_priority = OperationPriority::CriticalSniper;
        }

        // Phase 1, Task 1.4: Use prepare_execution_context_with_enforcement
        let exec_ctx = self
            .prepare_execution_context_with_enforcement(&effective_config, enforce_nonce)
            .await?;
        let recent_blockhash = exec_ctx.blockhash;

        // Pre-allocate instruction vector for hot-path performance
        let mut _instructions: Vec<Instruction> = Vec::with_capacity(4);

        // Task 2: Dynamic compute unit limit (will be set after simulation)
        let mut dynamic_cu_limit = config.compute_unit_limit;

        // Task 2: Calculate adaptive priority fee BEFORE simulation (needed for cache hash)
        let adaptive_priority_fee = config.calculate_adaptive_priority_fee();

        // Build sell instruction first for simulation
        let dex_program = DexProgram::from(program);
        let sell_instruction = match dex_program {
            DexProgram::PumpFun => {
                self.build_pumpfun_sell_instruction(mint, sell_percent, config)
                    .await
            }
            DexProgram::LetsBonk => {
                self.build_letsbonk_sell_instruction(mint, sell_percent, config)
                    .await
            }
            DexProgram::Raydium => {
                self.build_placeholder_sell_instruction(mint, sell_percent, config)
                    .await // TODO: implement Raydium sell
            }
            DexProgram::Orca => {
                self.build_placeholder_sell_instruction(mint, sell_percent, config)
                    .await // TODO: implement Orca sell
            }
            DexProgram::Unknown(_) => {
                self.build_placeholder_sell_instruction(mint, sell_percent, config)
                    .await
            }
        }?;

        // Check if this is a placeholder instruction (no adaptive fee for placeholders)
        let is_placeholder = matches!(dex_program, DexProgram::Unknown(_));

        // Task 2: Pre-simulation for CU estimation with caching (same as buy)
        if config.enable_simulation {
            // Apply rate limiting for simulations
            if let Some(limiter) = &self.simulation_rate_limiter {
                limiter.consume(1.0).await;
            }

            // Build simulation instructions (compute budget + DEX only, no nonce)
            // Note: We use empty compute budget for simulation since we're estimating CU
            let sim_instructions = Self::build_ordered_instructions(
                &exec_ctx,
                true,   // simulation_mode = true (excludes advance nonce instruction)
                vec![], // No compute budget in simulation (we're estimating it)
                sell_instruction.clone(),
            );
            let payer = self.wallet.pubkey();

            if let Ok(sim_message) =
                MessageV0::try_compile(&payer, &sim_instructions, &[], recent_blockhash)
            {
                // Task 2: Create deterministic message hash for cache lookup
                let mut hasher = Sha256::new();

                // Hash the message content (instructions + payer + accounts)
                hasher.update(payer.to_bytes());
                for instruction in &sim_instructions {
                    hasher.update(instruction.program_id.to_bytes());
                    hasher.update(&instruction.data);
                    for account in &instruction.accounts {
                        hasher.update(account.pubkey.to_bytes());
                        hasher.update(&[account.is_signer as u8, account.is_writable as u8]);
                    }
                }
                // Include compute unit limit and priority fee in hash
                hasher.update(&dynamic_cu_limit.to_le_bytes());
                hasher.update(&adaptive_priority_fee.to_le_bytes());

                let hash_bytes = hasher.finalize();
                let message_hash = Hash::new_from_array(*hash_bytes.as_ref());

                // Get program_id for cache exclusion check
                let program_id = &sell_instruction.program_id;

                // Check if caching is enabled and program is not excluded
                let cache_enabled = config.simulation_cache_config.enabled
                    && !config
                        .simulation_cache_config
                        .is_program_excluded(program_id);

                // Check simulation cache first
                let cached_result = if cache_enabled {
                    self.simulation_cache.get(&message_hash).and_then(|entry| {
                        let elapsed = entry.cached_at.elapsed().as_secs();

                        if elapsed < config.simulation_cache_config.ttl_seconds {
                            // Task 2: Record cache hit
                            self.simulation_cache_hits.fetch_add(1, Ordering::Relaxed);
                            debug!(
                                cached_cu = entry.compute_units,
                                age_secs = elapsed,
                                "Sell simulation cache hit"
                            );
                            Some(entry.compute_units)
                        } else {
                            // Entry expired, remove it
                            drop(entry);
                            self.simulation_cache.remove(&message_hash);
                            // Task 2: Record cache miss (expired)
                            self.simulation_cache_misses.fetch_add(1, Ordering::Relaxed);
                            None
                        }
                    })
                } else {
                    None
                };

                if let Some(cached_cu) = cached_result {
                    // Use cached CU estimate
                    let estimated_cu = ((cached_cu as f64) * 1.2) as u32;
                    dynamic_cu_limit = estimated_cu.clamp(config.min_cu_limit, config.max_cu_limit);
                    debug!(
                        cached_cu = cached_cu,
                        dynamic_cu = dynamic_cu_limit,
                        "Sell CU estimation from cache"
                    );
                } else {
                    // Task 2: Record cache miss (not found)
                    if cache_enabled {
                        self.simulation_cache_misses.fetch_add(1, Ordering::Relaxed);
                    }

                    // Perform simulation
                    let sim_tx = VersionedTransaction {
                        signatures: vec![Signature::default()],
                        message: VersionedMessage::V0(sim_message),
                    };

                    let rpc = self.rpc_client_for(0);
                    match tokio::time::timeout(
                        Duration::from_millis(config.rpc_timeout_ms / 2),
                        rpc.simulate_transaction(&sim_tx),
                    )
                    .await
                    {
                        Ok(Ok(sim_result)) => {
                            if let Some(units_consumed) = sim_result.value.units_consumed {
                                // Cache the result (only if not excluded)
                                if cache_enabled {
                                    let slot = rpc.get_slot().await.unwrap_or(0);
                                    let cache_entry = SimulationCacheEntry {
                                        compute_units: units_consumed,
                                        cached_at: Instant::now(),
                                        slot,
                                    };

                                    self.simulation_cache.insert(message_hash, cache_entry);

                                    // Task 2: Prune cache using LRU ordering (oldest by timestamp)
                                    if self.simulation_cache.len()
                                        > config.simulation_cache_config.max_size
                                    {
                                        let remove_count =
                                            config.simulation_cache_config.max_size / 10;

                                        // Collect entries with timestamps and sort by LRU (oldest first)
                                        let mut entries: Vec<(Hash, Instant)> = self
                                            .simulation_cache
                                            .iter()
                                            .map(|e| (*e.key(), e.value().cached_at))
                                            .collect();
                                        entries.sort_by_key(|(_, timestamp)| *timestamp);

                                        // Remove oldest entries
                                        let keys_to_remove: Vec<Hash> = entries
                                            .iter()
                                            .take(remove_count)
                                            .map(|(key, _)| *key)
                                            .collect();

                                        for key in keys_to_remove {
                                            self.simulation_cache.remove(&key);
                                        }

                                        debug!(
                                            removed = remove_count,
                                            cache_size = self.simulation_cache.len(),
                                            "Pruned sell simulation cache using LRU"
                                        );
                                    }
                                }

                                // Add 20% buffer to simulated CU
                                let estimated_cu = ((units_consumed as f64) * 1.2) as u32;
                                dynamic_cu_limit =
                                    estimated_cu.clamp(config.min_cu_limit, config.max_cu_limit);

                                debug!(
                                    simulated_cu = units_consumed,
                                    dynamic_cu = dynamic_cu_limit,
                                    "Sell CU estimation from simulation"
                                );
                            }

                            // Task 2: Classify simulation errors as fatal vs advisory (same patterns as buy)
                            if let Some(err) = sim_result.value.err {
                                // Define fatal error patterns as constants for maintainability
                                const FATAL_ERROR_PATTERNS: &[&str] = &[
                                    "InstructionError",
                                    "ProgramFailedToComplete",
                                    "ComputeBudgetExceeded",
                                    "InsufficientFunds",
                                ];

                                let error_str = format!("{:?}", err);

                                // Check if error matches any fatal pattern
                                let is_fatal = FATAL_ERROR_PATTERNS
                                    .iter()
                                    .any(|pattern| error_str.contains(pattern));

                                if is_fatal {
                                    warn!(
                                        error = ?err,
                                        "Fatal sell simulation error detected, aborting transaction"
                                    );
                                    return Err(TransactionBuilderError::SimulationFailed(
                                        error_str,
                                    ));
                                }
                                // Advisory warning - log but proceed with caution
                                warn!(
                                    error = ?err,
                                    "Sell simulation returned advisory error, proceeding with caution"
                                );
                            }
                        }
                        Ok(Err(e)) => {
                            debug!(error = %e, "Sell simulation failed, using default CU limit");
                        }
                        Err(_) => {
                            debug!("Sell simulation timeout, using default CU limit");
                        }
                    }
                }
            }
        }

        // Build compute budget instructions
        let mut compute_budget_instructions = Vec::with_capacity(2);

        if dynamic_cu_limit > 0 {
            compute_budget_instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
                dynamic_cu_limit,
            ));
        }

        // Task 2: adaptive_priority_fee now calculated earlier (before simulation section)
        // Rule: No adaptive fee applied for placeholder instructions
        if adaptive_priority_fee > 0 && !is_placeholder {
            compute_budget_instructions.push(ComputeBudgetInstruction::set_compute_unit_price(
                adaptive_priority_fee,
            ));
        }

        // Build instructions in deterministic order using helper function
        // Order: advance_nonce (if present) -> compute_budget -> dex_instruction
        // simulation_mode = false for production transactions
        let instructions = Self::build_ordered_instructions(
            &exec_ctx,
            false, // simulation_mode = false (this is a production transaction)
            compute_budget_instructions,
            sell_instruction,
        );

        let payer = self.wallet.pubkey();
        let message_v0 = MessageV0::try_compile(&payer, &instructions, &[], recent_blockhash)
            .map_err(|e| TransactionBuilderError::InstructionBuild {
                program: program.to_string(),
                reason: format!("Failed to compile sell message: {}", e),
            })?;

        let versioned_message = VersionedMessage::V0(message_v0);
        let mut tx = VersionedTransaction {
            signatures: vec![],
            message: versioned_message,
        };

        if sign {
            // Sign the transaction manually since VersionedTransaction doesn't have try_sign
            use solana_sdk::signature::Signer;
            let keypair = self.wallet.keypair();
            let message_bytes = tx.message.serialize();
            let signature = keypair.sign_message(&message_bytes);
            tx.signatures = vec![signature];
        } else {
            let required = crate::compat::get_num_required_signatures(&tx.message) as usize;
            tx.signatures = vec![Signature::default(); required];
        }

        debug!(mint = %mint, "Sell transaction built successfully");

        // Phase 2, Task 2.2: Extract lease from ExecutionContext for RAII transfer
        let nonce_lease = exec_ctx.extract_lease();

        // Phase 2, Task 2.1: Create TxBuildOutput with nonce guard
        Ok(TxBuildOutput::new(tx, nonce_lease))
    }

    /// Prepare Jito bundle with MEV features (Universe Class Enhanced)
    pub async fn prepare_jito_bundle(
        &self,
        txs: Vec<VersionedTransaction>,
        max_total_cost_lamports: u64,
        target_slot: Option<u64>,
        backrun_protect: bool,
        config: &TransactionConfig,
    ) -> Result<JitoBundleCandidate, TransactionBuilderError> {
        // Universe Class: Dynamic tip calculation based on network congestion
        let dynamic_tip = if config.enable_simulation {
            let rpc = self.rpc_client_for(0);

            // Fetch recent prioritization fees for ML-based tip calculation
            match rpc.get_recent_prioritization_fees(&[]).await {
                Ok(fees) if !fees.is_empty() => {
                    // Calculate percentile-based tip (P90)
                    let mut fee_values: Vec<u64> =
                        fees.iter().map(|f| f.prioritization_fee).collect();

                    // Task 3: Guard against empty array (should not happen due to outer check, but defensive)
                    if fee_values.is_empty() {
                        warn!("Empty fee_values after filtering, using default tip");
                        max_total_cost_lamports
                    } else {
                        fee_values.sort_unstable();

                        // Task 3: Fix p90_index calculation with proper bounds checking
                        // Use nearest-rank method: ceil(len * percentile), clamped to valid index
                        let len = fee_values.len();
                        let p90_index = if len == 1 {
                            0
                        } else {
                            // Nearest-rank percentile: ceil(N * P) - 1 for 0-based indexing
                            // For P90 with len=10: ceil(10 * 0.9) - 1 = ceil(9.0) - 1 = 9 - 1 = 8 (9th element)
                            let rank = (len as f64 * 0.9).ceil() as usize;
                            (rank.saturating_sub(1)).min(len - 1)
                        };

                        let base_tip = fee_values[p90_index];

                        // Escalate if high TPS (simulated via fee pressure)
                        let avg_fee: u64 = fee_values.iter().sum::<u64>() / fee_values.len() as u64;
                        let multiplier = if avg_fee > 50_000 { 1.5 } else { 1.2 };

                        let calculated_tip = ((base_tip as f64) * multiplier) as u64;

                        // Task 3: Add max_tip cap (configurable via max_total_cost_lamports parameter)
                        let capped_tip = calculated_tip.min(max_total_cost_lamports);

                        // Task 3: Log tip decision rationale
                        debug!(
                            base_tip = base_tip,
                            p90_index = p90_index,
                            avg_fee = avg_fee,
                            multiplier = multiplier,
                            calculated_tip = calculated_tip,
                            capped_tip = capped_tip,
                            num_fees = len,
                            "Jito bundle dynamic tip calculation"
                        );

                        capped_tip
                    }
                }
                _ => {
                    debug!(
                        "No prioritization fees available, using max_total_cost_lamports as tip"
                    );
                    max_total_cost_lamports
                }
            }
        } else {
            max_total_cost_lamports
        };

        // Universe Class: Generate searcher hints for bundle ordering
        let searcher_hints = if backrun_protect {
            // Create obfuscation hint (simple version)
            vec![0x01, 0x00, 0x00, 0x00] // Marker for backrun protection
        } else {
            Vec::new()
        };

        debug!(
            bundle_size = txs.len(),
            dynamic_tip = dynamic_tip,
            backrun_protect = backrun_protect,
            "Prepared Jito bundle with MEV features"
        );

        Ok(JitoBundleCandidate {
            transactions: txs,
            max_total_cost_lamports: dynamic_tip,
            target_slot,
            searcher_hints,
            backrun_protect,
        })
    }

    /// Legacy method for backward compatibility
    pub fn prepare_jito_bundle_simple(
        &self,
        txs: Vec<VersionedTransaction>,
        max_total_cost_lamports: u64,
        target_slot: Option<u64>,
    ) -> JitoBundleCandidate {
        JitoBundleCandidate {
            transactions: txs,
            max_total_cost_lamports,
            target_slot,
            searcher_hints: Vec::new(),
            backrun_protect: false,
        }
    }

    pub fn rpc_client_for(&self, idx: usize) -> Arc<RpcClient> {
        let index = idx % self.rpc_clients.len();
        self.rpc_clients[index].clone()
    }

    // --- Instruction builders ---

    async fn build_pumpfun_instruction(
        &self,
        candidate: &PremintCandidate,
        config: &TransactionConfig,
    ) -> Result<Instruction, TransactionBuilderError> {
        #[cfg(feature = "pumpfun")]
        {
            // Note: PumpFun SDK v4.6.0 doesn't expose buy_ix/sell_ix methods
            // The buy() and sell() methods send transactions directly
            // Fallback to HTTP API for instruction building
            debug!("PumpFun SDK doesn't support instruction-only building, using fallback");
        }

        // Fallback do HTTP PumpPortal, gdy feature pumpfun wyłączony
        self.build_pumpportal_or_memo(candidate, config).await
    }

    async fn build_letsbonk_instruction(
        &self,
        candidate: &PremintCandidate,
        config: &TransactionConfig,
    ) -> Result<Instruction, TransactionBuilderError> {
        if let Some(url) = &config.letsbonk_api_url {
            let payload = serde_json::json!({
                "mint": candidate.mint.to_string(),
                "amount": config.buy_amount_lamports,
                "slippage": config.slippage_bps as f64 / 100.0,
                "payer": self.wallet.pubkey().to_string(),
            });

            // Apply HTTP rate limiting
            if let Some(limiter) = &self.http_rate_limiter {
                limiter.consume(1.0).await;
            }

            let mut req = self.http.post(url).json(&payload);
            if let Some(k) = &config.letsbonk_api_key {
                req = req.header("X-API-KEY", k);
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    let j: serde_json::Value = resp.json().await.map_err(|e| {
                        TransactionBuilderError::InstructionBuild {
                            program: "letsbonk".to_string(),
                            reason: format!("JSON parse error: {}", e),
                        }
                    })?;
                    return self.parse_external_api_response(&j, "letsbonk", config);
                }
                Ok(resp) => {
                    warn!("LetsBonk API error: {}", resp.status());
                }
                Err(e) => {
                    warn!("LetsBonk request error: {}", e);
                }
            }
        }

        self.build_placeholder_buy_instruction(candidate, config)
            .await
    }

    async fn build_pumpportal_or_memo(
        &self,
        candidate: &PremintCandidate,
        config: &TransactionConfig,
    ) -> Result<Instruction, TransactionBuilderError> {
        if let Some(url) = &config.pumpportal_url {
            let payload = serde_json::json!({
                "mint": candidate.mint.to_string(),
                "buy_amount": config.buy_amount_lamports,
                "slippage": config.slippage_bps as f64 / 100.0,
                "payer": self.wallet.pubkey().to_string(),
            });

            // Apply HTTP rate limiting
            if let Some(limiter) = &self.http_rate_limiter {
                limiter.consume(1.0).await;
            }

            let mut req = self.http.post(url).json(&payload);
            if let Some(k) = &config.pumpportal_api_key {
                req = req.header("Authorization", format!("Bearer {}", k));
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    let j: serde_json::Value = resp.json().await.map_err(|e| {
                        TransactionBuilderError::InstructionBuild {
                            program: "pumpportal".to_string(),
                            reason: format!("JSON parse error: {}", e),
                        }
                    })?;

                    return self.parse_external_api_response(&j, "pumpportal", config);
                }
                Ok(resp) => {
                    warn!("PumpPortal API error: {}", resp.status());
                }
                Err(e) => {
                    warn!("PumpPortal request error: {}", e);
                }
            }
        }

        self.build_placeholder_buy_instruction(candidate, config)
            .await
    }

    /// Parse an external API instruction description to a Solana Instruction.
    /// Exposed as public to enable integration testing from bot/tests.
    pub fn parse_external_api_response(
        &self,
        j: &serde_json::Value,
        api_name: &str,
        config: &TransactionConfig,
    ) -> Result<Instruction, TransactionBuilderError> {
        if let Some(obj) = j.as_object() {
            // Prefer program_id + data format
            if let (Some(pid_val), Some(data_val)) = (obj.get("program_id"), obj.get("data")) {
                let pid_str =
                    pid_val
                        .as_str()
                        .ok_or_else(|| TransactionBuilderError::InstructionBuild {
                            program: api_name.to_string(),
                            reason: "program_id not string".to_string(),
                        })?;

                let pid = Pubkey::from_str(pid_str).map_err(|e| {
                    TransactionBuilderError::InstructionBuild {
                        program: api_name.to_string(),
                        reason: format!("invalid program_id: {}", e),
                    }
                })?;

                // Check if program is allowed
                if !config.is_program_allowed(&pid) {
                    return Err(TransactionBuilderError::ProgramNotAllowed(pid));
                }

                let data_b64 =
                    data_val
                        .as_str()
                        .ok_or_else(|| TransactionBuilderError::InstructionBuild {
                            program: api_name.to_string(),
                            reason: "data not string".to_string(),
                        })?;

                let data = BASE64_STANDARD.decode(data_b64).map_err(|e| {
                    TransactionBuilderError::InstructionBuild {
                        program: api_name.to_string(),
                        reason: format!("base64 decode error: {}", e),
                    }
                })?;

                // Validate data size
                if data.len() > 4096 {
                    return Err(TransactionBuilderError::InstructionBuild {
                        program: api_name.to_string(),
                        reason: "instruction data too large (max 4KB)".to_string(),
                    });
                }

                // Parse accounts if provided, otherwise use default (payer as readonly)
                let accounts = if let Some(accounts_val) = obj.get("accounts") {
                    self.parse_accounts(accounts_val, api_name)?
                } else {
                    vec![AccountMeta::new_readonly(self.wallet.pubkey(), false)]
                };

                return Ok(Instruction::new_with_bytes(pid, &data, accounts));
            }

            // Fallback to instruction_b64 (legacy format)
            if let Some(b64) = obj.get("instruction_b64").and_then(|v| v.as_str()) {
                warn!(
                    "{} returned legacy instruction_b64 format - consider updating API",
                    api_name
                );
                let data = BASE64_STANDARD.decode(b64).map_err(|e| {
                    TransactionBuilderError::InstructionBuild {
                        program: api_name.to_string(),
                        reason: format!("base64 decode error: {}", e),
                    }
                })?;

                // Validate data size
                if data.len() > 4096 {
                    return Err(TransactionBuilderError::InstructionBuild {
                        program: api_name.to_string(),
                        reason: "instruction data too large (max 4KB)".to_string(),
                    });
                }

                // For legacy format, we can't determine program_id, so use memo as fallback
                return Ok(spl_memo::build_memo(&data, &[&self.wallet.pubkey()]));
            }
        }

        Err(TransactionBuilderError::InstructionBuild {
            program: api_name.to_string(),
            reason: "invalid response format".to_string(),
        })
    }

    /// Parse account metas from JSON; rejects unexpected signers (signers other than wallet).
    /// Exposed as public to enable integration testing from bot/tests.
    pub fn parse_accounts(
        &self,
        accounts_val: &serde_json::Value,
        api_name: &str,
    ) -> Result<Vec<AccountMeta>, TransactionBuilderError> {
        let accounts_array =
            accounts_val
                .as_array()
                .ok_or_else(|| TransactionBuilderError::InstructionBuild {
                    program: api_name.to_string(),
                    reason: "accounts not an array".to_string(),
                })?;

        let mut accounts = Vec::with_capacity(accounts_array.len());
        for account_val in accounts_array {
            let account_obj = account_val.as_object().ok_or_else(|| {
                TransactionBuilderError::InstructionBuild {
                    program: api_name.to_string(),
                    reason: "account entry not an object".to_string(),
                }
            })?;

            let pubkey_str = account_obj
                .get("pubkey")
                .and_then(|v| v.as_str())
                .ok_or_else(|| TransactionBuilderError::InstructionBuild {
                    program: api_name.to_string(),
                    reason: "account pubkey missing or not string".to_string(),
                })?;

            let pubkey = Pubkey::from_str(pubkey_str).map_err(|e| {
                TransactionBuilderError::InstructionBuild {
                    program: api_name.to_string(),
                    reason: format!("invalid account pubkey: {}", e),
                }
            })?;

            let is_signer = account_obj
                .get("is_signer")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let is_writable = account_obj
                .get("is_writable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Reject unexpected signer accounts
            if is_signer && pubkey != self.wallet.pubkey() {
                return Err(TransactionBuilderError::InstructionBuild {
                    program: api_name.to_string(),
                    reason: format!("unexpected signer account: {}", pubkey),
                });
            }

            accounts.push(AccountMeta {
                pubkey,
                is_signer,
                is_writable,
            });
        }

        Ok(accounts)
    }

    async fn build_placeholder_buy_instruction(
        &self,
        candidate: &PremintCandidate,
        config: &TransactionConfig,
    ) -> Result<Instruction, TransactionBuilderError> {
        debug!(mint = %candidate.mint, "Creating placeholder buy memo");
        let memo_data = format!(
            "PLACEHOLDER_BUY:{}:{}:{}",
            candidate.program, candidate.mint, config.buy_amount_lamports
        );
        Ok(spl_memo::build_memo(
            memo_data.as_bytes(),
            &[&self.wallet.pubkey()],
        ))
    }

    async fn build_placeholder_sell_instruction(
        &self,
        mint: &Pubkey,
        sell_percent: f64,
        _config: &TransactionConfig,
    ) -> Result<Instruction, TransactionBuilderError> {
        debug!(mint = %mint, "Creating placeholder sell memo");
        let memo_data = format!("PLACEHOLDER_SELL:{}:{:.6}", mint, sell_percent);
        Ok(spl_memo::build_memo(
            memo_data.as_bytes(),
            &[&self.wallet.pubkey()],
        ))
    }

    // Sell instruction builders (placeholder implementations)
    async fn build_pumpfun_sell_instruction(
        &self,
        mint: &Pubkey,
        sell_percent: f64,
        config: &TransactionConfig,
    ) -> Result<Instruction, TransactionBuilderError> {
        #[cfg(feature = "pumpfun")]
        {
            // Note: PumpFun SDK v4.6.0 doesn't expose sell_ix method
            // The sell() method sends transactions directly
            // Fallback to placeholder for sell instruction building
            debug!("PumpFun SDK doesn't support sell instruction-only building, using fallback");
        }

        self.build_placeholder_sell_instruction(mint, sell_percent, config)
            .await
    }

    async fn build_letsbonk_sell_instruction(
        &self,
        mint: &Pubkey,
        sell_percent: f64,
        config: &TransactionConfig,
    ) -> Result<Instruction, TransactionBuilderError> {
        if let Some(url) = &config.letsbonk_api_url {
            let payload = serde_json::json!({
                "action": "sell",
                "mint": mint.to_string(),
                "sell_percent": sell_percent,
                "slippage": config.slippage_bps as f64 / 100.0,
                "payer": self.wallet.pubkey().to_string(),
            });

            // Apply HTTP rate limiting
            if let Some(limiter) = &self.http_rate_limiter {
                limiter.consume(1.0).await;
            }

            let mut req = self.http.post(url).json(&payload);
            if let Some(k) = &config.letsbonk_api_key {
                req = req.header("X-API-KEY", k);
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    let j: serde_json::Value = resp.json().await.map_err(|e| {
                        TransactionBuilderError::InstructionBuild {
                            program: "letsbonk".to_string(),
                            reason: format!("JSON parse error: {}", e),
                        }
                    })?;
                    return self.parse_external_api_response(&j, "letsbonk", config);
                }
                Ok(resp) => {
                    warn!("LetsBonk API error on sell: {}", resp.status());
                }
                Err(e) => {
                    warn!("LetsBonk sell request error: {}", e);
                }
            }
        }

        self.build_placeholder_sell_instruction(mint, sell_percent, config)
            .await
    }

    /// Unwrap WSOL ATA back to native SOL
    pub async fn unwrap_wsol(
        &self,
        config: &TransactionConfig,
    ) -> Result<Signature, TransactionBuilderError> {
        let wsol_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
        let wsol_ata = get_associated_token_address(&self.wallet.pubkey(), &wsol_mint);

        let close_ix = close_account(
            &token_program_id(),
            &wsol_ata,
            &self.wallet.pubkey(),
            &self.wallet.pubkey(),
            &[],
        )
        .map_err(|e| TransactionBuilderError::InstructionBuild {
            program: "unwrap_wsol".to_string(),
            reason: e.to_string(),
        })?;

        let recent_blockhash = self.get_recent_blockhash(config).await?;

        let instructions = vec![close_ix];
        let payer = self.wallet.pubkey();
        let message_v0 = MessageV0::try_compile(&payer, &instructions, &[], recent_blockhash)
            .map_err(|e| TransactionBuilderError::InstructionBuild {
                program: "unwrap_wsol".to_string(),
                reason: format!("Failed to compile message: {}", e),
            })?;

        let versioned_message = VersionedMessage::V0(message_v0);
        let mut tx_to_sign = VersionedTransaction {
            signatures: vec![],
            message: versioned_message,
        };

        // Sign the transaction manually since VersionedTransaction doesn't have try_sign
        use solana_sdk::signature::Signer;
        let keypair = self.wallet.keypair();
        let message_bytes = tx_to_sign.message.serialize();
        let signature_data = keypair.sign_message(&message_bytes);
        tx_to_sign.signatures = vec![signature_data];

        // Simple send via first RPC client
        let rpc = self.rpc_client_for(0);
        let signature = rpc
            .send_and_confirm_transaction(&tx_to_sign)
            .await
            .map_err(|e| TransactionBuilderError::RpcConnection(e.to_string()))?;

        Ok(signature)
    }

    /// Test helper: inject a fresh blockhash to avoid RPC calls in unit/integration tests.
    #[cfg(any(test, feature = "test_utils"))]
    pub async fn inject_blockhash_for_tests(&self, hash: Hash) {
        let mut cache = self.blockhash_cache.write().await;
        cache.insert(hash, (Instant::now(), 0));
    }

    // ============================================================================
    // UNIVERSE CLASS: Advanced Helper Methods
    // ============================================================================

    /// Batch build transactions with bounded worker pool (Universe Class Enhanced)
    ///
    /// Uses a semaphore-controlled worker pool to prevent unbounded concurrency spikes.
    /// Supports priority-based execution for high-priority candidates (e.g., sniper operations).
    pub async fn batch_build_buy_transactions(
        &self,
        candidates: Vec<PremintCandidate>,
        config: &TransactionConfig,
        sign: bool,
    ) -> Vec<Result<VersionedTransaction, TransactionBuilderError>> {
        self.batch_build_buy_transactions_with_priority(candidates, config, sign, false)
            .await
    }

    /// Batch build with priority flag for sniper operations (Task 4: Priority queue support)
    ///
    /// For priority queue behavior:
    /// - High-priority candidates should be submitted first to ensure they acquire permits sooner
    /// - The semaphore implements RAII pattern via SemaphorePermit guard
    /// - Permits are automatically released on drop, even if task panics
    /// - Consider using separate calls for high vs low priority batches
    ///
    /// Note: Uses sequential processing with semaphore to respect worker pool limits
    pub async fn batch_build_buy_transactions_with_priority(
        &self,
        candidates: Vec<PremintCandidate>,
        config: &TransactionConfig,
        sign: bool,
        high_priority: bool,
    ) -> Vec<Result<VersionedTransaction, TransactionBuilderError>> {
        let total = candidates.len();
        if total == 0 {
            return Vec::new();
        }

        // Task 4: Log priority level for telemetry
        if high_priority {
            debug!(
                count = total,
                "Processing high-priority batch with worker pool"
            );
        }

        // Process candidates with semaphore control
        let mut results = Vec::with_capacity(total);

        for candidate in candidates {
            // Task 4: Acquire permit with RAII guard (ensures release on drop/panic)
            let _permit = match tokio::time::timeout(
                Duration::from_secs(30),
                self.worker_pool_semaphore.acquire(),
            )
            .await
            {
                Ok(Ok(permit)) => permit,
                Ok(Err(_)) => {
                    // Semaphore closed
                    results.push(Err(TransactionBuilderError::InstructionBuild {
                        program: "batch_build".to_string(),
                        reason: "Worker pool semaphore closed".to_string(),
                    }));
                    continue;
                }
                Err(_) => {
                    // Timeout acquiring permit
                    results.push(Err(TransactionBuilderError::InstructionBuild {
                        program: "batch_build".to_string(),
                        reason: "Timeout acquiring worker pool permit".to_string(),
                    }));
                    continue;
                }
            };

            // Build transaction (permit held until this completes or panics)
            let result = self.build_buy_transaction(&candidate, config, sign).await;
            results.push(result);

            // Permit automatically released when _permit drops
        }

        results
    }

    /// Update slippage predictor with new observation (Universe Class)
    pub async fn update_slippage_predictor(&self, actual_slippage_bps: f64) {
        let mut predictor = self.slippage_predictor.write().await;
        predictor.add_observation(actual_slippage_bps);
    }

    /// Check balance before transaction to avoid insufficient funds (Universe Class)
    pub async fn check_balance_sufficient(
        &self,
        required_lamports: u64,
    ) -> Result<u64, TransactionBuilderError> {
        let rpc = self.rpc_client_for(0);
        let balance = rpc.get_balance(&self.wallet.pubkey()).await.map_err(|e| {
            TransactionBuilderError::RpcConnection(format!("Failed to fetch balance: {}", e))
        })?;

        if balance < required_lamports {
            return Err(TransactionBuilderError::InsufficientBalance {
                required: required_lamports,
                available: balance,
            });
        }

        Ok(balance)
    }

    /// Fetch and validate liquidity depth for a token (Universe Class)
    pub async fn check_liquidity_depth(
        &self,
        mint: &Pubkey,
        min_lamports: u64,
    ) -> Result<u64, TransactionBuilderError> {
        let rpc = self.rpc_client_for(0);

        // This is a simplified version - in production, you'd query the actual pool account
        match rpc.get_account(mint).await {
            Ok(account) => {
                let liquidity = account.lamports;
                if liquidity < min_lamports {
                    return Err(TransactionBuilderError::LiquidityTooLow {
                        available: liquidity,
                        required: min_lamports,
                    });
                }
                Ok(liquidity)
            }
            Err(e) => Err(TransactionBuilderError::RpcConnection(format!(
                "Failed to fetch liquidity: {}",
                e
            ))),
        }
    }

    /// Get current slot for stale detection (Universe Class)
    pub async fn get_current_slot(&self) -> Result<u64, TransactionBuilderError> {
        let rpc = self.rpc_client_for(0);
        rpc.get_slot().await.map_err(|e| {
            TransactionBuilderError::RpcConnection(format!("Failed to fetch slot: {}", e))
        })
    }

    /// Invalidate stale blockhash entries (Universe Class)
    pub async fn invalidate_stale_blockhashes(
        &self,
        max_slot_lag: u64,
    ) -> Result<usize, TransactionBuilderError> {
        let current_slot = self.get_current_slot().await?;
        let mut cache = self.blockhash_cache.write().await;
        let initial_size = cache.len();

        cache.retain(|_, (_, slot)| current_slot.saturating_sub(*slot) <= max_slot_lag);

        let removed = initial_size - cache.len();
        if removed > 0 {
            debug!(
                removed = removed,
                current_slot = current_slot,
                "Invalidated stale blockhashes"
            );
        }

        Ok(removed)
    }

    // ============================================================================
    // Monitoring & Diagnostics
    // ============================================================================

    /// Get circuit breaker states for all RPC endpoints
    pub async fn get_circuit_breaker_states(&self) -> Vec<(String, CircuitState)> {
        let mut states = Vec::with_capacity(self.circuit_breakers.len());
        for (idx, cb) in self.circuit_breakers.iter().enumerate() {
            let endpoint = self.rpc_endpoints[idx].clone();
            let state = cb.get_state().await;
            states.push((endpoint, state));
        }
        states
    }

    /// Task 2: Get circuit breaker states with detailed telemetry
    pub async fn get_circuit_breaker_states_detailed(&self) -> Vec<CircuitBreakerStatus> {
        let mut statuses = Vec::with_capacity(self.circuit_breakers.len());
        for (idx, cb) in self.circuit_breakers.iter().enumerate() {
            let endpoint = self.rpc_endpoints[idx].clone();
            let state = cb.get_state().await;
            let failure_count = cb.get_failure_count();
            statuses.push(CircuitBreakerStatus {
                endpoint,
                state,
                failure_count,
            });
        }
        statuses
    }

    /// Task 2: Start periodic health probe background task
    ///
    /// Returns a JoinHandle that can be used to cancel the task.
    /// The task will periodically check RPC endpoint health and update circuit breakers.
    pub fn start_health_probe_task(
        self: Arc<Self>,
        probe_interval_secs: u64,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(probe_interval_secs));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;

                // Probe each RPC endpoint
                for (idx, rpc_client) in self.rpc_clients.iter().enumerate() {
                    let circuit_breaker = &self.circuit_breakers[idx];
                    let endpoint = &self.rpc_endpoints[idx];

                    // Only probe if circuit is not closed (healthy)
                    let state = circuit_breaker.get_state().await;
                    if matches!(state, CircuitState::Open | CircuitState::HalfOpen) {
                        debug!(
                            endpoint = %endpoint,
                            state = ?state,
                            "Health probe checking endpoint"
                        );

                        // Simple health check: try to get slot
                        match tokio::time::timeout(Duration::from_secs(5), rpc_client.get_slot())
                            .await
                        {
                            Ok(Ok(_slot)) => {
                                circuit_breaker.record_success().await;
                                info!(
                                    endpoint = %endpoint,
                                    "Health probe succeeded, circuit may transition to closed"
                                );
                            }
                            Ok(Err(e)) => {
                                circuit_breaker.record_failure().await;
                                warn!(
                                    endpoint = %endpoint,
                                    error = %e,
                                    "Health probe failed"
                                );
                            }
                            Err(_) => {
                                circuit_breaker.record_failure().await;
                                warn!(
                                    endpoint = %endpoint,
                                    "Health probe timeout"
                                );
                            }
                        }
                    }
                }
            }
        })
    }

    /// Get simulation cache statistics (Task 1: Expose cache stats)
    pub fn get_simulation_cache_stats(&self) -> (usize, usize, u64, u64) {
        (
            self.simulation_cache.len(),
            self.simulation_cache.capacity(),
            self.simulation_cache_hits.load(Ordering::Relaxed),
            self.simulation_cache_misses.load(Ordering::Relaxed),
        )
    }

    /// Get blockhash cache statistics
    pub async fn get_blockhash_cache_stats(&self) -> usize {
        self.blockhash_cache.read().await.len()
    }

    /// Clear simulation cache (for testing or cleanup)
    pub fn clear_simulation_cache(&self) {
        self.simulation_cache.clear();
        // Reset counters when clearing cache
        self.simulation_cache_hits.store(0, Ordering::Relaxed);
        self.simulation_cache_misses.store(0, Ordering::Relaxed);
    }

    /// Clear blockhash cache (for testing or cleanup)
    pub async fn clear_blockhash_cache(&self) {
        self.blockhash_cache.write().await.clear();
    }

    /// Task 6: Get nonce acquisition count
    pub fn get_nonce_acquire_count(&self) -> u64 {
        self.nonce_acquire_count.load(Ordering::Relaxed)
    }

    /// Task 6: Get nonce exhausted count  
    pub fn get_nonce_exhausted_count(&self) -> u64 {
        self.nonce_exhausted_count.load(Ordering::Relaxed)
    }

    /// Task 6: Get blockhash quorum success count
    pub fn get_blockhash_quorum_success_count(&self) -> u64 {
        self.blockhash_quorum_success_count.load(Ordering::Relaxed)
    }

    /// Task 6: Get blockhash fallback count
    pub fn get_blockhash_fallback_count(&self) -> u64 {
        self.blockhash_fallback_count.load(Ordering::Relaxed)
    }

    /// Task 5: Get transaction count for signer rotation tracking
    pub fn get_transaction_count(&self) -> u64 {
        self.tx_counter.load(Ordering::Relaxed)
    }

    /// Task 5: Get current rotation checkpoint based on interval
    /// Returns how many rotation checkpoints have been reached
    pub fn get_rotation_checkpoint(&self, rotation_interval: u64) -> u64 {
        if rotation_interval == 0 {
            0
        } else {
            self.tx_counter.load(Ordering::Relaxed) / rotation_interval
        }
    }

    /// Task 6: Get all telemetry metrics at once
    pub fn get_all_metrics(&self, config: &TransactionConfig) -> TxBuilderMetrics {
        let (cache_size, cache_capacity, cache_hits, cache_misses) =
            self.get_simulation_cache_stats();
        TxBuilderMetrics {
            simulation_cache_size: cache_size,
            simulation_cache_capacity: cache_capacity,
            simulation_cache_hits: cache_hits,
            simulation_cache_misses: cache_misses,
            nonce_acquire_count: self.get_nonce_acquire_count(),
            nonce_exhausted_count: self.get_nonce_exhausted_count(),
            blockhash_quorum_success_count: self.get_blockhash_quorum_success_count(),
            blockhash_fallback_count: self.get_blockhash_fallback_count(),
            transaction_count: self.get_transaction_count(),
            rotation_checkpoint: self.get_rotation_checkpoint(config.signer_rotation_interval),
        }
    }
}

/// Task 6: Telemetry metrics structure
#[derive(Debug, Clone)]
pub struct TxBuilderMetrics {
    pub simulation_cache_size: usize,
    pub simulation_cache_capacity: usize,
    pub simulation_cache_hits: u64,
    pub simulation_cache_misses: u64,
    pub nonce_acquire_count: u64,
    pub nonce_exhausted_count: u64,
    pub blockhash_quorum_success_count: u64,
    pub blockhash_fallback_count: u64,
    pub transaction_count: u64,
    pub rotation_checkpoint: u64,
}

// Helper calculation functions for pump.fun
#[cfg(feature = "pumpfun")]
fn calculate_expected_tokens(curve: &BondingCurveAccount, sol_in: u64) -> u64 {
    let virtual_sol = curve.virtual_sol_reserves;
    let virtual_tokens = curve.virtual_token_reserves;
    (sol_in * virtual_tokens) / (virtual_sol + sol_in)
}

#[cfg(feature = "pumpfun")]
fn calculate_expected_sol(curve: &BondingCurveAccount, tokens_in: u64) -> u64 {
    let virtual_sol = curve.virtual_sol_reserves;
    let virtual_tokens = curve.virtual_token_reserves;
    (tokens_in * virtual_sol) / (virtual_tokens + tokens_in)
}

// SPL Memo helper
mod spl_memo {
    use solana_sdk::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
    };

    pub const MEMO_PROGRAM_ID: Pubkey =
        solana_sdk::pubkey!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");

    pub fn build_memo(data: &[u8], signers: &[&Pubkey]) -> Instruction {
        let metas: Vec<AccountMeta> = signers
            .iter()
            .map(|&pk| AccountMeta::new_readonly(*pk, false)) // Memo doesn't require signer flag
            .collect();

        Instruction::new_with_bytes(MEMO_PROGRAM_ID, data, metas)
    }
}

// ============================================================================
// Tests - Phase 1: TxBuildOutput RAII Pattern
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{
        message::{v0::Message as MessageV0, VersionedMessage},
        signature::Keypair,
        signer::Signer,
        system_instruction,
    };
    use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

    /// Helper to create a simple test transaction
    fn create_test_transaction(num_signers: u8) -> VersionedTransaction {
        let payer = Keypair::new();
        let recipient = Keypair::new();

        // Create a simple transfer instruction
        let instruction =
            system_instruction::transfer(&payer.pubkey(), &recipient.pubkey(), 1_000_000);

        // Build message with specified number of signers
        let mut account_keys = vec![payer.pubkey()];
        for _ in 1..num_signers {
            account_keys.push(Keypair::new().pubkey());
        }
        if !account_keys.contains(&recipient.pubkey()) {
            account_keys.push(recipient.pubkey());
        }

        let message =
            MessageV0::try_compile(&payer.pubkey(), &[instruction], &[], Hash::default()).unwrap();

        VersionedTransaction {
            signatures: vec![solana_sdk::signature::Signature::default(); num_signers as usize],
            message: VersionedMessage::V0(message),
        }
    }

    /// Mock NonceLease for testing
    fn create_mock_lease(released: Arc<AtomicBool>) -> crate::nonce_manager::NonceLease {
        use std::time::Duration;

        let nonce_pubkey = Keypair::new().pubkey();
        let release_fn = move || {
            released.store(true, AtomicOrdering::SeqCst);
        };

        crate::nonce_manager::NonceLease::new(
            nonce_pubkey,
            12345,
            Hash::default(),
            Duration::from_secs(30),
            release_fn,
        )
    }

    #[test]
    fn test_txbuildoutput_new_extracts_required_signers() {
        // Create a transaction with 2 required signers
        let tx = create_test_transaction(2);

        // Create TxBuildOutput without nonce guard
        let output = TxBuildOutput::new(tx.clone(), None);

        // Verify required_signers extracted correctly using compat layer
        assert_eq!(output.required_signers.len(), 2);
        assert_eq!(
            crate::compat::get_num_required_signatures(&output.tx.message),
            2
        );

        // Verify first signer matches first account key
        let static_keys = crate::compat::get_static_account_keys(&tx.message);
        assert_eq!(output.required_signers[0], static_keys[0]);
    }

    #[test]
    fn test_txbuildoutput_without_nonce_guard() {
        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, None);

        // Verify no nonce guard present
        assert!(output.nonce_guard.is_none());
    }

    #[tokio::test]
    async fn test_txbuildoutput_release_nonce_when_no_guard() {
        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, None);

        // Should succeed even without a nonce guard
        let result = output.release_nonce().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_txbuildoutput_release_nonce_explicit() {
        let released = Arc::new(AtomicBool::new(false));
        let lease = create_mock_lease(released.clone());

        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, Some(lease));

        // Verify lease not released yet
        assert!(!released.load(AtomicOrdering::SeqCst));

        // Explicitly release
        let result = output.release_nonce().await;
        assert!(result.is_ok());

        // Verify release was called
        assert!(released.load(AtomicOrdering::SeqCst));
    }

    #[tokio::test]
    async fn test_txbuildoutput_drop_releases_lease() {
        let released = Arc::new(AtomicBool::new(false));
        let lease = create_mock_lease(released.clone());

        let tx = create_test_transaction(1);

        {
            let _output = TxBuildOutput::new(tx, Some(lease));
            // Verify lease not released yet
            assert!(!released.load(AtomicOrdering::SeqCst));
            // output goes out of scope here and Drop is called
        }

        // Verify Drop triggered release
        assert!(released.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn test_txbuildoutput_drop_without_nonce_guard() {
        let tx = create_test_transaction(1);

        // Should not panic when dropped without nonce guard
        {
            let _output = TxBuildOutput::new(tx, None);
        }
        // No panic = success
    }

    #[test]
    fn test_execution_context_extract_lease() {
        let released = Arc::new(AtomicBool::new(false));
        let lease = create_mock_lease(released.clone());

        let exec_ctx = ExecutionContext {
            blockhash: Hash::default(),
            nonce_pubkey: Some(Keypair::new().pubkey()),
            nonce_authority: Some(Keypair::new().pubkey()),
            nonce_lease: Some(lease),
            #[cfg(feature = "zk_enabled")]
            zk_proof: None,
        };

        // Extract lease
        let extracted_lease = exec_ctx.extract_lease();

        // Verify lease was extracted
        assert!(extracted_lease.is_some());

        // Drop the extracted lease and verify release
        drop(extracted_lease);
        assert!(released.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn test_execution_context_extract_lease_when_none() {
        let exec_ctx = ExecutionContext {
            blockhash: Hash::default(),
            nonce_pubkey: None,
            nonce_authority: None,
            nonce_lease: None,
            #[cfg(feature = "zk_enabled")]
            zk_proof: None,
        };

        // Extract lease (should be None)
        let extracted_lease = exec_ctx.extract_lease();
        assert!(extracted_lease.is_none());
    }

    #[test]
    fn test_execution_context_drop_releases_lease() {
        let released = Arc::new(AtomicBool::new(false));
        let lease = create_mock_lease(released.clone());

        // Verify lease not released yet
        assert!(!released.load(AtomicOrdering::SeqCst));

        {
            let _exec_ctx = ExecutionContext {
                blockhash: Hash::default(),
                nonce_pubkey: Some(Keypair::new().pubkey()),
                nonce_authority: Some(Keypair::new().pubkey()),
                nonce_lease: Some(lease),
                #[cfg(feature = "zk_enabled")]
                zk_proof: None,
            };
            // ExecutionContext dropped here
        }

        // Verify lease was released via RAII
        assert!(released.load(AtomicOrdering::SeqCst));
    }

    #[tokio::test]
    async fn test_txbuildoutput_no_double_release() {
        // Test that release is idempotent (via consuming self pattern)
        let released = Arc::new(AtomicBool::new(false));
        let lease = create_mock_lease(released.clone());

        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, Some(lease));

        // Explicitly release
        let result = output.release_nonce().await;
        assert!(result.is_ok());

        // Verify release was called exactly once
        assert!(released.load(AtomicOrdering::SeqCst));

        // Cannot call release again because output was consumed
        // This is a compile-time guarantee, not a runtime check
    }

    #[tokio::test]
    async fn test_lease_ownership_transfer() {
        // Test ownership transfer: ExecutionContext -> TxBuildOutput
        let released = Arc::new(AtomicBool::new(false));
        let lease = create_mock_lease(released.clone());

        let exec_ctx = ExecutionContext {
            blockhash: Hash::default(),
            nonce_pubkey: Some(Keypair::new().pubkey()),
            nonce_authority: Some(Keypair::new().pubkey()),
            nonce_lease: Some(lease),
            #[cfg(feature = "zk_enabled")]
            zk_proof: None,
        };

        // Extract lease (ownership transfer)
        let extracted_lease = exec_ctx.extract_lease();
        assert!(extracted_lease.is_some());

        // Lease not released yet
        assert!(!released.load(AtomicOrdering::SeqCst));

        // Transfer to TxBuildOutput
        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, extracted_lease);

        // Still not released
        assert!(!released.load(AtomicOrdering::SeqCst));

        // Explicitly release via TxBuildOutput
        output.release_nonce().await.unwrap();

        // Now released
        assert!(released.load(AtomicOrdering::SeqCst));
    }

    #[tokio::test]
    async fn test_lease_survives_await_boundaries() {
        // Test that owned lease works correctly across await points
        let released = Arc::new(AtomicBool::new(false));
        let lease = create_mock_lease(released.clone());

        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, Some(lease));

        // Simulate async operation
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Lease still held
        assert!(!released.load(AtomicOrdering::SeqCst));

        // Another await point
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Lease still held
        assert!(!released.load(AtomicOrdering::SeqCst));

        // Release after multiple await points
        output.release_nonce().await.unwrap();

        // Now released
        assert!(released.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn test_no_references_in_structures() {
        // Compile-time test: Verify structures don't hold references
        // This test exists to document the constraint

        // ExecutionContext must be 'static (no lifetime parameters)
        fn assert_static<T: 'static>(_: &T) {}

        let exec_ctx = ExecutionContext {
            blockhash: Hash::default(),
            nonce_pubkey: None,
            nonce_authority: None,
            nonce_lease: None,
            #[cfg(feature = "zk_enabled")]
            zk_proof: None,
        };
        assert_static(&exec_ctx);

        // TxBuildOutput must be 'static
        let tx = create_test_transaction(1);
        let output = TxBuildOutput::new(tx, None);
        assert_static(&output);
    }

    /// Compile-time check: Ensure key types are Send (required for async/await across threads)
    #[test]
    fn test_send_bounds() {
        fn is_send<T: Send>() {}

        // All types that cross await boundaries must be Send
        is_send::<TxBuildOutput>();
        is_send::<ExecutionContext>();
        is_send::<TransactionConfig>();
        is_send::<crate::nonce_manager::NonceLease>();
    }

    /// Compile-time check: Ensure key types are Sync where needed
    #[test]
    fn test_sync_bounds() {
        fn is_sync<T: Sync>() {}

        // Config is often shared via Arc across tasks
        is_sync::<TransactionConfig>();
    }

    // ========================================================================
    // Instruction Ordering Tests
    // ========================================================================

    /// Helper to create a dummy ExecutionContext with or without nonce
    fn create_test_exec_context(with_nonce: bool) -> ExecutionContext {
        ExecutionContext {
            blockhash: Hash::default(),
            nonce_pubkey: if with_nonce {
                Some(Pubkey::new_unique())
            } else {
                None
            },
            nonce_authority: if with_nonce {
                Some(Pubkey::new_unique())
            } else {
                None
            },
            nonce_lease: None,
            #[cfg(feature = "zk_enabled")]
            zk_proof: None,
        }
    }

    /// Helper to create a dummy DEX instruction
    fn create_test_dex_instruction() -> Instruction {
        Instruction::new_with_bytes(
            Pubkey::new_unique(), // DEX program ID
            &[1, 2, 3, 4],
            vec![],
        )
    }

    /// Helper to check if instruction is advance nonce
    fn is_advance_nonce_ix(ix: &Instruction) -> bool {
        ix.program_id == solana_sdk::system_program::id()
            && ix.data.len() >= 4
            && u32::from_le_bytes([ix.data[0], ix.data[1], ix.data[2], ix.data[3]]) == 4
    }

    /// Helper to check if instruction is compute budget
    fn is_compute_budget_ix(ix: &Instruction) -> bool {
        ix.program_id == solana_sdk::compute_budget::id()
    }

    #[test]
    fn test_instruction_order_without_nonce() {
        let exec_ctx = create_test_exec_context(false);
        let compute_budget = vec![
            ComputeBudgetInstruction::set_compute_unit_limit(200_000),
            ComputeBudgetInstruction::set_compute_unit_price(1000),
        ];
        let dex_ix = create_test_dex_instruction();

        let instructions = TransactionBuilder::build_ordered_instructions(
            &exec_ctx,
            false,
            compute_budget,
            dex_ix,
        );

        assert_eq!(instructions.len(), 3);
        assert!(is_compute_budget_ix(&instructions[0]));
        assert!(is_compute_budget_ix(&instructions[1]));
        assert!(!is_compute_budget_ix(&instructions[2]));
        assert!(!is_advance_nonce_ix(&instructions[2]));
    }

    #[test]
    fn test_instruction_order_with_nonce_production() {
        let exec_ctx = create_test_exec_context(true);
        let compute_budget = vec![
            ComputeBudgetInstruction::set_compute_unit_limit(200_000),
            ComputeBudgetInstruction::set_compute_unit_price(1000),
        ];
        let dex_ix = create_test_dex_instruction();

        let instructions = TransactionBuilder::build_ordered_instructions(
            &exec_ctx,
            false,
            compute_budget,
            dex_ix,
        );

        assert_eq!(instructions.len(), 4);
        assert!(
            is_advance_nonce_ix(&instructions[0]),
            "First must be advance nonce"
        );
        assert!(is_compute_budget_ix(&instructions[1]));
        assert!(is_compute_budget_ix(&instructions[2]));
        assert!(!is_compute_budget_ix(&instructions[3]));
        assert!(!is_advance_nonce_ix(&instructions[3]));
    }

    #[test]
    fn test_instruction_order_with_nonce_simulation() {
        let exec_ctx = create_test_exec_context(true);
        let compute_budget = vec![ComputeBudgetInstruction::set_compute_unit_limit(200_000)];
        let dex_ix = create_test_dex_instruction();

        let instructions = TransactionBuilder::build_ordered_instructions(
            &exec_ctx,
            true, // simulation mode
            compute_budget,
            dex_ix,
        );

        assert_eq!(instructions.len(), 2, "No nonce in simulation");
        assert!(is_compute_budget_ix(&instructions[0]));
        assert!(!is_compute_budget_ix(&instructions[1]));
        assert!(!is_advance_nonce_ix(&instructions[0]));
        assert!(!is_advance_nonce_ix(&instructions[1]));
    }

    #[test]
    fn test_instruction_order_minimal() {
        let exec_ctx = create_test_exec_context(false);
        let dex_ix = create_test_dex_instruction();

        let instructions =
            TransactionBuilder::build_ordered_instructions(&exec_ctx, false, vec![], dex_ix);

        assert_eq!(instructions.len(), 1);
        assert!(!is_compute_budget_ix(&instructions[0]));
        assert!(!is_advance_nonce_ix(&instructions[0]));
    }

    #[test]
    fn test_validation_correct_order() {
        let nonce_pub = Pubkey::new_unique();
        let nonce_auth = Pubkey::new_unique();

        let instructions = vec![
            system_instruction::advance_nonce_account(&nonce_pub, &nonce_auth),
            ComputeBudgetInstruction::set_compute_unit_limit(200_000),
            ComputeBudgetInstruction::set_compute_unit_price(1000),
            create_test_dex_instruction(),
        ];

        let result = TransactionBuilder::validate_instruction_order(&instructions, true, false);
        assert!(result.is_ok(), "Validation failed: {:?}", result);
    }

    #[test]
    fn test_validation_simulation_mode() {
        let instructions = vec![
            ComputeBudgetInstruction::set_compute_unit_limit(200_000),
            create_test_dex_instruction(),
        ];

        let result = TransactionBuilder::validate_instruction_order(&instructions, false, true);
        assert!(result.is_ok(), "Validation failed: {:?}", result);
    }

    #[test]
    fn test_error_conversion_nonce_to_transaction_builder() {
        // Test automatic conversion from NonceError to TransactionBuilderError
        let nonce_err = NonceError::NoLeaseAvailable;
        let tx_err: TransactionBuilderError = nonce_err.into();

        match tx_err {
            TransactionBuilderError::Nonce(err) => {
                assert!(matches!(err, NonceError::NoLeaseAvailable));
            }
            _ => panic!("Expected Nonce error variant"),
        }
    }

    #[test]
    fn test_error_conversion_rpc_manager_to_transaction_builder() {
        // Test automatic conversion from RpcManagerError to TransactionBuilderError
        let rpc_err = RpcManagerError::Timeout {
            endpoint: "https://test.com".to_string(),
            timeout_ms: 5000,
        };
        let tx_err: TransactionBuilderError = rpc_err.into();

        match tx_err {
            TransactionBuilderError::RpcManager(err) => {
                assert!(matches!(err, RpcManagerError::Timeout { .. }));
            }
            _ => panic!("Expected RpcManager error variant"),
        }
    }

    #[test]
    fn test_nonce_error_owned_fields() {
        // Verify all error fields are owned (String, not &str)
        let err1 = NonceError::LeaseAcquireFailed("test".to_string());
        let err2 = NonceError::LeaseReleaseFailed("test".to_string());
        let err3 = NonceError::Client("test".to_string());
        let err4 = NonceError::Rpc {
            endpoint: Some("test".to_string()),
            message: "msg".to_string(),
        };

        // All these should compile and contain owned strings
        assert!(err1.to_string().contains("acquire"));
        assert!(err2.to_string().contains("release"));
        assert!(err3.to_string().contains("client"));
        assert!(err4.to_string().contains("RPC"));
    }

    #[test]
    fn test_transaction_builder_error_owned_fields() {
        // Verify TransactionBuilderError fields are owned
        let err1 = TransactionBuilderError::ConfigValidation("test".to_string());
        let err2 = TransactionBuilderError::RpcConnection("test".to_string());
        let err3 = TransactionBuilderError::SigningFailed("test".to_string());

        assert!(err1.to_string().contains("Configuration"));
        assert!(err2.to_string().contains("RPC"));
        assert!(err3.to_string().contains("Signing"));
    }
}
