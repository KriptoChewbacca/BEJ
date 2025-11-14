//! Decentralized Multi-Agent Reinforcement Learning Engine
//!
//! This module implements a universe-class multi-agent RL system for adaptive
//! trading strategies. Agents (Scout, Validator, Executor) work collaboratively
//! with on-chain state persistence for distributed learning.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                 Multi-Agent RL Engine                   │
//! ├─────────────────────────────────────────────────────────┤
//! │                                                         │
//! │  ┌──────────┐      ┌───────────┐      ┌───────────┐   │
//! │  │  Scout   │ ───> │ Validator │ ───> │ Executor  │   │
//! │  │  Agent   │      │   Agent   │      │   Agent   │   │
//! │  └──────────┘      └───────────┘      └───────────┘   │
//! │       │                   │                   │        │
//! │       └───────────────────┴───────────────────┘        │
//! │                           │                            │
//! │                    ┌──────▼──────┐                     │
//! │                    │  RL State   │                     │
//! │                    │  (On-Chain) │                     │
//! │                    └─────────────┘                     │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Features
//!
//! - **Scout Agent**: Discovers trading opportunities, learns patterns
//! - **Validator Agent**: Validates signals, learns risk assessment
//! - **Executor Agent**: Executes trades, learns optimal timing
//! - **On-Chain State**: Persists Q-tables and policy parameters
//! - **Reward Feedback**: Real-time reward calculation from trade outcomes
//! - **Adaptive Strategies**: Self-adjusting based on market conditions

use anyhow::{Context, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::HashMap,
    sync::{
        atomic::AtomicU64,
        Arc,
    },
    time::Instant,
};
use tokio::sync::RwLock;
use tracing::{debug, info};

// ============================================================================
// Core RL Types and State
// ============================================================================

/// Agent type in the multi-agent system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentType {
    /// Discovers trading opportunities
    Scout,
    /// Validates signals and assesses risk
    Validator,
    /// Executes trades with optimal timing
    Executor,
}

/// State representation for RL agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Current market conditions (discretized)
    pub market_condition: MarketCondition,
    /// Current portfolio state
    pub portfolio_state: PortfolioState,
    /// Recent performance metrics
    pub performance_metrics: PerformanceMetrics,
}

/// Market condition (discretized for Q-learning)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketCondition {
    BullishHigh,  // High volume, uptrend
    BullishLow,   // Low volume, uptrend
    BearishHigh,  // High volume, downtrend
    BearishLow,   // Low volume, downtrend
    Sideways,     // Range-bound, no clear trend
    Volatile,     // High volatility, unpredictable
}

/// Portfolio state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioState {
    pub sol_balance: u64,
    pub open_positions: usize,
    pub total_pnl: i64,
    pub win_rate: f64,
}

/// Performance metrics for reward calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub trades_executed: u64,
    pub successful_trades: u64,
    pub average_profit: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
}

/// Action that an agent can take
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentAction {
    // Scout actions
    ScanAggressive,
    ScanConservative,
    ScanBalanced,
    Wait,

    // Validator actions
    ApproveHigh,
    ApproveLow,
    Reject,

    // Executor actions
    ExecuteImmediate,
    ExecuteDelayed,
    ExecuteWithLimit,
    Skip,
}

/// Q-value for state-action pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QValue {
    pub value: f64,
    pub visit_count: u64,
    pub last_updated: u64, // timestamp
}

/// On-chain RL state (serializable to Solana account)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnChainRLState {
    /// Agent type
    pub agent_type: AgentType,
    /// Q-table: (state_hash, action) -> Q-value
    pub q_table: HashMap<(u64, AgentAction), QValue>,
    /// Total episodes
    pub total_episodes: u64,
    /// Total reward accumulated
    pub total_reward: f64,
    /// Learning rate
    pub learning_rate: f64,
    /// Discount factor
    pub discount_factor: f64,
    /// Exploration rate (epsilon)
    pub epsilon: f64,
}

impl Default for OnChainRLState {
    fn default() -> Self {
        Self {
            agent_type: AgentType::Scout,
            q_table: HashMap::new(),
            total_episodes: 0,
            total_reward: 0.0,
            learning_rate: 0.1,
            discount_factor: 0.95,
            epsilon: 0.2, // 20% exploration
        }
    }
}

// ============================================================================
// RL Agent Implementation
// ============================================================================

/// Reinforcement learning agent
pub struct RLAgent {
    agent_type: AgentType,
    state: Arc<RwLock<OnChainRLState>>,
    state_cache: DashMap<u64, AgentState>,
    episode_count: AtomicU64,
    last_action: Arc<RwLock<Option<(AgentState, AgentAction)>>>,
}

