//! Authority rotation and multisig management for nonce accounts
//! 
//! This module implements Step 2 requirements:
//! - Authority rotation process (proposal → execute → commit → finalize)
//! - Audit logging for each step
//! - Multisig/timelock support for critical operations
//! - Explicit rollback path for failed rotations
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    system_instruction,
    transaction::Transaction,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{info, warn, instrument};
use serde::{Deserialize, Serialize};

use super::nonce_errors::{NonceError, NonceResult};

/// Authority rotation state machine
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationState {
    /// No rotation in progress
    Idle,
    
    /// Rotation proposed, awaiting approval
    Proposed {
        proposal_id: String,
        proposed_at: SystemTime,
    },
    
    /// Rotation approved, ready to execute
    Approved {
        proposal_id: String,
        approved_at: SystemTime,
        approvers: Vec<Pubkey>,
    },
    
    /// Rotation transaction sent, awaiting confirmation
    Executing {
        proposal_id: String,
        signature: Signature,
        sent_at: SystemTime,
    },
    
    /// Rotation confirmed on-chain
    Committed {
        proposal_id: String,
        confirmed_at: SystemTime,
        signature: Signature,
    },
    
    /// Rotation finalized and complete
    Finalized {
        proposal_id: String,
        finalized_at: SystemTime,
    },
    
    /// Rotation failed, can be rolled back
    Failed {
        proposal_id: String,
        error: String,
        failed_at: SystemTime,
    },
    
    /// Rotation rolled back
    RolledBack {
        proposal_id: String,
        rolled_back_at: SystemTime,
        reason: String,
    },
}

/// Authority rotation proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationProposal {
    pub id: String,
    pub nonce_account: Pubkey,
    pub current_authority: Pubkey,
    pub new_authority: Pubkey,
    pub created_at: SystemTime,
    pub created_by: Pubkey,
    pub reason: String,
    pub state: RotationState,
    pub approvals: Vec<Approval>,
    pub required_approvals: usize,
    pub timelock_duration: Option<Duration>,
}

/// Approval from a signer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub approver: Pubkey,
    pub signature: String,
    pub approved_at: SystemTime,
}

/// Audit log entry for rotation events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationAuditLog {
    pub proposal_id: String,
    pub event_type: RotationEventType,
    pub timestamp: SystemTime,
    pub actor: Pubkey,
    pub details: String,
    pub signature_history: Vec<Signature>,
    pub endpoints_used: Vec<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RotationEventType {
    Proposed,
    Approved,
    Executed,
    Confirmed,
    Finalized,
    Failed,
    RolledBack,
}

/// Authority rotation manager
#[derive(Debug)]
pub struct AuthorityRotationManager {
    proposals: Arc<RwLock<Vec<RotationProposal>>>,
    audit_log: Arc<RwLock<Vec<RotationAuditLog>>>,
    multisig_enabled: bool,
    required_approvals: usize,
    timelock_duration: Option<Duration>,
}

impl AuthorityRotationManager {
    /// Create a new rotation manager
    pub fn new(
        multisig_enabled: bool,
        required_approvals: usize,
        timelock_duration: Option<Duration>,
    ) -> Self {
        Self {
            proposals: Arc::new(RwLock::new(Vec::new())),
            audit_log: Arc::new(RwLock::new(Vec::new())),
            multisig_enabled,
            required_approvals,
            timelock_duration,
        }
    }
    
    /// Step 1: Propose a new authority rotation
    #[instrument(skip(self))]
    pub async fn propose_rotation(
        &self,
        nonce_account: Pubkey,
        current_authority: Pubkey,
        new_authority: Pubkey,
        proposed_by: Pubkey,
        reason: String,
    ) -> NonceResult<String> {
        let proposal_id = uuid::Uuid::new_v4().to_string();
        let now = SystemTime::now();
        
        let proposal = RotationProposal {
            id: proposal_id.clone(),
            nonce_account,
            current_authority,
            new_authority,
            created_at: now,
            created_by: proposed_by,
            reason: reason.clone(),
            state: RotationState::Proposed {
                proposal_id: proposal_id.clone(),
                proposed_at: now,
            },
            approvals: Vec::new(),
            required_approvals: if self.multisig_enabled {
                self.required_approvals
            } else {
                1
            },
            timelock_duration: self.timelock_duration,
        };
        
        // Add to proposals
        self.proposals.write().await.push(proposal);
        
        // Log event
        self.log_event(RotationAuditLog {
            proposal_id: proposal_id.clone(),
            event_type: RotationEventType::Proposed,
            timestamp: now,
            actor: proposed_by,
            details: format!(
                "Proposed rotation for nonce {} from {} to {}: {}",
                nonce_account, current_authority, new_authority, reason
            ),
            signature_history: vec![],
            endpoints_used: vec![],
            duration_ms: None,
        }).await;
        
        info!(
            proposal_id = %proposal_id,
            nonce_account = %nonce_account,
            "Authority rotation proposed"
        );
        
        Ok(proposal_id)
    }
    
