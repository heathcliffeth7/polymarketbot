# Codebase Concerns

**Analysis Date:** 2026-03-02

## Tech Debt

### Monolithic Main.rs (10,755 lines)

**Issue:** `crates/bot-runner/src/main.rs` contains the entire runtime orchestration, trade flow engine, dual-leg DCA logic, and expression evaluation in a single file.

**Files:** `crates/bot-runner/src/main.rs` (lines 1-10755)

**Impact:**
- Extremely difficult to navigate and maintain
- High risk of unintended side effects when modifying functions
- Impossible to unit test individual components in isolation
- Code reuse across modules is limited
- New developers face steep learning curve

**Fix approach:** Extract into modules:
1. `flow_engine.rs` - Flow definition runtime, node execution, graph traversal
2. `expression_eval.rs` - JSONLogic evaluation, context resolution
3. `trade_flow_loop.rs` - Trade flow processing loop
4. `dual_dca_runner.rs` - Dual-leg DCA execution (currently in `dca.rs` but needs refactoring)
5. `market_discovery.rs` - Market discovery logic, scope resolution
6. Keep `main.rs` for orchestration only

---

### Unsafe `.unwrap()` / `.expect()` Calls in Critical Paths (234 occurrences)

**Issue:** Production code contains 234 unwrap/expect calls that can panic at runtime. Critical issues at:

**Files with high-risk unwraps:**
- `crates/bot-runner/src/main.rs:1000` - `.panic!()` on insufficient balance (startup check)
- `crates/bot-runner/src/main.rs:3742-3753` - `.unwrap()` on `StdMutex::lock()` in market cache (can panic if lock poisoned)
- `crates/bot-runner/src/main.rs:4565` - `.expect()` on JSON object normalization
- `crates/bot-runner/src/main.rs:7527, 7740` - Multiple `.expect()` in expression evaluation
- `crates/bot-runner/src/main.rs:8619, 8636, 8665` - `.expect()` on graph seed node selection (test code leaking to production)
- `crates/bot-infra/src/exchange.rs:26` - `.expect()` on HTTP client build
- `crates/bot-infra/src/exchange.rs:506` - `.unwrap()` on UUID byte slice conversion
- `crates/bot-infra/src/signer.rs:214, 239` - Multiple unwraps in signing logic

**Impact:**
- Unhandled panic crashes entire bot process without graceful shutdown
- No signal for monitoring/alerting before crash
- Mutex lock poisoning causes cascading failures
- JSON parsing failures terminate unexpectedly
- Loss of in-flight orders or state corruption

**Fix approach:**
1. Replace all `.unwrap()` with `.context()/?.` and return `Result<T>`
2. Add explicit lock poisoning recovery: `.unwrap_or_else(|e| e.into_inner())`
3. Create `enum CriticalError` for shutdown scenarios that require panic
4. Add telemetry/logging before any Result propagation
5. Test panic recovery with chaos engineering (kill -9 simulation)

---

### Polling-Based State Synchronization (Reconciliation Lag)

**Issue:** Trade flow engine and dual-DCA loop use polling with fixed intervals (`FLOW_DEFINITION_PROCESS_LIMIT`, `FLOW_DUAL_DCA_JOB_PROCESS_LIMIT`) rather than event-driven processing.

**Files:**
- `crates/bot-runner/src/main.rs` (trade flow loop, ~lines 2180-2253)
- `crates/bot-runner/src/dca.rs` (dual-DCA job loop, lines 39-87)
- `crates/bot-infra/src/db.rs` (query batching, no change notification)

**Impact:**
- Up to `loop_interval_ms` (default 500-1000ms) latency before detecting state changes
- Potential race conditions between market cycles and state updates
- Inefficient database queries on every cycle (100+ rows per poll)
- WebSocket fill events processed immediately, but flow steps processed on next poll

**Fix approach:**
1. Add `NOTIFY`/`LISTEN` PostgreSQL pubsub for trade state changes
2. Implement event channel to trigger flow step processing immediately
3. Keep polling as fallback for resilience
4. Add metrics: `flow_step_delay_ms`, `dca_job_latency_ms`

---

### Global Static Cache with StdMutex (Lock Poisoning Risk)

**Issue:** Market discovery caches market list in a global `LazyLock<StdMutex<HashMap>>` with TTL-based expiration.

**Files:** `crates/bot-runner/src/main.rs` (lines 58-59, 3740-3757)