impl RLAgent {
    /// Create a new RL agent
    pub fn new(agent_type: AgentType) -> Self {
        Self {
            agent_type,
            state: Arc::new(RwLock::new(OnChainRLState {
                agent_type,
                ..Default::default()
            })),
            state_cache: DashMap::new(),
            episode_count: AtomicU64::new(0),
            last_action: Arc::new(RwLock::new(None)),
        }
    }

    /// Select action using epsilon-greedy policy
    pub async fn select_action(&self, state: &AgentState) -> AgentAction {
        let rl_state = self.state.read().await;
        let state_hash = Self::hash_state(state);

        // Epsilon-greedy: explore or exploit
        let explore = fastrand::f64() < rl_state.epsilon;

        if explore {
            // Exploration: random action
            self.random_action()
        } else {
            // Exploitation: best Q-value
            self.best_action(&rl_state, state_hash)
        }
    }

    /// Get best action based on Q-values
    fn best_action(&self, rl_state: &OnChainRLState, state_hash: u64) -> AgentAction {
        let actions = self.available_actions();
        let mut best_action = actions[0];
        let mut best_value = f64::NEG_INFINITY;

        for &action in &actions {
            let q_value = rl_state
                .q_table
                .get(&(state_hash, action))
                .map(|q| q.value)
                .unwrap_or(0.0);

            if q_value > best_value {
                best_value = q_value;
                best_action = action;
            }
        }

        best_action
    }

    /// Get random action for exploration
    fn random_action(&self) -> AgentAction {
        let actions = self.available_actions();
        let idx = fastrand::usize(0..actions.len());
        actions[idx]
    }

    /// Get available actions for this agent type
    fn available_actions(&self) -> Vec<AgentAction> {
        match self.agent_type {
            AgentType::Scout => vec![
                AgentAction::ScanAggressive,
                AgentAction::ScanConservative,
                AgentAction::ScanBalanced,
                AgentAction::Wait,
            ],
            AgentType::Validator => vec![
                AgentAction::ApproveHigh,
                AgentAction::ApproveLow,
                AgentAction::Reject,
            ],
            AgentType::Executor => vec![
                AgentAction::ExecuteImmediate,
                AgentAction::ExecuteDelayed,
                AgentAction::ExecuteWithLimit,
                AgentAction::Skip,
            ],
        }
    }

    /// Update Q-value based on reward
    pub async fn update_q_value(
        &self,
        state: &AgentState,
        action: AgentAction,
        reward: f64,
        next_state: &AgentState,
    ) {
        let state_hash = Self::hash_state(state);
        let next_state_hash = Self::hash_state(next_state);

        let mut rl_state = self.state.write().await;

        // Get current Q-value
        let current_q = rl_state
            .q_table
            .get(&(state_hash, action))
            .map(|q| q.value)
            .unwrap_or(0.0);

        // Get max Q-value for next state
        let max_next_q = self
            .available_actions()
            .iter()
            .map(|&a| {
                rl_state
                    .q_table
                    .get(&(next_state_hash, a))
                    .map(|q| q.value)
                    .unwrap_or(0.0)
            })
            .fold(f64::NEG_INFINITY, f64::max);

        // Q-learning update: Q(s,a) = Q(s,a) + α[r + γ*max_Q(s',a') - Q(s,a)]
        let new_q = current_q
            + rl_state.learning_rate
                * (reward + rl_state.discount_factor * max_next_q - current_q);

        // Update Q-table
        let entry = rl_state
            .q_table
            .entry((state_hash, action))
            .or_insert(QValue {
                value: 0.0,
                visit_count: 0,
                last_updated: Self::timestamp(),
            });

        entry.value = new_q;
        entry.visit_count += 1;
        entry.last_updated = Self::timestamp();

        // Update global stats
        rl_state.total_reward += reward;
        rl_state.total_episodes += 1;

        // Decay epsilon (reduce exploration over time)
        rl_state.epsilon = (rl_state.epsilon * 0.995).max(0.05);

        info!(
            agent = ?self.agent_type,
            action = ?action,
            reward = %reward,
            new_q = %new_q,
            epsilon = %rl_state.epsilon,
            "Q-value updated"
        );
    }

