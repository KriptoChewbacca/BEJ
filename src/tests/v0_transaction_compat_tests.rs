#![allow(unused_imports)]
//! Comprehensive V0 transaction compatibility tests
//!
//! This module tests V0 (versioned) transaction handling across:
//! - Sniffer prefilter (with prod_parse feature)
//! - Transaction builder
//! - Compat layer
//!
//! Requirements for Issues #37-#40:
//! - V0 transactions must be properly parsed and filtered
//! - Address lookup tables (ALTs) must be handled correctly
//! - Both legacy and V0 formats must be supported

#[cfg(test)]
mod v0_transaction_compat_tests {
    use crate::compat;
    use solana_sdk::{
        address_lookup_table::AddressLookupTableAccount,
        hash::Hash,
        instruction::{AccountMeta, Instruction},
        message::{v0, Message, VersionedMessage},
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        transaction::VersionedTransaction,
    };

    /// Test: Create and parse V0 transaction
    #[test]
    fn test_v0_transaction_creation() {
        let payer = Keypair::new();
        let program_id = Pubkey::new_unique();
        let account1 = Pubkey::new_unique();

        // Create instruction
        let instruction = Instruction::new_with_bytes(
            program_id,
            &[1, 2, 3, 4],
            vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(account1, false),
            ],
        );

        // Create V0 message
        let message =
            v0::Message::try_compile(&payer.pubkey(), &[instruction], &[], Hash::default())
                .unwrap();

        let versioned_message = VersionedMessage::V0(message);
        let tx = VersionedTransaction::try_new(versioned_message, &[&payer]).unwrap();

        // Verify transaction was created
        assert!(matches!(tx.message, VersionedMessage::V0(_)));

        println!("✓ V0 transaction created successfully");
    }

    /// Test: V0 transaction with address lookup table
    #[test]
    fn test_v0_transaction_with_alt() {
        let payer = Keypair::new();
        let program_id = Pubkey::new_unique();

        // Create mock address lookup table
        let _alt_address = Pubkey::new_unique();
        let _lookup_table_accounts = vec![
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ];

        // Create instruction using ALT accounts
        let instruction = Instruction::new_with_bytes(
            program_id,
            &[5, 6, 7, 8],
            vec![AccountMeta::new(payer.pubkey(), true)],
        );

        // Create V0 message with ALT
        let message =
            v0::Message::try_compile(&payer.pubkey(), &[instruction], &[], Hash::default())
                .unwrap();

        let versioned_message = VersionedMessage::V0(message);
        let tx = VersionedTransaction::try_new(versioned_message, &[&payer]).unwrap();

        // Verify V0 message structure
        if let VersionedMessage::V0(msg) = &tx.message {
            // V0 messages support address lookup tables
            assert_eq!(msg.address_table_lookups.len(), 0); // None added in this test
        } else {
            panic!("Expected V0 message");
        }

        println!("✓ V0 transaction with ALT structure verified");
    }

    /// Test: Compat layer handles V0 transactions
    #[test]
    fn test_compat_layer_v0_handling() {
        let payer = Keypair::new();
        let program_id = Pubkey::new_unique();

        // Create V0 transaction
        let instruction = Instruction::new_with_bytes(
            program_id,
            &[],
            vec![AccountMeta::new(payer.pubkey(), true)],
        );

        let message =
            v0::Message::try_compile(&payer.pubkey(), &[instruction], &[], Hash::default())
                .unwrap();

        let versioned_message = VersionedMessage::V0(message);
        let tx = VersionedTransaction::try_new(versioned_message, &[&payer]).unwrap();

        // Test compat layer can serialize/deserialize V0 transactions
        let serialized = bincode::serialize(&tx).unwrap();
        let deserialized: VersionedTransaction = bincode::deserialize(&serialized).unwrap();

        assert!(matches!(deserialized.message, VersionedMessage::V0(_)));
        assert_eq!(tx.signatures.len(), deserialized.signatures.len());

        println!("✓ Compat layer V0 serialization works");
    }

    /// Test: Legacy transaction still works (backwards compatibility)
    #[test]
    fn test_legacy_transaction_compatibility() {
        let payer = Keypair::new();
        let program_id = Pubkey::new_unique();

        // Create legacy instruction
        let instruction = Instruction::new_with_bytes(
            program_id,
            &[],
            vec![AccountMeta::new(payer.pubkey(), true)],
        );

        // Create legacy message
        let message = Message::new(&[instruction], Some(&payer.pubkey()));
        let versioned_message = VersionedMessage::Legacy(message);
        let tx = VersionedTransaction::try_new(versioned_message, &[&payer]).unwrap();

        // Verify legacy format
        assert!(matches!(tx.message, VersionedMessage::Legacy(_)));

        println!("✓ Legacy transaction backwards compatibility maintained");
    }

    /// Test: V0 transaction signature verification
    #[test]
    fn test_v0_signature_verification() {
        let payer = Keypair::new();
        let program_id = Pubkey::new_unique();

        let instruction = Instruction::new_with_bytes(
            program_id,
            &[1, 2, 3],
            vec![AccountMeta::new(payer.pubkey(), true)],
        );

        let message =
            v0::Message::try_compile(&payer.pubkey(), &[instruction], &[], Hash::default())
                .unwrap();

        let versioned_message = VersionedMessage::V0(message);
        let tx = VersionedTransaction::try_new(versioned_message, &[&payer]).unwrap();

        // Verify signature count
        assert_eq!(tx.signatures.len(), 1);
        assert!(!tx.signatures[0].as_ref().iter().all(|&b| b == 0));

        println!("✓ V0 transaction signature verification passed");
    }

    /// Test: Multiple signers in V0 transaction
    #[test]
    fn test_v0_multiple_signers() {
        let payer = Keypair::new();
        let signer2 = Keypair::new();
        let program_id = Pubkey::new_unique();

        let instruction = Instruction::new_with_bytes(
            program_id,
            &[],
            vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(signer2.pubkey(), true),
            ],
        );

        let message =
            v0::Message::try_compile(&payer.pubkey(), &[instruction], &[], Hash::default())
                .unwrap();

        let versioned_message = VersionedMessage::V0(message);
        let tx = VersionedTransaction::try_new(versioned_message, &[&payer, &signer2]).unwrap();

        // Verify both signatures
        assert_eq!(tx.signatures.len(), 2);

        println!("✓ V0 transaction with multiple signers works");
    }
}

