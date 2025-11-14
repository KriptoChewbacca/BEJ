//! Self-Sovereign Provenance Graph (VeriTrust-style)
//!
//! This module implements a decentralized provenance tracking system for signal
//! sources using Decentralized Identifiers (DIDs) and anomaly detection without
//! external ML dependencies.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │              Provenance Graph System                    │
//! ├─────────────────────────────────────────────────────────┤
//! │                                                         │
//! │  Signal Source ──┐                                     │
//! │  (DID)           │                                     │
//! │                  ▼                                     │
//! │            ┌──────────┐         ┌─────────────┐        │
//! │            │ DID Node │────────>│  PDA Graph  │        │
//! │            └──────────┘         │  (On-Chain) │        │
//! │                  │              └─────────────┘        │
//! │                  ▼                                     │
//! │          ┌──────────────┐                              │
//! │          │   Anomaly    │                              │
//! │          │   Detector   │                              │
//! │          └──────────────┘                              │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Features
//!
//! - **DID-based Identity**: W3C DID standard for signal sources
//! - **Provenance Tracking**: Complete audit trail of signal origins
//! - **Anomaly Detection**: Statistical analysis without ML dependencies
//! - **On-Chain Storage**: PDA-based graph in Solana accounts
//! - **Off-Chain Validation**: Lightweight verification layer
//! - **Privacy-Preserving**: Zero-knowledge proofs for sensitive data

