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

for f in "$ROOT_DIR"/migrations/*.sql; do
  echo "Applying $f"
  psql -X "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$f"
done

echo "Migrations applied"
