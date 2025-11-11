# Quick Start: Implementing Nonce Management Enhancements

## Prerequisites

✅ **The codebase must compile without errors**
```bash
cargo build  # Must succeed
cargo test   # Must pass
```

If you have compilation errors, **STOP** and fix those first before proceeding.

---

## Step-by-Step Implementation Guide

### Task 1: Add enforce_nonce Parameter (2-3 hours)

#### Step 1.1: Add the TxBuildOutput struct first (needed for Task 2)

Open `src/tx_builder.rs` and add after the imports section (around line 200):

```rust
/// Output from transaction building with nonce lease (Task 2)
pub struct TxBuildOutput {
    pub tx: VersionedTransaction,
    pub nonce_guard: Option<crate::nonce_manager::NonceLease>,
    pub required_signers: Vec<Pubkey>,
}

impl TxBuildOutput {
    pub fn new(tx: VersionedTransaction, nonce_guard: Option<crate::nonce_manager::NonceLease>) -> Self {
        let num_signers = tx.message.header().num_required_signatures as usize;
        let required_signers = tx.message.static_account_keys()
            .iter()
            .take(num_signers)
            .copied()
            .collect();
        
        Self { tx, nonce_guard, required_signers }
    }
    
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
            warn!("TxBuildOutput dropped with active nonce guard");
        }
    }
}
```

#### Step 1.2: Modify ExecutionContext to allow extracting lease

Find the `ExecutionContext` struct (around line 598) and add this method:

```rust
impl ExecutionContext {
    /// Extract the nonce lease, consuming it
    pub fn extract_lease(mut self) -> Option<crate::nonce_manager::NonceLease> {
        self._nonce_lease.take()
    }
}
```

#### Step 1.3: Add new prepare_execution_context variant

Find `prepare_execution_context` method (around line 1430) and add this new variant after it:

```rust
/// Prepare execution context with explicit enforce_nonce control
async fn prepare_execution_context_with_enforcement(
    &self,
    config: &TransactionConfig,
    enforce_nonce: bool,
) -> Result<ExecutionContext, TransactionBuilderError> {
    if !enforce_nonce {
        // Use recent blockhash
        let blockhash = self.get_recent_blockhash(config).await?;
        return Ok(ExecutionContext {
            blockhash,
            nonce_pubkey: None,
            nonce_authority: None,
            _nonce_lease: None,
            zk_proof: None,
        });
    }
    
    // Original nonce-based logic
    self.prepare_execution_context(config).await
}
```

#### Step 1.4: Create new build_buy_transaction variants

Find `build_buy_transaction` (around line 1544) and **rename it** to `build_buy_transaction_internal`:

```rust
async fn build_buy_transaction_internal(
    &self,
    candidate: &PremintCandidate,
    config: &TransactionConfig,
    sign: bool,
    enforce_nonce: bool,
    return_output: bool,
) -> Result<TxBuildOutput, TransactionBuilderError> {
    config.validate()?;
    
    // Default to CriticalSniper if not set
    let mut effective_config = config.clone();
    if effective_config.operation_priority == OperationPriority::Utility {
        effective_config.operation_priority = OperationPriority::CriticalSniper;
    }
    
    // Validate nonce availability if needed
    if enforce_nonce && effective_config.operation_priority.requires_nonce() {
        let available = self.nonce_manager.available_permits();
        if available == 0 {
            return Err(TransactionBuilderError::NonceAcquisition(
                "No available nonces for durable mode".to_string()
            ));
        }
    }
    
    info!(mint = %candidate.mint, enforce_nonce = enforce_nonce, "Building buy transaction");
    
    // ... rest of the existing build logic, but use effective_config ...
    // ... and prepare_execution_context_with_enforcement ...
    
    let exec_ctx = self.prepare_execution_context_with_enforcement(&effective_config, enforce_nonce).await?;
    
    // ... existing build logic continues ...
    // At the end, instead of returning tx directly:
    
    let nonce_lease = if return_output {
        exec_ctx.extract_lease()
    } else {
        None
    };
    
    Ok(TxBuildOutput::new(tx, nonce_lease))
}
```

Then add these public wrappers:

```rust
/// Build buy transaction (default: enforce_nonce=true)
pub async fn build_buy_transaction(
    &self,
    candidate: &PremintCandidate,
    config: &TransactionConfig,
    sign: bool,
) -> Result<VersionedTransaction, TransactionBuilderError> {
    let output = self.build_buy_transaction_internal(candidate, config, sign, true, false).await?;
    Ok(output.tx)
}

/// Build buy transaction with nonce control
pub async fn build_buy_transaction_with_nonce(
    &self,
    candidate: &PremintCandidate,
    config: &TransactionConfig,
    sign: bool,
    enforce_nonce: bool,
) -> Result<VersionedTransaction, TransactionBuilderError> {
    let output = self.build_buy_transaction_internal(candidate, config, sign, enforce_nonce, false).await?;
    warn!("Legacy API: releasing nonce early - migrate to build_buy_transaction_output");
    Ok(output.tx)
}

/// Build buy transaction with output (holds nonce lease)
pub async fn build_buy_transaction_output(
    &self,
    candidate: &PremintCandidate,
    config: &TransactionConfig,
    sign: bool,
    enforce_nonce: bool,
) -> Result<TxBuildOutput, TransactionBuilderError> {
    self.build_buy_transaction_internal(candidate, config, sign, enforce_nonce, true).await
}
```

#### Step 1.5: Apply same changes to build_sell_transaction

Repeat steps 1.4 for `build_sell_transaction` (around line 1868).

#### Step 1.6: Write tests

