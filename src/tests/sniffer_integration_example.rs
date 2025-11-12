#![allow(unused_imports)]
//! Sniffer Integration Example
//!
//! This file demonstrates how to integrate the Sniffer module with buy_engine.rs
//! and provides usage examples for the ultra-lightweight Sniffer.

/*
use std::sync::Arc;
use tokio::sync::mpsc;
use anyhow::Result;

// Example integration with buy_engine
pub async fn example_sniffer_integration() -> Result<()> {
    // 1. Create Sniffer configuration
    let config = sniffer::SnifferConfig {
        grpc_endpoint: "http://your-geyser-endpoint:10000".to_string(),
        channel_capacity: 1024,
        max_retry_attempts: 5,
        initial_backoff_ms: 100,
        max_backoff_ms: 5000,
        telemetry_interval_secs: 5,
        ema_alpha_short: 0.2,
        ema_alpha_long: 0.05,
        initial_threshold: 1.5,
    };

    // 2. Create Sniffer instance
    let sniffer = sniffer::Sniffer::new(config);

    // 3. Start sniffing - returns receiver for candidates
    let candidate_rx = sniffer.start_sniff().await?;

    // 4. Create buy_engine with the receiver
    let buy_engine = BuyEngine::new(
        rpc_broadcaster,
        nonce_manager,
        candidate_rx,  // Use the receiver from sniffer
        app_state,
        config,
        Some(tx_builder),
    );

    // 5. Run buy_engine to consume candidates
    tokio::spawn(async move {
        buy_engine.run().await;
    });

    // 6. Monitor metrics
    let metrics = sniffer.get_metrics();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            let snapshot = metrics.snapshot();
            println!("Sniffer Metrics: {}", snapshot);
        }
    });

    // 7. Keep running until shutdown signal
    tokio::signal::ctrl_c().await?;

    // 8. Graceful shutdown
    sniffer.stop();

    Ok(())
}

// Example: Custom prefilter configuration
pub fn example_custom_prefilter() {
    // The prefilter module can be extended with custom patterns
    
    // Example: Add custom program ID detection
    const CUSTOM_PROGRAM_ID: [u8; 32] = [/* your program ID bytes */];
    
    fn contains_custom_program(tx_bytes: &[u8]) -> bool {
        tx_bytes.windows(32).any(|window| window == CUSTOM_PROGRAM_ID)
    }
    
    // Example: Custom filtering logic
    fn custom_should_process(tx_bytes: &[u8]) -> bool {
        // Fast rejection checks
        if tx_bytes.len() < 128 {
            return false;
        }
        
        // Custom business logic
        contains_custom_program(tx_bytes) 
            && prefilter::contains_spl_token(tx_bytes)
            && !prefilter::is_vote_tx(tx_bytes)
    }
}

// Example: Monitoring and alerting
pub async fn example_monitoring() -> Result<()> {
    let config = sniffer::SnifferConfig::default();
    let sniffer = sniffer::Sniffer::new(config);
    let _rx = sniffer.start_sniff().await?;
    
    let metrics = sniffer.get_metrics();
    
    // Alert on high drop rate
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            
            let tx_seen = metrics.tx_seen.load(std::sync::atomic::Ordering::Relaxed);
            let dropped = metrics.dropped_full_buffer.load(std::sync::atomic::Ordering::Relaxed);
            
            if tx_seen > 0 {
                let drop_rate = (dropped as f64 / tx_seen as f64) * 100.0;
                
                if drop_rate > 2.0 {
                    eprintln!("WARNING: Drop rate {:.2}% exceeds 2% threshold!", drop_rate);
                    // Send alert to monitoring system
                }
            }
        }
    });
    
    Ok(())
}

