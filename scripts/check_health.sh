#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

ok() { echo "[OK] $*"; }
warn() { echo "[WARN] $*"; }

check_service() {
  local name="$1"
  local st
  st="$(systemctl is-active "$name" 2>/dev/null || true)"
  if [[ "$st" == "active" ]]; then
    ok "service $name is active"
  else
    warn "service $name is $st"
  fi
}

check_service postgresql
check_service redis-server
check_service dextrabot

if command -v psql >/dev/null 2>&1; then
  if [[ -n "${DATABASE_URL:-}" ]]; then
    if psql "$DATABASE_URL" -tAc 'select 1' >/dev/null 2>&1; then
      ok "database connectivity check passed"
    else
      warn "database connectivity check failed"
    fi
  else
    warn "DATABASE_URL not set; skipping DB query check"
  fi
else
  warn "psql not found; skipping DB query check"
fi

if systemctl is-active dextrabot >/dev/null 2>&1; then
  echo "----- last dextrabot journal lines -----"
  journalctl -u dextrabot -n 100 --no-pager || true
fi

if [[ -f "$ROOT_DIR/target/release/bot-runner" ]]; then
  ok "release binary exists"
else
  warn "release binary missing (run: cargo build --release -p bot-runner)"
fi
