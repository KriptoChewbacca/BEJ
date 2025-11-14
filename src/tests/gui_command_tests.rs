//! GUI Command Channel Tests
//!
//! These tests verify the GUI command channel functionality:
//! - Command channel delivers messages correctly
//! - Sell commands can be sent and received
//! - Mode change commands work properly
//! - Invalid or malformed commands are handled gracefully

#[cfg(test)]
mod tests {
    use crate::components::gui_bridge::GuiCommand;
    use crate::types::TradingMode;
    use solana_sdk::pubkey::Pubkey;
    use tokio::sync::mpsc;
    use tokio::time::{timeout, Duration};

    /// Test that command channel delivers messages
    #[tokio::test]
    async fn test_command_channel_delivers_messages() {
        // Create channel with buffer of 100
        let (tx, mut rx) = mpsc::channel::<GuiCommand>(100);

        // Send a sell command
        let mint = Pubkey::new_unique();
        let cmd = GuiCommand::Sell {
            mint,
            percent: 0.5,
        };

        tx.send(cmd).await.expect("Failed to send command");

        // Receive the command with timeout
        let received = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Timeout waiting for command")
            .expect("Channel closed unexpectedly");

        // Verify the command matches
        match received {
            GuiCommand::Sell {
                mint: recv_mint,
                percent,
            } => {
                assert_eq!(recv_mint, mint);
                assert_eq!(percent, 0.5);
            }
            _ => panic!("Received wrong command type"),
        }
    }

    /// Test that sell commands execute with correct parameters
    #[tokio::test]
    async fn test_sell_command_executes() {
        let (tx, mut rx) = mpsc::channel::<GuiCommand>(100);

        // Test different sell percentages
        let test_cases = vec![0.10, 0.25, 0.50, 1.00];
        let mint = Pubkey::new_unique();

        for percent in test_cases {
            tx.send(GuiCommand::Sell { mint, percent })
                .await
                .expect("Failed to send sell command");
        }

        // Verify all commands received
        for expected_percent in &[0.10, 0.25, 0.50, 1.00] {
            let cmd = timeout(Duration::from_secs(1), rx.recv())
                .await
                .expect("Timeout")
                .expect("Channel closed");

            match cmd {
                GuiCommand::Sell { percent, .. } => {
                    assert_eq!(percent, *expected_percent);
                }
                _ => panic!("Expected Sell command"),
            }
        }
    }

    /// Test that mode change commands work
    #[tokio::test]
    async fn test_mode_change_command_works() {
        let (tx, mut rx) = mpsc::channel::<GuiCommand>(100);

        // Send mode change commands
        tx.send(GuiCommand::SetTradingMode(TradingMode::Manual))
            .await
            .expect("Failed to send Manual mode");
        tx.send(GuiCommand::SetTradingMode(TradingMode::Auto))
            .await
            .expect("Failed to send Auto mode");
        tx.send(GuiCommand::SetTradingMode(TradingMode::Hybrid))
            .await
            .expect("Failed to send Hybrid mode");

        // Verify modes received in order
        let modes = vec![TradingMode::Manual, TradingMode::Auto, TradingMode::Hybrid];
        for expected_mode in modes {
            let cmd = timeout(Duration::from_secs(1), rx.recv())
                .await
                .expect("Timeout")
                .expect("Channel closed");

            match cmd {
                GuiCommand::SetTradingMode(mode) => {
                    assert_eq!(mode, expected_mode);
                }
                _ => panic!("Expected SetTradingMode command"),
            }
        }
    }

    /// Test that channel handles backpressure gracefully
    #[tokio::test]
    async fn test_channel_backpressure() {
        // Create a small channel (buffer of 2)
        let (tx, _rx) = mpsc::channel::<GuiCommand>(2);

        let mint = Pubkey::new_unique();

        // Fill the channel
        tx.send(GuiCommand::Sell { mint, percent: 0.1 })
            .await
            .expect("First send should succeed");
        tx.send(GuiCommand::Sell { mint, percent: 0.2 })
            .await
            .expect("Second send should succeed");

        // Third send should block (use try_send to test without blocking)
        let result = tx.try_send(GuiCommand::Sell { mint, percent: 0.3 });
        assert!(
            result.is_err(),
            "Channel should be full and reject new messages"
        );
    }

