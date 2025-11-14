//! Geyser gRPC streaming implementation (stub)
//!
//! This module is a placeholder for future Geyser gRPC streaming support.
//! Enable with the `geyser-stream` feature flag.
//!
//! ## Status
//!
//! Currently not implemented. Use WebSocket streaming (`ws-stream` feature) instead.

#![allow(dead_code)]

use solana_sdk::pubkey::Pubkey;

/// Geyser stream configuration
#[derive(Debug, Clone)]
pub struct GeyserConfig {
    pub endpoint: String,
    pub x_token: Option<String>,
}

/// Geyser stream client (stub)
pub struct GeyserStream {
    config: GeyserConfig,
}

impl GeyserStream {
    /// Create a new Geyser stream client
    pub fn new(config: GeyserConfig) -> Self {
        Self { config }
    }

    /// Connect to Geyser endpoint
    pub async fn connect(&self) -> Result<(), Box<dyn std::error::Error>> {
        todo!("Geyser gRPC streaming not yet implemented. Use ws-stream feature instead.")
    }

    /// Subscribe to program updates
    pub async fn subscribe_program(
        &self,
        _program_id: &Pubkey,
    ) -> Result<(), Box<dyn std::error::Error>> {
        todo!("Geyser gRPC streaming not yet implemented. Use ws-stream feature instead.")
    }
}
