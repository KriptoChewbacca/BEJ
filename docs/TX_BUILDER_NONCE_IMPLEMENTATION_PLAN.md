````markdown name=docs/TX_BUILDER_NONCE_IMPLEMENTATION_PLAN.md url=https://github.com/KriptoChewbacca/BEJ/blob/main/docs/TX_BUILDER_NONCE_IMPLEMENTATION_PLAN.md
# Transaction Builder Nonce Management Implementation Plan (Universe‑grade, end‑to‑end)

This plan defines precise, sequenced tasks to enhance nonce management and transaction building around durable nonce usage. It elevates safety (RAII), correctness (instruction ordering), determinism (tests), and maximum concurrency. It is the single source of truth for Agents during implementation and review.

Scope includes:
- Durable nonce as first‑class path (with configurable enforcement)
- RAII lease lifetime handling via TxBuildOutput
- Correct instruction ordering and nonce‑aware simulation
- E2E, performance, and stress validation (p95 overhead < 5 ms)
- Integration in BuyEngine broadcast paths
- Observability and CI gates to keep regressions out

---

## 0) Pre‑requisites, Guardrails and CI Gates

Required branch protection jobs (must be green):
- tests-nightly (baseline = default features; plus all-features) with artifacts
- format-check (rustfmt)
- clippy (initially allow warnings; later enforce -D warnings)
- cargo-deny (licenses/bans/sources offline)

Feature gating hygiene:
- test_utils only under `#[cfg(any(test, feature = "test_utils")]`
- Nonce sanity checks under `#[cfg(debug_assertions)]` or feature `nonce_sanity_checks`

Determinism in tests:
- Use `#[tokio::test(flavor = "current_thread")]` where concurrency order matters
- Use `tokio::time::pause()` and `advance()` instead of sleeps
- Seed RNG (`fastrand::seed(42)`) in shared test setup

Performance SLO (to be measured in Phase 4):
- TxBuilder added overhead: p95 < 5 ms per tx (hot path)
- No blocking APIs in async hot path; no global Mutex contention
- Zero leaks; zero double‑acquire across 1k concurrent build attempts

Observability SLO:
- Latency histograms for acquire_lease_ms, build_to_land_ms
- Counters for total_acquires/releases/failures
- Export diagnostics every 60s (JSON/metrics)

---

## Task 1: Default Nonce Mode and Safe Acquisition (No TOCTTOU)

### Objective
- Durable nonce by default for trade‑critical ops (buy/sell), toggle via `enforce_nonce`.
- Eliminate TOCTTOU races; acquisition must be atomic.

### Changes Required

#### 1.1 `enforce_nonce` (backward‑compatible)
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

Apply to `build_sell_transaction`.

#### 1.2 Priority defaulting policy (configurable)
- Prefer configuration‑driven default (`default_operation_priority` in `TransactionConfig`).
- Minimal version if config change is out of scope now:
```rust
let mut effective = config.clone();
if enforce_nonce && matches!(effective.operation_priority, OperationPriority::Utility) {
    effective.operation_priority = OperationPriority::CriticalSniper;
}
```

#### 1.3 Safe acquisition (atomic)
- Do NOT pre‑check `available_permits()`.  
- Acquire in `ExecutionContext` using `try_acquire()`; fail fast with `TransactionBuilderError::NonceAcquisition`:  
```rust
// inside prepare_execution_context_with_enforcement
let mut lease: Option<NonceLease> = None;
if enforce_nonce && effective.operation_priority.requires_nonce() {
    lease = self
        .nonce_manager
        .try_acquire(/* ttl from config */)
        .ok_or_else(|| TransactionBuilderError::NonceAcquisition("No available nonces".into()))?;
}
```

#### 1.4 Execution context API
```rust
async fn prepare_execution_context_with_enforcement(
    &self,
    config: &TransactionConfig,
    enforce_nonce: bool,
) -> Result<ExecutionContext, TransactionBuilderError>;
```
Behavior:
- `enforce_nonce == true`: acquire `NonceLease` with configurable TTL (default 30s in config)
- `enforce_nonce == false`: fetch recent blockhash via quorum; no lease

#### 1.5 BuyEngine defaults
File: `src/buy_engine.rs`
```rust
let tx = builder
    .build_buy_transaction_with_nonce(&candidate, &config, /*sign=*/false, /*enforce_nonce=*/true)
    .await?;
```
For utility ops (e.g., unwrap WSOL): `enforce_nonce=false`.

### Tests
- Unit: default priority escalation when `enforce_nonce=true`
- Integration: pool exhausted → `NonceAcquisition` error
- Concurrency: 100 concurrent attempts → no double‑acquire; deterministic time

### DoD (Task 1)
- New signatures compiled and in use
- `try_acquire` used (no `available_permits()` pre‑check)
- TTL configurable; metric hook for lease age present
- Deterministic tests; CI green

---

## Task 2: Lease Lifetime (RAII) with TxBuildOutput

### Objective
- Hold nonce until broadcast completes via RAII; explicit `release_nonce()` on success; safe drop on errors.

### Changes Required

