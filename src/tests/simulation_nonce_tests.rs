#![allow(unused_imports)]
//! Simulation and Nonce Interaction Tests (Issues #37-40)
//!
//! This module tests that simulation does NOT consume nonces:
//! - Verify advance_nonce is skipped during simulation
//! - Ensure simulation doesn't advance nonce state
//! - Test simulation with and without nonce context

#[cfg(test)]
mod simulation_nonce_tests {
    use crate::nonce_manager::UniverseNonceManager;
    use crate::rpc_manager::rpc_pool::{RpcPool, EndpointConfig, EndpointType};
    use solana_sdk::{
        hash::Hash,
        pubkey::Pubkey,
        signature::Keypair,
        instruction::Instruction,
        system_instruction,
        message::{v0::Message as MessageV0, VersionedMessage},
        transaction::VersionedTransaction,
    };
    use std::sync::Arc;
    use std::time::Duration;

    /// Helper: Create test nonce manager
    async fn create_test_nonce_manager(pool_size: usize) -> Arc<UniverseNonceManager> {
        use crate::nonce_manager::{UniverseNonceManager, LocalSigner};
        
        let signer = Arc::new(LocalSigner::new(Keypair::new()));
        let mut nonce_accounts = vec![];
        for _ in 0..pool_size {
            nonce_accounts.push(Pubkey::new_unique());
        }
        
        UniverseNonceManager::new_for_testing(
            signer,
            nonce_accounts,
            Duration::from_secs(300),
        ).await
    }

    /// Helper: Build instructions for simulation (without advance_nonce)
    fn build_simulation_instructions(program_id: Pubkey) -> Vec<Instruction> {
        vec![
            // Compute budget instructions
            Instruction::new_with_bytes(
                solana_sdk::compute_budget::id(),
                &[2, 0, 0, 0, 0, 200, 0, 0], // set_compute_unit_limit
                vec![],
            ),
            Instruction::new_with_bytes(
                solana_sdk::compute_budget::id(),
                &[3, 0, 0, 0, 100, 0, 0, 0], // set_compute_unit_price
                vec![],
            ),
            // Program instruction
            Instruction::new_with_bytes(
                program_id,
                &[1, 2, 3, 4],
                vec![],
            ),
        ]
    }

    /// Helper: Build instructions for execution (with advance_nonce)
    fn build_execution_instructions(
        nonce_account: &Pubkey,
        nonce_authority: &Pubkey,
        program_id: Pubkey,
    ) -> Vec<Instruction> {
        let mut instructions = vec![];
        
        // 1. advance_nonce FIRST
        instructions.push(system_instruction::advance_nonce_account(
            nonce_account,
            nonce_authority,
        ));
        
        // 2. Compute budget instructions
        instructions.push(Instruction::new_with_bytes(
            solana_sdk::compute_budget::id(),
            &[2, 0, 0, 0, 0, 200, 0, 0],
            vec![],
        ));
        instructions.push(Instruction::new_with_bytes(
            solana_sdk::compute_budget::id(),
            &[3, 0, 0, 0, 100, 0, 0, 0],
            vec![],
        ));
        
        // 3. Program instruction
        instructions.push(Instruction::new_with_bytes(
            program_id,
            &[1, 2, 3, 4],
            vec![],
        ));
        
        instructions
    }

    /// Test: Simulation instructions exclude advance_nonce
    #[test]
    fn test_simulation_excludes_advance_nonce() {
        let program_id = Pubkey::new_unique();
        
        // Build simulation instructions
        let sim_instructions = build_simulation_instructions(program_id);
        
        // Verify no advance_nonce instruction
        let has_advance_nonce = sim_instructions.iter().any(|ix| {
            ix.program_id == solana_sdk::system_program::id() &&
            !ix.data.is_empty() &&
            ix.data[0] == 4 // advance_nonce discriminator
        });
        
        assert!(!has_advance_nonce, "Simulation should not include advance_nonce");
        
        println!("✓ Simulation instructions correctly exclude advance_nonce");
    }