**Code:**
```rust
static AUTO_SCOPE_MARKET_CACHE: LazyLock<StdMutex<HashMap<String, (Instant, Vec<GammaMarket>)>>>
  = LazyLock::new(|| StdMutex::new(HashMap::new()));

// Usage:
let cache_hit = AUTO_SCOPE_MARKET_CACHE
    .lock()
    .unwrap()  // ← PANIC if lock poisoned
    .get(&market_scope)
    .filter(|(t, _)| t.elapsed() < std::time::Duration::from_secs(AUTO_SCOPE_CACHE_TTL_SECS))
    .map(|(_, m)| m.clone());
```

**Impact:**
- If any thread panics while holding lock, all subsequent cache accesses panic
- No TTL cleanup - map grows unbounded if many scopes accessed
- Shared mutable state across async runtime (not Send/Sync guaranteed)
- No cache invalidation strategy for stale market data

**Fix approach:**
1. Replace `StdMutex` with `tokio::sync::Mutex` (async-aware)
2. Add explicit lock poisoning recovery
3. Implement background cache eviction task
4. Add `cache_size`, `cache_age_secs` metrics
5. Consider using `Arc<DashMap>` for lock-free access (no TTL though)

---

## Known Bugs

### Market Discovery State Machine Incomplete

**Issue:** Market discovery has states (`Init`, `Cooldown`, `Fetching`) but transition logic is incomplete. Can get stuck in `Fetching` state if Gamma API timeout occurs.

**Files:** `crates/bot-runner/src/main.rs` (lines 1431-1613)

**Trigger:**
1. Start bot
2. Gamma API endpoint unavailable (network issue)
3. `discover_live_market()` times out
4. State never transitions back to `Init` or `Cooldown`
5. Bot continues polling indefinitely without recovery

**Workaround:** Manual restart required. Monitor `market_discovery_timeout_sec` env var (default 30s).

**Fix approach:**
1. Add explicit timeout handler: `match tokio::time::timeout(...).await`
2. Always transition to `Cooldown` on error
3. Add max consecutive discovery failures before halt
4. Implement exponential backoff for retries

---

### JSON Normalization Can Lose Data

**Issue:** Trade flow context normalization has two different normalization paths that may produce different results.

**Files:** `crates/bot-runner/src/main.rs` (lines 4540-4557, 7720-7732)

**Code:** Two conflicting normalizations:
```rust
// Path 1: Flow context normalization (lines 4540-4557)
// Iterates over keys and creates empty objects

// Path 2: Expression evaluation context (lines 7720-7732)
// Flattens from flowContext, state, vars, refs, nodeState
```

**Impact:**
- Expressions may evaluate incorrectly if context keys accessed during flatten
- Fields present in one normalization absent in other
- Expressions using nested paths fail silently (return null instead of error)

**Fix approach:**
1. Single source of truth: `fn normalize_flow_context(context: &Value) -> Value`
2. Document nesting rules explicitly
3. Add unit tests for all access patterns
4. Return `Err` instead of `null` for missing fields

---

### Expression Evaluation Division by Zero

**Issue:** JSONLogic division operator doesn't validate denominator.

**Files:** `crates/bot-runner/src/main.rs` (lines 7800-7806)

**Code:**
```rust
"/" => {
    if numeric_values.len() < 2 || numeric_values[1] == 0.0 {
        return Value::Null;  // ← Returns null, not error
    }
    // ...
}
```

**Impact:**
- Division by zero silently returns `null`
- Expression continues with null values
- Hard to debug why condition failed
- No audit trail of mathematical errors

**Fix approach:**
1. Return `Err(ExpressionError::DivisionByZero)` instead of null
2. Add error propagation to node execution
3. Log and record failures in `trade_flow_run_steps`
4. Halt flow with clear reason

---

## Security Considerations

### Config Encryption Key Exposed in Startup Logs

**Issue:** `CONFIG_ENCRYPTION_KEY` must be base64-decoded at startup. If decoding fails, error message may be logged with partial key material.

**Files:** `crates/bot-runner/src/main.rs` (lines 1012-1027)

**Code:**
```rust
fn load_config_encryption_key() -> Result<[u8; 32]> {
    let encoded = env::var("CONFIG_ENCRYPTION_KEY")?;
    let decoded = BASE64_STANDARD
        .decode(encoded.trim().as_bytes())
        .context("CONFIG_ENCRYPTION_KEY must be valid base64")?;  // ← May log encoded value
    // ...
}
```

**Impact:**
- Base64 key visible in error logs
- Logs may be shipped to centralized system
- Encrypted credentials become decryptable if key recovered

