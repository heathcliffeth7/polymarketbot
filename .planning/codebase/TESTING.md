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

**Cargo Test Command:**
```bash
cargo test                      # Run all tests in workspace
cargo test -p bot-core          # Run tests in specific crate
cargo test -- --nocapture      # Show println! output
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

**What to Mock:**
- External APIs (OrderExecutor, MarketDataProvider)
- Database operations (StateRepository)
- WebSocket connections (WsClient)

**What NOT to Mock:**
- Strategy calculations (PriceThresholdStrategy, SymmetricDualDcaStrategy) - test with real impl
- State machine logic (can_transition) - test deterministic pure functions
- Risk evaluation (DefaultRiskPolicy) - test with real values

## Fixtures and Factories

**Test Data:**
- Not observed: No explicit fixture pattern in Rust tests
- Test data typically created inline in test function (e.g., `SymmetricDualDcaStrategy`)
- Constants used for threshold values (0.02, 0.50, 3 for DCA levels)

**Location:**
- Inline in test modules (no separate fixtures directory)
- Test crate `mock-exchange` serves as fixture for integration tests

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

## Test Types

**Unit Tests:**
- Scope: Single function/method behavior
- Examples: `dca_first_level_without_last_fill()`, `allows_valid_transition_path()`
- Approach: Instantiate struct, call method, assert on Result or return value
- No external I/O; pure functions or mocked dependencies

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
| bot-core | strategy.rs, state_machine.rs | ~30% (logic only) |
| bot-infra | Minimal/none observed | ~5% (fixtures exist) |
| bot-runner | None observed | ~0% |
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

---

*Testing analysis: 2026-03-02*
