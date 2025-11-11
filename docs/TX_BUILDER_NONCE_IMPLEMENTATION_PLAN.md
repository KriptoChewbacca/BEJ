# Transaction Builder Nonce Management Implementation Plan

## Overview
This document outlines the implementation plan for three interrelated tasks that enhance nonce management in the `tx_builder.rs` module. These changes introduce proper RAII patterns for nonce leases, enforce durable nonce usage for trading operations, and ensure correct instruction ordering.

---

## Task 1: Default Nonce Mode Selection for Trading Operations

### Objective
Enforce durable nonce as the default for critical trading operations (buy/sell transactions) with explicit control via `enforce_nonce` parameter.

### Changes Required

#### 1.1 Add `enforce_nonce` Parameter

**File**: `src/tx_builder.rs`

**Method Signatures to Modify**:
```rust
// Current signature
pub async fn build_buy_transaction(
    &self,
    candidate: &PremintCandidate,
    config: &TransactionConfig,
    sign: bool,
) -> Result<VersionedTransaction, TransactionBuilderError>

// New primary signature (backward compatible wrapper)
pub async fn build_buy_transaction(
    &self,
    candidate: &PremintCandidate,
    config: &TransactionConfig,
    sign: bool,
) -> Result<VersionedTransaction, TransactionBuilderError> {
    self.build_buy_transaction_with_nonce(candidate, config, sign, true).await
}

// New detailed signature
pub async fn build_buy_transaction_with_nonce(
    &self,
    candidate: &PremintCandidate,
    config: &TransactionConfig,
    sign: bool,
    enforce_nonce: bool,
) -> Result<VersionedTransaction, TransactionBuilderError>
```

Apply similar changes to `build_sell_transaction`.

#### 1.2 Default Priority to CriticalSniper

In the method body of `build_buy_transaction_with_nonce`:
```rust
// Default to CriticalSniper priority if not explicitly set
let mut effective_config = config.clone();
if effective_config.operation_priority == OperationPriority::Utility {
    effective_config.operation_priority = OperationPriority::CriticalSniper;
}
```

#### 1.3 Nonce Availability Validation

Add validation before attempting nonce acquisition:
```rust
if enforce_nonce && effective_config.operation_priority.requires_nonce() {
    let available = self.nonce_manager.available_permits();
    if available == 0 {
        return Err(TransactionBuilderError::NonceAcquisition(
            "No available nonces for durable mode".to_string()
        ));
    }
}
```

#### 1.4 ExecutionContext Enhancement

Modify `prepare_execution_context` to accept `enforce_nonce`:
```rust
async fn prepare_execution_context_with_enforcement(
    &self,
    config: &TransactionConfig,
    enforce_nonce: bool,
) -> Result<ExecutionContext, TransactionBuilderError>
```

Logic:
- If `enforce_nonce == true`: Acquire nonce lease with TTL=30s
- If `enforce_nonce == false`: Fallback to `get_recent_blockhash_with_quorum`

#### 1.5 BuyEngine Integration

**File**: `src/buy_engine.rs`

Update `try_buy` and related methods to use `enforce_nonce=true` by default:
```rust
let tx = builder.build_buy_transaction_with_nonce(&candidate, &config, false, true).await?;
```

For utility operations (e.g., unwrap WSOL), use `enforce_nonce=false`.

### Testing Requirements

1. **Unit Test**: Default priority enforcement
   ```rust
   #[tokio::test]
   async fn test_default_critical_sniper_priority() {
       // Create config with Utility priority
       // Call build_buy_transaction
       // Assert that CriticalSniper is used
   }
   ```

2. **Integration Test**: Nonce availability check
   ```rust
   #[tokio::test]
   async fn test_nonce_availability_error() {
       // Exhaust all nonces
       // Attempt build with enforce_nonce=true
       // Expect NonceAcquisition error
   }
   ```

3. **Integration Test**: Concurrent nonce acquisition
   ```rust
   #[tokio::test]
   async fn test_concurrent_nonce_acquisition_race() {
       // Spawn multiple concurrent build_buy_transaction calls
       // Verify no double-acquisition
       // Verify no invalid nonce hash usage
   }
   ```

---

## Task 2: Lease Lifetime (RAII) Management

### Objective
Introduce `TxBuildOutput` struct to hold nonce lease with RAII semantics, ensuring nonce is held until broadcast completes.

### Changes Required

#### 2.1 Define `TxBuildOutput` Struct

**File**: `src/tx_builder.rs`

