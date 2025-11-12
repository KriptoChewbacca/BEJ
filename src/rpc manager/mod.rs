//! RPC Manager Module
//!
//! Enhanced RPC pooling with health checks, batching, and intelligent rotation

use std::future::Future;
use std::pin::Pin;
use solana_sdk::{signature::Signature, transaction::VersionedTransaction};
use anyhow::Result;

// Submodules
pub mod rpc_pool;
pub mod rpc_config;
pub mod rpc_errors;
pub mod rpc_metrics;
pub mod rpc_atomics;

// Re-exports for convenience
pub use rpc_pool::EndpointConfig;
pub use rpc_pool::EndpointType;
pub use rpc_errors::RpcManagerError;

// Re-export RpcPool only for internal use
#[allow(unused_imports)]
pub(crate) use rpc_pool::RpcPool;

/// Trait for RPC broadcasting functionality
pub trait RpcBroadcaster: Send + Sync + std::fmt::Debug {
    /// Send transactions to multiple RPC endpoints
    fn send_on_many_rpc<'a>(
        &'a self,
        txs: Vec<VersionedTransaction>,
        correlation_id: Option<crate::observability::CorrelationId>,
    ) -> Pin<Box<dyn Future<Output = Result<Signature>> + Send + 'a>>;
}
