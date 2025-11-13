//! Ultra - Advanced Solana Trading Bot Library
//!
//! This library exposes core modules for testing and integration purposes.

// Re-export the compat module for testing
pub mod compat;

// Export the new modular tx_builder supercomponent
pub mod tx_builder;

// Re-export commonly used types
pub use solana_sdk::{message::VersionedMessage, pubkey::Pubkey, signature::Signature};
