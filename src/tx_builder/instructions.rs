//! Instruction planning and ordering validation
//!
//! This module handles building instruction lists with correct ordering
//! for durable nonce transactions:
//! 1. advance_nonce_account (if durable)
//! 2. Compute budget instructions (CU limit, priority fee)
//! 3. DEX/program instructions
//!
//! ## Key Features
//! - Stateless instruction planning functions
//! - Order validation (debug/test only)
//! - Support for various DEX instruction types
//!
//! ## Implementation Status
//! **COMPLETED (Task 3)**: Instruction planning and validation

use crate::tx_builder::errors::TransactionBuilderError;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
    pubkey::Pubkey,
    system_instruction, system_program,
};

/// Plan of instructions with ordering metadata
///
/// This struct represents a complete plan of instructions for a transaction,
/// with metadata indicating whether it uses durable nonce.
///
/// # Fields
/// - `instructions`: The ordered list of instructions
/// - `is_durable`: Whether this plan uses durable nonce (affects validation)
#[derive(Debug, Clone)]
pub struct InstructionPlan {
    /// The ordered list of instructions for the transaction
    pub instructions: Vec<Instruction>,
    
    /// Whether this plan uses durable nonce
    /// - `true`: First instruction must be advance_nonce_account
    /// - `false`: Standard blockhash-based transaction
    pub is_durable: bool,
}

impl InstructionPlan {
    /// Create a new InstructionPlan
    pub fn new(instructions: Vec<Instruction>, is_durable: bool) -> Self {
        Self {
            instructions,
            is_durable,
        }
    }
}

/// Plan buy instructions with correct ordering for durable nonce transactions
///
/// This function creates a properly ordered instruction list following the
/// required structure for durable nonce transactions:
///
/// 1. `advance_nonce_account` (if durable)
/// 2. Compute budget instructions (CU limit, priority fee)
/// 3. DEX/program instruction
///
/// # Arguments
///
/// * `exec_durable` - Optional tuple of (nonce_account, nonce_authority) for durable mode
/// * `cu_limit` - Compute unit limit (0 = skip this instruction)
/// * `prio_fee` - Priority fee in micro-lamports (0 = skip this instruction)
/// * `buy_ix` - The main DEX/program instruction
///
/// # Returns
///
/// An `InstructionPlan` with properly ordered instructions
///
/// # Errors
///
/// Returns `TransactionBuilderError::Configuration` if the buy instruction is invalid
///
/// # Example
///
/// ```no_run
/// use solana_sdk::{instruction::Instruction, pubkey::Pubkey};
/// # use bot::tx_builder::instructions::plan_buy_instructions;
///
/// # fn example(dex_ix: Instruction) -> Result<(), Box<dyn std::error::Error>> {
/// let nonce_account = Pubkey::new_unique();
/// let nonce_authority = Pubkey::new_unique();
///
/// // Durable nonce transaction
/// let plan = plan_buy_instructions(
///     Some((nonce_account, nonce_authority)),
///     200_000,  // CU limit
///     10_000,   // Priority fee
///     dex_ix,
/// )?;
///
/// // Non-durable transaction (recent blockhash)
/// let plan = plan_buy_instructions(
///     None,
///     200_000,
///     10_000,
///     dex_ix,
/// )?;
/// # Ok(())
/// # }
/// ```
pub fn plan_buy_instructions(
    exec_durable: Option<(Pubkey, Pubkey)>,
    cu_limit: u32,
    prio_fee: u64,
    buy_ix: Instruction,
) -> Result<InstructionPlan, TransactionBuilderError> {
    // Validate the buy instruction
    if buy_ix.accounts.is_empty() {
        return Err(TransactionBuilderError::Configuration(
            "Buy instruction has no accounts".to_string(),
        ));
    }

    // Pre-allocate vector with capacity for all instructions
    // Maximum: advance_nonce (1) + compute_budget (2) + buy_ix (1) = 4
    let mut instructions = Vec::with_capacity(4);
    
    let is_durable = exec_durable.is_some();

    // 1. Add advance_nonce_account if durable mode
    if let Some((nonce_account, nonce_authority)) = exec_durable {
        instructions.push(system_instruction::advance_nonce_account(
            &nonce_account,
            &nonce_authority,
        ));
    }

    // 2. Add compute budget instructions
    // Note: Order within compute budget instructions doesn't matter,
    // but they should come after advance_nonce and before program instructions
    
    // Add CU limit if specified
    if cu_limit > 0 {
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(cu_limit));
    }
    
    // Add priority fee if specified
    if prio_fee > 0 {
        instructions.push(ComputeBudgetInstruction::set_compute_unit_price(prio_fee));
    }

    // 3. Add the main DEX/program instruction
    instructions.push(buy_ix);

    Ok(InstructionPlan::new(instructions, is_durable))
}