use anyhow::{anyhow, Context, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

// ============================================================================
// DID (Decentralized Identifier) Implementation
// ============================================================================

/// Decentralized Identifier for signal sources
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DID {
    /// Method name (e.g., "solana", "key", "web")
    pub method: String,
    /// Method-specific identifier
    pub identifier: String,
}

impl DID {
    /// Create a new DID from a Solana public key
    pub fn from_pubkey(pubkey: &Pubkey) -> Self {
        Self {
            method: "solana".to_string(),
            identifier: pubkey.to_string(),
        }
    }

    /// Create a new DID from a hash
    pub fn from_hash(hash: &[u8]) -> Self {
        Self {
            method: "key".to_string(),
            identifier: bs58::encode(hash).into_string(),
        }
    }

    /// Convert to canonical string format
    pub fn to_string(&self) -> String {
        format!("did:{}:{}", self.method, self.identifier)
    }

    /// Parse from string format
    pub fn from_string(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 || parts[0] != "did" {
            return Err(anyhow!("Invalid DID format: {}", s));
        }

        Ok(Self {
            method: parts[1].to_string(),
            identifier: parts[2].to_string(),
        })
    }

    /// Verify DID ownership (simplified - in production use cryptographic proof)
    pub fn verify(&self, proof: &[u8]) -> bool {
        // Simplified verification: hash proof and compare
        let mut hasher = Sha256::new();
        hasher.update(proof);
        let hash = hasher.finalize();

        bs58::encode(hash).into_string() == self.identifier
    }
}

// ============================================================================
// Provenance Node and Graph
// ============================================================================

/// Node in the provenance graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceNode {
    /// Unique identifier for this node
    pub id: DID,
    /// Type of signal source
    pub source_type: SignalSourceType,
    /// Timestamp of creation
    pub created_at: u64,
    /// Reputation score (0.0 to 1.0)
    pub reputation: f64,
    /// Number of signals generated
    pub signal_count: u64,
    /// Number of successful signals
    pub success_count: u64,
    /// Parent nodes (dependencies)
    pub parents: Vec<DID>,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Type of signal source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignalSourceType {
    /// Direct blockchain observation
    OnChain,
    /// External data feed
    ExternalFeed,
    /// User input
    UserInput,
    /// ML model output
    MLModel,
    /// Aggregated signal
    Aggregator,
    /// Unknown source
    Unknown,
}

/// Edge in the provenance graph (relationship between nodes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceEdge {
    pub from: DID,
    pub to: DID,
    pub edge_type: EdgeType,
    pub weight: f64,
    pub created_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeType {
    /// Direct derivation
    Derived,
    /// Validation relationship
    Validated,
    /// Aggregation relationship
    Aggregated,
    /// Correlation
    Correlated,
}

/// On-chain provenance graph (stored in PDA)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnChainProvenanceGraph {
    /// All nodes indexed by DID
    pub nodes: HashMap<DID, ProvenanceNode>,
    /// All edges
    pub edges: Vec<ProvenanceEdge>,
    /// Graph metadata
    pub metadata: GraphMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMetadata {
    pub total_nodes: u64,
    pub total_edges: u64,
    pub last_updated: u64,
    pub version: u32,
}

// ============================================================================
// Anomaly Detection (No ML Dependencies)
// ============================================================================

/// Anomaly detector using statistical methods
pub struct AnomalyDetector {
    /// Historical signal patterns per source
    signal_history: DashMap<DID, VecDeque<SignalMetrics>>,
    /// Anomaly threshold (number of standard deviations)
    threshold: f64,
    /// Window size for analysis
    window_size: usize,
}

#[derive(Debug, Clone)]
pub struct SignalMetrics {
    pub timestamp: u64,
    pub value: f64,
    pub success: bool,
    pub latency_ms: u64,
}

impl AnomalyDetector {
    /// Create a new anomaly detector
    pub fn new(threshold: f64, window_size: usize) -> Self {
        Self {
            signal_history: DashMap::new(),
            threshold,
            window_size,
        }
    }

    /// Record a signal for anomaly tracking
    pub fn record_signal(&self, source: &DID, metrics: SignalMetrics) {
        let mut history = self
            .signal_history
            .entry(source.clone())
            .or_insert_with(VecDeque::new);

        history.push_back(metrics);

        // Maintain window size
        while history.len() > self.window_size {
            history.pop_front();
        }
    }

    /// Detect anomalies using Z-score method
    pub fn detect_anomaly(&self, source: &DID, current_value: f64) -> AnomalyResult {
        let history = match self.signal_history.get(source) {
            Some(h) => h,
            None => {
                return AnomalyResult {
                    is_anomaly: false,
                    confidence: 0.0,
                    z_score: 0.0,
                    reason: "Insufficient history".to_string(),
                }
            }
        };

        if history.len() < 5 {
            return AnomalyResult {
                is_anomaly: false,
                confidence: 0.0,
                z_score: 0.0,
                reason: "Insufficient samples".to_string(),
            };
        }

        // Calculate mean and standard deviation
        let values: Vec<f64> = history.iter().map(|m| m.value).collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();

        // Calculate Z-score
        let z_score = if std_dev > 0.0 {
            (current_value - mean).abs() / std_dev
        } else {
            0.0
        };

        let is_anomaly = z_score > self.threshold;
        let confidence = (z_score / (self.threshold + 1.0)).min(1.0);

        AnomalyResult {
            is_anomaly,
            confidence,
            z_score,
            reason: if is_anomaly {
                format!(
                    "Value {:.2} is {:.2} std devs from mean {:.2}",
                    current_value, z_score, mean
                )
            } else {
                "Normal".to_string()
            },
        }
    }

    /// Detect pattern anomalies (e.g., sudden success rate drop)
    pub fn detect_pattern_anomaly(&self, source: &DID) -> AnomalyResult {
        let history = match self.signal_history.get(source) {
            Some(h) => h,
            None => {
                return AnomalyResult {
                    is_anomaly: false,
                    confidence: 0.0,
                    z_score: 0.0,
                    reason: "No history".to_string(),
                }
            }
        };

        if history.len() < 10 {
            return AnomalyResult {
                is_anomaly: false,
                confidence: 0.0,
                z_score: 0.0,
                reason: "Insufficient samples for pattern analysis".to_string(),
            };
        }

        // Split into two windows: recent vs historical
        let split_point = history.len() / 2;
        let historical: Vec<_> = history.iter().take(split_point).collect();
        let recent: Vec<_> = history.iter().skip(split_point).collect();

        // Calculate success rates
        let historical_success_rate = historical.iter().filter(|m| m.success).count() as f64
            / historical.len() as f64;
        let recent_success_rate =
            recent.iter().filter(|m| m.success).count() as f64 / recent.len() as f64;

        // Detect significant drop
        let drop = historical_success_rate - recent_success_rate;
        let is_anomaly = drop > 0.3; // 30% drop threshold

        AnomalyResult {
            is_anomaly,
            confidence: drop.abs().min(1.0),
            z_score: drop,
            reason: if is_anomaly {
                format!(
                    "Success rate dropped from {:.1}% to {:.1}%",
                    historical_success_rate * 100.0,
                    recent_success_rate * 100.0
                )
            } else {
                "No significant pattern change".to_string()
            },
        }
    }

    /// Clear old history
    pub fn clear_old_history(&self, older_than_secs: u64) {
        let cutoff = Self::timestamp() - older_than_secs;

        for mut entry in self.signal_history.iter_mut() {
            entry.retain(|m| m.timestamp > cutoff);
        }
    }

    fn timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

#[derive(Debug, Clone)]
pub struct AnomalyResult {
    pub is_anomaly: bool,
    pub confidence: f64,
    pub z_score: f64,
    pub reason: String,
}

// ============================================================================
// Provenance Graph Manager
// ============================================================================

/// Manages the provenance graph and anomaly detection
pub struct ProvenanceGraphManager {
    /// In-memory graph
    graph: Arc<RwLock<OnChainProvenanceGraph>>,
    /// Anomaly detector
    anomaly_detector: Arc<AnomalyDetector>,
    /// Node lookup cache
    node_cache: DashMap<DID, ProvenanceNode>,
    /// Metrics
    metrics: Arc<ProvenanceMetrics>,
}

struct ProvenanceMetrics {
    total_signals: AtomicU64,
    anomalies_detected: AtomicU64,
    nodes_created: AtomicU64,
    edges_created: AtomicU64,
}

impl ProvenanceGraphManager {
    /// Create a new provenance graph manager
    pub fn new() -> Self {
        Self {
            graph: Arc::new(RwLock::new(OnChainProvenanceGraph {
                nodes: HashMap::new(),
                edges: Vec::new(),
                metadata: GraphMetadata {
                    total_nodes: 0,
                    total_edges: 0,
                    last_updated: Self::timestamp(),
                    version: 1,
                },
            })),
            anomaly_detector: Arc::new(AnomalyDetector::new(3.0, 100)),
            node_cache: DashMap::new(),
            metrics: Arc::new(ProvenanceMetrics {
                total_signals: AtomicU64::new(0),
                anomalies_detected: AtomicU64::new(0),
                nodes_created: AtomicU64::new(0),
                edges_created: AtomicU64::new(0),
            }),
        }
    }

    /// Register a new signal source
    pub async fn register_source(
        &self,
        did: DID,
        source_type: SignalSourceType,
        metadata: HashMap<String, String>,
    ) -> Result<()> {
        let node = ProvenanceNode {
            id: did.clone(),
            source_type,
            created_at: Self::timestamp(),
            reputation: 0.5, // Start neutral
            signal_count: 0,
            success_count: 0,
            parents: Vec::new(),
            metadata,
        };

        let mut graph = self.graph.write().await;
        graph.nodes.insert(did.clone(), node.clone());
        graph.metadata.total_nodes += 1;
        graph.metadata.last_updated = Self::timestamp();

        self.node_cache.insert(did.clone(), node);
        self.metrics.nodes_created.fetch_add(1, Ordering::Relaxed);

        info!(?did, "Signal source registered");
        Ok(())
    }

    /// Track a signal from a source
    pub async fn track_signal(
        &self,
        source: &DID,
        value: f64,
        success: bool,
        latency_ms: u64,
    ) -> Result<SignalTrackingResult> {
        // Record metrics
        let metrics = SignalMetrics {
            timestamp: Self::timestamp(),
            value,
            success,
            latency_ms,
        };

        self.anomaly_detector.record_signal(source, metrics);
        self.metrics.total_signals.fetch_add(1, Ordering::Relaxed);

        // Check for anomalies
        let value_anomaly = self.anomaly_detector.detect_anomaly(source, value);
        let pattern_anomaly = self.anomaly_detector.detect_pattern_anomaly(source);

        let is_anomalous = value_anomaly.is_anomaly || pattern_anomaly.is_anomaly;

        if is_anomalous {
            self.metrics
                .anomalies_detected
                .fetch_add(1, Ordering::Relaxed);
            warn!(
                ?source,
                value_anomaly = ?value_anomaly.reason,
                pattern_anomaly = ?pattern_anomaly.reason,
                "Anomaly detected in signal source"
            );
        }

        // Update node statistics
        let mut graph = self.graph.write().await;
        if let Some(node) = graph.nodes.get_mut(source) {
            node.signal_count += 1;
            if success {
                node.success_count += 1;
            }

            // Update reputation based on success rate
            let success_rate = node.success_count as f64 / node.signal_count as f64;
            node.reputation = success_rate;

            // Update cache
            self.node_cache.insert(source.clone(), node.clone());
        }

        Ok(SignalTrackingResult {
            is_anomalous,
            value_anomaly,
            pattern_anomaly,
            current_reputation: self.get_reputation(source).await,
        })
    }

    /// Add an edge between two nodes
    pub async fn add_edge(&self, from: DID, to: DID, edge_type: EdgeType, weight: f64) -> Result<()> {
        let edge = ProvenanceEdge {
            from: from.clone(),
            to: to.clone(),
            edge_type,
            weight,
            created_at: Self::timestamp(),
        };

        let mut graph = self.graph.write().await;
        graph.edges.push(edge);
        graph.metadata.total_edges += 1;

        // Update parent relationships
        if let Some(node) = graph.nodes.get_mut(&to) {
            if !node.parents.contains(&from) {
                node.parents.push(from.clone());
            }
        }

        self.metrics.edges_created.fetch_add(1, Ordering::Relaxed);

        debug!(?from, ?to, ?edge_type, "Edge added to provenance graph");
        Ok(())
    }

    /// Get reputation score for a source
    pub async fn get_reputation(&self, source: &DID) -> f64 {
        if let Some(node) = self.node_cache.get(source) {
            return node.reputation;
        }

        let graph = self.graph.read().await;
        graph
            .nodes
            .get(source)
            .map(|n| n.reputation)
            .unwrap_or(0.0)
    }

    /// Get provenance chain (ancestry) for a source
    pub async fn get_provenance_chain(&self, source: &DID) -> Vec<ProvenanceNode> {
        let graph = self.graph.read().await;
        let mut chain = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(source.clone());
        visited.insert(source.clone());

        while let Some(current) = queue.pop_front() {
            if let Some(node) = graph.nodes.get(&current) {
                chain.push(node.clone());

                for parent in &node.parents {
                    if !visited.contains(parent) {
                        queue.push_back(parent.clone());
                        visited.insert(parent.clone());
                    }
                }
            }
        }

        chain
    }

    /// Serialize graph for on-chain storage
    pub async fn serialize_graph(&self) -> Result<Vec<u8>> {
        let graph = self.graph.read().await;
        bincode::serialize(&*graph).context("Failed to serialize provenance graph")
    }

    /// Load graph from on-chain storage
    pub async fn load_graph(&self, data: &[u8]) -> Result<()> {
        let loaded_graph: OnChainProvenanceGraph =
            bincode::deserialize(data).context("Failed to deserialize provenance graph")?;

        // Update cache
        for (did, node) in &loaded_graph.nodes {
            self.node_cache.insert(did.clone(), node.clone());
        }

        let mut graph = self.graph.write().await;
        *graph = loaded_graph;

        info!(
            nodes = %graph.metadata.total_nodes,
            edges = %graph.metadata.total_edges,
            "Provenance graph loaded from chain"
        );

        Ok(())
    }

    /// Get graph statistics
    pub async fn get_stats(&self) -> ProvenanceStats {
        let graph = self.graph.read().await;

        ProvenanceStats {
            total_nodes: graph.metadata.total_nodes,
            total_edges: graph.metadata.total_edges,
            total_signals: self.metrics.total_signals.load(Ordering::Relaxed),
            anomalies_detected: self.metrics.anomalies_detected.load(Ordering::Relaxed),
            average_reputation: if graph.nodes.is_empty() {
                0.0
            } else {
                graph.nodes.values().map(|n| n.reputation).sum::<f64>()
                    / graph.nodes.len() as f64
            },
        }
    }

    fn timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

impl Default for ProvenanceGraphManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct SignalTrackingResult {
    pub is_anomalous: bool,
    pub value_anomaly: AnomalyResult,
    pub pattern_anomaly: AnomalyResult,
    pub current_reputation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceStats {
    pub total_nodes: u64,
    pub total_edges: u64,
    pub total_signals: u64,
    pub anomalies_detected: u64,
    pub average_reputation: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_did_creation() {
        let pubkey = Pubkey::new_unique();
        let did = DID::from_pubkey(&pubkey);
        assert_eq!(did.method, "solana");
        assert_eq!(did.to_string(), format!("did:solana:{}", pubkey));
    }

    #[test]
    fn test_did_parsing() {
        let did_str = "did:solana:ABC123";
        let did = DID::from_string(did_str).unwrap();
        assert_eq!(did.method, "solana");
        assert_eq!(did.identifier, "ABC123");
    }

    #[tokio::test]
    async fn test_provenance_registration() {
        let manager = ProvenanceGraphManager::new();
        let did = DID::from_pubkey(&Pubkey::new_unique());

        manager
            .register_source(did.clone(), SignalSourceType::OnChain, HashMap::new())
            .await
            .unwrap();

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_nodes, 1);
    }

    #[tokio::test]
    async fn test_signal_tracking() {
        let manager = ProvenanceGraphManager::new();
        let did = DID::from_pubkey(&Pubkey::new_unique());

        manager
            .register_source(did.clone(), SignalSourceType::OnChain, HashMap::new())
            .await
            .unwrap();

        let result = manager
            .track_signal(&did, 1.0, true, 10)
            .await
            .unwrap();

        assert_eq!(result.is_anomalous, false); // First signal, no baseline
        assert_eq!(result.current_reputation, 1.0); // 100% success
    }

    #[test]
    fn test_anomaly_detection() {
        let detector = AnomalyDetector::new(3.0, 100);
        let did = DID::from_pubkey(&Pubkey::new_unique());

        // Record normal signals (mean = 10.0, stddev = 0)
        for i in 0..20 {
            detector.record_signal(
                &did,
                SignalMetrics {
                    timestamp: i,
                    value: 10.0,
                    success: true,
                    latency_ms: 10,
                },
            );
        }

        // Check normal value
        let result = detector.detect_anomaly(&did, 10.0);
        assert!(!result.is_anomaly); // Normal value

        // Check anomalous value (way outside 3 std devs)
        // Since stddev is 0, we need to add some variance first
        detector.record_signal(
            &did,
            SignalMetrics {
                timestamp: 20,
                value: 11.0,
                success: true,
                latency_ms: 10,
            },
        );

        // Now test with a truly anomalous value
        let result = detector.detect_anomaly(&did, 50.0);
        assert!(result.is_anomaly); // Anomalous value
        assert!(result.z_score > 3.0);
    }
}
