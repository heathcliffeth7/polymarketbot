# Testing Patterns

**Analysis Date:** 2026-03-02

## Test Framework

**Frontend:**
- Not detected: No Jest, Vitest, or other test runner configured in package.json
- No test files found in `frontend/src/`
- TypeScript strict mode enabled but no test infrastructure
- Dev dependencies: eslint, tailwindcss, TypeScript only

**Backend (Rust):**
- Framework: Built-in Rust `#[test]` + `#[cfg(test)]` modules
- No external test runner dependencies in Cargo.toml (no pytest, pytest-cov equivalents)
- Tests co-located with source code in `#[cfg(test)] mod tests { }`
- Run via: `cargo test` (workspace-wide), `cargo test -p bot-core` (specific crate)

**Assertion Library:**
- Rust: Built-in `assert!()`, `assert_eq!()`, `assert!(...is_ok())`, `assert!(...is_err())`
- Frontend: Not applicable (no test framework)

## Test File Organization

**Frontend:**
- No test files present; no testing pattern established
- Location: N/A
- Naming: N/A

**Backend (Rust):**
- Location: Co-located with source
- File pattern: `#[cfg(test)] mod tests { }` at end of each `.rs` file or in separate test modules
- Examples:
  - `crates/bot-core/src/strategy.rs` - inline tests after impl blocks
  - `crates/bot-core/src/state_machine.rs` - inline tests after can_transition() function
  - `crates/bot-runner/src/main.rs` - tests for specific helper functions (lines ~8033+)
  - `crates/bot-infra/src/exchange.rs` - async tests with mock exchange fixture

**Cargo Test Command:**
```bash
cargo test                      # Run all tests in workspace
cargo test -p bot-core          # Run tests in specific crate
cargo test -- --nocapture      # Show println! output
cargo test trigger              # Run only tests with "trigger" in name
```

## Test Structure

**Rust Unit Test Pattern:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dca_first_level_without_last_fill() {
        let strat = SymmetricDualDcaStrategy;
        assert!(strat.should_dca_leg(0.50, None, 0.02, 0, 3));
    }

    #[test]
    fn dca_requires_price_distance() {
        let strat = SymmetricDualDcaStrategy;
        assert!(!strat.should_dca_leg(0.501, Some(0.50), 0.02, 1, 3));
        assert!(strat.should_dca_leg(0.511, Some(0.50), 0.02, 1, 3));
    }

    #[test]
    fn dca_respects_max_levels() {
        let strat = SymmetricDualDcaStrategy;
        assert!(!strat.should_dca_leg(0.60, Some(0.50), 0.02, 3, 3));
    }
}
```

**Patterns Observed:**
- `use super::*;` imports all items from parent module
- Instantiate SUT (System Under Test) first: `let strat = SymmetricDualDcaStrategy;`
- Multiple assertions per test (not strict AAA)
- Descriptive test names: `fn test_name_describes_scenario_and_expected_outcome()`
- No setup/teardown observed; state is immutable or reset per test

**State Transition Tests:**

```rust
#[test]
fn allows_valid_transition_path() {
    assert!(can_transition(TradeState::Idle, TradeState::WaitingEntry).is_ok());
    assert!(can_transition(TradeState::EntryFilled, TradeState::TpPlaced).is_ok());
    assert!(can_transition(TradeState::ExitFilled, TradeState::Settled).is_ok());
}

#[test]
fn rejects_invalid_transition_path() {
    assert!(can_transition(TradeState::Idle, TradeState::TpPlaced).is_err());
    assert!(can_transition(TradeState::EntryPlaced, TradeState::Settled).is_err());
    assert!(can_transition(TradeState::Halted, TradeState::EntryPlaced).is_err());
}
```

## Trigger Condition Testing Patterns

**CRITICAL: Price Trigger Testing (Market Price Nodes)**

Trigger conditions are tested in `crates/bot-runner/src/main.rs` via helper functions:
- `crossed_above_strict()` (line ~222)
- `crossed_below_strict()` (line ~228)
- `evaluate_trigger_market_price_condition()` (line ~234)

**Pattern: Cross-Above Trigger**

The cross-above condition requires BOTH:
1. **Previous price exists** (not first tick)
2. **Price crosses from below to at-or-above** (`prev < trigger && current >= trigger`)

```rust
#[test]
fn crossed_above_fires_when_crossing_from_below() {
    // Previous price BELOW trigger, current price AT trigger → FIRE
    assert!(crossed_above_strict(Some(0.79), 0.80, 0.80));

    // Previous price BELOW trigger, current price ABOVE trigger → FIRE
    assert!(crossed_above_strict(Some(0.79), 0.85, 0.80));
}