**Current mitigation:** None observed. Key is printed if `env::var` fails.

**Recommendations:**
1. Never log `encoded` value directly
2. Use `anyhow::anyhow!("failed to decode encryption key")` (generic message)
3. Add `redact_secrets_from_logs` middleware
4. Store `CONFIG_ENCRYPTION_KEY` in process environment only, never files
5. Rotate key regularly (implement versioning in `enc:v1:` prefix)

---

### API Credentials Passed as Plaintext in Requests

**Issue:** CLOB API credentials (private key, passphrase, API key/secret) are stored in config and passed to signing functions. If network proxy or TLS intercept occurs, credentials visible.

**Files:**
- `crates/bot-infra/src/exchange.rs` (SOCKS5_PROXY_URL support, line 16-26)
- `crates/bot-infra/src/signer.rs` (EIP-712 signing, credentials in headers)

**Impact:**
- SOCKS5 proxy may cache/log headers
- Plaintext credentials in memory (but salted/hashed by wallet signer)
- No credential rotation mechanism

**Current mitigation:** Headers signed with HMAC-SHA256, orders signed with EIP-712.

**Recommendations:**
1. Implement credential key rotation (separate env vars per rotation period)
2. Add `--audit-mode` flag that redacts sensitive headers from logs
3. Use `zeroize` crate to clear sensitive data from memory
4. Validate TLS certificate pinning for exchange endpoints
5. Add rate limiting on failed auth attempts (guard against credential scanning)

---

## Performance Bottlenecks

### Database Connection Pool Too Small

**Issue:** PostgreSQL connection pool configured with `max_connections(5)`, which is the default.

**Files:** `crates/bot-infra/src/db.rs` (line 267)

**Code:**
```rust
let pool = PgPoolOptions::new()
    .max_connections(5)  // ← Hardcoded
    .connect(database_url)
    .await?;
```

**Impact:**
- Concurrent flow step processing waits for available connections
- Long-running queries (market discovery fetches) block other operations
- Under load, connection acquisition timeout likely
- No monitoring of pool exhaustion

**Capacity math:**
- Trade flow loop: 1 connection per poll cycle (~100 rows fetched)
- Dual-DCA loop: 1 connection per poll cycle
- Manual order processing: 1 per order update
- Risk events: 1 per evaluation
- **Estimated peak: 5+ concurrent connections** → pool saturated

**Fix approach:**
1. Increase to `max_connections(20)` minimum
2. Make configurable via `DATABASE_POOL_SIZE` env var
3. Add pool metrics: `pool_available_connections`, `pool_wait_duration_ms`
4. Add connection acquisition timeout (currently no timeout)
5. Profile long-running queries and add indexes

---

### JSONLogic Evaluation in Hot Loop

**Issue:** Expression evaluation for every flow step uses recursive traversal without memoization. Complex expressions re-evaluated on every node execution.

**Files:** `crates/bot-runner/src/main.rs` (lines 7735-7850+)

**Code:**
```rust
fn evaluate_jsonlogic(expression: &Value, data: &Value) -> Value {
    // ... recursive evaluation, clones all Values, no caching
}
```

**Impact:**
- Each edge condition evaluation walks entire expression tree
- Multiple clones of `data` context on every level
- No compilation to bytecode
- Expressions with 10+ operators * 100 steps = 1000+ evaluations per cycle

**Example:** If flow has 50 steps and each has 2 conditions (edge guards):
- 100 expression evaluations per cycle
- At 1000ms cycle, that's 100 evals/sec or 0.1 evaluations per step

**Fix approach:**
1. Pre-compile expressions to bytecode on flow publish
2. Implement expression cache: `HashMap<expression_hash, compiled_bytecode>`
3. Profile hot paths with flamegraph
4. Consider `jsonlogic-rules` crate (but requires JSON schema)

---

### WebSocket Reconnection Blocks Market Discovery

**Issue:** If WebSocket disconnects during trade flow execution, reconnection logic blocks market discovery loop waiting for recovery.

**Files:** `crates/bot-runner/src/main.rs` (trade flow loop polling, 2180-2253)

**Impact:**
- Single WebSocket issue halts all market discovery for that scope
- New trade flows can't start until WS recovers
- Timeout not enforced - can hang indefinitely

**Fix approach:**
1. Separate WS recovery loop from market discovery
2. Add `max_reconnection_attempts` before fallback to REST
3. Use circuit breaker pattern: fail-open for market discovery
4. Add WS health check independent of trade flow loop

---

## Fragile Areas