/// Validate instruction ordering for durable nonce transactions (debug/test only)
///
/// This function performs a sanity check on instruction ordering to ensure
/// correctness. It is only compiled in debug/test builds to avoid production overhead.
///
/// Expected order for durable nonce transactions:
/// 1. `advance_nonce_account` (REQUIRED FIRST for durable transactions)
/// 2. Compute budget instructions (optional)
/// 3. Program instructions
///
/// # Arguments
///
/// * `instructions` - The list of instructions to validate
/// * `is_durable` - Whether this is a durable nonce transaction
///
/// # Returns
///
/// `Ok(())` if ordering is valid, or an error describing the problem
///
/// # Errors
///
/// Returns `TransactionBuilderError::InvalidInstructionOrder` if:
/// - Instruction list is empty
/// - Durable transaction doesn't start with advance_nonce
/// - Multiple advance_nonce instructions found
/// - advance_nonce found in non-durable transaction
///
/// # Note
///
/// This function is only compiled in debug/test builds via `cfg(debug_assertions)`.
/// In production builds, it is optimized away to zero overhead.
#[cfg(debug_assertions)]
pub fn sanity_check_ix_order(
    instructions: &[Instruction],
    is_durable: bool,
) -> Result<(), TransactionBuilderError> {
    if instructions.is_empty() {
        return Err(TransactionBuilderError::invalid_order(
            "Instruction list is empty",
        ));
    }

    // Helper: Check if instruction is advance_nonce_account
    let is_advance_nonce = |ix: &Instruction| -> bool {
        // Must be system program
        if ix.program_id != system_program::id() {
            return false;
        }

        // advance_nonce_account has discriminator 4
        // Format: [4, 0, 0, 0] (u32 little-endian)
        ix.data.len() >= 4
            && ix.data[0] == 4
            && ix.data[1] == 0
            && ix.data[2] == 0
            && ix.data[3] == 0
    };

    let first_is_advance_nonce = is_advance_nonce(&instructions[0]);

    if is_durable {
        // Durable mode: first instruction MUST be advance_nonce
        if !first_is_advance_nonce {
            return Err(TransactionBuilderError::invalid_order(format!(
                "Durable nonce transaction must start with advance_nonce_account, got program_id: {}",
                instructions[0].program_id
            )));
        }

        // Verify no duplicate advance_nonce instructions
        for (idx, ix) in instructions.iter().enumerate().skip(1) {
            if is_advance_nonce(ix) {
                return Err(TransactionBuilderError::invalid_order(format!(
                    "Multiple advance_nonce_account instructions found (at position {}). Only one allowed at position 0",
                    idx
                )));
            }
        }
    } else {
        // Non-durable mode: should NOT have advance_nonce
        if first_is_advance_nonce {
            return Err(TransactionBuilderError::invalid_order(
                "Non-durable transaction should not have advance_nonce_account instruction"
                    .to_string(),
            ));
        }

        // Verify no advance_nonce anywhere in non-durable transaction
        for (idx, ix) in instructions.iter().enumerate() {
            if is_advance_nonce(ix) {
                return Err(TransactionBuilderError::invalid_order(format!(
                    "Non-durable transaction should not have advance_nonce_account (found at position {})",
                    idx
                )));
            }
        }
    }

    Ok(())
}

