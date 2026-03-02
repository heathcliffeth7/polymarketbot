# Coding Conventions

**Analysis Date:** 2026-03-02

## Naming Patterns

**Files:**
- TypeScript/React: kebab-case (e.g., `use-dashboard.ts`, `bot-control-panel.tsx`)
- Utilities and types: kebab-case (e.g., `flow-canvas-utils.ts`, `http-client.ts`)
- Rust: snake_case (e.g., `state_machine.rs`, `risk.rs`)
- API routes: kebab-case segments in path structure (e.g., `api/bot/control/route.ts`)

**Functions:**
- TypeScript: camelCase (e.g., `getTrades()`, `requestJson()`, `handleAction()`, `isAllowedFile()`)
- React hooks: prefix with `use` in camelCase (e.g., `useDashboard()`, `usePolling()`, `useBotStatus()`)
- Private helpers: camelCase with underscore or unprefixed (e.g., `sleep()`, `isAbortError()`, `shouldRetry()`)
- Rust: snake_case (e.g., `can_transition()`, `evaluate_risk()`, `entry_signal()`)

**Variables:**
- Constants: UPPER_SNAKE_CASE (e.g., `DEFAULT_TIMEOUT_MS`, `CONFIG_ENC_PREFIX`, `COOKIE_NAME`)
- Local/state: camelCase (e.g., `controlUnavailable`, `loading`, `paramIdx`, `filters`)
- Database columns: snake_case (e.g., `opened_at`, `exchange_order_id`, `market_slug`)
- React state: camelCase with semantic names (e.g., `data`, `mutate`, `error`, `isLoading`)

**Types:**
- TypeScript: PascalCase (e.g., `Trade`, `Order`, `RiskDecision`, `ClientRequestError`, `TradeState`)
- Type aliases: PascalCase (e.g., `OrderIntent`, `OrderStatus`, `ExecutionMode`, `TradeFlowGraph`)
- Interface properties: camelCase matching database snake_case (e.g., `id`, `market_id`, `entry_price`)
- Rust types: PascalCase (e.g., `TradeState`, `RiskPolicy`, `OrderExecutor`, `StateRepository`)
- Rust trait methods: snake_case (e.g., `entry_signal()`, `take_profit_price()`, `evaluate()`)

## Code Style

**Formatting:**
- Prettier not explicitly configured; eslint.config.mjs uses ESLint 9 with Next.js defaults
- Indentation: 2 spaces (Next.js/React convention)
- Line length: No explicit limit observed, but code typically stays under 100 chars
- Trailing commas: Used in object/array literals

**Linting:**
- Framework: ESLint 9 with `eslint-config-next` (core-web-vitals + typescript presets)
- Config file: `frontend/eslint.config.mjs`
- Rules: Default Next.js + TypeScript rules enforced
- No explicit Prettier config; formatting conventions follow Next.js patterns

**TypeScript Settings:**
- Strict mode: Enabled (`"strict": true`)
- Target: ES2017
- Module resolution: bundler
- Module system: esnext
- JSX: react-jsx
- Path aliases: `@/*` → `./src/*`

**Rust Edition:**
- Edition: 2021
- Workspace dependencies centralized in `Cargo.toml`

## Import Organization

**TypeScript Order:**

1. External libraries (React, Next.js, dependencies)
2. Internal lib utilities and types (`@/lib/...`)
3. Internal components and hooks (`@/components/...`, `@/hooks/...`)

**Example from `bot-control-panel.tsx`:**
```typescript
'use client';

import { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { useBotStatus } from '@/hooks/use-bot-status';
import { Play, Square, RotateCcw } from 'lucide-react';
```

**Path Aliases:**
- TypeScript: `@/*` maps to `frontend/src/*`
- Used consistently across app (queries, hooks, lib, components)
- Enables clean imports: `import { getTrades } from '@/lib/queries/trades'`

**Rust:**
```rust
use anyhow::{Context, Result};
use bot_core::{...traits and types...};
use bot_infra::{...infrastructure...};
use std::{...standard library...};
use tokio::{...async runtime...};
use tracing::{...logging...};
```

## Error Handling

**TypeScript/JavaScript:**

**Custom Error Classes:**
- `ClientRequestError` wraps network/timeout/http/parse errors
  - Fields: `kind` (error category), `endpoint` (URL), `status` (HTTP status), `cause` (original error)
  - Used in `http-client.ts` for classified error handling
  - Formatted for user display via `formatClientRequestError()`

**Try-Catch Pattern:**
```typescript
// API routes catch all errors and return 500 with generic message
export async function GET(req: NextRequest) {
  try {
    const result = await getTrades(filters);
    return NextResponse.json(result);
  } catch (err) {
    console.error('Trades error:', err);
    return NextResponse.json({ error: 'Failed to load trades' }, { status: 500 });
  }
}
```

**Fetch Retry Logic:**
```typescript
// requestJson() implements exponential backoff for network/timeout errors
for (let attempt = 0; ; attempt += 1) {
  try {
    const res = await fetch(endpoint, { signal: controller.signal });
    if (!res.ok) throw new ClientRequestError(...);
    return (await res.json()) as T;
  } catch (err) {
    const normalized = normalizeClientError(err, endpoint, timeoutMs);
    if (shouldRetry(normalized, attempt, retries)) {
      await sleep(retryDelayMs);
      continue;
    }
    throw normalized;
  }
}
```

