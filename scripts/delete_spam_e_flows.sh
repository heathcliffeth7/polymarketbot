#!/usr/bin/env bash
set -euo pipefail

EXPECTED_COUNT="${EXPECTED_COUNT:-3828}"
TARGET_DATE="${TARGET_DATE:-2026-02-25}"
HELPER_INDEX_NAME="idx_trade_flow_events_version_cleanup"
HELPER_INDEX_CREATED=0
MODE="${MODE:-exact-name}"
MATCH_NAME="${MATCH_NAME:-e}"

if ! command -v psql >/dev/null 2>&1; then
  echo "psql is required. Install postgresql-client first."
  exit 1
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "DATABASE_URL is required, e.g.:"
  echo "export DATABASE_URL=postgres://dextrabot_app:<password>@127.0.0.1:5432/dextrabot"
  exit 1
fi

usage() {
  cat <<'EOF'
Usage:
  scripts/delete_spam_e_flows.sh                        # dry-run exact-name cleanup
  scripts/delete_spam_e_flows.sh --apply                # execute exact-name cleanup
  scripts/delete_spam_e_flows.sh --incident-day         # dry-run all matching incident-day drafts
  scripts/delete_spam_e_flows.sh --incident-day --apply # execute incident-day cleanup

Environment:
  DATABASE_URL   Required Postgres connection string
  EXPECTED_COUNT Expected number of target definitions (default: 3828)
  TARGET_DATE    Spam incident date in UTC (default: 2026-02-25)
  MATCH_NAME     Exact flow name to target in exact-name mode (default: e)
EOF
}

APPLY=0
if [[ $# -gt 2 ]]; then
  usage
  exit 1
fi

for arg in "$@"; do
  case "$arg" in
    --apply)
      APPLY=1
      ;;
    --incident-day)
      MODE="incident-day"
      ;;
    *)
      usage
      exit 1
      ;;
  esac
done

if [[ "$MODE" == "incident-day" && "${EXPECTED_COUNT:-3828}" == "3828" ]]; then
  EXPECTED_COUNT=230
fi

if [[ "$MODE" != "exact-name" && "$MODE" != "incident-day" ]]; then
    usage
    exit 1
fi

cleanup_helper_index() {
  if [[ "$HELPER_INDEX_CREATED" -eq 1 ]]; then
    echo "Dropping helper index $HELPER_INDEX_NAME..."
    PGOPTIONS='-c statement_timeout=0' \
      psql -X "$DATABASE_URL" -v ON_ERROR_STOP=1 \
      -c "DROP INDEX CONCURRENTLY IF EXISTS $HELPER_INDEX_NAME;" >/dev/null
  fi
}

trap cleanup_helper_index EXIT

echo "Running spam flow precheck for mode=$MODE, match_name=$MATCH_NAME, date=$TARGET_DATE, expected_count=$EXPECTED_COUNT"
psql -X "$DATABASE_URL" -v ON_ERROR_STOP=1 \
  -v expected_count="$EXPECTED_COUNT" \
  -v target_date="$TARGET_DATE" \
  -v mode="$MODE" \
  -v match_name="$MATCH_NAME" <<'SQL'
\pset tuples_only on
\pset format unaligned

SELECT 'precheck_target_count=' || COUNT(*)
FROM trade_flow_definitions d
WHERE (:'mode' <> 'exact-name' OR LOWER(d.name) = LOWER(:'match_name'))
  AND d.status = 'draft'
  AND d.published_version_id IS NULL
  AND d.created_at::date = DATE :'target_date'
  AND NOT EXISTS (
    SELECT 1
    FROM trade_flow_runs r
    WHERE r.definition_id = d.id
  );

SELECT 'precheck_total_definitions=' || COUNT(*)
FROM trade_flow_definitions;

SELECT 'precheck_published_definitions=' || COUNT(*)
FROM trade_flow_definitions
WHERE status = 'published'
  AND published_version_id IS NOT NULL;

SELECT 'precheck_running_runs=' || COUNT(*)
FROM trade_flow_runs
WHERE status = 'running';
SQL