    /// Step 2: Approve a rotation proposal
    #[instrument(skip(self, approver_signature))]
    pub async fn approve_rotation(
        &self,
        proposal_id: &str,
        approver: Pubkey,
        approver_signature: String,
    ) -> NonceResult<()> {
        let mut proposals = self.proposals.write().await;
        let proposal = proposals
            .iter_mut()
            .find(|p| p.id == proposal_id)
            .ok_or_else(|| NonceError::Internal(format!("Proposal {} not found", proposal_id)))?;
        
        // Verify state
        match &proposal.state {
            RotationState::Proposed { .. } | RotationState::Approved { .. } => {}
            _ => {
                return Err(NonceError::Internal(format!(
                    "Proposal {} is in state {:?}, cannot approve",
                    proposal_id, proposal.state
                )));
            }
        }
        
        // Check if already approved by this signer
        if proposal.approvals.iter().any(|a| a.approver == approver) {
            return Err(NonceError::Internal(format!(
                "Approver {} has already approved proposal {}",
                approver, proposal_id
            )));
        }
        
        // Add approval
        let now = SystemTime::now();
        proposal.approvals.push(Approval {
            approver,
            signature: approver_signature,
            approved_at: now,
        });
        
        // Check if we have enough approvals
        if proposal.approvals.len() >= proposal.required_approvals {
            proposal.state = RotationState::Approved {
                proposal_id: proposal_id.to_string(),
                approved_at: now,
                approvers: proposal.approvals.iter().map(|a| a.approver).collect(),
            };
            
            // Log approval
            self.log_event(RotationAuditLog {
                proposal_id: proposal_id.to_string(),
                event_type: RotationEventType::Approved,
                timestamp: now,
                actor: approver,
                details: format!(
                    "Proposal approved ({}/{} approvals)",
                    proposal.approvals.len(),
                    proposal.required_approvals
                ),
                signature_history: vec![],
                endpoints_used: vec![],
                duration_ms: None,
            }).await;
            
            info!(
                proposal_id = %proposal_id,
                approvals = proposal.approvals.len(),
                "Rotation proposal approved"
            );
        }
        
        Ok(())
    }
    
    /// Step 3: Execute approved rotation on-chain
    #[instrument(skip(self, rpc_client, current_authority_keypair))]
    pub async fn execute_rotation(
        &self,
        proposal_id: &str,
        rpc_client: &RpcClient,
        current_authority_keypair: &Keypair,
        endpoint_url: String,
    ) -> NonceResult<Signature> {
        let start = std::time::Instant::now();
        let mut proposals = self.proposals.write().await;
        let proposal = proposals
            .iter_mut()
            .find(|p| p.id == proposal_id)
            .ok_or_else(|| NonceError::Internal(format!("Proposal {} not found", proposal_id)))?;
        
        // Verify state
        match &proposal.state {
            RotationState::Approved { approved_at, .. } => {
                // Check timelock if enabled
                if let Some(timelock) = proposal.timelock_duration {
                    let elapsed = SystemTime::now().duration_since(*approved_at)
                        .map_err(|e| NonceError::Internal(format!("Time error: {}", e)))?;
                    
                    if elapsed < timelock {
                        return Err(NonceError::Internal(format!(
                            "Timelock not expired: {:?} remaining",
                            timelock - elapsed
                        )));
                    }
                }
            }
            _ => {
                return Err(NonceError::Internal(format!(
                    "Proposal {} is in state {:?}, cannot execute",
                    proposal_id, proposal.state
                )));
            }
        }
        
        // Build authority change transaction
        let ix = system_instruction::authorize_nonce_account(
            &proposal.nonce_account,
            &current_authority_keypair.pubkey(),
            &proposal.new_authority,
        );
        
        let mut tx = Transaction::new_with_payer(
            &[ix],
            Some(&current_authority_keypair.pubkey()),
        );
        
        let blockhash = rpc_client.get_latest_blockhash().await
            .map_err(|e| NonceError::from_client_error(e, Some(endpoint_url.clone())))?;
        
        tx.sign(&[current_authority_keypair], blockhash);
        
        // Send transaction
        let signature = rpc_client.send_transaction(&tx).await
            .map_err(|e| NonceError::from_client_error(e, Some(endpoint_url.clone())))?;
        
        // Update state
        let now = SystemTime::now();
        proposal.state = RotationState::Executing {
            proposal_id: proposal_id.to_string(),
            signature,
            sent_at: now,
        };
        
        // Log event
        self.log_event(RotationAuditLog {
            proposal_id: proposal_id.to_string(),
            event_type: RotationEventType::Executed,
            timestamp: now,
            actor: current_authority_keypair.pubkey(),
            details: format!("Transaction sent: {}", signature),
            signature_history: vec![signature],
            endpoints_used: vec![endpoint_url],
            duration_ms: Some(start.elapsed().as_millis() as u64),
        }).await;
        
        info!(
            proposal_id = %proposal_id,
            signature = %signature,
            "Authority rotation transaction executed"
        );
        
        Ok(signature)
    }
    
