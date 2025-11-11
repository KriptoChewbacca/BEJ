//! Formal dataflow contract and domain boundaries
//!
//! Defines the data flow pipeline between Sniffer layers:
//! [Geyser Stream] → Bytes
//! [prefilter.rs] → Option<Bytes>
//! [extractor.rs] → Result<PremintCandidate, ExtractError>
//! [security.rs] → ValidatedCandidate (Option<PremintCandidate>)
//! [handoff.rs] → mpsc::Sender<PremintCandidate>
//! [buy_engine.rs]

use std::time::Instant;
use serde::{Deserialize, Serialize};

/// Unique identifier for tracing candidates through the pipeline
pub type CandidateId = u64;

/// Event types for telemetry tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnifferEvent {
    /// Raw bytes received from Geyser stream
    BytesReceived {
        trace_id: CandidateId,
        timestamp: Instant,
        size: usize,
    },
    /// Transaction passed prefilter
    PrefilterPassed {
        trace_id: CandidateId,
        timestamp: Instant,
        latency_us: u64,
    },
    /// Transaction rejected by prefilter
    PrefilterRejected {
        trace_id: CandidateId,
        timestamp: Instant,
        reason: &'static str,
    },
    /// Candidate extracted successfully
    CandidateExtracted {
        trace_id: CandidateId,
        timestamp: Instant,
        latency_us: u64,
        priority: super::extractor::PriorityLevel,
    },
    /// Extraction failed
    ExtractionFailed {
        trace_id: CandidateId,
        timestamp: Instant,
        error: String,
    },
    /// Security validation passed
    SecurityPassed {
        trace_id: CandidateId,
        timestamp: Instant,
        latency_us: u64,
    },
    /// Security validation failed
    SecurityRejected {
        trace_id: CandidateId,
        timestamp: Instant,
        reason: &'static str,
    },
    /// Candidate sent to handoff
    HandoffSent {
        trace_id: CandidateId,
        timestamp: Instant,
        latency_us: u64,
        queue_depth: usize,
    },
    /// Candidate dropped due to backpressure
    HandoffDropped {
        trace_id: CandidateId,
        timestamp: Instant,
        reason: &'static str,
    },
}

impl SnifferEvent {
    /// Get the trace ID for this event
    pub fn trace_id(&self) -> CandidateId {
        match self {
            Self::BytesReceived { trace_id, .. }
            | Self::PrefilterPassed { trace_id, .. }
            | Self::PrefilterRejected { trace_id, .. }
            | Self::CandidateExtracted { trace_id, .. }
            | Self::ExtractionFailed { trace_id, .. }
            | Self::SecurityPassed { trace_id, .. }
            | Self::SecurityRejected { trace_id, .. }
            | Self::HandoffSent { trace_id, .. }
            | Self::HandoffDropped { trace_id, .. } => *trace_id,
        }
    }

    /// Get the timestamp for this event
    pub fn timestamp(&self) -> Instant {
        match self {
            Self::BytesReceived { timestamp, .. }
            | Self::PrefilterPassed { timestamp, .. }
            | Self::PrefilterRejected { timestamp, .. }
            | Self::CandidateExtracted { timestamp, .. }
            | Self::ExtractionFailed { timestamp, .. }
            | Self::SecurityPassed { timestamp, .. }
            | Self::SecurityRejected { timestamp, .. }
            | Self::HandoffSent { timestamp, .. }
            | Self::HandoffDropped { timestamp, .. } => *timestamp,
        }
    }

    /// Get event type name
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::BytesReceived { .. } => "bytes_received",
            Self::PrefilterPassed { .. } => "prefilter_passed",
            Self::PrefilterRejected { .. } => "prefilter_rejected",
            Self::CandidateExtracted { .. } => "candidate_extracted",
            Self::ExtractionFailed { .. } => "extraction_failed",
            Self::SecurityPassed { .. } => "security_passed",
            Self::SecurityRejected { .. } => "security_rejected",
            Self::HandoffSent { .. } => "handoff_sent",
            Self::HandoffDropped { .. } => "handoff_dropped",
        }
    }
}

/// Validated candidate wrapper
#[derive(Debug, Clone)]
pub struct ValidatedCandidate {
    pub candidate: super::extractor::PremintCandidate,
    pub validation_latency_us: u64,
}

impl ValidatedCandidate {
    /// Create a new validated candidate
    pub fn new(candidate: super::extractor::PremintCandidate, validation_latency_us: u64) -> Self {
        Self {
            candidate,
            validation_latency_us,
        }
    }

    /// Get the underlying candidate
    pub fn into_inner(self) -> super::extractor::PremintCandidate {
        self.candidate
    }
}

/// Domain boundary trait - modules must not access full transaction data
/// Each module only sees what is necessary for its operation
pub trait DomainBoundary {
    type Input;
    type Output;
    type Error;

    /// Process input within domain boundaries
    fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sniffer_event_trace_id() {
        let event = SnifferEvent::BytesReceived {
            trace_id: 123,
            timestamp: Instant::now(),
            size: 256,
        };
        assert_eq!(event.trace_id(), 123);
        assert_eq!(event.event_type(), "bytes_received");
    }

    #[test]
    fn test_validated_candidate() {
        use smallvec::SmallVec;
        use solana_sdk::pubkey::Pubkey;
        use super::super::extractor::{PremintCandidate, PriorityLevel};

        let candidate = PremintCandidate::new(
            Pubkey::new_unique(),
            SmallVec::new(),
            1.0,
            1,
            PriorityLevel::High,
        );

        let validated = ValidatedCandidate::new(candidate.clone(), 100);
        assert_eq!(validated.validation_latency_us, 100);
        
        let inner = validated.into_inner();
        assert_eq!(inner.trace_id, candidate.trace_id);
    }
}
