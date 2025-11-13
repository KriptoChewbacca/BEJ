# Universe-grade TxBuilder Supercomponent — Plan podziału i zadania wdrożeniowe

Cel: wyciąć monolit transakcyjny w modułowy, ekstremalnie konkurencyjny “Universe-grade” superkomponent TxBuilder, z jasno zdefiniowanymi granicami, interfejsami i kontraktami. Ten komponent ma przewyższać dzisiejsze standardy: deterministyczność, RAII na noncach, poprawny porządek instrukcji, opcjonalny bundling Jito, zero copy w hot-path, ścisłe SLO i pełna obserwowalność.

Dokument to instrukcja wykonawcza dla zespołu (Agenci). Kolejność zadań = kolejność realizacji. Każde zadanie zawiera:
- Zakres i rezultat
- Interfejsy/public API
- Wymagania konkurencyjności i wydajności
- Testy i kryteria akceptacji (DoD)
- Ryzyka i checklista

---

## 0) Architektura modułowa (docelowy podział)

Struktura katalogu `src/tx_builder/`:
- `mod.rs` — fasada publiczna i re-eksporty
- `builder.rs` — główna implementacja TxBuilder (klejenie modułów)
- `context.rs` — ExecutionContext (hash/nonce/lease/authority, try_acquire)
- `output.rs` — TxBuildOutput (RAII na NonceLease)
- `instructions.rs` — budowanie i walidacja kolejności instrukcji
- `simulate.rs` — ścieżka symulacji (świadoma durable nonce; skip advance)
- `errors.rs` — błędy (jedno miejsce, spójna taksonomia)
- `legacy.rs` — legacy wrappery (kompatybilność)
- `bundle.rs` — Bundler (Jito) + interfejs (prepare/simulate/send)

Dodatkowo:
- Integracja z `buy_engine.rs`: wstrzyknięcie `Arc<dyn Bundler>` (opcjonalnie pod feature `jito`).
- Wszystkie moduły async-safe: `Send + Sync`, bez blokujących operacji w hot-path.

---

## Zadanie 1 — Skeleton i fasada publiczna (mod.rs) + taksonomia błędów

Zakres:
- Utwórz pliki modułów z minimalnymi definicjami.
- Zdefiniuj spójny `TransactionBuilderError` oraz re-eksporty w `mod.rs`.

Public API (draft):
```rust
// src/tx_builder/errors.rs
#[derive(thiserror::Error, Debug)]
pub enum TransactionBuilderError {
  #[error("Nonce acquisition failed: {0}")] NonceAcquisition(String),
  #[error("Instruction build error (program={program}): {reason}")]
  InstructionBuild { program: String, reason: String },
  #[error("Simulation failed: {0}")] Simulation(String),
  #[error("Signing failed: {0}")] Signing(String),
  #[error("Internal error: {0}")] Internal(String),
}
```

Konkurencyjność i wydajność:
- Brak ciężkich zależności; tylko definicje typów.
- Docelowo zero dodatkowych alokacji w ścieżce błędów (stringi krótkie/kompaktowe).

Testy i DoD:
- Kompilacja na wszystkich wspieranych kombinacjach cech (baseline = domyślne featury).
- Lint: brak ostrzeżeń w module errors.
- Minimalny test konwersji błędów (Display/Debug).

Ryzyka i checklist:
- Spójność nazewnictwa i jednolite mapowanie błędów z innych warstw (nonce/rpc/dex).

---

## Zadanie 2 — ExecutionContext i Nonce RAII (context.rs, output.rs)

Zakres:
- Zaimplementuj `ExecutionContext` z dwoma trybami:
  - enforce_nonce = true: `try_acquire()` lease z NonceManager (fail-fast).
  - enforce_nonce = false: `get_recent_blockhash_with_quorum()`.
- `ExecutionContext::extract_lease()` przenosi lease do `TxBuildOutput`.
- `TxBuildOutput` trzyma `VersionedTransaction`, opcjonalny `NonceLease` i listę required signers.
- Drop semantyka: `TxBuildOutput::drop()` tylko ostrzega; faktyczne zwalnianie zasobu odbywa się w `NonceLease::Drop` (szybkie, nie-async).

Public API (draft):
```rust
// src/tx_builder/context.rs
pub struct ExecutionContext { /* blockhash, nonce_pubkey, nonce_authority, nonce_lease */ }
impl ExecutionContext {
  pub fn extract_lease(self) -> Option<NonceLease>;
  pub fn is_durable(&self) -> bool;
}

// src/tx_builder/output.rs
pub struct TxBuildOutput {
  pub tx: VersionedTransaction,
  pub nonce_guard: Option<NonceLease>,
  pub required_signers: Vec<Pubkey>,
}
impl TxBuildOutput {
  pub fn new(tx: VersionedTransaction, nonce_guard: Option<NonceLease>) -> Self;
  pub async fn release_nonce(self) -> Result<(), TransactionBuilderError>;
}
```