### Trade Flow Graph Execution Engine

**Files:** `crates/bot-runner/src/main.rs` (lines 2180-2253 loop, node execution scattered)

**Why fragile:**
1. **Graph Traversal:** No explicit graph validation before execution. Invalid node references crash at runtime.
2. **State Mutation:** Flow step state (`status`, `output_json`, `error_text`) mutated via raw SQL `UPDATE` statements without transaction isolation.
3. **Async Boundaries:** Each node execution awaits independently. If node times out, parent step hangs indefinitely.
4. **Error Recovery:** Failed node transitions to `status = 'error'` but parent flow continues (soft failure). May mask cascading failures.

**Safe modification approach:**
1. Validate graph topology on publish (DAG check, no orphaned nodes)
2. Add explicit node timeout: `tokio::time::timeout(60s, execute_node(...)).await`
3. Use `PostgreSQL` savepoints for node execution (rollback if panic)
4. Implement `BackoffStrategy` for node retries

**Test coverage gaps:**
- No test for graph with cycles (infinite loop potential)
- No test for missing node dependencies (dangling references)
- No test for async timeout during node execution
- No test for concurrent flow runs on same node key

---

### Dual-Leg DCA Job Loop

**Files:** `crates/bot-runner/src/dca.rs` (lines 39-87, 95-500+)

**Why fragile:**
1. **Job Status Transitions:** Job status (`'pending'`, `'running'`, `'completed'`, `'error'`) not atomic with order creation.
2. **Price Fetching:** Relies on WebSocket for current prices. If WS stale, DCA order placed at stale price.
3. **Fill Reconciliation:** Order fills sync'd via `sync_recent_trade_builder_fills()` but timing unclear. May miss fills or double-count.
4. **Resource Exhaustion:** No limit on number of concurrent `FLOW_DUAL_DCA_JOB_PROCESS_LIMIT` (100) jobs. Each creates order, query, potential panic.

**Safe modification approach:**
1. Make job status atomic with order creation: `BEGIN; INSERT job SET status='running'; INSERT order; COMMIT;`
2. Add price staleness check: only use WS price if `last_update < 2 seconds ago`
3. Add explicit fill deduplication: check `fill_id` exists before insert
4. Add circuit breaker: if 10 consecutive jobs fail, pause job processing

**Test coverage gaps:**
- No test for price staleness recovery
- No test for order creation failure mid-DCA
- No test for duplicate fill handling
- No test for job timeout (stuck in 'running' state)

---

### Manual Order Processing

**Files:** `crates/bot-runner/src/main.rs` (manual order processing loop, scattered)

**Why fragile:**
1. **Order Status Polling:** Orders checked every cycle but no backoff. Stale orders in `'pending'` status polled forever.
2. **Price Update Race:** Order price updated, but WS fill event arrives before DB update completes.
3. **Trigger Fire Limit:** Orders have `max_triggers` limit but counter not enforced atomically.

**Test coverage gaps:**
- No test for order stuck in 'pending' status
- No test for concurrent fill and price update
- No test for trigger counter overflow

---

## Scaling Limits

### Single Database Connection for All Concurrent Operations

**Current capacity:** PostgreSQL with `max_connections=5`, single replica.

**Peak load scenario:**
- 100 flows processing simultaneously (each polls DB)
- 20 manual orders being managed
- 10 DCA jobs in progress
- **Estimated connections needed: 130+**

**Limit:** Database refuses new connections at pool exhaustion.

**Scaling path:**
1. Add read replicas for queries (use `postgres_replica_url` env var)
2. Implement connection pooling at application level (PgBouncer)
3. Implement query result caching (Redis)
4. Shard by flow_definition_id for horizontal scaling

---

### Single Bot Runner Process

**Current:** Single `bot-runner` instance with advisory lock (`BOT_RUNNER_DB_LOCK_KEY`).

**Limit:** Cannot scale beyond single server. If runner crashes, all flows halt.

**Scaling path:**
1. Implement distributed scheduler (separate process)
2. Multiple runners with queue-based job assignment
3. K8s StatefulSet with ordered termination

---

### Memory Growth from Market Cache

**Issue:** `AUTO_SCOPE_MARKET_CACHE` has no size limit and no TTL cleanup.

**Impact:** After running for days, cache may grow to 100s of MB if many market scopes accessed.

**Scaling path:**
1. Implement LRU eviction (max 1000 entries)
2. Add background cleanup task (every 5 minutes)
3. Monitor with `cache_size_bytes` metric

---