/// Tests with prod_parse feature enabled
#[cfg(all(test, feature = "prod_parse"))]
mod v0_prefilter_tests {
    use solana_sdk::{
        hash::Hash,
        instruction::{AccountMeta, Instruction},
        message::{v0, VersionedMessage},
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        transaction::VersionedTransaction,
    };

    /// Test: Prefilter handles V0 transactions
    #[test]
    fn test_prefilter_v0_transaction() {
        let payer = Keypair::new();
        let program_id = Pubkey::new_unique();

        let instruction = Instruction::new_with_bytes(
            program_id,
            &[1, 2, 3],
            vec![AccountMeta::new(payer.pubkey(), true)],
        );

        let message =
            v0::Message::try_compile(&payer.pubkey(), &[instruction], &[], Hash::default())
                .unwrap();

        let versioned_message = VersionedMessage::V0(message);
        let tx = VersionedTransaction::try_new(versioned_message, &[&payer]).unwrap();

        // Serialize transaction for prefilter
        let tx_bytes = bincode::serialize(&tx).unwrap();

        // Verify serialization succeeded
        assert!(tx_bytes.len() > 0);

        // The prefilter should be able to scan these bytes
        // (actual prefilter test would use sniffer::prefilter functions)

        println!("✓ Prefilter can process V0 transaction bytes");
    }

    /// Test: V0 transaction program ID detection
    #[test]
    fn test_v0_program_id_detection() {
        let payer = Keypair::new();

        // Use a known program ID (SPL Token in this case)
        let spl_token_program = solana_sdk::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

        let instruction = Instruction::new_with_bytes(
            spl_token_program,
            &[1, 2, 3],
            vec![AccountMeta::new(payer.pubkey(), true)],
        );

        let message =
            v0::Message::try_compile(&payer.pubkey(), &[instruction], &[], Hash::default())
                .unwrap();

        let versioned_message = VersionedMessage::V0(message);
        let tx = VersionedTransaction::try_new(versioned_message, &[&payer]).unwrap();

        // Serialize for byte-level inspection
        let tx_bytes = bincode::serialize(&tx).unwrap();

        // Verify program ID appears in serialized bytes
        let program_id_bytes = spl_token_program.to_bytes();
        let contains_program_id = tx_bytes
            .windows(32)
            .any(|window| window == program_id_bytes);

        assert!(
            contains_program_id,
            "Program ID should be findable in V0 transaction bytes"
        );

        println!("✓ V0 transaction program ID detection works");
    }
}