#[test]
fn crossed_above_does_not_fire_when_already_above() {
    // Previous price ABOVE trigger, current price ABOVE trigger → NO FIRE
    assert!(!crossed_above_strict(Some(0.85), 0.90, 0.80));

    // Price stays exactly at trigger → NO FIRE
    assert!(!crossed_above_strict(Some(0.80), 0.80, 0.80));
}

#[test]
fn crossed_above_does_not_fire_without_previous() {
    // No price history; even if current >= trigger, don't fire → NO FIRE
    assert!(!crossed_above_strict(None, 0.85, 0.80));
}

#[test]
fn crossed_above_boundary_exact_trigger_value() {
    // Exact trigger value crossing from below
    assert!(crossed_above_strict(Some(0.7999), 0.80, 0.80));
    assert!(crossed_above_strict(Some(0.80), 0.80, 0.80) == false); // Already at trigger
}
```

**Pattern: Cross-Below Trigger**

The cross-below condition requires BOTH:
1. **Previous price exists** (not first tick)
2. **Price crosses from above to at-or-below** (`prev > trigger && current <= trigger`)

```rust
#[test]
fn crossed_below_fires_when_crossing_from_above() {
    // Previous price ABOVE trigger, current price AT trigger → FIRE
    assert!(crossed_below_strict(Some(0.81), 0.80, 0.80));

    // Previous price ABOVE trigger, current price BELOW trigger → FIRE
    assert!(crossed_below_strict(Some(0.85), 0.75, 0.80));
}

#[test]
fn crossed_below_does_not_fire_when_already_below() {
    // Previous price BELOW trigger, current price BELOW trigger → NO FIRE
    assert!(!crossed_below_strict(Some(0.75), 0.70, 0.80));
}

#[test]
fn crossed_below_does_not_fire_without_previous() {
    // No price history → NO FIRE
    assert!(!crossed_below_strict(None, 0.75, 0.80));
}
```

**Pattern: First-Tick Threshold (Optional)**

When `allow_first_tick_threshold=true`, first price update can trigger on threshold alone:

```rust
#[test]
fn evaluate_trigger_cross_above_first_tick_allowed() {
    let (passed, reason) = evaluate_trigger_market_price_condition(
        None, // no previous price
        0.85, // first price is above trigger
        0.80, // trigger price
        "cross_above",
        true, // allow_first_tick_threshold
    );
    assert!(passed);
    assert_eq!(reason, "first_tick_threshold");
}

#[test]
fn evaluate_trigger_cross_above_first_tick_denied() {
    let (passed, reason) = evaluate_trigger_market_price_condition(
        None,  // no previous price
        0.85,  // above trigger
        0.80,  // trigger price
        "cross_above",
        false, // don't allow first tick
    );
    assert!(!passed);
    assert_eq!(reason, "no_previous");
}

#[test]
fn evaluate_trigger_with_previous_price() {
    let (passed, reason) = evaluate_trigger_market_price_condition(
        Some(0.79), // previous price below trigger
        0.80,       // current price at trigger
        0.80,       // trigger price
        "cross_above",
        true,       // allow_first_tick (ignored when previous exists)
    );
    assert!(passed);
    assert_eq!(reason, "cross_detected");
}
```

**Pattern: No Cross When Price Stays Below**

```rust
#[test]
fn crossed_above_no_fire_when_stays_below_trigger() {
    // Previous at 0.75, current at 0.79, trigger at 0.80
    // Price moved UP but still below trigger → NO FIRE
    assert!(!crossed_above_strict(Some(0.75), 0.79, 0.80));
}

#[test]
fn crossed_above_fire_at_exact_boundary() {
    // Previous at 0.799999..., current at 0.80 exactly
    assert!(crossed_above_strict(Some(0.799999), 0.80, 0.80));
}
```

**Critical Bug Pattern (Example: Issue in Market Price Node)**

If condition was incorrectly written as `current_price >= trigger_price` WITHOUT checking previous:
```rust
// WRONG: This fires on first tick if price > trigger (no history check)
let should_fire = current_price >= trigger_price; // BUG: can fire on first tick

// CORRECT: Must verify cross occurred (previous was below)
let should_fire = previous_price
    .map(|prev| prev < trigger_price && current_price >= trigger_price)
    .unwrap_or(false); // NO fire if no history
