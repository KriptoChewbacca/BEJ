# Transaction Builder Nonce Management Implementation Plan

This plan defines precise, sequenced tasks to enhance nonce management and transaction building in `tx_builder.rs`, with safety (RAII), correctness (instruction ordering), and determinism (tests). It is written to be directly actionable by multiple agents working in parallel where possible.

---

## Pre-requisites and CI gates

- CI required jobs (branch protection):
  - tests-nightly (baseline + all-features, artifacts)
  - format-check (rustfmt)
  - clippy (no deny at first; introduce gradually)
  - cargo-deny (licenses/bans/sources offline)
- Feature gating hygiene:
  - test_utils only under `#[cfg(any(test, feature = "test_utils"))]`
  - Sanity checks for nonce order under debug/test flag (see Task 3.2)

---

## Task 1: Default Nonce Mode and Safe Acquisition

### Objective
- Durable nonce is used by default for trade-critical operations (buy/sell), with explicit control via `enforce_nonce`.
- Avoid TOCTTOU and race conditions in nonce allocation.

### Changes Required

#### 1.1 Add `enforce_nonce` Parameter (backward-compatible)
File: `src/tx_builder.rs`

```rust
// Current
pub async fn build_buy_transaction(
    &self,
    candidate: &PremintCandidate,
    config: &TransactionConfig,
    sign: bool,
) -> Result<VersionedTransaction, TransactionBuilderError>;

// New wrapper (preserves signature)
pub async fn build_buy_transaction(
    &self,
    candidate: &PremintCandidate,
    config: &TransactionConfig,
    sign: bool,
) -> Result<VersionedTransaction, TransactionBuilderError> {
    self.build_buy_transaction_with_nonce(candidate, config, sign, true).await
}

// Detailed API
pub async fn build_buy_transaction_with_nonce(
    &self,
    candidate: &PremintCandidate,
    config: &TransactionConfig,
    sign: bool,
    enforce_nonce: bool,
) -> Result<VersionedTransaction, TransactionBuilderError>;
```

Apply same to `build_sell_transaction`.

#### 1.2 Priority defaulting policy (configurable)
- Prefer configuration-driven default:
  - Add optional `default_operation_priority` field in `TransactionConfig` (or global config).
  - If not set and operation is trade-critical and `enforce_nonce == true`, then default to `OperationPriority::CriticalSniper` when caller passed `Utility`.
- Minimal change version (if config change is not desired now):

```rust
let mut effective = config.clone();
if enforce_nonce && matches!(effective.operation_priority, OperationPriority::Utility) {
    effective.operation_priority = OperationPriority::CriticalSniper;
}
```

Note: Document this behavior; make it explicit and tested.

#### 1.3 Safe nonce acquisition (no TOCTTOU)
- Do NOT check `available_permits()` prior to acquisition.
- Perform acquisition atomically in `ExecutionContext` preparation via `try_acquire()` or equivalent, returning `NonceLease` on success:

```rust
// Pseudocode inside prepare_execution_context_with_enforcement
let mut lease: Option<NonceLease> = None;
if enforce_nonce && effective.operation_priority.requires_nonce() {
    lease = self.nonce_manager
        .try_acquire(/* ttl from config */)
        .ok_or_else(|| TransactionBuilderError::NonceAcquisition("No available nonces".into()))?;
}
```

#### 1.4 Enhance `prepare_execution_context` and return lease
```rust
async fn prepare_execution_context_with_enforcement(
    &self,
    config: &TransactionConfig,
    enforce_nonce: bool,
) -> Result<ExecutionContext, TransactionBuilderError>;
```

- Behavior:
  - When `enforce_nonce == true`: acquire `NonceLease` with configurable TTL (default 30s; move value to config).
  - When `enforce_nonce == false`: obtain recent blockhash via `get_recent_blockhash_with_quorum`, no nonce lease.

#### 1.5 BuyEngine integration (defaults to durable nonce for trading)
File: `src/buy_engine.rs`

```rust
let tx = builder
    .build_buy_transaction_with_nonce(&candidate, &config, /*sign=*/false, /*enforce_nonce=*/true)
    .await?;
```

- For utility flows (e.g., unwrap WSOL), pass `enforce_nonce=false`.

### Tests (deterministic)