Konkurencyjność i wydajność:
- `try_acquire()` gwarantuje brak TOCTTOU.
- `NonceLease::Drop` zwalnia permit natychmiast (bez async); brak blokowania executorów.
- Wyciąganie required signers bez kopiowania całych struktur (read-only view; kopiujemy jedynie kilka Pubkey).

Testy i DoD:
- Testy jednostkowe: sukces i błąd `try_acquire`, `extract_lease`, idempotencja release (po sukcesie).
- Testy współbieżności: 100 równoległych prób acquire bez podwójnej akwizycji (deterministyczny czas tokio::time::pause/advance).
- Brak wycieków (drop guard → oddaje permit; memory leak check w stress).

Ryzyka i checklist:
- Upewnij się, że `NonceLease` nie wymaga async w Drop. Jeśli tak, dodaj internal release-queue i background task.

---

## Zadanie 3 — Budowa instrukcji i walidacja kolejności (instructions.rs)

Zakres:
- Funkcje planujące instrukcje dla durable nonce z poprawną kolejnością:
  1) `advance_nonce_account`
  2) Compute Budget (CU limit, CU price)
  3) Instrukcja DEX
- Walidator `sanity_check_ix_order()` (tylko debug/test; brak narzutu w prod).

Public API (draft):
```rust
pub struct InstructionPlan { pub instructions: Vec<Instruction>, pub is_durable: bool }

pub fn plan_buy_instructions(
  exec_durable: Option<(Pubkey, Pubkey)>,
  cu_limit: u32,
  prio_fee: u64,
  buy_ix: Instruction,
) -> Result<InstructionPlan, TransactionBuilderError>;

pub fn sanity_check_ix_order(
  instructions: &[Instruction],
  is_durable: bool,
) -> Result<(), TransactionBuilderError>;
```

Konkurencyjność i wydajność:
- Stateless; brak locków; działa na przekazanych danych.
- `sanity_check_ix_order` pod `cfg(debug_assertions)`; w prod pomijane.

Testy i DoD:
- Jednostkowe: poprawny/niepoprawny porządek; empty list error.
- Integracyjne: transakcja durable nonce przyjęta na lokalnym validatorze; odwrotna kolejność → błąd.

Ryzyka i checklist:
- Nie polegać na „magic bytes”; sprawdzaj `program_id == system_program::id()` i semantykę (jeśli dekodowanie jest dostępne).

---

## Zadanie 4 — Ścieżka symulacji świadoma durable nonce (simulate.rs)

Zakres:
- Funkcja usuwająca `advance_nonce` z listy instrukcji do symulacji.
- Builder transakcji do symulacji z zachowaniem metadanych (kont/headers), ale bez konsumowania nonce.

Public API (draft):
```rust
pub fn strip_nonce_for_simulation(instructions: &[Instruction], is_durable: bool) -> Vec<Instruction>;
pub fn build_sim_tx_like(tx: &VersionedTransaction, sim_ix: Vec<Instruction>) -> VersionedTransaction;
```

Konkurencyjność i wydajność:
- Zero alokacji poza koniecznymi kopiami instrukcji; unikaj niepotrzebnych klonów message.

Testy i DoD:
- Jednostkowe: durable → pierwszy ix pominięty; niedurable → bez zmian.
- Integracyjne: symulacja transakcji durable nie konsumuje nonca; sukces symulacji.

Ryzyka i checklist:
- Zachowaj spójność kont i signerów w `VersionedMessage`.

---

## Zadanie 5 — Bundler (bundle.rs) jako osobny moduł

Zakres:
- Wydziel moduł bundlera z interfejsem i implementacją Jito (fallback przez RPC, gdy brak SDK).
- Logika: prepare → simulate → send (multi-region, priorytety, dynamiczny tip).
- Integracja z `recent_fees` (P90/P50; kapowanie przez `max_tip_lamports`, eskalacja pod presją).

Public API (draft):
```rust
#[async_trait]
pub trait Bundler: Send + Sync {
  async fn prepare_bundle(&self, txs: Vec<VersionedTransaction>, target_slot: Option<u64>, backrun_protect: bool)
    -> Result<BundleCandidate, TransactionBuilderError>;
  async fn simulate_bundle(&self, bundle: &BundleCandidate) -> Result<bool, TransactionBuilderError>;
  async fn send_bundle_multi_region(&self, bundle: BundleCandidate) -> Result<Signature, TransactionBuilderError>;
}

pub struct JitoBundler<R: RpcLike> { /* config, recent_fees, rpc facade */ }
```

