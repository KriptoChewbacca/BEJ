///! Non-blocking nonce refresh with background monitoring
///!
///! This module provides:
///! - Non-blocking transaction sending
///! - Background signature monitoring
///! - Telemetry collection for refresh operations
///! - Automatic slot updates on confirmation

use super::nonce_errors::{NonceError, NonceResult};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    pubkey::Pubkey,
    signature::Signature,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, warn};

/// Refresh telemetry data
#[derive(Debug, Clone)]
pub struct RefreshTelemetry {
    pub nonce_account: Pubkey,
    pub signature: Signature,
    pub started_at: Instant,
    pub confirmed_at: Option<Instant>,
    pub attempts: u32,
    pub endpoint: String,
    pub success: bool,
    pub error: Option<String>,
}

impl RefreshTelemetry {
    pub fn latency_ms(&self) -> Option<u64> {
        self.confirmed_at
            .map(|confirmed| confirmed.duration_since(self.started_at).as_millis() as u64)
    }
}

/// Status of a refresh operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshStatus {
    Pending,
    Confirmed,
    Failed(String),
    Timeout,
}

/// Result of a signature monitoring operation
pub struct MonitorResult {
    pub signature: Signature,
    pub status: RefreshStatus,
    pub last_valid_slot: Option<u64>,
    pub telemetry: RefreshTelemetry,
}

/// Background signature monitor
pub struct SignatureMonitor {
    rpc_client: Arc<RpcClient>,
    endpoint: String,
    check_interval: Duration,
    timeout: Duration,
    results_tx: mpsc::UnboundedSender<MonitorResult>,
}

impl SignatureMonitor {
    pub fn new(
        rpc_client: Arc<RpcClient>,
        endpoint: String,
        check_interval: Duration,
        timeout: Duration,
        results_tx: mpsc::UnboundedSender<MonitorResult>,
    ) -> Self {
        Self {
            rpc_client,
            endpoint,
            check_interval,
            timeout,
            results_tx,
        }
    }
    
    /// Monitor a signature in the background
    pub fn monitor(
        self,
        signature: Signature,
        nonce_account: Pubkey,
        started_at: Instant,
    ) {
        tokio::spawn(async move {
            self.monitor_internal(signature, nonce_account, started_at).await;
        });
    }
    
    async fn monitor_internal(
        &self,
        signature: Signature,
        nonce_account: Pubkey,
        started_at: Instant,
    ) {
        debug!(
            signature = %signature,
            nonce_account = %nonce_account,
            "Starting signature monitoring"
        );
        
        let mut attempts = 0;
        let mut interval = tokio::time::interval(self.check_interval);
        
        loop {
            interval.tick().await;
            attempts += 1;
            
            // Check for timeout
            if started_at.elapsed() >= self.timeout {
                warn!(
                    signature = %signature,
                    attempts = attempts,
                    timeout_sec = self.timeout.as_secs(),
                    "Signature monitoring timed out"
                );
                
                let telemetry = RefreshTelemetry {
                    nonce_account,
                    signature,
                    started_at,
                    confirmed_at: None,
                    attempts,
                    endpoint: self.endpoint.clone(),
                    success: false,
                    error: Some("Timeout".to_string()),
                };
                
                let _ = self.results_tx.send(MonitorResult {
                    signature,
                    status: RefreshStatus::Timeout,
                    last_valid_slot: None,
                    telemetry,
                });
                
                return;
            }
            
            // Check signature status
            match self.rpc_client.get_signature_status(&signature).await {
                Ok(Some(status)) => {
                    if let Some(err) = status.err {
                        error!(
                            signature = %signature,
                            error = ?err,
                            "Transaction failed"
                        );
                        
                        let telemetry = RefreshTelemetry {
                            nonce_account,
                            signature,
                            started_at,
                            confirmed_at: Some(Instant::now()),
                            attempts,
                            endpoint: self.endpoint.clone(),
                            success: false,
                            error: Some(err.to_string()),
                        };
                        
                        let _ = self.results_tx.send(MonitorResult {
                            signature,
                            status: RefreshStatus::Failed(err.to_string()),
                            last_valid_slot: None,
                            telemetry,
                        });
                        
                        return;
                    }
                    
                    // Check if confirmed
                    if status.satisfies_commitment(CommitmentConfig {
                        commitment: CommitmentLevel::Confirmed,
                    }) {
                        debug!(
                            signature = %signature,
                            attempts = attempts,
                            latency_ms = started_at.elapsed().as_millis(),
                            "Transaction confirmed"
                        );
                        
                        // Try to get the updated slot info
                        let last_valid_slot = self.get_nonce_last_valid_slot(nonce_account).await;
                        
                        let telemetry = RefreshTelemetry {
                            nonce_account,
                            signature,
                            started_at,
                            confirmed_at: Some(Instant::now()),
                            attempts,
                            endpoint: self.endpoint.clone(),
                            success: true,
                            error: None,
                        };
                        
                        let _ = self.results_tx.send(MonitorResult {
                            signature,
                            status: RefreshStatus::Confirmed,
                            last_valid_slot,
                            telemetry,
                        });
                        
                        return;
                    }
                }
                Ok(None) => {
                    // Signature not found yet, keep waiting
                    debug!(
                        signature = %signature,
                        attempts = attempts,
                        "Signature not found, continuing to monitor"
                    );
                }
                Err(err) => {
                    warn!(
                        signature = %signature,
                        error = %err,
                        "Error checking signature status"
                    );
                    // Continue monitoring despite errors
                }
            }
        }
    }
    
