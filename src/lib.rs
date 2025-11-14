//! Ultra - Advanced Solana Trading Bot Library
//!
//! This library exposes core modules for testing and integration purposes.

// Re-export the compat module for testing
pub mod compat;

// Export metrics module
pub mod metrics;

// Export observability module
pub mod observability;

// Export the nonce_manager module (with path attribute for directory with space)
#[path = "nonce manager/mod.rs"]
pub mod nonce_manager;

// Export the rpc_manager module (with path attribute for directory with space)
#[path = "rpc manager/mod.rs"]
pub mod rpc_manager;

// Export the new modular tx_builder supercomponent
pub mod tx_builder;

// Export GUI integration components
pub mod components;

// Export position tracker module
pub mod position_tracker;

// Export types module
pub mod types;

// Export GUI module (only when gui_monitor feature is enabled)
#[cfg(feature = "gui_monitor")]
pub mod gui;

// Re-export commonly used types
pub use solana_sdk::{message::VersionedMessage, pubkey::Pubkey, signature::Signature};