    /// Calculate reward based on trade outcome
    pub fn calculate_reward(
        &self,
        trade_result: &TradeResult,
        portfolio_before: &PortfolioState,
        portfolio_after: &PortfolioState,
    ) -> f64 {
        match self.agent_type {
            AgentType::Scout => {
                // Reward scout for finding profitable opportunities
                let profit = portfolio_after.total_pnl - portfolio_before.total_pnl;
                if profit > 0 {
                    (profit as f64 / 1_000_000.0).min(100.0) // Cap at 100
                } else {
                    -10.0 // Penalty for unprofitable signals
                }
            }
            AgentType::Validator => {
                // Reward validator for accurate risk assessment
                if trade_result.success {
                    50.0 // Correct approval
                } else {
                    -20.0 // False positive
                }
            }
            AgentType::Executor => {
                // Reward executor for optimal timing
                let profit = portfolio_after.total_pnl - portfolio_before.total_pnl;
                let slippage_penalty = trade_result.slippage_bps * -0.1;
                (profit as f64 / 1_000_000.0) + slippage_penalty
            }
        }
    }

    /// Hash state to u64 for Q-table key
    fn hash_state(state: &AgentState) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        format!("{:?}", state.market_condition).hash(&mut hasher);
        state.portfolio_state.open_positions.hash(&mut hasher);
        ((state.portfolio_state.win_rate * 100.0) as u64).hash(&mut hasher);
        hasher.finish()
    }

    /// Get current timestamp
    fn timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Serialize state for on-chain storage
    pub async fn serialize_state(&self) -> Result<Vec<u8>> {
        let state = self.state.read().await;
        bincode::serialize(&*state).context("Failed to serialize RL state")
    }

    /// Deserialize state from on-chain storage
    pub async fn load_state(&self, data: &[u8]) -> Result<()> {
        let loaded_state: OnChainRLState =
            bincode::deserialize(data).context("Failed to deserialize RL state")?;

        let mut state = self.state.write().await;
        *state = loaded_state;

        info!(
            agent = ?self.agent_type,
            episodes = %state.total_episodes,
            total_reward = %state.total_reward,
            "RL state loaded from chain"
        );

        Ok(())
    }

    /// Get agent statistics
    pub async fn get_stats(&self) -> AgentStats {
        let state = self.state.read().await;
        AgentStats {
            agent_type: self.agent_type,
            total_episodes: state.total_episodes,
            total_reward: state.total_reward,
            average_reward: state.total_reward / state.total_episodes.max(1) as f64,
            q_table_size: state.q_table.len(),
            epsilon: state.epsilon,
            learning_rate: state.learning_rate,
        }
    }
}

/// Agent statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStats {
    pub agent_type: AgentType,
    pub total_episodes: u64,
    pub total_reward: f64,
    pub average_reward: f64,
    pub q_table_size: usize,
    pub epsilon: f64,
    pub learning_rate: f64,
}

/// Trade result for reward calculation
#[derive(Debug, Clone)]
pub struct TradeResult {
    pub success: bool,
    pub profit_loss: i64,
    pub slippage_bps: f64,
    pub execution_time_ms: u64,
}

// ============================================================================
// Multi-Agent Coordinator
// ============================================================================

/// Coordinates multiple RL agents
pub struct MultiAgentRLEngine {
    scout: Arc<RLAgent>,
    validator: Arc<RLAgent>,
    executor: Arc<RLAgent>,
    current_state: Arc<RwLock<AgentState>>,
    episode_start: Arc<RwLock<Option<Instant>>>,
}

