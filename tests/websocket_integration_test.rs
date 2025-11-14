#![cfg(feature = "ws-stream")]

use bot::streaming::websocket_stream::WebSocketStream;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tokio::sync::mpsc;

#[tokio::test]
#[ignore] // Ignore by default as it requires network access
async fn test_websocket_connection() {
    let ws_url = "wss://api.devnet.solana.com";
    let stream = WebSocketStream::new(ws_url.to_string());

    let result = stream.connect().await;
    assert!(result.is_ok(), "Failed to connect to WebSocket: {:?}", result);
}

#[tokio::test]
#[ignore] // Ignore by default as it requires network access
async fn test_program_subscription() {
    // Use a known devnet program (Token Program)
    let program_id = Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap();
    
    let ws_url = "wss://api.devnet.solana.com";
    let stream = WebSocketStream::new(ws_url.to_string());

    // Connect
    let client = stream.connect().await.expect("Failed to connect");

    // Create channel for updates
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Subscribe to program
    let result = stream.subscribe_program(client, &program_id, tx).await;
    assert!(result.is_ok(), "Failed to subscribe to program: {:?}", result);

    // Wait briefly to see if we receive any updates (optional)
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        rx.recv()
    ).await.ok();
}

#[tokio::test]
#[ignore] // Ignore by default as it requires network access
async fn test_signature_subscription() {
    use solana_sdk::signature::Signature;
    use std::str::FromStr;
    
    // Use a test signature
    let sig = Signature::from_str("1111111111111111111111111111111111111111111111111111111111111111111111111111111111111111").unwrap();
    
    let ws_url = "wss://api.devnet.solana.com";
    let stream = WebSocketStream::new(ws_url.to_string());

    // Connect
    let client = stream.connect().await.expect("Failed to connect");

    // Create channel for updates
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Subscribe to signatures
    let result = stream.subscribe_signatures(client, &sig, tx).await;
    assert!(result.is_ok(), "Failed to subscribe to signatures: {:?}", result);

    // Wait briefly to see if we receive any updates (optional)
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        rx.recv()
    ).await.ok();
}

#[tokio::test]
async fn test_websocket_creation() {
    // Test that we can create a WebSocket stream without errors
    let ws_url = "wss://api.devnet.solana.com";
    let _stream = WebSocketStream::new(ws_url.to_string());
    // If we get here without panicking, test passes
}