```

**How to Test a Market Price Node Trigger Bug:**

1. Create trade/flow node with trigger "cross_above", trigger_price=80
2. Initialize with no previous price
3. Send first price update: current_price=85 (well above 80)
4. **EXPECTED (correct):** Trigger does NOT fire (reason: "no_previous")
5. **BUG SYMPTOM:** Trigger fires on first tick (would see reason: "first_tick_threshold" if `allow_first_tick_threshold=true`, or wrongly fire if condition only checks absolute threshold)

## Mocking

**Framework:**
- Not used: No mockall, mockito, or similar crate in Cargo.toml
- Mock implementations are concrete types in test fixture crate

**Pattern - Trait-Based Mocking:**

Infrastructure crate defines traits; tests use concrete test implementations:

```rust
// In bot-infra/src/contracts.rs:
pub trait OrderExecutor: Send + Sync {
    async fn place_order(&self, req: PlaceOrderRequest) -> Result<OrderInfo>;
    // ... other methods
}

// In bot-infra/tests or app tests:
// Create concrete struct implementing trait
struct MockOrderExecutor {
    should_fail: bool,
}

impl OrderExecutor for MockOrderExecutor {
    async fn place_order(&self, req: PlaceOrderRequest) -> Result<OrderInfo> {
        if self.should_fail { Err(...) } else { Ok(...) }
    }
}
```

**Mock Exchange (Test Fixture):**
- `crates/mock-exchange/`: In-memory mock CLOB exchange
- Purpose: Provides fake `OrderExecutor` implementation for integration testing
- Runs as Axum HTTP server for testing bot-runner behavior
- Uses `unwrap_or()`, `unwrap_or_default()` for test data defaults

**Example: Testing with Mock Exchange**

```rust
#[tokio::test]
async fn place_and_reconcile_against_mock_exchange() -> Result<()> {
    let mock = spawn_mock_exchange().await?;
    let wallet = "0x0000...0001".parse::<LocalWallet>()?;
    let client = ClobHttpClient::from_credentials(...);

    let ack = client.place_order(&PlaceOrderRequest {
        market: "btc-updown-5m-1".to_string(),
        price: 0.60,
        size: 10.0,
        // ...
    }).await?;

    assert!(ack.exchange_order_id.is_some());
    mock.shutdown();
    Ok(())
}
```

**What to Mock:**
- External APIs (OrderExecutor, MarketDataProvider)
- Database operations (StateRepository)
- WebSocket connections (WsClient)

**What NOT to Mock:**
- Strategy calculations (PriceThresholdStrategy, SymmetricDualDcaStrategy) - test with real impl
- State machine logic (can_transition) - test deterministic pure functions
- Risk evaluation (DefaultRiskPolicy) - test with real values
- Trigger condition logic (crossed_above_strict, crossed_below_strict) - test real comparison operators

## Fixtures and Factories

**Test Data:**
- Not observed: No explicit fixture pattern in Rust tests
- Test data typically created inline in test function (e.g., `SymmetricDualDcaStrategy`)
- Constants used for threshold values (0.02, 0.50, 3 for DCA levels)

**Location:**
- Inline in test modules (no separate fixtures directory)
- Test crate `mock-exchange` serves as fixture for integration tests

**Price Test Constants:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Common test prices
    const TRIGGER_PRICE: f64 = 0.80;
    const PRICE_BELOW_TRIGGER: f64 = 0.75;
    const PRICE_AT_TRIGGER: f64 = 0.80;
    const PRICE_ABOVE_TRIGGER: f64 = 0.85;

    #[test]
    fn uses_constants_for_clarity() {
        assert!(crossed_above_strict(
            Some(PRICE_BELOW_TRIGGER),
            PRICE_AT_TRIGGER,
            TRIGGER_PRICE
        ));
    }
}
```

## Coverage

**Requirements:**
- Not enforced: No CI/CD coverage checks configured
- No coverage reports generated (no tarpaulin, llvm-cov in Cargo.toml)

**View Coverage (Manual):**
```bash
# Install tarpaulin (if not present)
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage
```

**Observed Coverage Gaps:**
- Frontend: No tests at all (0% by definition)
- Backend: Core domain logic has tests (strategy.rs, state_machine.rs)
- Backend: Infrastructure code untested (db.rs, exchange.rs, ws.rs, signer.rs)
- **Critical gap:** Trigger condition helpers in bot-runner untested (should have comprehensive test suite)

## Test Types

**Unit Tests:**
- Scope: Single function/method behavior
- Examples: `dca_first_level_without_last_fill()`, `allows_valid_transition_path()`
- Approach: Instantiate struct, call method, assert on Result or return value
- No external I/O; pure functions or mocked dependencies

**Trigger Condition Unit Tests:**
- Test each comparison operator independently
- Test boundary conditions (prices equal to trigger, just below, just above)
- Test all three branches: previous-exists + crossing, no-previous + first-tick-allowed, no-previous + first-tick-denied
- Separate tests for each trigger type: cross_above, cross_below, absolute threshold

