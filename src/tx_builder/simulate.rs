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
//! **COMPLETED (Task 4)**: Simulation utilities for E2E testing

use solana_sdk::{
    instruction::Instruction,
    message::{v0::Message as MessageV0, VersionedMessage},
    pubkey::Pubkey,
    signature::Signature,
    signer::Signer,
    system_program,
    transaction::VersionedTransaction,
};

/// Strip advance_nonce instruction from a list of instructions for simulation
///
/// When simulating a transaction with a durable nonce, we need to remove the
/// advance_nonce instruction to avoid consuming the nonce during simulation.
///
/// # Arguments
///
/// * `instructions` - The full list of instructions including advance_nonce
/// * `is_durable` - Whether this is a durable nonce transaction
///
/// # Returns
///
/// A new vector of instructions without the advance_nonce instruction
///
/// # Example
///
/// ```no_run
/// let sim_instructions = strip_nonce_for_simulation(&instructions, true);
/// // sim_instructions will have all instructions except advance_nonce
/// ```
pub fn strip_nonce_for_simulation(
    instructions: &[Instruction],
    is_durable: bool,
) -> Vec<Instruction> {
    if !is_durable || instructions.is_empty() {
        return instructions.to_vec();
    }

    // Check if first instruction is advance_nonce (system program with discriminator 4)
    let first_ix = &instructions[0];
    if first_ix.program_id == system_program::id()
        && first_ix.data.len() >= 4
        && first_ix.data[0] == 4
        && first_ix.data[1] == 0
        && first_ix.data[2] == 0
        && first_ix.data[3] == 0
    {
        // Skip first instruction (advance_nonce)
        instructions[1..].to_vec()
    } else {
        // No advance_nonce found, return all instructions
        instructions.to_vec()
    }
}

/// Build a simulation transaction from the original transaction
///
/// Creates a new transaction with the same payer and blockhash but
/// with stripped instructions (no advance_nonce).
///
/// # Arguments
///
/// * `tx` - The original transaction
/// * `sim_instructions` - Instructions for simulation (without advance_nonce)
/// * `payer` - The transaction payer
///
/// # Returns
///
/// A new VersionedTransaction for simulation
///
/// # Note
///
/// This function creates an unsigned transaction suitable for simulation.
/// The signatures are placeholder values.
pub fn build_sim_tx_like(
    tx: &VersionedTransaction,
    sim_instructions: Vec<Instruction>,
    payer: &Pubkey,
) -> VersionedTransaction {
    // Extract blockhash from original transaction
    let blockhash = match &tx.message {
        VersionedMessage::V0(msg) => msg.recent_blockhash,
        VersionedMessage::Legacy(msg) => msg.recent_blockhash,
    };

    // Build simulation message
    let message = MessageV0::try_compile(payer, &sim_instructions, &[], blockhash)
        .expect("Failed to compile simulation message");

    // Create transaction with empty signatures
    // For simulation, signatures don't need to be valid
    let num_signatures = tx.signatures.len();
    let signatures = vec![Signature::default(); num_signatures];

    VersionedTransaction {
        signatures,
        message: VersionedMessage::V0(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{hash::Hash, system_instruction};

    #[test]
    fn test_strip_nonce_for_simulation_durable() {
        let nonce_account = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();

        let mut instructions = vec![
            system_instruction::advance_nonce_account(&nonce_account, &nonce_authority),
            system_instruction::transfer(&Pubkey::new_unique(), &Pubkey::new_unique(), 1000),
        ];

        let sim_ix = strip_nonce_for_simulation(&instructions, true);

        // Should have 1 instruction (transfer only)
        assert_eq!(sim_ix.len(), 1);
        assert_eq!(sim_ix[0].program_id, system_program::id());
    }

    #[test]
    fn test_strip_nonce_for_simulation_non_durable() {
        let instructions = vec![system_instruction::transfer(
            &Pubkey::new_unique(),
            &Pubkey::new_unique(),
            1000,
        )];

        let sim_ix = strip_nonce_for_simulation(&instructions, false);

        // Should have same instructions
        assert_eq!(sim_ix.len(), instructions.len());
    }

    #[test]
    fn test_strip_nonce_no_advance_nonce() {
        // Instructions without advance_nonce
        let instructions = vec![
            system_instruction::transfer(&Pubkey::new_unique(), &Pubkey::new_unique(), 1000),
            system_instruction::transfer(&Pubkey::new_unique(), &Pubkey::new_unique(), 2000),
        ];

        let sim_ix = strip_nonce_for_simulation(&instructions, true);

        // Should return all instructions since there's no advance_nonce
        assert_eq!(sim_ix.len(), instructions.len());
    }

    #[test]
    fn test_build_sim_tx_like() {
        let payer = Pubkey::new_unique();
        let nonce_account = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();
        let blockhash = Hash::new_unique();

        // Create original transaction
        let instructions = vec![
            system_instruction::advance_nonce_account(&nonce_account, &nonce_authority),
            system_instruction::transfer(&payer, &Pubkey::new_unique(), 1000),
        ];

        let message = MessageV0::try_compile(&payer, &instructions, &[], blockhash).unwrap();
        let original_tx = VersionedTransaction {
            signatures: vec![Signature::default()],
            message: VersionedMessage::V0(message),
        };

        // Build simulation transaction
        let sim_instructions = strip_nonce_for_simulation(&instructions, true);
        let sim_tx = build_sim_tx_like(&original_tx, sim_instructions, &payer);

        // Verify blockhash is preserved
        match &sim_tx.message {
            VersionedMessage::V0(msg) => {
                assert_eq!(msg.recent_blockhash, blockhash);
                // Should have 1 instruction (transfer only)
                assert_eq!(msg.instructions.len(), 1);
            }
            _ => panic!("Expected V0 message"),
        }
    }
}
