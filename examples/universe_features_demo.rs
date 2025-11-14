//! Integration Example - Universe-Grade Features
//!
//! This example demonstrates how to use all three universe-grade features together:
//! 1. Multi-Agent RL Engine for adaptive trading
//! 2. Provenance Graph for signal source tracking
//! 3. Quantum Pruner for code optimization analysis
//!
//! Run with: cargo run --example universe_features_demo

use anyhow::Result;
use bot::components::{
    multi_agent_rl::{
        AgentType, MarketCondition, MultiAgentRLEngine, PortfolioState, TradeResult,
        TradingOpportunity,
    },
    provenance_graph::{
        DID, ProvenanceGraphManager, SignalMetrics, SignalSourceType,
    },
    quantum_pruner::PathPruner,
};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<()> {
    println!("\n=== Universe-Grade Features Integration Demo ===\n");

    // ========================================================================
    // Part 1: Multi-Agent RL Engine
    // ========================================================================
    println!("1️⃣  Multi-Agent RL Engine Demo");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let rl_engine = MultiAgentRLEngine::new();

    // Start a trading episode in bullish market
    rl_engine
        .start_episode(MarketCondition::BullishHigh)
        .await;
    println!("✓ Trading episode started (Bullish High market)\n");

    // Simulate discovering a trading opportunity
    let opportunity = TradingOpportunity {
        mint: Pubkey::new_unique(),
        price: 0.001,
        volume: 1_000_000,
        confidence: 0.85,
    };

    // Run the agent pipeline: Scout → Validator → Executor
    let decision = rl_engine.execute_pipeline(opportunity).await?;
    println!("Agent Decision: {:?}\n", decision);

    // Simulate trade execution and outcome
    let trade_result = TradeResult {
        success: true,
        profit_loss: 50_000, // 0.05 SOL profit
        slippage_bps: 10.0,  // 0.1% slippage
        execution_time_ms: 50,
    };

    // Update agents with feedback
    rl_engine
        .update_from_trade(trade_result, MarketCondition::BullishHigh)
        .await;
    println!("✓ Agents updated with trade feedback\n");

    // Get agent statistics
    let stats = rl_engine.get_stats().await;
    println!("Agent Statistics:");
    println!("  Scout:");
    println!("    - Episodes: {}", stats.scout.total_episodes);
    println!("    - Avg Reward: {:.2}", stats.scout.average_reward);
    println!("    - Q-Table Size: {}", stats.scout.q_table_size);
    println!("  Validator:");
    println!("    - Episodes: {}", stats.validator.total_episodes);
    println!("    - Avg Reward: {:.2}", stats.validator.average_reward);
    println!("  Executor:");
    println!("    - Episodes: {}", stats.executor.total_episodes);
    println!("    - Avg Reward: {:.2}", stats.executor.average_reward);

    // ========================================================================
    // Part 2: Provenance Graph System
    // ========================================================================
    println!("\n\n2️⃣  Provenance Graph System Demo");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let prov_graph = ProvenanceGraphManager::new();

    // Register signal sources with DIDs
    let onchain_source = DID::from_pubkey(&Pubkey::new_unique());
    let ml_source = DID::from_pubkey(&Pubkey::new_unique());

    prov_graph
        .register_source(
            onchain_source.clone(),
            SignalSourceType::OnChain,
            HashMap::new(),
        )
        .await?;
    println!("✓ Registered on-chain signal source: {}", onchain_source.to_string());

    let mut ml_metadata = HashMap::new();
    ml_metadata.insert("model".to_string(), "RandomForest".to_string());
    ml_metadata.insert("version".to_string(), "v1.2.3".to_string());

    prov_graph
        .register_source(ml_source.clone(), SignalSourceType::MLModel, ml_metadata)
        .await?;
    println!("✓ Registered ML model source: {}\n", ml_source.to_string());

    // Track signals from sources
    for i in 0..15 {
        let value = 10.0 + (i as f64 * 0.5);
        let result = prov_graph
            .track_signal(&onchain_source, value, true, 10)
            .await?;

        if i == 0 || i == 14 {
            println!(
                "Signal #{}: value={:.1}, anomaly={}, reputation={:.2}",
                i + 1,
                value,
                result.is_anomalous,
                result.current_reputation
            );
        }
    }

    println!("\n✓ Tracked 15 signals (all successful)\n");

    // Track an anomalous signal
    let anomaly_result = prov_graph
        .track_signal(&onchain_source, 100.0, false, 500)
        .await?;

    println!("Anomaly Detection:");
    println!("  - Is Anomalous: {}", anomaly_result.is_anomalous);
    println!("  - Z-Score: {:.2}", anomaly_result.value_anomaly.z_score);
    println!("  - Reason: {}", anomaly_result.value_anomaly.reason);
    println!(
        "  - Current Reputation: {:.2}",
        anomaly_result.current_reputation
    );

    // Get graph statistics
    let graph_stats = prov_graph.get_stats().await;
    println!("\nProvenance Graph Statistics:");
    println!("  - Total Nodes: {}", graph_stats.total_nodes);
    println!("  - Total Edges: {}", graph_stats.total_edges);
    println!("  - Total Signals: {}", graph_stats.total_signals);
    println!("  - Anomalies: {}", graph_stats.anomalies_detected);
    println!("  - Avg Reputation: {:.2}", graph_stats.average_reputation);

    // ========================================================================
    // Part 3: Quantum Pruner Tool
    // ========================================================================
    println!("\n\n3️⃣  Quantum-Inspired Static Pruner Demo");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let pruner = PathPruner::new(0.01); // 1% threshold

    // Analyze the components directory
    let analysis = pruner.analyze_directory("src/components".as_ref())?;

    println!("Code Analysis Results:");
    println!("  - Files Analyzed: {}", analysis.files_analyzed);
    println!("  - Total Paths: {}", analysis.total_paths);
    println!(
        "  - Low-Probability Paths: {}",
        analysis.total_low_probability_paths
    );
    println!(
        "  - Pruning Potential: {:.1}%\n",
        (analysis.total_low_probability_paths as f64 / analysis.total_paths.max(1) as f64)
            * 100.0
    );

    // Get pruning suggestions
    let suggestions = pruner.get_suggestions(&analysis);

    if !suggestions.is_empty() {
        println!("Optimization Suggestions:");
        for (i, suggestion) in suggestions.iter().take(3).enumerate() {
            println!(
                "  {}. {}:{}",
                i + 1,
                suggestion.file.display(),
                suggestion.line
            );
            println!("     Type: {:?}", suggestion.suggestion_type);
            println!("     Impact: {:?}", suggestion.impact);
        }
    } else {
        println!("✓ No prunable paths found!");
    }

    // ========================================================================
    // Integration Summary
    // ========================================================================
    println!("\n\n4️⃣  Integration Summary");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("Universe-Grade Features Working Together:");
    println!("  ✓ Multi-Agent RL: Adaptive trading decisions");
    println!("  ✓ Provenance Graph: Signal source verification");
    println!("  ✓ Quantum Pruner: Code optimization analysis\n");

    println!("Combined Capabilities:");
    println!("  • Agents learn from {} episodes", stats.scout.total_episodes);
    println!("  • Tracking {} signal sources", graph_stats.total_nodes);
    println!("  • Analyzing {} code paths", analysis.total_paths);
    println!("  • Achieving {:.1}% reputation score", graph_stats.average_reputation * 100.0);
    println!("  • Discovering {:.1}% optimization potential\n", 
        (analysis.total_low_probability_paths as f64 / analysis.total_paths.max(1) as f64) * 100.0
    );

    println!("🎉 All features operational and integrated!");
    println!("\n=== Demo Complete ===\n");

    Ok(())
}