Konkurencyjność i wydajność:
- Brak globalnych locków; `recent_fees` pod `RwLock<Vec<u64>>` tylko do odczytu.
- `send_bundle_multi_region`: pętla po endpointach w kolejności priorytetów (na początek sekwencyjnie; opcjonalnie równoległy “race” z cancel-on-first-success — do rozważenia wg polityki kosztów).
- Brak busy waiting; wszystko async.

Testy i DoD:
- Jednostkowe: dynamic_tip (różne rozkłady fees), prepare_bundle (hints/flags).
- Integracyjne: fallback przez RPC działa; multi-region fallback: pierwszy fail, drugi sukces.
- E2E: z `buy_engine` — bundler podmieniony na mock (100/100 sukcesów w stress).

Ryzyka i checklist:
- Zewnętrzny SDK Jito pod feature `jito`; fallback zawsze dostępny.
- Limit kosztów: `max_total_cost_lamports` respektowany.

---

## Zadanie 6 — Główna implementacja TxBuilder (builder.rs) + Legacy API (legacy.rs)

Zakres:
- `TxBuilder` łączy: context → instructions → simulate → assemble → output.
- Legacy wrappery (`build_buy_transaction`, itp.) wołają nowy pipeline (domyślnie `enforce_nonce=true`), zachowując kompatybilność.
- W `buy_engine` używać `build_*_output` i trzymać guard do końca broadcastu.

Public API (core):
```rust
pub struct TxBuilder<'a> { /* nonce_mgr ref, fee policy, dex encoder, ... */ }

impl<'a> TxBuilder<'a> {
  pub async fn build_buy_transaction_output(
    &self, candidate: &PremintCandidate, config: TransactionConfig, sign: bool, enforce_nonce: bool
  ) -> Result<TxBuildOutput, TransactionBuilderError>;

  pub async fn build_buy_transaction_with_nonce(
    &self, candidate: &PremintCandidate, config: &TransactionConfig, sign: bool, enforce_nonce: bool
  ) -> Result<VersionedTransaction, TransactionBuilderError>;
}
```

Konkurencyjność i wydajność:
- Zero-blocking: cały path async; brak `std::thread::sleep`, brak `std::sync::Mutex` w hot-path.
- Minimalizuj kopie wektorów instrukcji; prealloc `Vec::with_capacity(4)`.
- Polityka CU/fee—lekka (obliczenia O(n) po krótkich wektorach).

Testy i DoD:
- Jednostkowe: poprawny porządek instrukcji; brak porzuconych lease.
- Integracyjne: durable i nondurable przebieg z symulacją; legacy wrappery kompatybilne.
- E2E: z `buy_engine` (buy→passive→sell→sniffing) w deterministycznym czasie.

Ryzyka i checklist:
- Legacy wrappery nie powinny spamować logami (użyj “once” warn).

---

## Zadanie 7 — Integracja Bundler ↔ BuyEngine

Zakres:
- `BuyEngine` otrzymuje `Option<Arc<dyn Bundler>>`.
- Jeśli bundler jest dostępny i mamy batch txs → ścieżka bundle.
- W przeciwnym wypadku fallback do pojedynczej transakcji.

Konkurencyjność i wydajność:
- Brak współdzielenia ciężkich struktur między engine a bundlerem; parametry przez referencje/Arc.
- Możliwa implementacja “race”: równoległa wysyłka na multi-region (feature-flagowana).

Testy i DoD:
- Integracyjne: ścieżka bundle i ścieżka single tx; metryki build_to_land.
- Stress: 100/100 concurrency; brak podwójnego acquire nonca; brak deadlocków.

Ryzyka i checklist:
- Upewnij się, że release nonce następuje po wyniku wysyłki (sukces → explicit release; błąd → drop RAII).

---

## Zadanie 8 — Konkurencyjność i skalowanie (maksymalna konkurencyjność)

Cele i twarde SLO:
- Przepustowość: 1000+ tx/s zdolności (miarodajny wewnętrzny throughput buildera).
- Overhead buildera: < 5 ms / tx (p95).
- Zero deadlocków; brak gorących locków.
- Land rate: ≥ 95% w produkcyjnym flow (poza zakresem buildera, ale instrumentacja obecna).

Strategie:
- Sharding nonców (opcja): per-nonce semafory; odciążenie globalnego locka.
- Atomiki i RwLocki tylko do odczytu (np. `recent_fees`).
- Kanały: unbounded w miejscach sygnałowych; bounded w miejscach backpressure.
- Tokio: użycie multi-thread runtime; krytyczne taski oznacz `instrument`.