// Example: Testing with mock data
pub async fn example_testing() -> Result<()> {
    use tokio::sync::mpsc;
    
    // Create a channel
    let (tx, mut rx) = mpsc::channel(100);
    
    // Simulate sniffer sending candidates
    tokio::spawn(async move {
        for i in 0..10 {
            let candidate = sniffer::PremintCandidate::new(
                solana_sdk::pubkey::Pubkey::new_unique(),
                smallvec::SmallVec::new(),
                1.0 + (i as f64) * 0.1,
                i,
                sniffer::PriorityLevel::High,
            );
            
            tx.send(candidate).await.unwrap();
        }
    });
    
    // Simulate buy_engine consuming candidates
    while let Some(candidate) = rx.recv().await {
        println!("Received candidate: mint={}, trace_id={}, priority={:?}",
                 candidate.mint, candidate.trace_id, candidate.priority);
        
        // Process candidate...
    }
    
    Ok(())
}

// Example: Production deployment configuration
pub fn example_production_config() -> sniffer::SnifferConfig {
    sniffer::SnifferConfig {
        // Use production gRPC endpoint
        grpc_endpoint: std::env::var("GEYSER_ENDPOINT")
            .unwrap_or_else(|_| "http://127.0.0.1:10000".to_string()),
        
        // Larger buffer for production
        channel_capacity: 2048,
        
        // More aggressive retry
        max_retry_attempts: 10,
        initial_backoff_ms: 50,
        max_backoff_ms: 10000,
        
        // Frequent telemetry
        telemetry_interval_secs: 1,
        
        // Tuned EMA parameters
        ema_alpha_short: 0.3,  // More reactive
        ema_alpha_long: 0.03,   // Smoother baseline
        
        // Conservative threshold
        initial_threshold: 2.0,
    }
}

// Example: Performance profiling
pub async fn example_performance_profiling() -> Result<()> {
    let config = sniffer::SnifferConfig::default();
    let sniffer = sniffer::Sniffer::new(config);
    let _rx = sniffer.start_sniff().await?;
    
    let metrics = sniffer.get_metrics();
    let start = std::time::Instant::now();
    
    // Run for 60 seconds
    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    
    let elapsed = start.elapsed();
    let tx_seen = metrics.tx_seen.load(std::sync::atomic::Ordering::Relaxed);
    let candidates = metrics.candidates_sent.load(std::sync::atomic::Ordering::Relaxed);
    
    println!("Performance Report:");
    println!("  Duration: {:?}", elapsed);
    println!("  Transactions seen: {}", tx_seen);
    println!("  Candidates sent: {}", candidates);
    println!("  Throughput: {:.2} tx/s", tx_seen as f64 / elapsed.as_secs_f64());
    
    if tx_seen > 0 {
        let filter_rate = ((tx_seen - candidates) as f64 / tx_seen as f64) * 100.0;
        println!("  Filter rate: {:.2}%", filter_rate);
    }
    
    sniffer.stop();
    Ok(())
}
*/

// Integration checklist for buy_engine.rs:
//
// 1. Replace the existing candidate_rx channel creation with sniffer.start_sniff()
//
// 2. Update the type imports to include:
//    use crate::sniffer::{Sniffer, SnifferConfig, PremintCandidate};
//
// 3. In the main application, initialize sniffer before buy_engine:
//    let sniffer = Sniffer::new(SnifferConfig::default());
//    let candidate_rx = sniffer.start_sniff().await?;
//
// 4. Pass candidate_rx to BuyEngine::new() as usual
//
// 5. Monitor metrics via sniffer.get_metrics() for observability
//
// 6. Call sniffer.stop() on shutdown

// API compatibility notes:
//
// The Sniffer produces mpsc::Receiver<PremintCandidate>
// This is compatible with the existing CandidateReceiver type if defined as:
//   pub type CandidateReceiver = mpsc::Receiver<PremintCandidate>;
//
// The PremintCandidate struct matches the expected interface:
//   - mint: Pubkey
//   - accounts: SmallVec<[Pubkey; 8]>
//   - price_hint: f64
//   - trace_id: u64
//   - priority: PriorityLevel
