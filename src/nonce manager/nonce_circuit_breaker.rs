//! Universe-specific Circuit Breaker Extensions
//!
//! This module provides Universe-specific circuit breaker implementations
//! that extend the base circuit breaker functionality from nonce_retry.rs
//! with additional features:
//! - UniverseCircuitBreaker: Per-RPC circuit breaker with atomic operations
//! - GlobalCircuitBreaker: System-wide health monitoring
//! - RLAgent: Reinforcement learning for adaptive retry strategies
use crossbeam::atomic::AtomicCell;
use rand::Rng;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

// ============================================================================
// CIRCUIT BREAKER STATE
// ============================================================================

/// Circuit Breaker State
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    Closed,
    Open,
    HalfOpen,
}

// ============================================================================
// UNIVERSE CIRCUIT BREAKER (PER RPC/TIER)
// ============================================================================

/// Universe Circuit Breaker (per RPC/tier)
///
/// Lightweight circuit breaker using atomic operations for lock-free state management.
/// This is optimized for high-frequency RPC calls where traditional mutex-based
/// circuit breakers would create contention.
#[derive(Debug)]
pub struct UniverseCircuitBreaker {
    state: AtomicCell<BreakerState>,
    failure_count: AtomicU64,
    success_count: AtomicU64,
    failure_threshold: u64,
    success_threshold: u64,
    last_state_change: Arc<RwLock<Instant>>,
    timeout: Duration,
}

impl UniverseCircuitBreaker {
    pub fn new(failure_threshold: u64, success_threshold: u64, timeout: Duration) -> Self {
        Self {
            state: AtomicCell::new(BreakerState::Closed),
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            failure_threshold,
            success_threshold,
            last_state_change: Arc::new(RwLock::new(Instant::now())),
            timeout,
        }
    }

    pub fn can_execute(&self) -> bool {
        match self.state.load() {
            BreakerState::Closed => true,
            BreakerState::HalfOpen => true,
            BreakerState::Open => {
                // Check if timeout has elapsed
                false // Will be checked by async method
            }
        }
    }