#### 2.1 `TxBuildOutput` ergonomia
```rust
pub struct TxBuildOutput {
    pub tx: VersionedTransaction,
    pub nonce_guard: Option<NonceLease>,
    pub required_signers: Vec<Pubkey>,
}

impl TxBuildOutput {
    pub fn new(tx: VersionedTransaction, nonce_guard: Option<NonceLease>) -> Self { /* ... */ }
    pub fn tx_ref(&self) -> &VersionedTransaction { &self.tx }
    pub fn into_tx(self) -> VersionedTransaction { self.tx }
    pub fn required_signers(&self) -> &[Pubkey] { &self.required_signers }
    pub async fn release_nonce(mut self) -> Result<(), TransactionBuilderError> { /* guard.release() */ }
}

impl Drop for TxBuildOutput {
    fn drop(&mut self) {
        if self.nonce_guard.is_some() {
            warn!("TxBuildOutput dropped with active nonce guard");
        }
    }
}
```

#### 2.2 ExecutionContext: transfer własności
```rust
impl ExecutionContext { pub fn extract_lease(mut self) -> Option<NonceLease> { self._nonce_lease.take() } }
```

#### 2.3 Metody *_output
```rust
pub async fn build_buy_transaction_output( /* … */ ) -> Result<TxBuildOutput, TransactionBuilderError> {
    let exec_ctx = self.prepare_execution_context_with_enforcement(config, enforce_nonce).await?;
    let tx = /* build */;
    let lease = exec_ctx.extract_lease();
    Ok(TxBuildOutput::new(tx, lease))
}

pub async fn build_buy_transaction_with_nonce(/* … */) -> Result<VersionedTransaction, TransactionBuilderError> {
    static WARN_ONCE: std::sync::Once = std::sync::Once::new();
    let output = self.build_buy_transaction_output(/* … */).await?;
    WARN_ONCE.call_once(|| warn!("Legacy API: releasing nonce early – migrate to *_output"));
    Ok(output.tx)
}
```

#### 2.4 `NonceLease::Drop`
- `Drop` zwraca permit (szybko, nie‑async). Jeśli potrzebny async cleanup, spawn background task.

#### 2.5 Integracja w BuyEngine
- Trzymaj `TxBuildOutput` do końca broadcastu; `release_nonce()` na sukces; `drop(output)` na błąd.

### Tests
- Unit: `TxBuildOutput` drop warns; explicit release ok
- Concurrency: równoległe holdy bez deadlocków
- Integration: błąd broadcastu → lease oddany przez `Drop`

### DoD (Task 2)
- `*_output` używane w krytycznych ścieżkach
- Brak leaków; deterministyczne testy

---

## Task 3: Durable Nonce Instruction Ordering and Nonce‑Aware Simulation

### Objective
- `advance_nonce_account` musi być PIERWSZY; compute budget + DEX po nim.
- Symulacja nie konsumuje nonca (pomija advance_nonce w sim‑tx).

### Changes Required

#### 3.1 Kolejność instrukcji
```rust
if let (Some(nonce_pub), Some(nonce_auth)) = (exec_ctx.nonce_pubkey, exec_ctx.nonce_authority) {
    instructions.push(system_instruction::advance_nonce_account(&nonce_pub, &nonce_auth));
}
if dynamic_cu_limit > 0 { instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(dynamic_cu_limit)); }
if adaptive_priority_fee > 0 { instructions.push(ComputeBudgetInstruction::set_compute_unit_price(adaptive_priority_fee)); }
instructions.push(buy_or_sell_ix);
```

#### 3.2 Sanity check (debug/test only)
- `#[cfg(debug_assertions)]` lub feature `nonce_sanity_checks`
- Porównuj `program_id` i kształt metas; opcjonalnie dekoduj do `SystemInstruction`

#### 3.3 Symulacja – skip advance
```rust
let is_durable = exec_ctx.nonce_pubkey.is_some();
let sim_ix = if is_durable { instructions.iter().skip(1).cloned().collect() } else { instructions.clone() };
let sim_tx = build_sim_tx_like(&tx, sim_ix);
```

### Tests
- Unit: poprawny/niepoprawny porządek; pusta lista → błąd
- Integration (local validator): złą kolejność → `NonceAdvanceFailed`; dobra → ok
- Unit: symulacja pomija advance nonce; compute i DEX zachowane

### DoD (Task 3)
- Enforced order; sanity check w debug/test
- Symulacja nonce‑aware
- CI green

---

## Task 4: E2E, Performance i Stress (produkcyjne warunki)

### Cel
- Zweryfikować integrację Task 1–3 pod obciążeniem i w scenariuszach błędów.

### Zakres
- E2E: pełny przepływ od acquire → build → simulate → sign → broadcast → release (sukces i błąd)
- Performance: pomiar overhead (< 5 ms p95), CPU, alokacje; brak GC w hot‑path
- Stress: 1k równoległych buildów; brak double‑acquire; stabilna pamięć; brak deadlocków

### Implementacja
- Testy E2E (4+):
  - poprawna kolejność i sukces broadcastu
  - błąd broadcastu → Drop zwalnia lease
  - sekwencja wielu transakcji (lease → release) bez leaków
  - walidacja metryk (latencje, liczniki)
