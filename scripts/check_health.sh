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

if [[ -f "$ROOT_DIR/target/release/bot-runner" ]] && systemctl is-active dextrabot >/dev/null 2>&1; then
  main_pid="$(systemctl show -p MainPID --value dextrabot 2>/dev/null || true)"
  binary_mtime_epoch="$(stat -c '%Y' "$ROOT_DIR/target/release/bot-runner" 2>/dev/null || true)"
  if [[ -n "$main_pid" && "$main_pid" =~ ^[0-9]+$ && "$main_pid" -gt 0 && -n "$binary_mtime_epoch" ]]; then
    pid_start_raw="$(ps -o lstart= -p "$main_pid" 2>/dev/null || true)"
    if [[ -n "$pid_start_raw" ]]; then
      pid_start_epoch="$(date -d "$pid_start_raw" +%s 2>/dev/null || true)"
      if [[ -n "$pid_start_epoch" && "$pid_start_epoch" =~ ^[0-9]+$ ]]; then
        if (( pid_start_epoch < binary_mtime_epoch )); then
          warn "dextrabot process looks stale: binary is newer than running PID $main_pid (restart required)"
        else
          ok "dextrabot process start time is newer than bot-runner binary"
        fi
      else
        warn "could not parse dextrabot process start time for stale-binary check"
      fi
    else
      warn "could not read dextrabot MainPID start time for stale-binary check"
    fi
  else
    warn "could not resolve dextrabot MainPID for stale-binary check"
  fi
fi
