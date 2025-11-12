# Issue #11: PORT TEST: buy_enters_passive_and_sell_returns_to_sniffing - Full MockTxBuilder Integration

## Overview

This test has been successfully migrated to the new API (UnboundedReceiver, UniverseNonceManager::new_for_testing) but requires comprehensive mocking infrastructure to execute the full transaction flow and state transitions.

## Current Status

- **API Migration:** ✅ Complete
- **Test Status:** Ignored (see src/buy_engine.rs line ~2619)
- **Blocking Issue:** Engine.run() not processing candidates due to missing integration mocking

## Requirements for Implementation

### 1. MockTxBuilder
Create a comprehensive mock transaction builder that:
- Implements the same interface as `TransactionBuilder`
- Returns deterministic `VersionedTransaction` instances
- Simulates success/failure scenarios
- Does not require network access

### 2. State Transition Validation
The test must verify:
- ✅ Initial state: `Mode::Sniffing`
- ⚠️ After buy: `Mode::PassiveToken(mint)`
- ⚠️ After sell: Return to `Mode::Sniffing`
- ✅ Holdings tracking (0.0 → 1.0 → 0.0)
- ✅ Active token management

### 3. Deterministic Time Management
- Use `tokio::time::pause()` and `advance()` for deterministic execution
- Properly coordinate spawned tasks with time advancement
- Ensure channel receives and timeouts work correctly with paused time

### 4. No Network Access
- All RPC calls mocked
- No real transaction submissions
- No actual nonce refreshes (handled via UniverseNonceManager::new_for_testing with 3600s TTL)

## Technical Challenges

### Challenge 1: Engine.run() Loop Execution
The `engine.run()` method has multiple checks and filters:
1. Circuit breaker `should_allow()` check
2. Mode verification (`is_sniffing()`)
3. Channel receive with timeout
4. Security validation
5. Rate limiting
6. Candidate filtering (`is_candidate_interesting()`)

**Solution Needed:** Mock or bypass these checks in test mode, or ensure test data passes all filters.

### Challenge 2: Async Coordination with Paused Time
When using `tokio::time::pause()`, spawned tasks don't progress unless time is advanced, but advancing time may cause the test task to progress faster than the spawned engine task.

**Solution Needed:** Either:
- Use multi-threaded runtime with careful time management
- Use single-threaded runtime with explicit task yielding
- Create test-specific engine.run() variant that doesn't use timeouts

### Challenge 3: Transaction Flow Completion
The placeholder transaction mechanism (when tx_builder is None) may not trigger all the necessary state transitions.

**Solution Needed:** Implement MockTxBuilder that properly simulates:
- Transaction building
- Nonce acquisition
- Broadcast success
- State updates

## Implementation Checklist

- [ ] Create `MockTxBuilder` struct in test module
  - [ ] Implement transaction building methods
  - [ ] Return deterministic transactions
  - [ ] Support success/failure scenarios
- [ ] Update test to use MockTxBuilder
  - [ ] Pass mock to `BuyEngine::new()` instead of None
  - [ ] Configure mock for success scenario
- [ ] Fix time coordination
  - [ ] Experiment with pause()/advance() patterns
  - [ ] OR use real sleep() with shorter timeouts
  - [ ] Ensure spawned task actually executes
- [ ] Verify state transitions
  - [ ] Add debug logging to track state changes
  - [ ] Ensure candidate passes all filters
  - [ ] Confirm buy operation completes
- [ ] Add documentation
  - [ ] Document MockTxBuilder usage
  - [ ] Explain coordination patterns
  - [ ] Provide example for similar tests

## Related Files

- `src/buy_engine.rs` (line ~2619) - Test implementation
- `src/tx_builder.rs` - TransactionBuilder interface
- `src/types.rs` - AppState and Mode definitions

## Acceptance Criteria

✅ Test runs without #[ignore] attribute
✅ Test passes consistently (no flakiness)
✅ No network access during test execution
✅ All state transitions verified
✅ Deterministic execution (seeded RNG, controlled time)
✅ Documentation updated with MockTxBuilder usage patterns

## Priority

**Medium** - This test represents important integration behavior, but the core API migration (primary goal of Issue #10) is complete. Four other buy_engine tests are passing successfully.

## Labels

- `test`
- `enhancement`
- `mocking`
- `buy-engine`