- Unit: defaulting behavior
```rust
#[tokio::test]
async fn test_default_critical_sniper_priority_when_enforced() {
    // config with Utility; enforce_nonce=true
    // expect CriticalSniper effective priority
}
```

- Integration: acquisition error
```rust
#[tokio::test]
async fn test_nonce_acquisition_error_when_pool_empty() {
    // exhaust pool -> try_acquire returns None -> expect NonceAcquisition error
}
```

- Concurrency: race-safety
```rust
#[tokio::test(flavor="current_thread")]
async fn test_concurrent_acquisition_no_double_alloc() {
    // spawn multiple builders; ensure unique leases; deterministic seed + paused time
}
```

### Definition of Done (Task 1)
- New signatures compiled and used in BuyEngine.
- try_acquire (or atomic acquisition) in context; no `available_permits()` pre-check.
- TTL configurable; metrics hook for TTL/lease age exists (even if stubbed).
- Tests deterministic; CI green.

---

## Task 2: Lease Lifetime (RAII) with TxBuildOutput

### Objective
- Hold nonce until broadcast completes using RAII.
- Ensure safe release on success and on error paths.

### Changes Required

#### 2.1 Define `TxBuildOutput` + ergonomics
File: `src/tx_builder.rs`

```rust
pub struct TxBuildOutput {
    pub tx: VersionedTransaction,
    pub nonce_guard: Option<NonceLease>,
    pub required_signers: Vec<Pubkey>,
}

impl TxBuildOutput {
    pub fn new(tx: VersionedTransaction, nonce_guard: Option<NonceLease>) -> Self { /* ... */ }

    pub fn tx_ref(&self) -> &VersionedTransaction { &self.tx }
    pub fn into_tx(self) -> VersionedTransaction { self.tx } // if needed
    pub fn required_signers(&self) -> &[Pubkey] { &self.required_signers }

    pub async fn release_nonce(mut self) -> Result<(), TransactionBuilderError> {
        if let Some(guard) = self.nonce_guard.take() {
            guard.release().await.map_err(|e| TransactionBuilderError::NonceAcquisition(e.to_string()))?;
        }
        Ok(())
    }
}

impl Drop for TxBuildOutput {
    fn drop(&mut self) {
        if self.nonce_guard.is_some() {
            // Only log here. Actual resource return must happen in NonceLease::drop
            warn!("TxBuildOutput dropped with active nonce guard");
        }
    }
}
```

Ergonomics reduce cloning and make integration straightforward.

#### 2.2 ExecutionContext: extract lease
```rust
struct ExecutionContext {
    blockhash: Hash,
    nonce_pubkey: Option<Pubkey>,
    nonce_authority: Option<Pubkey>,
    _nonce_lease: Option<NonceLease>,
    // ...
}

impl ExecutionContext {
    pub fn extract_lease(mut self) -> Option<NonceLease> {
        self._nonce_lease.take()
    }
}
```

#### 2.3 Output-builder methods
```rust
pub async fn build_buy_transaction_output(
    &self,
    candidate: &PremintCandidate,
    config: &TransactionConfig,
    sign: bool,
    enforce_nonce: bool,
) -> Result<TxBuildOutput, TransactionBuilderError> {
    let exec_ctx = self.prepare_execution_context_with_enforcement(config, enforce_nonce).await?;
    let tx = /* build */;
    let nonce_lease = exec_ctx.extract_lease();
    Ok(TxBuildOutput::new(tx, nonce_lease))
}

// Legacy wrapper (kept for compatibility; warn once)
pub async fn build_buy_transaction_with_nonce(/* ... */) -> Result<VersionedTransaction, TransactionBuilderError> {
    static WARN_ONCE: std::sync::Once = std::sync::Once::new();
    let output = self.build_buy_transaction_output(/*...*/).await?;
    WARN_ONCE.call_once(|| warn!("Legacy API: releasing nonce early - migrate to *_output"));
    Ok(output.tx)
}
```

Apply to `sell` as well.

#### 2.4 NonceLease Drop responsibility
- Ensure `NonceLease` returns permit/resources in its own `Drop` (non-async), to cover all error paths safely.
- If async work is required for release, spawn a background task within `Drop` (fire-and-forget) using a non-blocking channel/handle.

