# Dextrabot

## What This Is

Automated trading bot for Polymarket BTC/ETH/SOL 5m/15m Up/Down prediction markets. Rust backend handles market discovery, price streaming (WS+REST), entry/exit signals, order execution, and risk management. Next.js frontend provides dashboard, trade builder, and workflow orchestration. PostgreSQL stores trade state; Redis handles caching and distributed locks.

## Core Value

Deterministic trade state machine that never loses state across restarts, with correct price-driven entry/exit signals and risk guardrails that protect capital.

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

- Market discovery for BTC/ETH/SOL 5m/15m Up/Down markets
- CLOB WebSocket price streaming with REST snapshot fallback
- 11-state trade state machine with enforced transitions
- Paper + Live execution modes
- EIP-712 order signing for Polygon
- Dual-side DCA strategy (YES/NO legs)
- Risk policy with daily loss limits and kill switch
- Auto-claim for resolved positions
- Trade builder workflow engine with trigger nodes
- Frontend dashboard with trade/order/risk views
- JWT authentication for frontend
- Config encryption (AES-256-GCM)

### Active

<!-- Current scope: tick bug fixes. -->

- [ ] Tick handling correctness (milestone v1.1 — tickhatası)

### Out of Scope

- Multi-strategy plugin engine — V1 simplicity
- K8s / distributed worker topology — single instance sufficient
- External error tracking (Sentry) — systemd journal adequate for now

## Context

- Bot runs as single systemd service on Linux server
- Trade flow engine supports trigger nodes: `trigger.market_price` with cross_above/cross_below
- Tick processing has three key paths: first_tick_threshold, confirmation gate, reconcile
- Comments in code (Turkish) indicate known edge cases around first_tick behavior

## Constraints

- **Tech stack**: Rust 2021 + tokio async runtime — no changing core
- **Exchange**: Polymarket CLOB only — API contract locked
- **State machine**: 11-state model enforced by can_transition() — extend only

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust for backend | Performance + type safety for financial logic | ✓ Good |
| WS+REST reconcile | Resilience against WS gaps | ✓ Good |
| confirmation_secs gate | Filter transient spikes on auto_scope | — Pending review |
| first_tick_threshold in auto_scope | Avoid dead-on-arrival triggers when no previous_price | — Pending review |

---
*Last updated: 2026-03-03 after milestone v1.1 initialization*
