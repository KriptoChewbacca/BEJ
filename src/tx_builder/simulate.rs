//! Durable-nonce-aware transaction simulation
//!
//! This module provides utilities for simulating transactions while
//! preserving durable nonce accounts. Simulation should not consume
//! the nonce, so we strip the advance_nonce instruction.
//!
//! ## Key Features
//! - Strip advance_nonce for simulation
//! - Preserve transaction structure and metadata
//! - Build simulation transactions that match real transactions
//!
//! ## Implementation Status
//! **TODO (Task 4)**: Implement simulation utilities

// Placeholder for simulation implementation
// This will be implemented in Task 4

// Functions will be added in Task 4:
// pub fn strip_nonce_for_simulation(instructions: &[Instruction], is_durable: bool) -> Vec<Instruction> { ... }
// pub fn build_sim_tx_like(tx: &VersionedTransaction, sim_ix: Vec<Instruction>) -> VersionedTransaction { ... }
