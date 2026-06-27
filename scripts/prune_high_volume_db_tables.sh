#!/usr/bin/env bash
set -euo pipefail

RETENTION_DAYS="${RETENTION_DAYS:-3}"
BATCH_SIZE="${BATCH_SIZE:-50000}"
DELETE_SLEEP_SEC="${DELETE_SLEEP_SEC:-0}"
APPLY=0
VACUUM_ANALYZE=0

usage() {
  cat <<'EOF'
Usage:
  scripts/prune_high_volume_db_tables.sh              # dry-run
  scripts/prune_high_volume_db_tables.sh --apply      # delete rows older than retention
  scripts/prune_high_volume_db_tables.sh --apply --vacuum

Environment:
  DATABASE_URL       Required Postgres connection string
  RETENTION_DAYS     Days to keep, default: 3
  BATCH_SIZE         Rows to delete per batch, default: 50000
  DELETE_SLEEP_SEC   Optional sleep between batches, default: 0
EOF
}

for arg in "$@"; do
  case "$arg" in
    --apply)
      APPLY=1
      ;;
    --vacuum)
      VACUUM_ANALYZE=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 1
      ;;
  esac
done

if ! command -v psql >/dev/null 2>&1; then
  echo "psql is required. Install postgresql-client first." >&2
  exit 1
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "DATABASE_URL is required, e.g.:" >&2
  echo "export DATABASE_URL=postgres://dextrabot_app:<password>@127.0.0.1:5432/dextrabot" >&2
  exit 1
fi

if ! [[ "$RETENTION_DAYS" =~ ^[0-9]+$ ]] || [[ "$RETENTION_DAYS" -lt 1 ]]; then
  echo "RETENTION_DAYS must be a positive integer." >&2
  exit 1
fi

if ! [[ "$BATCH_SIZE" =~ ^[0-9]+$ ]] || [[ "$BATCH_SIZE" -lt 1 ]]; then
  echo "BATCH_SIZE must be a positive integer." >&2
  exit 1
fi

psql_scalar() {
  psql -X "$DATABASE_URL" -v ON_ERROR_STOP=1 -tAc "$1" | tr -d '[:space:]'
}

cutoff_sql="NOW() - make_interval(days => $RETENTION_DAYS)"
cutoff="$(psql -X "$DATABASE_URL" -v ON_ERROR_STOP=1 -tAc "SELECT $cutoff_sql;" | xargs)"

echo "DB retention cutoff: $cutoff (RETENTION_DAYS=$RETENTION_DAYS, BATCH_SIZE=$BATCH_SIZE, apply=$APPLY)"

prune_table() {
  local table="$1"
  local column="$2"
  local pending
  local deleted
  local total_deleted=0

  pending="$(psql_scalar "SELECT COUNT(*) FROM $table WHERE $column < $cutoff_sql;")"
  echo "table=$table column=$column pending_delete=$pending"

  if [[ "$APPLY" -eq 0 ]]; then
    echo "table=$table dry-run only"
    return 0
  fi

  while true; do
    deleted="$(psql_scalar "
      WITH candidates AS (
        SELECT id
        FROM $table
        WHERE $column < $cutoff_sql
        ORDER BY $column, id
        LIMIT $BATCH_SIZE
      ),
      deleted AS (
        DELETE FROM $table target
        USING candidates
        WHERE target.id = candidates.id
        RETURNING 1
      )
      SELECT COUNT(*) FROM deleted;
    ")"
    total_deleted=$((total_deleted + deleted))
    echo "table=$table batch_deleted=$deleted total_deleted=$total_deleted"

    if [[ "$deleted" -eq 0 ]]; then
      break
    fi

    if [[ "$DELETE_SLEEP_SEC" != "0" ]]; then
      sleep "$DELETE_SLEEP_SEC"
    fi
  done

  if [[ "$VACUUM_ANALYZE" -eq 1 && "$total_deleted" -gt 0 ]]; then
    echo "table=$table vacuum_analyze=starting"
    psql -X "$DATABASE_URL" -v ON_ERROR_STOP=1 -c "VACUUM (ANALYZE) $table;"
    echo "table=$table vacuum_analyze=done"
  fi
}

prune_table "trade_flow_run_steps" "created_at"
prune_table "trade_flow_events" "created_at"
prune_table "market_trade_ticks" "event_ts"
prune_table "market_price_second_snapshots" "second_ts"

echo "DB retention completed."