## Dependencies at Risk

### serde_json Parsing Without Bounds

**Issue:** Trade flow context and graph loaded from DB as JSON without size limits. Large payloads could cause OOM.

**Files:** Trade flow queries load `graph_json`, `context_json` without validation.

**Risk:** DOS attack if user creates flow with 1GB JSON blob.

**Migration plan:**
1. Add `MAX_FLOW_GRAPH_SIZE_BYTES = 1_000_000` (1MB)
2. Validate on flow publish: reject if `graph_json.len() > limit`
3. Add metric: `flow_graph_size_bytes` histogram

---

### Outdated CLOB API Endpoint References

**Issue:** Code has hardcoded Gamma API base URL. If Polymarket changes endpoint, code breaks.

**Files:** `crates/bot-infra/src/exchange.rs` (endpoint construction)

**Risk:** Missing API version negotiation.

**Migration plan:**
1. Add API version to config (default to `v1`)
2. Implement health check for endpoint availability
3. Add fallback endpoint list

---

## Missing Critical Features

### No Observability (Monitoring/Alerting)

**Problem:** Bot has logging but no:
- Structured metrics (Prometheus format)
- Tracing (OpenTelemetry)
- Custom dashboards
- Alert thresholds

**Blocks:** Production deployment. Can't detect performance degradation or anomalies.

**Implementation:** Add dependency on `prometheus` crate, expose `/metrics` endpoint.

---

### No Configuration Validation at Startup

**Problem:** Invalid config file silently uses defaults. No schema validation.

**Example:** `max_dca_levels_per_leg: -5` is accepted and parsed as `4294967291`.

**Blocks:** Risk management. Invalid risk parameters not caught until runtime.

**Implementation:** Add `jsonschema` validation on `AppConfig` load.

---

### No Gradual Rollout / Feature Flags

**Problem:** All new features enabled/disabled via config file. No gradual rollout.

**Example:** Dual-DCA is all-or-nothing. Can't run 5% of flows as dual-DCA for testing.

**Blocks:** Safe feature testing in production.

**Implementation:** Add `feature_flags` config section with percentage rollouts.

---

## Test Coverage Gaps

### Trade State Machine Transitions Not Fully Tested

**What's not tested:**
- Invalid state transitions (should return error)
- Transitions during concurrent order updates
- Recovery after restart with partial order fills
- Race condition: market closes before exit filled

**Files:** Tests scattered, no dedicated state machine test suite.

**Risk:** High. State corruption would break entire trade.

**Priority:** High

---

### Expression Evaluation Missing Edge Cases

**What's not tested:**
- Deeply nested expressions (> 10 levels)
- Type mismatches (comparing string to number)
- Missing variables in data context
- Expressions with 100+ operators

**Files:** No dedicated expression test file. Expressions tested only in integration tests.

**Risk:** Medium. Silent failures or wrong decisions.

**Priority:** Medium

---

### WebSocket Reconnection Not Tested

**What's not tested:**
- Connection drops mid-order-fill
- Reconnection while market discovery active
- Multiple rapid disconnects
- Permanent connection loss (shutdown)

**Risk:** High. Order fills could be missed or duplicated.

**Priority:** High

---

### Database Lock/Deadlock Scenarios

**What's not tested:**
- Two runners trying to acquire same advisory lock (should fail gracefully)
- Transaction conflicts during concurrent order updates
- Connection pool exhaustion

**Files:** Database tests use SQLite mock, not real PostgreSQL.

**Risk:** High. Production data corruption.

**Priority:** Critical

---

## Recommendations by Priority

### Critical (Do First)
1. Replace all `.unwrap()` in critical paths with proper error handling
2. Fix lock poisoning in market cache (add recovery)
3. Test database lock scenarios with real PostgreSQL
4. Add config validation schema at startup

### High (Do This Sprint)
1. Extract main.rs into modules (flow_engine, expression_eval, market_discovery)
2. Add expression evaluation edge case tests
3. Add WebSocket reconnection tests
4. Implement lock poisoning recovery for global cache

### Medium (Do Next Sprint)
1. Replace StdMutex with tokio::sync::Mutex
2. Add NOTIFY/LISTEN for event-driven flow processing
3. Increase database pool size to 20 and make configurable
4. Add JSONLogic compilation/caching

### Low (Backlog)
1. Implement LRU cache eviction for market cache
2. Add configuration validation on publish
3. Add feature flags for gradual rollout
4. Implement distributed scheduler for horizontal scaling

---

*Concerns audit: 2026-03-02*