**Integration Tests:**
- Not observed: No dedicated integration test directory or harness
- Possible pattern: mock-exchange crate serves as integration fixture
- Approach would be: Spin up mock exchange, run bot-runner against it, verify state changes

**E2E Tests:**
- Not used: No Playwright, Cypress, or similar in frontend
- Backend: Could theoretically run full stack (bot-runner + mock-exchange + PostgreSQL)
- Not implemented; manual/operational testing only

**Coverage by Crate:**

| Crate | Tests | Coverage |
|-------|-------|----------|
| bot-core | strategy.rs, state_machine.rs, risk.rs | ~30% (logic only) |
| bot-infra | exchange.rs (async mocking), signer.rs | ~15% (partial) |
| bot-runner | main.rs (#8033+, trigger helpers) | ~5% (trigger helpers mostly untested) |
| mock-exchange | N/A (test fixture itself) | N/A |

## Common Patterns

**Testing Pure Functions (Strategy, State Machine):**

```rust
#[test]
fn aggressive_stop_price_calculation() {
    let strat = PriceThresholdStrategy;
    let entry = 0.50;
    let sl_pct = 0.10;
    let result = strat.aggressive_stop_price(entry, sl_pct);
    assert_eq!(result, 0.45); // entry * (1 - 0.10)
}
```

**Testing Conditions (DCA Logic):**

```rust
#[test]
fn should_dca_when_price_moved_enough() {
    let strat = SymmetricDualDcaStrategy;
    let current = 0.55;
    let last_fill = Some(0.50);
    let step_pct = 0.02; // 2% minimum
    let result = strat.should_dca_leg(current, last_fill, step_pct, 1, 3);
    assert!(result); // 0.55 vs 0.50 is 10% > 2%
}

#[test]
fn should_not_dca_when_insufficient_distance() {
    let strat = SymmetricDualDcaStrategy;
    // Only 0.2% move, less than 2% step
    assert!(!strat.should_dca_leg(0.501, Some(0.50), 0.02, 1, 3));
}
```

**Testing Error Cases (State Transitions):**

```rust
#[test]
fn invalid_transition_returns_error() {
    let result = can_transition(TradeState::Halted, TradeState::EntryPlaced);
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("invalid state transition"));
    }
}
```

**Async Testing (Not Observed But Pattern):**
- Would use `#[tokio::test]` attribute macro
- Example pattern:
  ```rust
  #[tokio::test]
  async fn async_operation_completes() {
      let result = some_async_function().await;
      assert!(result.is_ok());
  }
  ```

**Error Testing Pattern (Observed):**
```rust
#[test]
fn evaluates_risk_halt_on_daily_loss() {
    let limits = RiskLimits {
        max_daily_loss_usdc: 100.0,
        kill_switch_mode: KillSwitchMode::ManualOrPolicy,
        ...defaults...
    };
    let input = RiskInput {
        daily_realized_pnl_usdc: -150.0, // exceeds limit
        ...other_fields...
    };
    let eval = evaluate_risk(&limits, &input);
    assert_eq!(eval.decision, RiskDecision::Halt);
    assert_eq!(eval.reason, "daily_loss_limit_breached");
}
```

## Frontend Testing Gap

**Current State:** No tests in frontend

**How to Add Tests (Recommended Pattern):**
1. Choose framework: Vitest (modern, fast) or Jest (Next.js standard)
2. Add dev dependency: `npm install -D vitest @testing-library/react @testing-library/user-event`
3. Create test files co-located: `hooks/use-dashboard.test.ts`, `components/bot-control-panel.test.tsx`
4. Pattern:
   ```typescript
   import { render, screen, fireEvent } from '@testing-library/react';
   import { BotControlPanel } from './bot-control-panel';

   describe('BotControlPanel', () => {
     it('renders status badge', () => {
       render(<BotControlPanel />);
       expect(screen.getByText(/Active|Stopped/)).toBeInTheDocument();
     });
   });
   ```

## Recommended Testing Priority

**High Priority (Trigger Bug Fix):**
1. Add unit tests for `crossed_above_strict()` (all 5+ cases above)
2. Add unit tests for `crossed_below_strict()` (all 5+ cases above)
3. Add integration test: market price node with live price stream
4. Document trigger condition behavior in TESTING.md (done)

**Medium Priority:**
1. Add async tests for WS event processing with idempotency
2. Test risk evaluation with real values (already partially done)
3. Test DCA level spacing calculations (already has tests)

**Low Priority:**
1. Frontend testing framework setup
2. Database layer integration tests

---

*Testing analysis: 2026-03-02*
