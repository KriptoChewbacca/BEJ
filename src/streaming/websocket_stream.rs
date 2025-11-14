//! WebSocket streaming implementation for real-time Solana data
//!
//! Provides free-tier alternative to Geyser gRPC using native Solana WebSocket API
//! or enhanced RPC providers (Helius, QuickNode, Alchemy)

use futures_util::StreamExt;
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_response::Response;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::sync::mpsc;

/// WebSocket streaming client for monitoring Solana programs
pub struct WebSocketStream {
    ws_url: String,
    commitment: CommitmentConfig,
}

impl WebSocketStream {
    /// Create new WebSocket stream
    pub fn new(ws_url: String) -> Self {
        Self {
            ws_url,
            commitment: CommitmentConfig::confirmed(),
        }
    }

    /// Connect to WebSocket endpoint and return client
    pub async fn connect(&self) -> Result<Arc<PubsubClient>, Box<dyn std::error::Error>> {
        log::info!("Connecting to WebSocket: {}", self.ws_url);

        let client = PubsubClient::new(&self.ws_url).await?;

        log::info!("WebSocket connected successfully");
        Ok(Arc::new(client))
    }

    /// Subscribe to program account updates
    pub async fn subscribe_program(
        &self,
        client: Arc<PubsubClient>,
        program_id: &Pubkey,
        tx: mpsc::UnboundedSender<ProgramUpdate>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Subscribing to program: {}", program_id);

        let commitment = self.commitment;
        let program_id = *program_id;

        // Spawn the subscription task
        tokio::spawn(async move {
            // Subscribe to program account changes - the stream will hold a reference to client
            let (mut notifications, unsubscribe) = match client
                .program_subscribe(
                    &program_id,
                    Some(RpcProgramAccountsConfig {
                        filters: None,
                        account_config: RpcAccountInfoConfig {
                            encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                            commitment: Some(commitment),
                            data_slice: None,
                            min_context_slot: None,
                        },
                        with_context: Some(true),
                        sort_results: None,
                    }),
                )
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    log::error!("Failed to subscribe to program {}: {}", program_id, e);
                    return;
                }
            };

            // Process notifications - client is kept alive by being in the task scope
            while let Some(response) = notifications.next().await {
                let program_update = ProgramUpdate::from_response(response);
                if tx.send(program_update).is_err() {
                    log::warn!("Receiver dropped, unsubscribing");
                    break;
                }
            }
            unsubscribe().await;
            // client is dropped here when the task ends
        });

        Ok(())
    }

    /// Subscribe to transaction signatures
    pub async fn subscribe_signatures(
        &self,
        client: Arc<PubsubClient>,
        signature: &solana_sdk::signature::Signature,
        tx: mpsc::UnboundedSender<SignatureUpdate>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Subscribing to signature: {}", signature);

        let commitment = self.commitment;
        let signature = *signature;
        let signature_str = signature.to_string();

        // Spawn the subscription task
        tokio::spawn(async move {
            // Subscribe to signature updates - the stream will hold a reference to client
            let (mut notifications, unsubscribe) = match client
                .signature_subscribe(
                    &signature,
                    Some(solana_client::rpc_config::RpcSignatureSubscribeConfig {
                        commitment: Some(commitment),
                        enable_received_notification: Some(true),
                    }),
                )
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    log::error!("Failed to subscribe to signature {}: {}", signature, e);
                    return;
                }
            };

            // Process notifications - client is kept alive by being in the task scope
            while let Some(response) = notifications.next().await {
                let sig_update = SignatureUpdate::from_response(response, signature_str.clone());
                if tx.send(sig_update).is_err() {
                    break;
                }
            }
            unsubscribe().await;
            // client is dropped here when the task ends
        });

        Ok(())
    }
}

/// Program account update event
#[derive(Debug, Clone)]
pub struct ProgramUpdate {
    pub pubkey: Pubkey,
    pub account_data: Vec<u8>,
    pub slot: u64,
}

/// Transaction signature update event
#[derive(Debug, Clone)]
pub struct SignatureUpdate {
    pub signature: String,
    pub slot: u64,
    pub err: Option<String>,
}

impl ProgramUpdate {
    /// Create from pubsub response
    pub fn from_response(
        response: Response<solana_client::rpc_response::RpcKeyedAccount>,
    ) -> Self {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::Engine;
        use solana_account_decoder::UiAccountData;
        use solana_sdk::pubkey::Pubkey;
        use std::str::FromStr;

        let keyed_account = response.value;
        let pubkey = Pubkey::from_str(&keyed_account.pubkey).unwrap_or_default();
        let account_data = match keyed_account.account.data {
            UiAccountData::Binary(data, _) => {
                // Decode base64 data
                BASE64.decode(&data).unwrap_or_default()
            }
            UiAccountData::Json(_) => vec![],
            UiAccountData::LegacyBinary(data) => BASE64.decode(&data).unwrap_or_default(),
        };

        Self {
            pubkey,
            account_data,
            slot: response.context.slot,
        }
    }
}

impl SignatureUpdate {
    /// Create from pubsub response
    pub fn from_response(
        response: Response<solana_client::rpc_response::RpcSignatureResult>,
        signature: String,
    ) -> Self {
        Self {
            signature,
            slot: response.context.slot,
            err: None, // RpcSignatureResult structure varies, set to None for now
        }
    }
}