    /// Step 4: Confirm rotation completion
    #[instrument(skip(self, rpc_client))]
    pub async fn confirm_rotation(
        &self,
        proposal_id: &str,
        rpc_client: &RpcClient,
    ) -> NonceResult<()> {
        let mut proposals = self.proposals.write().await;
        let proposal = proposals
            .iter_mut()
            .find(|p| p.id == proposal_id)
            .ok_or_else(|| NonceError::Internal(format!("Proposal {} not found", proposal_id)))?;
        
        // Verify state
        let signature = match &proposal.state {
            RotationState::Executing { signature, .. } => *signature,
            _ => {
                return Err(NonceError::Internal(format!(
                    "Proposal {} is in state {:?}, cannot confirm",
                    proposal_id, proposal.state
                )));
            }
        };
        
        // Check transaction status
        let status = rpc_client.get_signature_status(&signature).await
            .map_err(|e| NonceError::Rpc {
                endpoint: None,
                message: e.to_string(),
            })?;
        
        match status {
            Some(result) => {
                if let Some(err) = result.err() {
                    // Transaction failed
                    let now = SystemTime::now();
                    proposal.state = RotationState::Failed {
                        proposal_id: proposal_id.to_string(),
                        error: err.to_string(),
                        failed_at: now,
                    };
                    
                    self.log_event(RotationAuditLog {
                        proposal_id: proposal_id.to_string(),
                        event_type: RotationEventType::Failed,
                        timestamp: now,
                        actor: proposal.current_authority,
                        details: format!("Transaction failed: {}", err),
                        signature_history: vec![signature],
                        endpoints_used: vec![],
                        duration_ms: None,
                    }).await;
                    
                    return Err(NonceError::AdvanceFailed(
                        proposal.nonce_account,
                        err.to_string(),
                    ));
                }
                // Transaction succeeded
                let now = SystemTime::now();
                proposal.state = RotationState::Committed {
                    proposal_id: proposal_id.to_string(),
                    confirmed_at: now,
                    signature,
                };
                
                self.log_event(RotationAuditLog {
                    proposal_id: proposal_id.to_string(),
                    event_type: RotationEventType::Confirmed,
                    timestamp: now,
                    actor: proposal.current_authority,
                    details: "Transaction confirmed on-chain".to_string(),
                    signature_history: vec![signature],
                    endpoints_used: vec![],
                    duration_ms: None,
                }).await;
                
                info!(
                    proposal_id = %proposal_id,
                    signature = %signature,
                    "Authority rotation confirmed"
                );
            }
            None => {
                return Err(NonceError::ConfirmationFailed(
                    "Signature status not found".to_string()
                ));
            }
        }
        
        Ok(())
    }
    
    /// Step 5: Finalize rotation
    #[instrument(skip(self))]
    pub async fn finalize_rotation(&self, proposal_id: &str) -> NonceResult<()> {
        let mut proposals = self.proposals.write().await;
        let proposal = proposals
            .iter_mut()
            .find(|p| p.id == proposal_id)
            .ok_or_else(|| NonceError::Internal(format!("Proposal {} not found", proposal_id)))?;
        
        // Verify state
        match &proposal.state {
            RotationState::Committed { .. } => {}
            _ => {
                return Err(NonceError::Internal(format!(
                    "Proposal {} is in state {:?}, cannot finalize",
                    proposal_id, proposal.state
                )));
            }
        }
        
        // Finalize
        let now = SystemTime::now();
        proposal.state = RotationState::Finalized {
            proposal_id: proposal_id.to_string(),
            finalized_at: now,
        };
        
        self.log_event(RotationAuditLog {
            proposal_id: proposal_id.to_string(),
            event_type: RotationEventType::Finalized,
            timestamp: now,
            actor: proposal.new_authority,
            details: "Rotation finalized".to_string(),
            signature_history: vec![],
            endpoints_used: vec![],
            duration_ms: None,
        }).await;
        
        info!(
            proposal_id = %proposal_id,
            "Authority rotation finalized"
        );
        
        Ok(())
    }
    
