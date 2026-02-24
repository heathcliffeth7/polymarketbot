#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   DB_APP_PASSWORD='strong-pass' ./scripts/bootstrap_db.sh

if [[ -z "${DB_APP_PASSWORD:-}" ]]; then
  echo "DB_APP_PASSWORD env is required"
  exit 1
fi

sudo -u postgres psql -v ON_ERROR_STOP=1 -v app_pwd="$DB_APP_PASSWORD" <<'SQL'
DO
$do$
BEGIN
   IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'dextrabot_app') THEN
      CREATE ROLE dextrabot_app LOGIN PASSWORD :'app_pwd';
   ELSE
      ALTER ROLE dextrabot_app WITH LOGIN PASSWORD :'app_pwd';
   END IF;
END
$do$;
SQL

# Create database if needed (CREATE DATABASE cannot run in transaction block)
if ! sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname='dextrabot'" | grep -q 1; then
  sudo -u postgres psql -v ON_ERROR_STOP=1 -c "CREATE DATABASE dextrabot OWNER dextrabot_app"
fi

sudo -u postgres psql -d dextrabot -v ON_ERROR_STOP=1 <<'SQL'
GRANT ALL PRIVILEGES ON SCHEMA public TO dextrabot_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES TO dextrabot_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON SEQUENCES TO dextrabot_app;
SQL

echo "DB bootstrap complete. Set DATABASE_URL and run ./scripts/apply_migrations.sh"