Testy i DoD:
- Microbenchmarks (criterion): create context, plan instructions, assemble tx — p50/p95.
- Profil (flamegraph): brak niepotrzebnych alokacji w hot-path.
- Stress: 1k równoległych buildów z mock DEX, brak regresji czasu i pamięci.

---

## Zadanie 9 — Obserwowalność i bezpieczeństwo

Obserwowalność:
- Tracing: `trace_id`, `span_id`, `correlation_id` w builderze i bundlerze.
- Metryki: hist latencji (`build_to_land`, `acquire_lease_ms`, `prepare_bundle_ms`), liczniki sukcesów/porażek, histogram dynamic_tip.
- Export: JSON/Prometheus; p99 kalkulacje.

Bezpieczeństwo:
- Maskowanie sekretów w logach; zero kluczy w debug output.
- Taint tracking na wejściu (jeśli zasilanie z zewn. źródeł).
- Rotacja authority (po stronie NonceManager) — builder tylko korzysta.

Testy i DoD:
- Snapshoty logów bez sekretów.
- Metryki obecne i aktualizowane na ścieżkach sukces/failure.

---

## Zadanie 10 — CI, gating i kompatybilność

CI:
- Required: tests-nightly (baseline = domyślne featury), format-check, clippy (bez -D warnings na początku), cargo-deny (offline).
- Build matrix: tylko wspierane kombinacje; eksperymentalne w osobnym workflow/allow-failure.

Kompatybilność:
- Legacy API działa (wrappery); deprecacja tylko w logach (warn-once).
- Feature flags: `jito`, `pumpfun`, `test_utils`, `mock-mode`.
- Release tag (np. v0.2.0) po zielonej macierzy.

DoD:
- Zielony CI (required jobs).
- README/Docs zaktualizowane (sekcja “Migration” i “Usage”).

---

## Wspólne wymagania jakościowe (dla wszystkich zadań)

- Zero blocking w async (zakaz `std::thread::sleep`, `Mutex` w hot-path).
- Brak “globally mutable state”; wszystko przekazywane przez referencje/Arc.
- Determinizm testów: `tokio::time::pause/advance`, `fastrand::seed`.
- Zero log spam: poziomy `debug`/`trace` tylko przy włączonych feature/log level.
- Dokumentacja inline (rustdoc) i przykłady użycia (examples/*), kompilujące się w CI.

---

## Przykładowe sygnatury (ściąga dla Agentów)

```rust
// Context
pub async fn prepare_execution_context_with_enforcement(
  &self, enforce_nonce: bool
) -> Result<ExecutionContext, TransactionBuilderError>;

// Output
impl TxBuildOutput {
  pub async fn release_nonce(self) -> Result<(), TransactionBuilderError>;
}

// Instructions
pub fn plan_buy_instructions(
  durable: Option<(Pubkey, Pubkey)>, cu_limit: u32, prio_fee: u64, buy_ix: Instruction
) -> Result<InstructionPlan, TransactionBuilderError>;

// Simulation
pub fn strip_nonce_for_simulation(ix: &[Instruction], is_durable: bool) -> Vec<Instruction>;

// Bundler
#[async_trait]
pub trait Bundler {
  async fn prepare_bundle(
    &self, txs: Vec<VersionedTransaction>, target_slot: Option<u64>, backrun_protect: bool
  ) -> Result<BundleCandidate, TransactionBuilderError>;
  async fn simulate_bundle(&self, bundle: &BundleCandidate) -> Result<bool, TransactionBuilderError>;
  async fn send_bundle_multi_region(&self, bundle: BundleCandidate) -> Result<Signature, TransactionBuilderError>;
}
```

---

## Ryzyka globalne i mitigacje

- Podwójny acquire nonca w ekstremach — używaj `try_acquire` i/lub sharding semaforów.
- Regressje wydajności — microbench + flamegraph na PR.
- Kruchość walidacji kolejności — walidator tylko w debug/test; w prod ufamy builderowi.
- Zależność od SDK Jito — feature-flag + twardy fallback na RPC.

---

## Akceptacja końcowa (Universe-grade)

- Pełny podział na moduły; publiczna fasada `tx_builder`.
- SLO wydajności: p95 overhead < 5 ms; przepustowość 1000+ tx/s (wewnętrzny throughput).
- RAII nonca: brak wycieków lease w testach stress i E2E.
- Poprawny porządek instrukcji durable nonce (walidowany w testach).
- Bundler oddzielny, gotowy na realny Jito SDK; fallback działający.
- Zielone CI z wymaganymi jobami; dokumentacja i migracja gotowe.

---