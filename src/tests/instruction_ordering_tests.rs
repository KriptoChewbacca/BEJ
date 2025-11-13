#![allow(unused_imports)]
//! Instruction Ordering Tests (Issues #37-40)
//!
//! This module tests that advance_nonce instruction is correctly ordered:
//! - Positive: Verify advance_nonce comes first in nonce transactions
//! - Negative: Detect missing advance_nonce when nonce is used
//! - Sanity checks for debug/test builds

#[cfg(test)]
mod instruction_ordering_tests {
    use solana_sdk::{
        hash::Hash,
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        system_instruction, system_program,
    };

    /// Helper: Check if instruction is an advance_nonce_account instruction
    fn is_advance_nonce_instruction(ix: &Instruction) -> bool {
        // Must be system program
        if ix.program_id != system_program::id() {
            return false;
        }

        // advance_nonce_account has discriminator 4
        // Format: [4, 0, 0, 0] (u32 little-endian)
        if ix.data.len() >= 4
            && ix.data[0] == 4
            && ix.data[1] == 0
            && ix.data[2] == 0
            && ix.data[3] == 0
        {
            return true;
        }

        false
    }

    /// Helper: Verify nonce transaction instruction ordering
    ///
    /// Expected order for nonce transactions:
    /// 1. advance_nonce_account (REQUIRED FIRST)
    /// 2. compute_budget instructions (optional)
    /// 3. program instructions
    fn verify_nonce_instruction_order(instructions: &[Instruction]) -> Result<(), String> {
        if instructions.is_empty() {
            return Err("No instructions found".to_string());
        }

        // First instruction MUST be advance_nonce
        if !is_advance_nonce_instruction(&instructions[0]) {
            return Err(format!(
                "First instruction must be advance_nonce_account, got program_id: {}",
                instructions[0].program_id
            ));
        }

        // Verify no other advance_nonce instructions after the first
        for (idx, ix) in instructions.iter().enumerate().skip(1) {
            if is_advance_nonce_instruction(ix) {
                return Err(format!(
                    "advance_nonce_account instruction found at position {} (should only be first)",
                    idx
                ));
            }
        }

        Ok(())
    }

    /// Test: Valid nonce transaction with correct ordering
    #[test]
    fn test_valid_nonce_instruction_ordering() {
        let nonce_account = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();

        // Build instruction list with correct ordering
        let mut instructions = vec![];

        // 1. advance_nonce (MUST BE FIRST)
        instructions.push(system_instruction::advance_nonce_account(
            &nonce_account,
            &nonce_authority,
        ));

        // 2. Compute budget instruction (optional)
        instructions.push(Instruction::new_with_bytes(
            solana_sdk::compute_budget::id(),
            &[3, 0, 0, 0, 100, 0, 0, 0], // set_compute_unit_price
            vec![],
        ));

        // 3. Program instruction
        instructions.push(Instruction::new_with_bytes(
            program_id,
            &[1, 2, 3, 4],
            vec![AccountMeta::new(Pubkey::new_unique(), false)],
        ));

        // Verify ordering
        let result = verify_nonce_instruction_order(&instructions);
        assert!(result.is_ok(), "Valid ordering should pass: {:?}", result);

        println!("✓ Valid nonce instruction ordering verified");
    }

    /// Test: Invalid - advance_nonce not first
    #[test]
    fn test_invalid_advance_nonce_not_first() {
        let nonce_account = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();

        // Build instruction list with INCORRECT ordering
        let mut instructions = vec![];

        // 1. Program instruction (WRONG - should be nonce first)
        instructions.push(Instruction::new_with_bytes(
            program_id,
            &[1, 2, 3, 4],
            vec![AccountMeta::new(Pubkey::new_unique(), false)],
        ));

        // 2. advance_nonce (WRONG POSITION)
        instructions.push(system_instruction::advance_nonce_account(
            &nonce_account,
            &nonce_authority,
        ));

        // Verify ordering fails
        let result = verify_nonce_instruction_order(&instructions);
        assert!(result.is_err(), "Invalid ordering should fail");
        assert!(
            result
                .unwrap_err()
                .contains("First instruction must be advance_nonce"),
            "Error message should indicate first instruction requirement"
        );

        println!("✓ Invalid ordering correctly detected (advance_nonce not first)");
    }

    /// Test: Invalid - missing advance_nonce entirely
    #[test]
    fn test_invalid_missing_advance_nonce() {
        let program_id = Pubkey::new_unique();

        // Build instruction list WITHOUT advance_nonce
        let mut instructions = vec![];

        // Only program instructions (missing advance_nonce)
        instructions.push(Instruction::new_with_bytes(
            program_id,
            &[1, 2, 3, 4],
            vec![AccountMeta::new(Pubkey::new_unique(), false)],
        ));
        instructions.push(Instruction::new_with_bytes(
            program_id,
            &[5, 6, 7, 8],
            vec![AccountMeta::new(Pubkey::new_unique(), false)],
        ));

        // Verify ordering fails
        let result = verify_nonce_instruction_order(&instructions);
        assert!(result.is_err(), "Missing advance_nonce should fail");
        assert!(
            result
                .unwrap_err()
                .contains("First instruction must be advance_nonce"),
            "Error message should indicate missing advance_nonce"
        );

        println!("✓ Missing advance_nonce correctly detected");
    }