**Rust:**

**Result Type:**
- Use `anyhow::Result<T>` for fallible functions in bot-infra and bot-runner
- Use `thiserror::Error` for domain errors in bot-core (e.g., `TransitionError`)

**Pattern:**
```rust
#[derive(Debug, Error)]
pub enum TransitionError {
    #[error("invalid state transition: {from:?} -> {to:?}")]
    Invalid { from: TradeState, to: TradeState },
}

pub fn can_transition(from: TradeState, to: TradeState) -> Result<(), TransitionError> {
    if valid { Ok(()) } else { Err(TransitionError::Invalid { from, to }) }
}
```

**Unwrap in Test Fixtures:**
- `mock-exchange` (test fixture) uses `unwrap_or_else()` and `unwrap_or()` for defaults
- Production code avoids unwrap; uses Result propagation or ? operator

## Logging

**Framework:**
- TypeScript: `console.error()` for errors (e.g., in API routes)
- Rust: `tracing` crate with structured logging
  - Levels: `info!()`, `warn!()`, `error!()`
  - Attributes: None observed; unstructured strings used

**When to Log:**
- TypeScript: API errors, action failures (e.g., `console.error('Bot control error:', err)`)
- Rust: Critical state transitions, errors, reconnects (inferred from crates)

## Comments

**When to Comment:**
- Complex logic (trade state transitions, risk evaluation, DCA calculations)
- Non-obvious type guards or data transformations
- Rationale for unusual patterns (e.g., retry logic, encryption)

**Example (Observed in Strategy):**
No explicit comments in most code; logic is self-documenting via function names and type signatures.

## Function Design

**Size:**
- Small, focused functions (10-30 lines typical)
- Single responsibility principle observed
- Examples: `getTrades()`, `requestJson()`, `can_transition()`, `entry_signal()`

**Parameters:**
- Use object/interface for multiple related params (e.g., `TradeFilters`, `RequestJsonOptions`, `RiskLimits`)
- Single primitives okay for flags or simple values
- TypeScript: Type all parameters explicitly

**Return Values:**
- Async functions return Promise-wrapped types
- Query functions return `PaginatedResponse<T>` with data, total, page, limit, totalPages
- Strategy/risk functions return computed scalars (prices, booleans, decisions)
- Rust: Return Result<T, E> for fallible operations

## Module Design

**Exports:**
- Named exports (e.g., `export function getTrades()`, `export type Trade`)
- Single default export rare; avoid barrel re-exports of implementation
- Frontend lib structure: `index` not observed; import directly from files

**Barrel Files:**
- Not observed in frontend/src; imports are direct (e.g., `from '@/lib/queries/trades'`)
- Rust crates use pub mod declarations in lib.rs for module re-export

**Organization:**
- Queries co-located: `lib/queries/` (trades.ts, orders.ts, markets.ts)
- Hooks grouped: `hooks/` (use-dashboard.ts, use-polling.ts)
- Components grouped: `components/` (ui/, control/, trade-builder/)
- API routes: Next.js convention - `app/api/[segment]/route.ts`

## Client vs. Server

**'use client' Directive:**
- All React components marked with `'use client'` at top
- Hooks (useDashboard, usePolling) are client-side
- State management via useState + SWR for data fetching

**Server-Side:**
- API routes (app/api/) run on server
- Database pool initialized in `lib/db.ts` (singleton pattern with globalThis)
- Auth token generation/verification in `lib/auth.ts` (server context)

## Type Boundaries

**API Response Contracts:**
- All queries return typed responses (PaginatedResponse<Trade>, etc.)
- HTTP errors throw ClientRequestError with kind classification
- Response parsing catches JSON errors and treats as parse errors

**Serialization:**
- Dates come from DB as ISO strings; handled as strings in frontend (no Date object parsing)
- Numbers use JavaScript number type (fits DB decimals/floats)
- UUIDs handled as strings

## Rust Conventions (Backend)

**Module Structure:**
- `bot-core/src/`: Pure domain logic (no I/O)
  - `types.rs`: Enums and data structures
  - `strategy.rs`: Strategy trait and implementations
  - `risk.rs`: Risk policy trait and evaluation
  - `state_machine.rs`: State transition rules
- `bot-infra/src/`: Infrastructure (I/O, DB, exchange, signing)
  - `contracts.rs`: Key trait definitions and implementations
  - `db.rs`: Database operations via PostgresRepository
  - `exchange.rs`: CLOB and Gamma exchange clients
  - `ws.rs`: WebSocket event handling
  - `signer.rs`: API request signing
- `bot-runner/src/`: Orchestration (main loop, market discovery)

**Trait-Based Design:**
- Strategy, DualSideStrategy, RiskPolicy, OrderExecutor, StateRepository are traits
- Allows testing with mock implementations (mock-exchange crate)
- All state changes go through StateRepository.transition_trade_state()

**Idempotency:**
- Every WS event checked against idempotency_keys table
- fill_id UNIQUE in fills table - duplicate inserts silently skipped via DB constraint

---

*Convention analysis: 2026-03-02*