/// No-op version of sanity_check_ix_order for release builds
///
/// In release builds, this function is optimized away to zero overhead.
/// It always returns Ok(()) without performing any validation.
#[cfg(not(debug_assertions))]
#[inline]
pub fn sanity_check_ix_order(
    _instructions: &[Instruction],
    _is_durable: bool,
) -> Result<(), TransactionBuilderError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{instruction::AccountMeta, pubkey::Pubkey};

    #[test]
    fn test_plan_buy_instructions_durable() {
        let nonce_account = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();
        let account = Pubkey::new_unique();

        let buy_ix = Instruction::new_with_bytes(
            program_id,
            &[1, 2, 3, 4],
            vec![AccountMeta::new(account, false)],
        );

        let plan = plan_buy_instructions(
            Some((nonce_account, nonce_authority)),
            200_000,
            10_000,
            buy_ix,
        )
        .expect("Should plan durable instructions");

        assert!(plan.is_durable);
        assert_eq!(plan.instructions.len(), 4); // advance_nonce + cu_limit + cu_price + buy

        // Verify order
        assert_eq!(plan.instructions[0].program_id, system_program::id());
        assert_eq!(
            plan.instructions[1].program_id,
            solana_sdk::compute_budget::id()
        );
        assert_eq!(
            plan.instructions[2].program_id,
            solana_sdk::compute_budget::id()
        );
        assert_eq!(plan.instructions[3].program_id, program_id);
    }

    #[test]
    fn test_plan_buy_instructions_non_durable() {
        let program_id = Pubkey::new_unique();
        let account = Pubkey::new_unique();

        let buy_ix = Instruction::new_with_bytes(
            program_id,
            &[1, 2, 3, 4],
            vec![AccountMeta::new(account, false)],
        );

        let plan = plan_buy_instructions(None, 200_000, 10_000, buy_ix)
            .expect("Should plan non-durable instructions");

        assert!(!plan.is_durable);
        assert_eq!(plan.instructions.len(), 3); // cu_limit + cu_price + buy

        // Verify no advance_nonce
        for ix in &plan.instructions {
            assert_ne!(ix.program_id, system_program::id());
        }
    }

    #[test]
    fn test_plan_buy_instructions_no_compute_budget() {
        let program_id = Pubkey::new_unique();
        let account = Pubkey::new_unique();

        let buy_ix = Instruction::new_with_bytes(
            program_id,
            &[1, 2, 3, 4],
            vec![AccountMeta::new(account, false)],
        );

        // No CU limit or priority fee
        let plan = plan_buy_instructions(None, 0, 0, buy_ix)
            .expect("Should plan without compute budget");

        assert!(!plan.is_durable);
        assert_eq!(plan.instructions.len(), 1); // Only buy instruction
        assert_eq!(plan.instructions[0].program_id, program_id);
    }

    #[test]
    fn test_plan_buy_instructions_only_cu_limit() {
        let program_id = Pubkey::new_unique();
        let account = Pubkey::new_unique();

        let buy_ix = Instruction::new_with_bytes(
            program_id,
            &[1, 2, 3, 4],
            vec![AccountMeta::new(account, false)],
        );

        let plan = plan_buy_instructions(None, 200_000, 0, buy_ix)
            .expect("Should plan with only CU limit");

        assert_eq!(plan.instructions.len(), 2); // cu_limit + buy
    }

    #[test]
    fn test_plan_buy_instructions_only_priority_fee() {
        let program_id = Pubkey::new_unique();
        let account = Pubkey::new_unique();

        let buy_ix = Instruction::new_with_bytes(
            program_id,
            &[1, 2, 3, 4],
            vec![AccountMeta::new(account, false)],
        );

        let plan = plan_buy_instructions(None, 0, 10_000, buy_ix)
            .expect("Should plan with only priority fee");

        assert_eq!(plan.instructions.len(), 2); // cu_price + buy
    }

    #[test]
    fn test_plan_buy_instructions_empty_buy_accounts() {
        let program_id = Pubkey::new_unique();

        let buy_ix = Instruction::new_with_bytes(program_id, &[1, 2, 3, 4], vec![]);

        let result = plan_buy_instructions(None, 200_000, 10_000, buy_ix);

        assert!(result.is_err());
        if let Err(TransactionBuilderError::Configuration(msg)) = result {
            assert!(msg.contains("no accounts"));
        } else {
            panic!("Expected Configuration error");
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    fn test_sanity_check_valid_durable() {
        let nonce_account = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();

        let instructions = vec![
            system_instruction::advance_nonce_account(&nonce_account, &nonce_authority),
            ComputeBudgetInstruction::set_compute_unit_limit(200_000),
            ComputeBudgetInstruction::set_compute_unit_price(10_000),
            Instruction::new_with_bytes(
                program_id,
                &[1, 2, 3, 4],
                vec![AccountMeta::new(Pubkey::new_unique(), false)],
            ),
        ];

        let result = sanity_check_ix_order(&instructions, true);
        assert!(result.is_ok());
    }

    #[cfg(debug_assertions)]
    #[test]
    fn test_sanity_check_valid_non_durable() {
        let program_id = Pubkey::new_unique();

        let instructions = vec![
            ComputeBudgetInstruction::set_compute_unit_limit(200_000),
            ComputeBudgetInstruction::set_compute_unit_price(10_000),
            Instruction::new_with_bytes(
                program_id,
                &[1, 2, 3, 4],
                vec![AccountMeta::new(Pubkey::new_unique(), false)],
            ),
        ];

        let result = sanity_check_ix_order(&instructions, false);
        assert!(result.is_ok());
    }

    #[cfg(debug_assertions)]
    #[test]
    fn test_sanity_check_empty_list() {
        let instructions = vec![];
        let result = sanity_check_ix_order(&instructions, false);
        assert!(result.is_err());
    }

    #[cfg(debug_assertions)]
    #[test]
    fn test_sanity_check_durable_missing_advance_nonce() {
        let program_id = Pubkey::new_unique();

        let instructions = vec![
            ComputeBudgetInstruction::set_compute_unit_limit(200_000),
            Instruction::new_with_bytes(
                program_id,
                &[1, 2, 3, 4],
                vec![AccountMeta::new(Pubkey::new_unique(), false)],
            ),
        ];

        let result = sanity_check_ix_order(&instructions, true);
        assert!(result.is_err());
        if let Err(TransactionBuilderError::InvalidInstructionOrder(msg)) = result {
            assert!(msg.contains("must start with advance_nonce"));
        } else {
            panic!("Expected InvalidInstructionOrder error");
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    fn test_sanity_check_non_durable_has_advance_nonce() {
        let nonce_account = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();

        let instructions = vec![
            system_instruction::advance_nonce_account(&nonce_account, &nonce_authority),
            Instruction::new_with_bytes(
                Pubkey::new_unique(),
                &[1, 2, 3, 4],
                vec![AccountMeta::new(Pubkey::new_unique(), false)],
            ),
        ];

        let result = sanity_check_ix_order(&instructions, false);
        assert!(result.is_err());
        if let Err(TransactionBuilderError::InvalidInstructionOrder(msg)) = result {
            assert!(msg.contains("should not have advance_nonce"));
        } else {
            panic!("Expected InvalidInstructionOrder error");
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    fn test_sanity_check_multiple_advance_nonce() {
        let nonce_account1 = Pubkey::new_unique();
        let nonce_account2 = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();

        let instructions = vec![
            system_instruction::advance_nonce_account(&nonce_account1, &nonce_authority),
            ComputeBudgetInstruction::set_compute_unit_limit(200_000),
            system_instruction::advance_nonce_account(&nonce_account2, &nonce_authority),
        ];

        let result = sanity_check_ix_order(&instructions, true);
        assert!(result.is_err());
        if let Err(TransactionBuilderError::InvalidInstructionOrder(msg)) = result {
            assert!(msg.contains("Multiple advance_nonce"));
        } else {
            panic!("Expected InvalidInstructionOrder error");
        }
    }
}