- Testy performance (microbench – criterion):
  - create context (durable / nondurable)
  - plan instructions
  - assemble minimalny tx
- Stress (tokio::spawn, paused time):
  - 100/500/1000 równoległych prób acquire/build
  - histogramy czasu acquire i build

### DoD (Task 4)
- Raport z wynikami (docs/PHASE4_SUMMARY.md) + artefakty CI
- p95 overhead < 5 ms; zero leaków; brak double‑acquire

---

## Task 5: Observability i CI twarde bramki

### Obserwowalność
- TraceContext: trace_id/span_id/correlation_id dostępne w builderze
- Metryki: acquire_lease_ms, prepare_bundle_ms (jeśli używane), build_to_land_ms
- Liczniki: total_acquires/releases/refreshes/failures
- Eksport co 60 s + CLI monitor (opcjonalnie)

### CI i build matrix
- Baseline = domyślne featury (bez `--no-default-features`)
- test-matrix: default, test_utils, all-features
- clippy, fmt, cargo‑deny w osobnych jobach (required)

### DoD (Task 5)
- Widoczne metryki w logach i/lub endpoint
- Zielony CI na wymaganych jobach

---

## Task 6: Integracja w BuyEngine (broadcast & release semantics)

### Zakres
- BuyEngine używa `*_output`; trzyma guard do końca wysyłki
- Na sukces: `release_nonce().await?`; na błąd: `drop(output)` (RAII → zwolnienie)
- Rejestracja metryk build_to_land + acquire_lease

### DoD (Task 6)
- Krytyczne ścieżki (buy/sell) na nowym API
- Test integracyjny: sukces i błąd z poprawnym zwolnieniem nonca

---

## Task 7 (opcjonalnie): Zgranie z Bundlerem (Jito)

Cel: Jeżeli używany jest bundler, TxBuilder pozostaje źródłem poprawnych instrukcji i RAII, a bundler odpowiada za prepare/simulate/send.

- Wydzielony moduł `tx_builder::bundle` (trait Bundler + JitoBundler)
- BuyEngine przyjmuje `Arc<dyn Bundler>`; ścieżka bundle vs single tx
- Metryki: prepare_bundle_ms, jito_success/failure per region

DoD (Task 7): mock bundler w testach integracyjnych; fallback RPC działa.

---

## Konkurencyjność (maksimum)

- Brak blokujących operacji w hot‑path (żadnych std::thread::sleep, brak globalnych Mutex)
- Semafory/atomiki per nonce; rozważać sharding puli nonców przy dużym contention
- RwLock tylko dla read‑heavy (np. recent_fees)
- Pre‑alokacje wektorów (Vec::with_capacity(4)) dla instrukcji
- Tokio multi‑thread; zadania krytyczne oznaczone `instrument`

---

## Backward Compatibility i Migracja

- Legacy metody zachowane jako wrappery (WARN once → INFO później)
- Przykład migracji:
```rust
let output = builder.build_buy_transaction_output(&candidate, &config, false, true).await?;
let sig = rpc.send_transaction(output.tx_ref()).await?;
output.release_nonce().await?;
```

---

## Risks i Mitigacje
- Lease timeout → TTL w config + metryka wieku; (przyszłe) przedłużenie
- Walidacja kolejności → tylko debug/test; w prod ufamy builderowi
- Regressje performance → microbench i flamegraph na PR

---

## Success Criteria (całościowe)
1) Poprawny porządek durable nonce, enforce + RAII
2) Brak double‑acquire i leaków w stress
3) p95 overhead < 5 ms
4) Deterministyczne testy; zielony CI (required jobs)
5) Dokumentacja gotowa i aktualna

---

## Agent‑friendly Checklist (per PR)

- PR A (Task 1 – API & acquisition)
  - [ ] `enforce_nonce` + wrappery buy/sell
  - [ ] `prepare_execution_context_with_enforcement` z `try_acquire`
  - [ ] TTL w config + hook metryki
  - [ ] BuyEngine call sites
  - [ ] Testy: defaulting, error, concurrency
  - [ ] CI green

- PR B (Task 2 – RAII & engine)
  - [ ] `TxBuildOutput` + ergonomia
  - [ ] `build_*_output`; legacy warn‑once
  - [ ] `NonceLease::Drop` zwraca zasób
  - [ ] Refactor BuyEngine
  - [ ] Testy: drop/release/concurrency
  - [ ] CI green

- PR C (Task 3 – Ordering & simulation)
  - [ ] Kolejność; sanity_check w debug/test
  - [ ] Symulacja skip advance
  - [ ] Testy unit + validator
  - [ ] CI green

- PR D (Task 4 – E2E/Perf/Stress)
  - [ ] E2E scenariusze
  - [ ] Microbench + stress
  - [ ] Raport i metryki
  - [ ] CI green

- PR E (Task 5–7 – Observability/BuyEngine/Bundler)
  - [ ] Metryki i tracing
  - [ ] Integracja w engine
  - [ ] (opcjonalnie) Bundler trait + mock/real
  - [ ] CI green