    pub async fn record_success(&self) {
        match self.state.load() {
            BreakerState::HalfOpen => {
                let count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                if count >= self.success_threshold {
                    self.state.store(BreakerState::Closed);
                    self.failure_count.store(0, Ordering::Relaxed);
                    self.success_count.store(0, Ordering::Relaxed);
                    *self.last_state_change.write().await = Instant::now();
                }
            }
            BreakerState::Closed => {
                self.failure_count.store(0, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    pub async fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= self.failure_threshold && self.state.load() == BreakerState::Closed {
            self.state.store(BreakerState::Open);
            *self.last_state_change.write().await = Instant::now();
        }
    }

    pub async fn check_and_transition(&self) {
        if self.state.load() == BreakerState::Open {
            let elapsed = self.last_state_change.read().await.elapsed();
            if elapsed >= self.timeout {
                self.state.store(BreakerState::HalfOpen);
                self.success_count.store(0, Ordering::Relaxed);
                *self.last_state_change.write().await = Instant::now();
            }
        }
    }

    pub fn get_state(&self) -> BreakerState {
        self.state.load()
    }
}

// ============================================================================
// GLOBAL CIRCUIT BREAKER FOR SYSTEM-WIDE HEALTH
// ============================================================================

/// Global Circuit Breaker for system-wide health
///
/// Monitors system-wide nonce pool health metrics:
/// - Percentage of locked nonces
/// - Average latency across all operations
/// - Decides when to open circuit breaker globally
#[derive(Debug)]
pub struct GlobalCircuitBreaker {
    pub breaker: UniverseCircuitBreaker,
    pub locked_nonces_count: AtomicU64,
    pub average_latency_ms: AtomicCell<f64>,
}

impl GlobalCircuitBreaker {
    pub fn new() -> Self {
        Self {
            breaker: UniverseCircuitBreaker::new(
                10, // failure threshold
                5,  // success threshold
                Duration::from_secs(30),
            ),
            locked_nonces_count: AtomicU64::new(0),
            average_latency_ms: AtomicCell::new(0.0),
        }
    }

    pub fn should_open(&self, total_nonces: u64) -> bool {
        let locked = self.locked_nonces_count.load(Ordering::Relaxed);
        let locked_pct = (locked as f64 / total_nonces as f64) * 100.0;
        let latency = self.average_latency_ms.load();

        // Open if >70% locked >10s or average latency >200ms
        locked_pct > 70.0 || latency > 200.0
    }
}

// ============================================================================
// REINFORCEMENT LEARNING AGENT (Q-LEARNING)
// ============================================================================

/// Reinforcement Learning Agent (Q-learning)
///
/// Uses Q-learning to adaptively determine optimal retry parameters
/// based on network congestion and historical failure patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CongestionLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RLState {
    pub congestion: CongestionLevel,
    pub failure_count: u8, // Bucketed 0-10
}

#[derive(Debug, Clone, Copy)]
pub struct RLAction {
    pub attempts: u32, // 1-10
    pub jitter: f64,   // 0.0-0.3
}

pub struct RLAgent {
    q_table: Arc<RwLock<HashMap<(RLState, usize), f64>>>, // Q(state, action) -> value
    actions: Vec<RLAction>,
    epsilon: AtomicCell<f64>, // Exploration rate
    alpha: f64,               // Learning rate
    gamma: f64,               // Discount factor
}

impl RLAgent {
    pub fn new() -> Self {
        let mut actions = Vec::new();
        for attempts in 1..=10 {
            for jitter_tenths in 0..=3 {
                actions.push(RLAction {
                    attempts,
                    jitter: jitter_tenths as f64 * 0.1,
                });
            }
        }

        Self {
            q_table: Arc::new(RwLock::new(HashMap::new())),
            actions,
            epsilon: AtomicCell::new(0.1), // Start with 10% exploration
            alpha: 0.1,
            gamma: 0.9,
        }
    }

    pub async fn choose_action(&self, state: RLState) -> (usize, RLAction) {
        let mut rng = rand::thread_rng();
        let epsilon = self.epsilon.load();

        // Epsilon-greedy exploration
        if rng.gen::<f64>() < epsilon {
            // Explore: random action
            let idx = rng.gen_range(0..self.actions.len());
            (idx, self.actions[idx])
        } else {
            // Exploit: best known action
            let q_table = self.q_table.read().await;
            let mut best_idx = 0;
            let mut best_value = f64::NEG_INFINITY;

            for (idx, _action) in self.actions.iter().enumerate() {
                let value = q_table.get(&(state, idx)).copied().unwrap_or(0.0);
                if value > best_value {
                    best_value = value;
                    best_idx = idx;
                }
            }

            (best_idx, self.actions[best_idx])
        }
    }

    pub async fn update(
        &self,
        state: RLState,
        action_idx: usize,
        reward: f64,
        next_state: RLState,
    ) {
        let mut q_table = self.q_table.write().await;

        // Find max Q-value for next state
        let mut max_next_q: f64 = 0.0;
        for idx in 0..self.actions.len() {
            let next_q = q_table.get(&(next_state, idx)).copied().unwrap_or(0.0);
            max_next_q = max_next_q.max(next_q);
        }

        // Q-learning update: Q(s,a) = Q(s,a) + alpha * (reward + gamma * max Q(s',a') - Q(s,a))
        let current_q = q_table.get(&(state, action_idx)).copied().unwrap_or(0.0);
        let new_q = current_q + self.alpha * (reward + self.gamma * max_next_q - current_q);
        q_table.insert((state, action_idx), new_q);
    }

    pub fn decay_epsilon(&self) {
        let current = self.epsilon.load();
        let new_epsilon = (current * 0.995).max(0.01); // Decay to min 1%
        self.epsilon.store(new_epsilon);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_universe_circuit_breaker_transitions() {
        let breaker = UniverseCircuitBreaker::new(3, 2, Duration::from_secs(1));

        // Initially closed
        assert_eq!(breaker.get_state(), BreakerState::Closed);

        // Record failures until threshold
        breaker.record_failure().await;
        breaker.record_failure().await;
        assert_eq!(breaker.get_state(), BreakerState::Closed);

        breaker.record_failure().await;
        assert_eq!(breaker.get_state(), BreakerState::Open);

        // Wait for timeout
        sleep(Duration::from_secs(1)).await;
        breaker.check_and_transition().await;
        assert_eq!(breaker.get_state(), BreakerState::HalfOpen);

        // Record successes to close
        breaker.record_success().await;
        breaker.record_success().await;
        assert_eq!(breaker.get_state(), BreakerState::Closed);
    }

    #[tokio::test]
    async fn test_global_circuit_breaker_threshold() {
        let global_breaker = GlobalCircuitBreaker::new();

        // Should not open with low locked percentage
        assert!(!global_breaker.should_open(100));

        // Should open with high locked percentage
        global_breaker
            .locked_nonces_count
            .store(71, Ordering::Relaxed);
        assert!(global_breaker.should_open(100));

        // Should open with high latency
        global_breaker
            .locked_nonces_count
            .store(0, Ordering::Relaxed);
        global_breaker.average_latency_ms.store(250.0);
        assert!(global_breaker.should_open(100));
    }

    #[tokio::test]
    async fn test_rl_agent_action_selection() {
        let agent = RLAgent::new();
        let state = RLState {
            congestion: CongestionLevel::Low,
            failure_count: 0,
        };

        // Should select an action
        let (idx, action) = agent.choose_action(state).await;
        assert!(idx < 40); // 10 attempts * 4 jitter levels
        assert!(action.attempts >= 1 && action.attempts <= 10);
        assert!(action.jitter >= 0.0 && action.jitter <= 0.3);
    }

    #[tokio::test]
    async fn test_rl_agent_learning() {
        let agent = RLAgent::new();
        let state = RLState {
            congestion: CongestionLevel::Medium,
            failure_count: 2,
        };
        let next_state = RLState {
            congestion: CongestionLevel::Low,
            failure_count: 0,
        };

        // Update Q-value
        agent.update(state, 0, 1.0, next_state).await;

        // Q-table should have entry
        let q_table = agent.q_table.read().await;
        assert!(q_table.contains_key(&(state, 0)));
    }
}