#### 2.5 BuyEngine integration
- Use `*_output` method; hold output until broadcast resolves; call `release_nonce()` on success, `drop(output)` on error.

### Tests

- Unit: `TxBuildOutput` drop behavior with mocked `NonceLease` counting releases.
- Concurrency: hold multiple outputs at once; no deadlocks; stable under paused time.
- Integration: broadcast error leads to immediate release via `Drop` of `NonceLease` (observable in mock metrics/log).

### Definition of Done (Task 2)
- `*_output` methods available and used in BuyEngine critical paths.
- `NonceLease::drop` guarantees resource return; no leaks after error paths.
- Deterministic tests covering drop-on-error and explicit release.

---

## Task 3: Durable Nonce Instruction Ordering and Simulation

### Objective
- Ensure `advance_nonce_account` is the FIRST instruction when using durable nonce.
- Keep compute budget and DEX instructions after it.
- Keep simulation realistic without consuming nonce.

### Changes Required

#### 3.1 Instruction ordering in builders
```rust
// Pseudocode inside build_*:
if let (Some(nonce_pub), Some(nonce_auth)) = (exec_ctx.nonce_pubkey, exec_ctx.nonce_authority) {
    let ix_advance = solana_sdk::system_instruction::advance_nonce_account(&nonce_pub, &nonce_auth);
    instructions.push(ix_advance); // FIRST
}
if dynamic_cu_limit > 0 {
    instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(dynamic_cu_limit));
}
if adaptive_priority_fee > 0 && !is_placeholder {
    instructions.push(ComputeBudgetInstruction::set_compute_unit_price(adaptive_priority_fee));
}
instructions.push(buy_or_sell_ix);
```

#### 3.2 Sanity check (debug/test only)
- Avoid brittle byte checks.
- Prefer recognizing instruction type via:
  - constructing a reference `advance_nonce_account` and comparing `program_id` and account metas shape; or
  - decoding instruction data to `SystemInstruction` using official helper if available.
- Gate under `#[cfg(debug_assertions)]` or a feature flag (`nonce_sanity_checks`):

```rust
#[cfg(any(test, debug_assertions))]
fn sanity_check_ix_order(instructions: &[Instruction], is_durable: bool) -> Result<(), TransactionBuilderError> {
    if !is_durable { return Ok(()); }
    let Some(first) = instructions.first() else { /* Err */ };
    if first.program_id != solana_sdk::system_program::id() {
        return Err(/* ... */);
    }
    // Option A: compare with a reference advance_nonce built with placeholder accounts’ discriminator
    // Option B: decode to SystemInstruction::AdvanceNonceAccount if helper available
    Ok(())
}
```

Call after instruction build in debug/test.

#### 3.3 Simulation path skips nonce advance
```rust
let is_durable = exec_ctx.nonce_pubkey.is_some();
let sim_instructions: Vec<Instruction> = if is_durable {
    instructions.iter().skip(1).cloned().collect()
} else {
    instructions.clone()
};
// Build a simulation transaction using sim_instructions; do not include advance_nonce
```

### Tests

- Unit: sanity check passes/fails with correct/wrong order.
- Integration (local validator): wrong order => `TransactionError::NonceAdvanceFailed`; correct order => success.
- Unit: simulation skips nonce advance while preserving compute budget + DEX instructions.

### Definition of Done (Task 3)
- Correct order enforced in builders.
- Sanity check active in debug/test builds, disabled in prod by default.
- Simulation path implemented and used in tests.
- CI green.

---

## Implementation Order and Assignment

1. Phase 1 (Task 1) — API + acquisition (Owner: Builder Core Team)
   - Add `enforce_nonce` param + wrappers.
   - Implement `prepare_execution_context_with_enforcement` with atomic `try_acquire`.
   - Make TTL configurable; add metric hook (lease age at broadcast).
   - Update BuyEngine call sites (only buy/sell).
   - Add/green tests (defaulting, acquisition error, concurrency).

2. Phase 2 (Task 2) — RAII output + engine integration (Owner: Builder Core + Engine Team)
   - Define `TxBuildOutput` with ergonomics.
   - Add `build_*_output`; keep legacy wrappers (warn once).
   - Ensure `NonceLease::drop` returns resources; adjust if async release is needed.
   - Refactor BuyEngine to use output and hold guard.
   - Tests for drop/release/concurrency in deterministic runtime.

