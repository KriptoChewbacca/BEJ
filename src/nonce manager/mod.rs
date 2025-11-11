//! Nonce Manager Module
//!
//! Universe Class Grade Nonce Manager with enterprise features

// Submodules
pub mod nonce_errors;
pub mod nonce_retry;
pub mod nonce_circuit_breaker;
pub mod nonce_authority;
pub mod nonce_integration;
pub mod nonce_lease;
pub mod nonce_manager_integrated;
pub mod nonce_predictive;
pub mod nonce_refresh;
pub mod nonce_security;
pub mod nonce_signer;
pub mod nonce_telemetry;

// Re-exports for convenience
// Use the integrated manager as the primary implementation (Task 1)
pub use nonce_manager_integrated::UniverseNonceManager as NonceManager;

// ZkProofData is only available when zk_enabled feature is active
#[cfg(feature = "zk_enabled")]
pub use nonce_manager_integrated::ZkProofData;

pub use nonce_errors::NonceError;
pub use nonce_lease::NonceLease;