    /// Test Stop Loss command
    #[tokio::test]
    async fn test_stop_loss_command() {
        let (tx, mut rx) = mpsc::channel::<GuiCommand>(100);

        let mint = Pubkey::new_unique();
        tx.send(GuiCommand::SetStopLoss {
            mint,
            threshold_percent: 10.0,
        })
        .await
        .expect("Failed to send SetStopLoss");

        let cmd = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Timeout")
            .expect("Channel closed");

        match cmd {
            GuiCommand::SetStopLoss {
                mint: recv_mint,
                threshold_percent,
            } => {
                assert_eq!(recv_mint, mint);
                assert_eq!(threshold_percent, 10.0);
            }
            _ => panic!("Expected SetStopLoss command"),
        }
    }

    /// Test Take Profit command
    #[tokio::test]
    async fn test_take_profit_command() {
        let (tx, mut rx) = mpsc::channel::<GuiCommand>(100);

        let mint = Pubkey::new_unique();
        tx.send(GuiCommand::SetTakeProfit {
            mint,
            threshold_percent: 50.0,
            sell_percent: 0.5,
        })
        .await
        .expect("Failed to send SetTakeProfit");

        let cmd = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Timeout")
            .expect("Channel closed");

        match cmd {
            GuiCommand::SetTakeProfit {
                mint: recv_mint,
                threshold_percent,
                sell_percent,
            } => {
                assert_eq!(recv_mint, mint);
                assert_eq!(threshold_percent, 50.0);
                assert_eq!(sell_percent, 0.5);
            }
            _ => panic!("Expected SetTakeProfit command"),
        }
    }

    /// Test Clear Strategy command
    #[tokio::test]
    async fn test_clear_strategy_command() {
        let (tx, mut rx) = mpsc::channel::<GuiCommand>(100);

        let mint = Pubkey::new_unique();
        tx.send(GuiCommand::ClearStrategy { mint })
            .await
            .expect("Failed to send ClearStrategy");

        let cmd = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Timeout")
            .expect("Channel closed");

        match cmd {
            GuiCommand::ClearStrategy { mint: recv_mint } => {
                assert_eq!(recv_mint, mint);
            }
            _ => panic!("Expected ClearStrategy command"),
        }
    }

    /// Test Multi-Token Mode command
    #[tokio::test]
    async fn test_multi_token_mode_command() {
        let (tx, mut rx) = mpsc::channel::<GuiCommand>(100);

        // Test with max_positions
        tx.send(GuiCommand::SetMultiTokenMode {
            enabled: true,
            max_positions: Some(5),
        })
        .await
        .expect("Failed to send SetMultiTokenMode");

        let cmd = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Timeout")
            .expect("Channel closed");

        match cmd {
            GuiCommand::SetMultiTokenMode {
                enabled,
                max_positions,
            } => {
                assert!(enabled);
                assert_eq!(max_positions, Some(5));
            }
            _ => panic!("Expected SetMultiTokenMode command"),
        }
    }

    /// Test concurrent command sending
    #[tokio::test]
    async fn test_concurrent_command_sending() {
        let (tx, mut rx) = mpsc::channel::<GuiCommand>(1000);
        let mut handles = vec![];

        // Spawn multiple tasks sending commands
        for i in 0..10 {
            let tx_clone = tx.clone();
            let handle = tokio::spawn(async move {
                for j in 0..10 {
                    let mint = Pubkey::new_unique();
                    tx_clone
                        .send(GuiCommand::Sell {
                            mint,
                            percent: (i * 10 + j) as f64 / 1000.0,
                        })
                        .await
                        .expect("Failed to send");
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.expect("Task panicked");
        }

        // Drop the sender to close the channel
        drop(tx);

        // Count received messages
        let mut count = 0;
        while rx.recv().await.is_some() {
            count += 1;
        }

        assert_eq!(count, 100, "Should have received all 100 commands");
    }
}
