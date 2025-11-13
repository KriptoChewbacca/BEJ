//! RPC Manager Module
//!
//! Enhanced RPC pooling with health checks, batching, and intelligent rotation

#![allow(unused_imports)] // Allow unused imports for re-exports that may not be used in all contexts

use anyhow::Result;
use solana_sdk::{signature::Signature, transaction::VersionedTransaction};
use std::future::Future;
use std::pin::Pin;

// Submodules
pub mod rpc_atomics;
pub mod rpc_config;
pub mod rpc_errors;
pub mod rpc_metrics;
pub mod rpc_pool;

// Re-exports for convenience
pub use rpc_errors::RpcManagerError;
pub use rpc_pool::{EndpointConfig, EndpointType, RpcPool};

/// Trait for RPC broadcasting functionality
pub trait RpcBroadcaster: Send + Sync + std::fmt::Debug {
    /// Send transactions to multiple RPC endpoints
    fn send_on_many_rpc<'a>(
        &'a self,
        txs: Vec<VersionedTransaction>,
        correlation_id: Option<crate::observability::CorrelationId>,
    ) -> Pin<Box<dyn Future<Output = Result<Signature>> + Send + 'a>>;
}