Create `src/tests/tx_builder_nonce_enforcement_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_default_critical_sniper_priority() {
        // Test that utility priority is upgraded to CriticalSniper
    }
    
    #[tokio::test]
    async fn test_nonce_availability_validation() {
        // Test error when no nonces available with enforce_nonce=true
    }
    
    #[tokio::test]
    async fn test_enforce_nonce_false_uses_recent_blockhash() {
        // Test that enforce_nonce=false bypasses nonce acquisition
    }
}
```

### Task 2: BuyEngine Integration (1 hour)

Open `src/buy_engine.rs` and find the `try_buy` method (around line 1936).

Replace transaction building logic:

```rust
// Old:
let tx = self.create_buy_transaction(&candidate, recent_blockhash).await?;

// New:
let output = match &self.tx_builder {
    Some(builder) => {
        let config = TransactionConfig::default();
        builder.build_buy_transaction_output(&candidate, &config, false, true).await?
    }
    None => {
        // Fallback for tests
        return Err(anyhow!("No transaction builder available"));
    }
};

// Hold output during broadcast
let result = self.rpc.send_on_many_rpc(vec![output.tx.clone()], Some(CorrelationId::new())).await;

match result {
    Ok(sig) => {
        output.release_nonce().await?;
        Ok(sig)
    }
    Err(e) => {
        drop(output); // Auto-releases nonce
        Err(e)
    }
}
```

### Task 3: Fix Instruction Ordering (2 hours)

#### Step 3.1: Reorder instructions in build_buy_transaction_internal

Find where instructions are built (around line 1800) and change the order:

```rust
// OLD ORDER (WRONG):
// compute budget -> nonce advance -> dex instruction

// NEW ORDER (CORRECT):
let mut instructions: Vec<Instruction> = Vec::with_capacity(4);

// FIRST: Nonce advance (if using durable nonce)
if let (Some(nonce_pub), Some(nonce_auth)) = (exec_ctx.nonce_pubkey, exec_ctx.nonce_authority) {
    let advance_nonce_ix = solana_sdk::system_instruction::advance_nonce_account(
        &nonce_pub,
        &nonce_auth,
    );
    instructions.push(advance_nonce_ix);
    debug!("Added nonce advance instruction FIRST");
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

#### Step 3.2: Add sanity check function

Add this function in `tx_builder.rs`:

```rust
/// Validate instruction order for durable nonce transactions
fn sanity_check_ix_order(
    instructions: &[Instruction],
    is_durable_nonce: bool,
) -> Result<(), TransactionBuilderError> {
    if !is_durable_nonce || instructions.is_empty() {
        return Ok(());
    }
    
    let first_ix = &instructions[0];
    let system_program = solana_sdk::system_program::id();
    
    if first_ix.program_id != system_program {
        return Err(TransactionBuilderError::InstructionBuild {
            program: "sanity_check".to_string(),
            reason: format!("Invalid nonce ix order: expected system program first"),
        });
    }
    
    if first_ix.data.len() < 4 || &first_ix.data[0..4] != &[4, 0, 0, 0] {
        return Err(TransactionBuilderError::InstructionBuild {
            program: "sanity_check".to_string(),
            reason: "Invalid nonce ix order: first instruction not advance_nonce".to_string(),
        });
    }
    
    Ok(())
}
```

#### Step 3.3: Call sanity check after building instructions

```rust
// After building all instructions:
let is_durable = exec_ctx.nonce_pubkey.is_some();
sanity_check_ix_order(&instructions, is_durable)?;
```

#### Step 3.4: Skip nonce advance in simulation

Find simulation code (around line 1620) and modify:

```rust
// Build simulation instructions (skip nonce advance if present)
let sim_instructions: Vec<Instruction> = if is_durable && !instructions.is_empty() {
    instructions.iter().skip(1).cloned().collect()
} else {
    instructions.clone()
};
```

Apply same changes to `build_sell_transaction`.

---

## Testing

### Run unit tests:
```bash
cargo test tx_builder_nonce_enforcement_tests
cargo test --test tx_builder_universe_tests
```

### Run integration tests:
```bash
cargo test --test buy_engine_tests
```

### Test manually with local validator:
```bash
# Start local validator
solana-test-validator

# Run your bot in test mode
cargo run -- --mode test
```

---

## Common Issues & Solutions

### Issue: "No available nonces"
**Solution**: Increase nonce pool size in config or disable enforce_nonce for utility operations.

### Issue: "Invalid nonce ix order"
**Solution**: Check that you've applied instruction reordering to BOTH buy and sell methods.

### Issue: "Lease not released"
**Solution**: Make sure you're using `build_*_output` methods and calling `release_nonce()` or letting Drop work.

### Issue: Compilation errors after changes
**Solution**: 
1. Check you renamed `build_buy_transaction` to `build_buy_transaction_internal`
2. Verify all imports are correct
3. Make sure `TxBuildOutput` is defined before use

---

## Validation Checklist

- [ ] Code compiles without errors
- [ ] All existing tests pass
- [ ] New unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing with local validator succeeds
- [ ] No memory leaks (nonces always released)
- [ ] Instruction order correct for durable nonce transactions
- [ ] Backward compatibility maintained (old code still works)
- [ ] Documentation updated

---

## Time Estimates

- Task 1 (enforce_nonce): 2-3 hours
- Task 2 (BuyEngine integration): 1 hour
- Task 3 (instruction ordering): 2 hours
- Testing and validation: 1-2 hours

**Total: 6-8 hours**

---

## Need Help?

Refer to the detailed implementation plan: `TX_BUILDER_NONCE_IMPLEMENTATION_PLAN.md`

This guide provides the quickest path to implementation. The detailed plan has additional context, edge cases, and advanced scenarios.