3. Phase 3 (Task 3) — Instruction order + simulation (Owner: Builder Core)
   - Enforce ordering; implement debug/test `sanity_check_ix_order`.
   - Implement simulation path to skip nonce advance.
   - Tests for order, validator behavior, and simulation.

4. Phase 4 — E2E/Perf/Stress (Owner: QA/Perf)
   - E2E combining Tasks 1–3 on local validator.
   - Perf target: added overhead < 5ms; memory stable; no leaks.
   - Stress tests with concurrent builds; no double-acquire, no stale nonce usage.

Parallelization notes:
- Phase 1 and 2 can be partially parallel: define types/interfaces first; integration follows.
- Phase 3 can start once builder returns ordered instruction lists (even behind feature flag).

---

## Backward Compatibility and Migration

- Legacy `build_*_transaction` signatures retained as wrappers calling new methods.
- Log deprecation at WARN only once via `Once`/`OnceCell`; subsequent calls at INFO or suppressed.
- Clear migration guidance in docs and examples.

Migration snippet:
```rust
// Legacy:
let tx = builder.build_buy_transaction(&candidate, &config, false).await?;

// Recommended:
let output = builder.build_buy_transaction_output(&candidate, &config, false, true).await?;
// keep `output` alive through broadcast
let sig = rpc.send_transaction(&output.tx).await?;
output.release_nonce().await?;
```

---

## Risks and Mitigations

- Nonce Lease Timeout:
  - TTL configurable; extension mechanism (future); metric for lease age at broadcast.
- Memory/resource leaks:
  - `NonceLease::drop` returns resource; watchdog (future) to reclaim expired; metric for hold duration.
- Performance cost of sanity checks:
  - Only in debug/test or feature-guarded; not in prod; O(1) check.
- Regression risk in BuyEngine paths:
  - Feature-flag rollout or staged PRs; integration tests with paused time and seeded RNG.

---

## Success Criteria

1. Correct instruction ordering for durable nonce transactions.
2. No double-acquisition or race issues under concurrency.
3. Overhead from RAII/validation < 5ms.
4. Leases always released (success or error).
5. Deterministic tests; CI green across required jobs.
6. Clear documentation and maintainable APIs.

---

## Appendix: Code Locations and Interfaces

- Modify:
  - `src/tx_builder.rs` (signatures, output types, ordering, simulation)
  - `src/buy_engine.rs` (integration with `*_output`)
  - `src/types.rs` (if config gains default priority and TTL fields)
  - `src/nonce manager/*` (`NonceLease` drop semantics if needed)
- Tests:
  - `src/tests/tx_builder_universe_tests.rs`
  - `src/tests/nonce_lease_tests.rs`
  - `tests/integration/*` (local validator)
- Feature flags:
  - `test_utils`
  - `nonce_sanity_checks` (optional; enabled in test/debug)

---

## Agent-friendly Checklist (per PR)

- PR A (Phase 1 – API & acquisition)
  - [ ] Add `enforce_nonce` param and wrappers for buy/sell
  - [ ] Implement `prepare_execution_context_with_enforcement` with `try_acquire`
  - [ ] TTL from config; add lease-age metric hook
  - [ ] Update BuyEngine calls
  - [ ] Add unit/integration tests (defaulting, error, concurrency)
  - [ ] CI green

- PR B (Phase 2 – RAII & engine)
  - [ ] Implement `TxBuildOutput` + ergonomics
  - [ ] Add `build_*_output`; legacy wrappers warn once
  - [ ] Ensure `NonceLease::drop` returns resources; adjust if async
  - [ ] Refactor BuyEngine to hold output through broadcast
  - [ ] Tests for drop/release and concurrency (deterministic)
  - [ ] CI green

- PR C (Phase 3 – Ordering & simulation)
  - [ ] Enforce instruction order; add debug/test sanity check
  - [ ] Implement simulation path (skip advance_nonce)
  - [ ] Tests: unit for check, integration on validator
  - [ ] CI green

- PR D (Phase 4 – E2E/Perf)
  - [ ] E2E tests (local validator)
  - [ ] Perf/Stress measurements (overhead < 5ms)
  - [ ] Docs update with migration guidance
  - [ ] CI green
