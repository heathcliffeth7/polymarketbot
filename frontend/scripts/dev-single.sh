#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK_FILE="${ROOT_DIR}/.next/dev/lock"

find_next_dev_pids() {
  ps -eo pid=,args= | awk -v root="${ROOT_DIR}" '
    $2 ~ /(^|\/)node$/ && $3 == root "/node_modules/.bin/next" && $4 == "dev" { print $1 }
  '
}

pids="$(find_next_dev_pids || true)"
if [ -n "${pids}" ]; then
  count="$(wc -w <<<"${pids}" | tr -d ' ')"
  echo "[dev] next dev already running (${count} process): ${pids}"
  echo "[dev] Use 'npm run dev:restart' if you want to restart it."
  exit 0
fi

if [ -f "${LOCK_FILE}" ]; then
  echo "[dev] Found stale lock, removing: ${LOCK_FILE}"
  rm -f "${LOCK_FILE}"
fi

echo "[dev] Starting next dev on http://localhost:3000 ..."
cd "${ROOT_DIR}"
exec next dev --webpack