    /// Rollback a failed rotation
    #[instrument(skip(self))]
    pub async fn rollback_rotation(
        &self,
        proposal_id: &str,
        reason: String,
    ) -> NonceResult<()> {
        let mut proposals = self.proposals.write().await;
        let proposal = proposals
            .iter_mut()
            .find(|p| p.id == proposal_id)
            .ok_or_else(|| NonceError::Internal(format!("Proposal {} not found", proposal_id)))?;
        
        // Can only rollback from Failed state
        match &proposal.state {
            RotationState::Failed { .. } => {}
            _ => {
                return Err(NonceError::Internal(format!(
                    "Proposal {} is in state {:?}, cannot rollback",
                    proposal_id, proposal.state
                )));
            }
        }
        
        let now = SystemTime::now();
        proposal.state = RotationState::RolledBack {
            proposal_id: proposal_id.to_string(),
            rolled_back_at: now,
            reason: reason.clone(),
        };
        
        self.log_event(RotationAuditLog {
            proposal_id: proposal_id.to_string(),
            event_type: RotationEventType::RolledBack,
            timestamp: now,
            actor: proposal.current_authority,
            details: format!("Rotation rolled back: {}", reason),
            signature_history: vec![],
            endpoints_used: vec![],
            duration_ms: None,
        }).await;
        
        warn!(
            proposal_id = %proposal_id,
            reason = %reason,
            "Authority rotation rolled back"
        );
        
        Ok(())
    }
    
    /// Get rotation status
    pub async fn get_rotation_status(&self, proposal_id: &str) -> Option<RotationState> {
        let proposals = self.proposals.read().await;
        proposals
            .iter()
            .find(|p| p.id == proposal_id)
            .map(|p| p.state.clone())
    }
    
    /// Get audit log for a proposal
    pub async fn get_audit_log(&self, proposal_id: &str) -> Vec<RotationAuditLog> {
        let log = self.audit_log.read().await;
        log.iter()
            .filter(|entry| entry.proposal_id == proposal_id)
            .cloned()
            .collect()
    }
    
    /// Get all audit logs
    pub async fn get_all_audit_logs(&self) -> Vec<RotationAuditLog> {
        self.audit_log.read().await.clone()
    }
    
    /// Internal: Log an event
    async fn log_event(&self, entry: RotationAuditLog) {
        self.audit_log.write().await.push(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_rotation_proposal() {
        let manager = AuthorityRotationManager::new(false, 1, None);
        
        let nonce_account = Pubkey::new_unique();
        let current_authority = Pubkey::new_unique();
        let new_authority = Pubkey::new_unique();
        let proposed_by = Pubkey::new_unique();
        
        let proposal_id = manager.propose_rotation(
            nonce_account,
            current_authority,
            new_authority,
            proposed_by,
            "Test rotation".to_string(),
        ).await.unwrap();
        
        let status = manager.get_rotation_status(&proposal_id).await;
        assert!(matches!(status, Some(RotationState::Proposed { .. })));
    }
    
    #[tokio::test]
    async fn test_rotation_approval() {
        let manager = AuthorityRotationManager::new(true, 2, None);
        
        let nonce_account = Pubkey::new_unique();
        let current_authority = Pubkey::new_unique();
        let new_authority = Pubkey::new_unique();
        let proposed_by = Pubkey::new_unique();
        
        let proposal_id = manager.propose_rotation(
            nonce_account,
            current_authority,
            new_authority,
            proposed_by,
            "Test rotation".to_string(),
        ).await.unwrap();
        
        // First approval
        let approver1 = Pubkey::new_unique();
        manager.approve_rotation(&proposal_id, approver1, "sig1".to_string()).await.unwrap();
        
        let status = manager.get_rotation_status(&proposal_id).await;
        assert!(matches!(status, Some(RotationState::Proposed { .. })));
        
        // Second approval should move to Approved state
        let approver2 = Pubkey::new_unique();
        manager.approve_rotation(&proposal_id, approver2, "sig2".to_string()).await.unwrap();
        
        let status = manager.get_rotation_status(&proposal_id).await;
        assert!(matches!(status, Some(RotationState::Approved { .. })));
    }
    
    #[tokio::test]
    async fn test_audit_log() {
        let manager = AuthorityRotationManager::new(false, 1, None);
        
        let nonce_account = Pubkey::new_unique();
        let current_authority = Pubkey::new_unique();
        let new_authority = Pubkey::new_unique();
        let proposed_by = Pubkey::new_unique();
        
        let proposal_id = manager.propose_rotation(
            nonce_account,
            current_authority,
            new_authority,
            proposed_by,
            "Test rotation".to_string(),
        ).await.unwrap();
        
        let audit_log = manager.get_audit_log(&proposal_id).await;
        assert_eq!(audit_log.len(), 1);
        assert!(matches!(audit_log[0].event_type, RotationEventType::Proposed));
    }
}