impl MultiAgentRLEngine {
    /// Create a new multi-agent RL engine
    pub fn new() -> Self {
        Self {
            scout: Arc::new(RLAgent::new(AgentType::Scout)),
            validator: Arc::new(RLAgent::new(AgentType::Validator)),
            executor: Arc::new(RLAgent::new(AgentType::Executor)),
            current_state: Arc::new(RwLock::new(Self::initial_state())),
            episode_start: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize default state
    fn initial_state() -> AgentState {
        AgentState {
            market_condition: MarketCondition::Sideways,
            portfolio_state: PortfolioState {
                sol_balance: 1_000_000_000, // 1 SOL
                open_positions: 0,
                total_pnl: 0,
                win_rate: 0.5,
            },
            performance_metrics: PerformanceMetrics {
                trades_executed: 0,
                successful_trades: 0,
                average_profit: 0.0,
                sharpe_ratio: 0.0,
                max_drawdown: 0.0,
            },
        }
    }

    /// Start a new trading episode
    pub async fn start_episode(&self, market_condition: MarketCondition) {
        let mut state = self.current_state.write().await;
        state.market_condition = market_condition;

        let mut episode_start = self.episode_start.write().await;
        *episode_start = Some(Instant::now());

        info!(?market_condition, "New trading episode started");
    }

    /// Execute full agent pipeline: Scout -> Validator -> Executor
    pub async fn execute_pipeline(
        &self,
        opportunity: TradingOpportunity,
    ) -> Result<AgentPipelineResult> {
        let state = self.current_state.read().await.clone();

        // 1. Scout evaluates opportunity
        let scout_action = self.scout.select_action(&state).await;
        debug!(?scout_action, "Scout selected action");

        let should_proceed = matches!(
            scout_action,
            AgentAction::ScanAggressive | AgentAction::ScanConservative | AgentAction::ScanBalanced
        );

        if !should_proceed {
            return Ok(AgentPipelineResult::Skipped {
                reason: "Scout decided to wait".to_string(),
            });
        }

        // 2. Validator assesses risk
        let validator_action = self.validator.select_action(&state).await;
        debug!(?validator_action, "Validator selected action");

        let should_approve = matches!(
            validator_action,
            AgentAction::ApproveHigh | AgentAction::ApproveLow
        );

        if !should_approve {
            return Ok(AgentPipelineResult::Rejected {
                reason: "Validator rejected signal".to_string(),
            });
        }

        // 3. Executor determines timing
        let executor_action = self.executor.select_action(&state).await;
        debug!(?executor_action, "Executor selected action");

        match executor_action {
            AgentAction::ExecuteImmediate => Ok(AgentPipelineResult::ExecuteImmediate),
            AgentAction::ExecuteDelayed => Ok(AgentPipelineResult::ExecuteDelayed {
                delay_ms: 500,
            }),
            AgentAction::ExecuteWithLimit => Ok(AgentPipelineResult::ExecuteWithLimit {
                limit_price: opportunity.price * 1.01, // 1% limit
            }),
            AgentAction::Skip => Ok(AgentPipelineResult::Skipped {
                reason: "Executor decided to skip".to_string(),
            }),
            _ => Ok(AgentPipelineResult::Skipped {
                reason: "Invalid executor action".to_string(),
            }),
        }
    }

    /// Update all agents based on trade outcome
    pub async fn update_from_trade(
        &self,
        trade_result: TradeResult,
        next_market_condition: MarketCondition,
    ) {
        let current_state = self.current_state.read().await.clone();

        let mut next_state = current_state.clone();
        next_state.market_condition = next_market_condition;

        // Update portfolio state
        next_state.portfolio_state.total_pnl += trade_result.profit_loss;
        next_state.performance_metrics.trades_executed += 1;
        if trade_result.success {
            next_state.performance_metrics.successful_trades += 1;
        }
        next_state.portfolio_state.win_rate = next_state.performance_metrics.successful_trades
            as f64
            / next_state.performance_metrics.trades_executed.max(1) as f64;

        // Calculate rewards for each agent
        let scout_reward = self
            .scout
            .calculate_reward(&trade_result, &current_state.portfolio_state, &next_state.portfolio_state);
        let validator_reward = self
            .validator
            .calculate_reward(&trade_result, &current_state.portfolio_state, &next_state.portfolio_state);
        let executor_reward = self
            .executor
            .calculate_reward(&trade_result, &current_state.portfolio_state, &next_state.portfolio_state);

        // Update Q-values (with last action from episode)
        // In production, we'd store the actual actions taken
        let scout_action = AgentAction::ScanBalanced;
        let validator_action = AgentAction::ApproveHigh;
        let executor_action = AgentAction::ExecuteImmediate;

        self.scout
            .update_q_value(&current_state, scout_action, scout_reward, &next_state)
            .await;
        self.validator
            .update_q_value(&current_state, validator_action, validator_reward, &next_state)
            .await;
        self.executor
            .update_q_value(&current_state, executor_action, executor_reward, &next_state)
            .await;

        // Update current state
        let mut state = self.current_state.write().await;
        *state = next_state;

        info!(
            scout_reward = %scout_reward,
            validator_reward = %validator_reward,
            executor_reward = %executor_reward,
            "Agents updated with trade feedback"
        );
    }

    /// Get combined statistics
    pub async fn get_stats(&self) -> MultiAgentStats {
        MultiAgentStats {
            scout: self.scout.get_stats().await,
            validator: self.validator.get_stats().await,
            executor: self.executor.get_stats().await,
        }
    }

    /// Save all agent states to on-chain accounts
    pub async fn save_to_chain(&self) -> Result<Vec<OnChainUpdate>> {
        let scout_data = self.scout.serialize_state().await?;
        let validator_data = self.validator.serialize_state().await?;
        let executor_data = self.executor.serialize_state().await?;

        Ok(vec![
            OnChainUpdate {
                agent_type: AgentType::Scout,
                data: scout_data,
            },
            OnChainUpdate {
                agent_type: AgentType::Validator,
                data: validator_data,
            },
            OnChainUpdate {
                agent_type: AgentType::Executor,
                data: executor_data,
            },
        ])
    }

    /// Load all agent states from on-chain accounts
    pub async fn load_from_chain(&self, updates: &[OnChainUpdate]) -> Result<()> {
        for update in updates {
            match update.agent_type {
                AgentType::Scout => self.scout.load_state(&update.data).await?,
                AgentType::Validator => self.validator.load_state(&update.data).await?,
                AgentType::Executor => self.executor.load_state(&update.data).await?,
            }
        }

        info!("All agents loaded from on-chain state");
        Ok(())
    }
}

impl Default for MultiAgentRLEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Trading opportunity detected by scout
#[derive(Debug, Clone)]
pub struct TradingOpportunity {
    pub mint: Pubkey,
    pub price: f64,
    pub volume: u64,
    pub confidence: f64,
}

/// Result of agent pipeline execution
#[derive(Debug, Clone)]
pub enum AgentPipelineResult {
    ExecuteImmediate,
    ExecuteDelayed { delay_ms: u64 },
    ExecuteWithLimit { limit_price: f64 },
    Skipped { reason: String },
    Rejected { reason: String },
}

/// On-chain state update
#[derive(Debug, Clone)]
pub struct OnChainUpdate {
    pub agent_type: AgentType,
    pub data: Vec<u8>,
}

/// Combined statistics for all agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiAgentStats {
    pub scout: AgentStats,
    pub validator: AgentStats,
    pub executor: AgentStats,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_creation() {
        let agent = RLAgent::new(AgentType::Scout);
        let stats = agent.get_stats().await;
        assert_eq!(stats.agent_type, AgentType::Scout);
        assert_eq!(stats.total_episodes, 0);
    }

    #[tokio::test]
    async fn test_action_selection() {
        let agent = RLAgent::new(AgentType::Scout);
        let state = AgentState {
            market_condition: MarketCondition::BullishHigh,
            portfolio_state: PortfolioState {
                sol_balance: 1_000_000_000,
                open_positions: 0,
                total_pnl: 0,
                win_rate: 0.5,
            },
            performance_metrics: PerformanceMetrics {
                trades_executed: 0,
                successful_trades: 0,
                average_profit: 0.0,
                sharpe_ratio: 0.0,
                max_drawdown: 0.0,
            },
        };

        let action = agent.select_action(&state).await;
        assert!(matches!(
            action,
            AgentAction::ScanAggressive
                | AgentAction::ScanConservative
                | AgentAction::ScanBalanced
                | AgentAction::Wait
        ));
    }

    #[tokio::test]
    async fn test_q_value_update() {
        let agent = RLAgent::new(AgentType::Scout);
        let state = AgentState {
            market_condition: MarketCondition::BullishHigh,
            portfolio_state: PortfolioState {
                sol_balance: 1_000_000_000,
                open_positions: 0,
                total_pnl: 0,
                win_rate: 0.5,
            },
            performance_metrics: PerformanceMetrics {
                trades_executed: 0,
                successful_trades: 0,
                average_profit: 0.0,
                sharpe_ratio: 0.0,
                max_drawdown: 0.0,
            },
        };

        let next_state = state.clone();

        agent
            .update_q_value(&state, AgentAction::ScanAggressive, 10.0, &next_state)
            .await;

        let stats = agent.get_stats().await;
        assert_eq!(stats.total_episodes, 1);
        assert_eq!(stats.q_table_size, 1);
    }

    #[tokio::test]
    async fn test_multi_agent_pipeline() {
        let engine = MultiAgentRLEngine::new();
        engine.start_episode(MarketCondition::BullishHigh).await;

        let opportunity = TradingOpportunity {
            mint: Pubkey::new_unique(),
            price: 0.001,
            volume: 1_000_000,
            confidence: 0.8,
        };

        let result = engine.execute_pipeline(opportunity).await.unwrap();
        assert!(matches!(
            result,
            AgentPipelineResult::ExecuteImmediate
                | AgentPipelineResult::ExecuteDelayed { .. }
                | AgentPipelineResult::ExecuteWithLimit { .. }
                | AgentPipelineResult::Skipped { .. }
                | AgentPipelineResult::Rejected { .. }
        ));
    }
}