if [[ "$APPLY" -eq 0 ]]; then
  echo "Dry-run only. Re-run with --apply to delete the incident rows."
  exit 0
fi

helper_index_exists="$(psql -X "$DATABASE_URL" -tAc \
  "SELECT 1 FROM pg_indexes WHERE schemaname = 'public' AND indexname = '$HELPER_INDEX_NAME'" 2>/dev/null || echo "")"

if [[ "$helper_index_exists" != "1" ]]; then
  echo "Creating helper index $HELPER_INDEX_NAME for version_id FK cleanup..."
  PGOPTIONS='-c statement_timeout=0' \
    psql -X "$DATABASE_URL" -v ON_ERROR_STOP=1 \
    -c "CREATE INDEX CONCURRENTLY $HELPER_INDEX_NAME ON trade_flow_events(version_id);" >/dev/null
  HELPER_INDEX_CREATED=1
fi

echo "Applying cleanup transaction..."
psql -X "$DATABASE_URL" -v ON_ERROR_STOP=1 \
  -v expected_count="$EXPECTED_COUNT" \
  -v target_date="$TARGET_DATE" \
  -v mode="$MODE" \
  -v match_name="$MATCH_NAME" <<'SQL'
BEGIN ISOLATION LEVEL REPEATABLE READ;
SET LOCAL statement_timeout = '45s';

CREATE TEMP TABLE cleanup_target_ids ON COMMIT DROP AS
SELECT d.id
FROM trade_flow_definitions d
WHERE (:'mode' <> 'exact-name' OR LOWER(d.name) = LOWER(:'match_name'))
  AND d.status = 'draft'
  AND d.published_version_id IS NULL
  AND d.created_at::date = DATE :'target_date'
  AND NOT EXISTS (
    SELECT 1
    FROM trade_flow_runs r
    WHERE r.definition_id = d.id
  );

SELECT COUNT(*) AS target_count,
       (COUNT(*) = CAST(:'expected_count' AS BIGINT)) AS target_count_matches
FROM cleanup_target_ids
\gset

\if :target_count_matches
\echo Verified target count matches expected value: :target_count
\else
ROLLBACK;
\echo Target count mismatch. Expected :expected_count but found :target_count
\quit 1
\endif

UPDATE trade_flow_run_steps
SET parent_step_id = NULL
WHERE run_id IN (
  SELECT r.id
  FROM trade_flow_runs r
  JOIN cleanup_target_ids t ON t.id = r.definition_id
)
  AND parent_step_id IS NOT NULL;

DELETE FROM trade_flow_runs r
USING cleanup_target_ids t
WHERE r.definition_id = t.id;

DELETE FROM trade_flow_definitions d
USING cleanup_target_ids t
WHERE d.id = t.id;

COMMIT;
SQL

echo "Post-cleanup verification..."
psql -X "$DATABASE_URL" -v ON_ERROR_STOP=1 \
  -v target_date="$TARGET_DATE" \
  -v mode="$MODE" \
  -v match_name="$MATCH_NAME" <<'SQL'
\pset border 2
\pset format aligned

SELECT COUNT(*) AS remaining_target_definitions
FROM trade_flow_definitions
WHERE (:'mode' <> 'exact-name' OR LOWER(name) = LOWER(:'match_name'))
  AND status = 'draft'
  AND published_version_id IS NULL
  AND created_at::date = DATE :'target_date'
  AND NOT EXISTS (
    SELECT 1
    FROM trade_flow_runs r
    WHERE r.definition_id = trade_flow_definitions.id
  );

SELECT COUNT(*) AS total_definitions
FROM trade_flow_definitions;

SELECT COUNT(*) AS published_definitions
FROM trade_flow_definitions
WHERE status = 'published'
  AND published_version_id IS NOT NULL;

SELECT COUNT(*) AS running_runs
FROM trade_flow_runs
WHERE status = 'running';

SELECT created_at::date AS day, COUNT(*) AS total
FROM trade_flow_definitions
GROUP BY 1
ORDER BY 1;
SQL