    async fn get_nonce_last_valid_slot(&self, nonce_account: Pubkey) -> Option<u64> {
        match self.rpc_client.get_account(&nonce_account).await {
            Ok(account) => {
                match solana_sdk::nonce::State::from_account(&account) {
                    Ok(state) => {
                        Some(state.last_valid_slot())
                    }
                    Err(err) => {
                        warn!(
                            nonce_account = %nonce_account,
                            error = %err,
                            "Failed to parse nonce state"
                        );
                        None
                    }
                }
            }
            Err(err) => {
                warn!(
                    nonce_account = %nonce_account,
                    error = %err,
                    "Failed to fetch nonce account"
                );
                None
            }
        }
    }
}

/// Non-blocking refresh manager
pub struct NonBlockingRefresh {
    results_rx: Arc<RwLock<mpsc::UnboundedReceiver<MonitorResult>>>,
    results_tx: mpsc::UnboundedSender<MonitorResult>,
}

impl NonBlockingRefresh {
    pub fn new() -> Self {
        let (results_tx, results_rx) = mpsc::unbounded_channel();
        Self {
            results_rx: Arc::new(RwLock::new(results_rx)),
            results_tx,
        }
    }
    
    /// Send a transaction without blocking for confirmation
    pub async fn send_refresh_transaction(
        &self,
        rpc_client: Arc<RpcClient>,
        endpoint: String,
        transaction: &solana_sdk::transaction::Transaction,
        nonce_account: Pubkey,
    ) -> NonceResult<Signature> {
        let started_at = Instant::now();
        
        // Send transaction
        let signature = rpc_client
            .send_transaction(transaction)
            .await
            .map_err(|e| NonceError::Rpc {
                endpoint: Some(endpoint.clone()),
                message: e.to_string(),
            })?;
        
        debug!(
            signature = %signature,
            nonce_account = %nonce_account,
            "Refresh transaction sent, monitoring in background"
        );
        
        // Start background monitoring
        let monitor = SignatureMonitor::new(
            rpc_client,
            endpoint,
            Duration::from_millis(500), // Check every 500ms
            Duration::from_secs(60),    // Timeout after 60 seconds
            self.results_tx.clone(),
        );
        
        monitor.monitor(signature, nonce_account, started_at);
        
        Ok(signature)
    }
    
    /// Get the next completed refresh result (non-blocking)
    pub async fn try_recv_result(&self) -> Option<MonitorResult> {
        self.results_rx.write().await.recv().await
    }
    
    /// Process pending refresh results
    pub async fn process_results<F>(&self, mut handler: F)
    where
        F: FnMut(MonitorResult),
    {
        let mut rx = self.results_rx.write().await;
        
        // Drain all available results
        while let Ok(result) = rx.try_recv() {
            handler(result);
        }
    }
}

impl Default for NonBlockingRefresh {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_refresh_telemetry_latency() {
        let start = Instant::now();
        std::thread::sleep(Duration::from_millis(10));
        let end = Instant::now();
        
        let telemetry = RefreshTelemetry {
            nonce_account: Pubkey::new_unique(),
            signature: Signature::default(),
            started_at: start,
            confirmed_at: Some(end),
            attempts: 1,
            endpoint: "test".to_string(),
            success: true,
            error: None,
        };
        
        let latency = telemetry.latency_ms();
        assert!(latency.is_some());
        assert!(latency.unwrap() >= 10);
    }
    
    #[test]
    fn test_refresh_status_equality() {
        assert_eq!(RefreshStatus::Pending, RefreshStatus::Pending);
        assert_eq!(RefreshStatus::Confirmed, RefreshStatus::Confirmed);
        assert_eq!(RefreshStatus::Timeout, RefreshStatus::Timeout);
        assert_ne!(RefreshStatus::Pending, RefreshStatus::Confirmed);
    }
    
    #[tokio::test]
    async fn test_non_blocking_refresh_creation() {
        let refresh = NonBlockingRefresh::new();
        
        // Should be able to try receiving without blocking
        let result = tokio::time::timeout(
            Duration::from_millis(10),
            refresh.try_recv_result()
        ).await;
        
        // Should timeout because there are no results
        assert!(result.is_err());
    }
}