```rust
/// Output from transaction building with nonce lease (Task 2)
/// 
/// This struct ensures the nonce lease is held via RAII until the transaction
/// is successfully broadcasted. The lease is automatically released when dropped.
pub struct TxBuildOutput {
    /// The built transaction ready for signing/broadcast
    pub tx: VersionedTransaction,
    
    /// Optional nonce lease guard (held until broadcast completes)
    /// Automatically released on drop via RAII pattern
    pub nonce_guard: Option<NonceLease>,
    
    /// List of required signers for this transaction
    /// Extracted from message.header.num_required_signatures
    pub required_signers: Vec<Pubkey>,
}

impl TxBuildOutput {
    /// Create new output with nonce guard
    pub fn new(
        tx: VersionedTransaction,
        nonce_guard: Option<NonceLease>,
    ) -> Self {
        // Extract required signers from transaction message
        let required_signers = Self::extract_required_signers(&tx);
        
        Self {
            tx,
            nonce_guard,
            required_signers,
        }
    }
    
    /// Extract required signers from transaction
    fn extract_required_signers(tx: &VersionedTransaction) -> Vec<Pubkey> {
        let num_signers = tx.message.header().num_required_signatures as usize;
        tx.message.static_account_keys()
            .iter()
            .take(num_signers)
            .copied()
            .collect()
    }
    
    /// Explicitly release nonce guard (if held)
    pub async fn release_nonce(mut self) -> Result<(), TransactionBuilderError> {
        if let Some(guard) = self.nonce_guard.take() {
            guard.release().await
                .map_err(|e| TransactionBuilderError::NonceAcquisition(e.to_string()))?;
        }
        Ok(())
    }
}

impl Drop for TxBuildOutput {
    fn drop(&mut self) {
        if self.nonce_guard.is_some() {
            warn!("TxBuildOutput dropped with active nonce guard - lease will be released");
        }
    }
}
```

#### 2.2 Add New Output Methods

```rust
/// Build buy transaction with output structure (Task 2)
/// 
/// Returns TxBuildOutput which holds the nonce lease until explicitly released
/// or dropped. This ensures proper RAII semantics for nonce management.
pub async fn build_buy_transaction_output(
    &self,
    candidate: &PremintCandidate,
    config: &TransactionConfig,
    sign: bool,
    enforce_nonce: bool,
) -> Result<TxBuildOutput, TransactionBuilderError> {
    // Similar to build_buy_transaction_with_nonce but:
    // 1. Keep exec_ctx in scope
    // 2. Extract nonce_lease from exec_ctx before drop
    // 3. Return TxBuildOutput with lease
    
    let exec_ctx = self.prepare_execution_context_with_enforcement(config, enforce_nonce).await?;
    
    // Build transaction...
    let tx = /* ... build transaction logic ... */;
    
    // Extract nonce lease from exec_ctx before it drops
    let nonce_lease = exec_ctx.extract_lease(); // New method needed
    
    Ok(TxBuildOutput::new(tx, nonce_lease))
}

/// Legacy wrapper - releases nonce early with warning
pub async fn build_buy_transaction_with_nonce(
    &self,
    candidate: &PremintCandidate,
    config: &TransactionConfig,
    sign: bool,
    enforce_nonce: bool,
) -> Result<VersionedTransaction, TransactionBuilderError> {
    let output = self.build_buy_transaction_output(candidate, config, sign, enforce_nonce).await?;
    
    warn!("Legacy API: releasing nonce early - migrate to build_buy_transaction_output for safety");
    
    Ok(output.tx)
    // nonce_guard dropped here (early release)
}
```

Apply similar changes to sell transaction methods.

#### 2.3 Modify ExecutionContext

Update `ExecutionContext` to allow extracting the lease:

```rust
struct ExecutionContext {
    blockhash: Hash,
    nonce_pubkey: Option<Pubkey>,
    nonce_authority: Option<Pubkey>,
    _nonce_lease: Option<NonceLease>,
    zk_proof: Option<ZkProofData>,
}

impl ExecutionContext {
    /// Extract the nonce lease, consuming it (Task 2)
    pub fn extract_lease(mut self) -> Option<NonceLease> {
        self._nonce_lease.take()
    }
}
```

#### 2.4 BuyEngine Integration

**File**: `src/buy_engine.rs`

Update `try_buy` to use output methods:

```rust
async fn try_buy(&self, candidate: PremintCandidate, ctx: PipelineContext) -> Result<Signature> {
    // Build transaction with output
    let output = self.tx_builder.unwrap()
        .build_buy_transaction_output(&candidate, &config, false, true)
        .await?;
    
    // Hold nonce guard during broadcast
    let result = self.rpc.send_on_many_rpc(vec![output.tx.clone()], Some(CorrelationId::new())).await;
    
    match result {
        Ok(sig) => {
            // Success - explicitly release nonce
            output.release_nonce().await?;
            Ok(sig)
        }
        Err(e) => {
            // Error - drop output (auto-releases nonce)
            drop(output);
            Err(e)
        }
    }
}
```

### Testing Requirements

