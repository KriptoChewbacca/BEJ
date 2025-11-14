//! Example demonstrating WebSocket streaming usage
//!
//! Run with: cargo run --example websocket_demo --features ws-stream

use bot::streaming::websocket_stream::{WebSocketStream, ProgramUpdate};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 WebSocket Streaming Demo");
    println!("============================\n");

    // Create WebSocket stream (using native Solana devnet)
    let ws_url = "wss://api.devnet.solana.com";
    println!("📡 Connecting to: {}", ws_url);
    
    let stream = WebSocketStream::new(ws_url.to_string());

    // Connect to WebSocket
    let client = stream.connect().await?;
    println!("✅ Connected successfully!\n");

    // Subscribe to Token Program updates (as an example)
    let token_program = Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")?;
    println!("👂 Subscribing to Token Program: {}", token_program);

    let (tx, mut rx) = mpsc::unbounded_channel::<ProgramUpdate>();

    // Subscribe
    stream.subscribe_program(client, &token_program, tx).await?;
    println!("✅ Subscription active!\n");
    println!("📊 Waiting for updates (Ctrl+C to stop)...\n");

    // Receive updates
    let mut count = 0;
    while let Some(update) = rx.recv().await {
        count += 1;
        println!(
            "📬 Update #{}: Account {} (slot: {}, data size: {} bytes)",
            count,
            update.pubkey,
            update.slot,
            update.account_data.len()
        );

        // Stop after 10 updates for demo
        if count >= 10 {
            println!("\n✅ Received 10 updates. Demo complete!");
            break;
        }
    }

    Ok(())
}