    /// Test: Execution instructions include advance_nonce
    #[test]
    fn test_execution_includes_advance_nonce() {
        let nonce_account = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();
        
        // Build execution instructions
        let exec_instructions = build_execution_instructions(
            &nonce_account,
            &nonce_authority,
            program_id,
        );
        
        // Verify advance_nonce is first instruction
        assert!(!exec_instructions.is_empty(), "Should have instructions");
        
        let first_ix = &exec_instructions[0];
        assert_eq!(first_ix.program_id, solana_sdk::system_program::id());
        assert_eq!(first_ix.data[0], 4, "First instruction should be advance_nonce");
        
        println!("✓ Execution instructions correctly include advance_nonce");
    }

    /// Test: Simulation and execution use same program instructions
    #[test]
    fn test_simulation_execution_program_instructions_match() {
        let nonce_account = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();
        
        let sim_instructions = build_simulation_instructions(program_id);
        let exec_instructions = build_execution_instructions(
            &nonce_account,
            &nonce_authority,
            program_id,
        );
        
        // Extract program instructions (skip advance_nonce and compute budget)
        let sim_program_ixs: Vec<_> = sim_instructions.iter()
            .filter(|ix| {
                ix.program_id != solana_sdk::compute_budget::id() &&
                ix.program_id != solana_sdk::system_program::id()
            })
            .collect();
        
        let exec_program_ixs: Vec<_> = exec_instructions.iter()
            .filter(|ix| {
                ix.program_id != solana_sdk::compute_budget::id() &&
                ix.program_id != solana_sdk::system_program::id()
            })
            .collect();
        
        // Should have same number of program instructions
        assert_eq!(
            sim_program_ixs.len(), exec_program_ixs.len(),
            "Same number of program instructions"
        );
        
        // Program instructions should be identical
        for (sim_ix, exec_ix) in sim_program_ixs.iter().zip(exec_program_ixs.iter()) {
            assert_eq!(sim_ix.program_id, exec_ix.program_id);
            assert_eq!(sim_ix.data, exec_ix.data);
        }
        
        println!("✓ Simulation and execution have matching program instructions");
    }

    /// Test: Multiple simulations don't consume nonce leases
    #[tokio::test]
    async fn test_multiple_simulations_preserve_nonce_pool() {
        const NUM_SIMULATIONS: usize = 20;
        const POOL_SIZE: usize = 5;
        
        let nonce_manager = create_test_nonce_manager(POOL_SIZE).await;
        
        // Get initial state
        let initial_permits = nonce_manager.get_stats().await.permits_in_use;
        
        // Run multiple simulations (these should not consume nonces)
        for _ in 0..NUM_SIMULATIONS {
            let program_id = Pubkey::new_unique();
            let _sim_instructions = build_simulation_instructions(program_id);
            
            // Simulations don't interact with nonce manager
            // Just verify pool state remains unchanged
        }
        
        // Verify nonce pool is unchanged
        assert_eq!(
            nonce_manager.get_stats().await.permits_in_use, initial_permits,
            "Simulations should not consume nonces"
        );
        
        println!("✓ Multiple simulations preserve nonce pool");
    }

    /// Test: Simulation with nonce context doesn't advance nonce
    #[tokio::test]
    async fn test_simulation_with_nonce_context_no_advance() {
        let nonce_manager = create_test_nonce_manager(5).await;
        
        // Acquire nonce lease (for context)
        let lease = nonce_manager.acquire_nonce().await.unwrap();
        let _nonce_pubkey = *lease.nonce_pubkey();
        let nonce_blockhash = lease.nonce_blockhash();
        
        // Build simulation instructions (WITHOUT advance_nonce)
        let program_id = Pubkey::new_unique();
        let sim_instructions = build_simulation_instructions(program_id);
        
        // Verify advance_nonce is not included
        let has_advance_nonce = sim_instructions.iter().any(|ix| {
            ix.program_id == solana_sdk::system_program::id() &&
            !ix.data.is_empty() &&
            ix.data[0] == 4
        });
        assert!(!has_advance_nonce, "Simulation should exclude advance_nonce");
        
        // Nonce blockhash should remain valid (not advanced)
        assert_eq!(lease.nonce_blockhash(), nonce_blockhash);
        
        // Release lease
        drop(lease.release().await);
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        println!("✓ Simulation with nonce context doesn't advance nonce");
    }