1. **Unit Test**: TxBuildOutput drop behavior
   ```rust
   #[tokio::test]
   async fn test_tx_build_output_drop_releases_lease() {
       // Create TxBuildOutput with mock nonce lease
       // Drop it
       // Verify lease release was called
   }
   ```

2. **Concurrent Test**: Multiple held leases
   ```rust
   #[tokio::test]
   async fn test_concurrent_lease_holding() {
       // Build multiple transactions with output
       // Hold them concurrently
       // Simulate delays
       // Verify no nonce race conditions
   }
   ```

3. **Integration Test**: Early drop on error
   ```rust
   #[tokio::test]
   async fn test_early_nonce_release_on_error() {
       // Build transaction
       // Simulate broadcast failure
       // Verify nonce is released immediately
   }
   ```

---

## Task 3: Instruction Ordering for Durable Nonce

### Objective
Ensure correct instruction ordering when using durable nonces: `advance_nonce_ix` must be first, followed by compute budget instructions, then DEX instruction.

### Changes Required

#### 3.1 Update Instruction Building Logic

**File**: `src/tx_builder.rs`

Current code builds instructions in this order in `build_buy_transaction`:
```rust
// Current (incorrect for durable nonce):
instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(...));
instructions.push(ComputeBudgetInstruction::set_compute_unit_price(...));
instructions.push(advance_nonce_ix); // Wrong position!
instructions.push(buy_instruction);
```

Change to:
```rust
// Task 3: Correct ordering for durable nonce
if let (Some(nonce_pub), Some(nonce_auth)) = (exec_ctx.nonce_pubkey, exec_ctx.nonce_authority) {
    // FIRST: Advance nonce instruction
    let advance_nonce_ix = solana_sdk::system_instruction::advance_nonce_account(
        &nonce_pub,
        &nonce_auth,
    );
    instructions.push(advance_nonce_ix);
    debug!("Added nonce advance instruction as FIRST instruction");
}

// SECOND: Compute budget instructions
if dynamic_cu_limit > 0 {
    instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(dynamic_cu_limit));
}
if adaptive_priority_fee > 0 && !is_placeholder {
    instructions.push(ComputeBudgetInstruction::set_compute_unit_price(adaptive_priority_fee));
}

// THIRD: DEX instruction
instructions.push(buy_instruction);
```

Apply same ordering to `build_sell_transaction`.

#### 3.2 Add Sanity Check Function

```rust
/// Sanity check instruction order for durable nonce transactions (Task 3)
/// 
/// Validates that if a transaction uses durable nonce, the advance_nonce
/// instruction is in the correct position (first instruction).
/// 
/// Returns Ok(()) if ordering is correct, Err if invalid.
fn sanity_check_ix_order(
    instructions: &[Instruction],
    is_durable_nonce: bool,
) -> Result<(), TransactionBuilderError> {
    if !is_durable_nonce {
        // No nonce, no ordering requirements
        return Ok(());
    }
    
    if instructions.is_empty() {
        return Err(TransactionBuilderError::InstructionBuild {
            program: "sanity_check".to_string(),
            reason: "Empty instruction list for durable nonce transaction".to_string(),
        });
    }
    
    // First instruction must be advance_nonce_account
    let first_ix = &instructions[0];
    let system_program = solana_sdk::system_program::id();
    
    if first_ix.program_id != system_program {
        return Err(TransactionBuilderError::InstructionBuild {
            program: "sanity_check".to_string(),
            reason: format!(
                "Invalid nonce ix order: first instruction program_id is {}, expected {}",
                first_ix.program_id, system_program
            ),
        });
    }
    
    // Check instruction data indicates advance_nonce (discriminator check)
    // advance_nonce_account has discriminator [4, 0, 0, 0] (instruction index 4)
    if first_ix.data.len() < 4 || &first_ix.data[0..4] != &[4, 0, 0, 0] {
        return Err(TransactionBuilderError::InstructionBuild {
            program: "sanity_check".to_string(),
            reason: "Invalid nonce ix order: first instruction is not advance_nonce_account".to_string(),
        });
    }
    
    info!("Nonce instruction order validated successfully");
    Ok(())
}
```

#### 3.3 Integrate Sanity Check

Call after building instructions:
```rust
// Build instructions
let mut instructions: Vec<Instruction> = Vec::with_capacity(4);
// ... build instructions ...

// Task 3: Validate instruction order
let is_durable = exec_ctx.nonce_pubkey.is_some();
sanity_check_ix_order(&instructions, is_durable)?;
```

#### 3.4 Update Simulation Logic

Skip advance_nonce instruction during simulation:
```rust
// Build simulation instructions (skip nonce advance)
let sim_instructions: Vec<Instruction> = if is_durable {
    // Skip first instruction (advance_nonce)
    instructions.iter().skip(1).cloned().collect()
} else {
    instructions.clone()
};

// Simulate with modified instruction list
let sim_tx = VersionedTransaction { /* ... */ };
```

