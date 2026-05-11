#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MAX_RECONCILE_ERROR_RATE_PCT="${MAX_RECONCILE_ERROR_RATE_PCT:-1.0}"
MAX_NON_POLICY_REJECT_RATE_PCT="${MAX_NON_POLICY_REJECT_RATE_PCT:-2.0}"

ok() { echo "[OK] $*"; }
fail() { echo "[FAIL] $*"; exit 1; }
warn() { echo "[WARN] $*"; }

cd "$ROOT_DIR"

ok "running compile checks"
cargo check >/dev/null

ok "running core test suite"
cargo test -p bot-core -p bot-infra -p mock-exchange >/dev/null

if [[ ! -f "$ROOT_DIR/config/exchange.toml" ]]; then
  fail "config/exchange.toml is missing"
fi

encrypted_direct=1
for key in api_address api_key api_secret api_passphrase; do
  if ! rg -n "^${key}\\s*=\\s*\"enc:v1:" "$ROOT_DIR/config/exchange.toml" >/dev/null; then
    encrypted_direct=0
  fi
done

if [[ $encrypted_direct -eq 1 ]]; then
  ok "exchange.toml has encrypted direct credentials"
else
  legacy_env_complete=1
  for key in api_address_env api_key_env api_secret_env api_passphrase_env; do
    if ! rg -n "^${key}\\s*=\\s*\"[^\"]+\"" "$ROOT_DIR/config/exchange.toml" >/dev/null; then
      legacy_env_complete=0
    fi
  done

  if [[ $legacy_env_complete -eq 1 ]]; then
    warn "exchange.toml is using legacy env credential mapping (recommended: encrypted direct credentials)"
  else
    fail "exchange.toml credential config is incomplete (need encrypted direct credentials or complete env mapping)"
  fi
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
  fail "DATABASE_URL is required for migration gate"
fi

if ! command -v psql >/dev/null 2>&1; then
  fail "psql is required for migration gate"
fi

ok "running migration gate"
DATABASE_URL="$DATABASE_URL" "$ROOT_DIR/scripts/apply_migrations.sh" >/dev/null
ok "tracked migrations checked"

if ! psql "$DATABASE_URL" -tAc "SELECT COUNT(*) FROM reconcile_runs" >/dev/null 2>&1; then
  fail "reconcile_runs table check failed"
fi
ok "reconcile schema checks passed"

reconcile_error_rate_pct="$(psql "$DATABASE_URL" -tAc \
  "SELECT COALESCE(ROUND(100.0 * SUM(CASE WHEN status = 'error' THEN 1 ELSE 0 END) / NULLIF(COUNT(*), 0), 4), 0.0) FROM reconcile_runs" \
  | tr -d '[:space:]')"

non_policy_reject_rate_pct="$(psql "$DATABASE_URL" -tAc \
  "SELECT COALESCE(ROUND(100.0 * SUM(CASE WHEN lower(status) = 'rejected' AND COALESCE(lower(reject_reason), '') NOT LIKE '%policy%' THEN 1 ELSE 0 END) / NULLIF(COUNT(*), 0), 4), 0.0) FROM orders" \
  | tr -d '[:space:]')"

if awk "BEGIN { exit !($reconcile_error_rate_pct < $MAX_RECONCILE_ERROR_RATE_PCT) }"; then
  ok "reconcile error rate ${reconcile_error_rate_pct}% is below ${MAX_RECONCILE_ERROR_RATE_PCT}%"
else
  fail "reconcile error rate ${reconcile_error_rate_pct}% is not below ${MAX_RECONCILE_ERROR_RATE_PCT}%"
fi

if awk "BEGIN { exit !($non_policy_reject_rate_pct < $MAX_NON_POLICY_REJECT_RATE_PCT) }"; then
  ok "non-policy reject rate ${non_policy_reject_rate_pct}% is below ${MAX_NON_POLICY_REJECT_RATE_PCT}%"
else
  fail "non-policy reject rate ${non_policy_reject_rate_pct}% is not below ${MAX_NON_POLICY_REJECT_RATE_PCT}%"
fi

ok "GO/NO-GO PASSED"