    /// Test: Invalid - multiple advance_nonce instructions
    #[test]
    fn test_invalid_multiple_advance_nonce() {
        let nonce_account1 = Pubkey::new_unique();
        let nonce_account2 = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();

        // Build instruction list with DUPLICATE advance_nonce
        let mut instructions = vec![];

        // 1. advance_nonce (correct)
        instructions.push(system_instruction::advance_nonce_account(
            &nonce_account1,
            &nonce_authority,
        ));

        // 2. Another advance_nonce (WRONG - should be only one)
        instructions.push(system_instruction::advance_nonce_account(
            &nonce_account2,
            &nonce_authority,
        ));

        // Verify ordering fails
        let result = verify_nonce_instruction_order(&instructions);
        assert!(result.is_err(), "Multiple advance_nonce should fail");
        assert!(
            result.unwrap_err().contains("should only be first"),
            "Error message should indicate duplicate advance_nonce"
        );

        println!("✓ Multiple advance_nonce correctly detected");
    }

    /// Test: Empty instruction list
    #[test]
    fn test_empty_instruction_list() {
        let instructions = vec![];

        let result = verify_nonce_instruction_order(&instructions);
        assert!(result.is_err(), "Empty instruction list should fail");
        assert!(
            result.unwrap_err().contains("No instructions"),
            "Error message should indicate empty list"
        );

        println!("✓ Empty instruction list correctly rejected");
    }

    /// Test: Non-nonce transaction (blockhash-based) should not have advance_nonce
    #[test]
    fn test_blockhash_transaction_no_advance_nonce() {
        let program_id = Pubkey::new_unique();

        // Build typical blockhash-based transaction (no nonce)
        let mut instructions = vec![];

        // Compute budget instruction
        instructions.push(Instruction::new_with_bytes(
            solana_sdk::compute_budget::id(),
            &[3, 0, 0, 0, 100, 0, 0, 0],
            vec![],
        ));

        // Program instruction
        instructions.push(Instruction::new_with_bytes(
            program_id,
            &[1, 2, 3, 4],
            vec![AccountMeta::new(Pubkey::new_unique(), false)],
        ));

        // Verify no advance_nonce instructions
        let has_advance_nonce = instructions.iter().any(is_advance_nonce_instruction);
        assert!(
            !has_advance_nonce,
            "Blockhash transaction should not have advance_nonce"
        );

        println!("✓ Blockhash transaction correctly has no advance_nonce");
    }

    /// Test: Verify advance_nonce instruction structure
    #[test]
    fn test_advance_nonce_instruction_structure() {
        let nonce_account = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();

        let ix = system_instruction::advance_nonce_account(&nonce_account, &nonce_authority);

        // Verify program ID
        assert_eq!(
            ix.program_id,
            system_program::id(),
            "Must be system program"
        );

        // Verify accounts
        // advance_nonce_account has 3 accounts:
        // 0. Nonce account (writable)
        // 1. RecentBlockhashes sysvar (read-only)
        // 2. Nonce authority (signer)
        assert_eq!(ix.accounts.len(), 3, "Should have 3 accounts");
        assert_eq!(
            ix.accounts[0].pubkey, nonce_account,
            "First account is nonce"
        );
        assert_eq!(
            ix.accounts[2].pubkey, nonce_authority,
            "Third account is authority"
        );

        // Verify it's detected as advance_nonce
        assert!(
            is_advance_nonce_instruction(&ix),
            "Should be detected as advance_nonce"
        );

        println!("✓ advance_nonce instruction structure verified");
    }

    /// Test: Complex valid transaction with multiple instruction types
    #[test]
    fn test_complex_valid_nonce_transaction() {
        let nonce_account = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();

        let mut instructions = vec![];

        // 1. advance_nonce (FIRST - REQUIRED)
        instructions.push(system_instruction::advance_nonce_account(
            &nonce_account,
            &nonce_authority,
        ));

        // 2. Multiple compute budget instructions
        instructions.push(Instruction::new_with_bytes(
            solana_sdk::compute_budget::id(),
            &[2, 0, 0, 0, 0, 200, 0, 0], // set_compute_unit_limit
            vec![],
        ));
        instructions.push(Instruction::new_with_bytes(
            solana_sdk::compute_budget::id(),
            &[3, 0, 0, 0, 100, 0, 0, 0], // set_compute_unit_price
            vec![],
        ));

        // 3. Multiple program instructions
        for i in 0..5 {
            instructions.push(Instruction::new_with_bytes(
                program_id,
                &[i as u8, 1, 2, 3],
                vec![AccountMeta::new(Pubkey::new_unique(), false)],
            ));
        }

        // Verify ordering
        let result = verify_nonce_instruction_order(&instructions);
        assert!(
            result.is_ok(),
            "Complex valid ordering should pass: {:?}",
            result
        );

        // Verify first instruction
        assert!(is_advance_nonce_instruction(&instructions[0]));

        // Verify no other advance_nonce instructions
        for ix in instructions.iter().skip(1) {
            assert!(!is_advance_nonce_instruction(ix));
        }

        println!("✓ Complex valid nonce transaction verified");
    }

    /// Test: Instruction ordering detection is deterministic
    #[test]
    fn test_ordering_detection_is_deterministic() {
        let nonce_account = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();

        // Build same instruction list multiple times
        for _ in 0..10 {
            let mut instructions = vec![];

            instructions.push(system_instruction::advance_nonce_account(
                &nonce_account,
                &nonce_authority,
            ));
            instructions.push(Instruction::new_with_bytes(
                program_id,
                &[1, 2, 3, 4],
                vec![AccountMeta::new(Pubkey::new_unique(), false)],
            ));

            // Should always pass
            let result = verify_nonce_instruction_order(&instructions);
            assert!(result.is_ok(), "Deterministic check should always pass");
        }

        println!("✓ Ordering detection is deterministic");
    }
}