### Testing Requirements

1. **Unit Test**: Instruction order validation
   ```rust
   #[test]
   fn test_sanity_check_correct_nonce_order() {
       // Build instruction list with correct order
       // Call sanity_check_ix_order
       // Expect Ok(())
   }
   
   #[test]
   fn test_sanity_check_wrong_nonce_order() {
       // Build instruction list with wrong order
       // Call sanity_check_ix_order
       // Expect Err
   }
   ```

2. **Integration Test**: Local validator submission
   ```rust
   #[tokio::test]
   async fn test_wrong_order_transaction_fails() {
       // Build transaction with wrong instruction order
       // Submit to local validator
       // Expect TransactionError::NonceAdvanceFailed
   }
   
   #[tokio::test]
   async fn test_correct_order_transaction_succeeds() {
       // Build transaction with correct order
       // Submit to local validator
       // Expect success
   }
   ```

3. **Unit Test**: Simulation with nonce skip
   ```rust
   #[tokio::test]
   async fn test_simulation_skips_advance_nonce() {
       // Build durable nonce transaction
       // Simulate
       // Verify advance_nonce not in simulated instructions
   }
   ```

---

## Implementation Order

1. **Phase 1**: Task 1 (enforce_nonce parameter and validation)
   - Add `enforce_nonce` parameter
   - Implement default CriticalSniper priority
   - Add nonce availability validation
   - Update BuyEngine integration
   - Write and pass unit tests

2. **Phase 2**: Task 2 (TxBuildOutput and RAII)
   - Define `TxBuildOutput` struct
   - Add `build_*_output` methods
   - Modify `ExecutionContext` to extract lease
   - Update BuyEngine to use output methods
   - Write and pass concurrent tests

3. **Phase 3**: Task 3 (instruction ordering)
   - Reorder instruction building logic
   - Implement `sanity_check_ix_order`
   - Update simulation to skip nonce advance
   - Write and pass integration tests with local validator

4. **Phase 4**: Integration testing
   - End-to-end tests combining all three tasks
   - Performance testing (latency impact)
   - Stress testing (concurrent operations)

---

## Backward Compatibility

### Legacy API Support
- Keep original `build_buy_transaction` and `build_sell_transaction` signatures
- Implement as wrappers calling new methods with defaults
- Add deprecation warnings in logs for legacy usage

### Migration Path
```rust
// Old code (still works but suboptimal):
let tx = builder.build_buy_transaction(&candidate, &config, false).await?;

// New code (optimal):
let output = builder.build_buy_transaction_output(&candidate, &config, false, true).await?;
// ... hold output.nonce_guard until broadcast completes ...
output.release_nonce().await?;
```

---

## Potential Issues & Mitigations

### Issue 1: Nonce Lease Timeout
**Problem**: If transaction building + broadcast takes longer than lease TTL (30s), nonce becomes invalid.

**Mitigation**:
- Add configurable TTL
- Implement lease extension mechanism
- Add telemetry for lease age at broadcast time

### Issue 2: Memory Leaks from Unreleased Leases
**Problem**: If `TxBuildOutput` is forgotten/leaked, nonce stays locked.

**Mitigation**:
- RAII Drop implementation ensures release
- Add watchdog task to reclaim expired leases
- Add metrics for lease hold duration

### Issue 3: Instruction Order Validation Performance
**Problem**: `sanity_check_ix_order` adds overhead to every transaction.

**Mitigation**:
- Make sanity check optional via feature flag in production
- Only run in debug/test builds by default
- Cache validation results for repeated patterns

---

## Success Criteria

1. **Correctness**: All transactions using durable nonces have correct instruction ordering
2. **Safety**: No nonce races or double-acquisitions in concurrent scenarios
3. **Performance**: < 5ms overhead from new validation/RAII logic
4. **Reliability**: Nonce leases always released, even on errors
5. **Testability**: 100% test coverage for new code paths
6. **Maintainability**: Clear documentation and examples

---

## Appendix: Code Locations

### Files to Modify
- `src/tx_builder.rs` (primary changes)
- `src/buy_engine.rs` (integration)
- `src/types.rs` (if TransactionConfig needs extension)

### Dependencies
- `NonceManager` from `src/nonce manager/mod.rs`
- `NonceLease` from `src/nonce manager/nonce_lease.rs`
- `ZkProofData` from `src/nonce manager/nonce_manager_integrated.rs`

### Test Files
- `src/tests/tx_builder_universe_tests.rs` (unit tests)
- `src/tests/nonce_lease_tests.rs` (lease RAII tests)
- Integration tests in `tests/integration/` (local validator tests)
