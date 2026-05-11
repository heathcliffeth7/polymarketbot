#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

if ! command -v psql >/dev/null 2>&1; then
  echo "psql is required. Install postgresql-client first."
  exit 1
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "DATABASE_URL is required, e.g.:"
  echo "export DATABASE_URL=postgres://dextrabot_app:<password>@127.0.0.1:5432/dextrabot"
  exit 1
fi

table_existed="$(psql -X "$DATABASE_URL" -tAc \
  "SELECT 1 FROM information_schema.tables WHERE table_name = 'schema_migrations'" 2>/dev/null || echo "")"

psql -X "$DATABASE_URL" -v ON_ERROR_STOP=1 -c \
  "CREATE TABLE IF NOT EXISTS schema_migrations (
     filename TEXT PRIMARY KEY,
     applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
   );"

if [[ "$table_existed" != "1" ]]; then
  has_existing="$(psql -X "$DATABASE_URL" -tAc \
    "SELECT 1 FROM information_schema.tables WHERE table_name = 'trade_builder_orders'" 2>/dev/null || echo "")"
  if [[ "$has_existing" == "1" ]]; then
    echo "Existing database detected. Seeding schema_migrations..."
    for f in "$ROOT_DIR"/migrations/*.sql; do
      filename="$(basename "$f")"
      psql -X "$DATABASE_URL" -v ON_ERROR_STOP=1 -c \
        "INSERT INTO schema_migrations (filename) VALUES ('$filename') ON CONFLICT DO NOTHING" \
        >/dev/null 2>&1 || true
    done
    echo "Seed complete. All existing migrations marked as applied."
    exit 0
  fi
fi

applied=0
skipped=0

for f in "$ROOT_DIR"/migrations/*.sql; do
  filename="$(basename "$f")"
  already="$(psql -X "$DATABASE_URL" -tAc \
    "SELECT 1 FROM schema_migrations WHERE filename = '$filename'" 2>/dev/null || echo "")"

  if [[ "$already" == "1" ]]; then
    skipped=$((skipped + 1))
    continue
  fi

  echo "Applying $filename"
  DEXTRABOT_REPO_DIR="$ROOT_DIR" psql -X "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$f"
  psql -X "$DATABASE_URL" -v ON_ERROR_STOP=1 -c \
    "INSERT INTO schema_migrations (filename) VALUES ('$filename') ON CONFLICT DO NOTHING" >/dev/null
  applied=$((applied + 1))
done

echo "Migrations: $applied applied, $skipped skipped"