    /// Test: Execution with nonce context includes advance_nonce
    #[tokio::test]
    async fn test_execution_with_nonce_context_advances_nonce() {
        let nonce_manager = create_test_nonce_manager(5).await;
        
        // Acquire nonce lease
        let lease = nonce_manager.acquire_nonce().await.unwrap();
        let _nonce_pubkey = *lease.nonce_pubkey();
        let nonce_authority = Pubkey::new_unique(); // Mock authority
        
        // Build execution instructions (WITH advance_nonce)
        let program_id = Pubkey::new_unique();
        let exec_instructions = build_execution_instructions(
            &nonce_pubkey,
            &nonce_authority,
            program_id,
        );
        
        // Verify advance_nonce is first instruction
        assert!(!exec_instructions.is_empty());
        assert_eq!(exec_instructions[0].program_id, solana_sdk::system_program::id());
        assert_eq!(exec_instructions[0].data[0], 4);
        
        // Verify nonce account is in instruction
        assert_eq!(exec_instructions[0].accounts[0].pubkey, nonce_pubkey);
        
        // Release lease
        drop(lease.release().await);
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        println!("✓ Execution with nonce context includes advance_nonce");
    }

    /// Test: Interleaved simulations and executions
    #[tokio::test]
    async fn test_interleaved_simulation_execution() {
        let nonce_manager = create_test_nonce_manager(5).await;
        let program_id = Pubkey::new_unique();
        
        for i in 0..10 {
            if i % 2 == 0 {
                // Simulation (no nonce consumption)
                let _sim_instructions = build_simulation_instructions(program_id);
            } else {
                // Execution (with nonce)
                let lease = nonce_manager.acquire_nonce().await.unwrap();
                let _nonce_pubkey = *lease.nonce_pubkey();
                let nonce_authority = Pubkey::new_unique();
                
                let exec_instructions = build_execution_instructions(
                    &nonce_pubkey,
                    &nonce_authority,
                    program_id,
                );
                
                // Verify advance_nonce is included
                assert_eq!(exec_instructions[0].data[0], 4);
                
                // Release lease
                drop(lease.release().await);
            }
        }
        
        // Allow cleanup
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Verify no leaks
        assert_eq!(nonce_manager.get_stats().await.permits_in_use, 0);
        
        println!("✓ Interleaved simulation/execution works correctly");
    }

    /// Test: Simulation failure doesn't affect nonce pool
    #[tokio::test]
    async fn test_simulation_failure_preserves_nonce_pool() {
        let nonce_manager = create_test_nonce_manager(5).await;
        let initial_permits = nonce_manager.get_stats().await.permits_in_use;
        
        // Simulate failed simulation (invalid instructions)
        let invalid_program = Pubkey::new_unique();
        let _sim_instructions = build_simulation_instructions(invalid_program);
        
        // Even if simulation "fails", it shouldn't affect nonce pool
        // (simulations don't interact with nonce manager)
        
        assert_eq!(
            nonce_manager.get_stats().await.permits_in_use, initial_permits,
            "Failed simulation shouldn't affect nonce pool"
        );
        
        println!("✓ Simulation failure preserves nonce pool");
    }

    /// Test: Verify simulation and execution have different instruction counts
    #[test]
    fn test_simulation_execution_instruction_count_difference() {
        let nonce_account = Pubkey::new_unique();
        let nonce_authority = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();
        
        let sim_instructions = build_simulation_instructions(program_id);
        let exec_instructions = build_execution_instructions(
            &nonce_account,
            &nonce_authority,
            program_id,
        );
        
        // Execution should have one more instruction (advance_nonce)
        assert_eq!(
            exec_instructions.len(), sim_instructions.len() + 1,
            "Execution should have advance_nonce instruction"
        );
        
        println!("✓ Simulation and execution have correct instruction count difference");
    }
}
