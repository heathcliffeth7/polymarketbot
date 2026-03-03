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
  echo "[dev:restart] Stopping next dev process(es): ${pids}"
  for pid in ${pids}; do
    kill "${pid}" 2>/dev/null || true
  done

  for _ in {1..10}; do
    remaining="$(find_next_dev_pids || true)"
    if [ -z "${remaining}" ]; then
      break
    fi
    sleep 0.5
  done

  remaining="$(find_next_dev_pids || true)"
  if [ -n "${remaining}" ]; then
    echo "[dev:restart] Force-killing remaining process(es): ${remaining}"
    for pid in ${remaining}; do
      kill -9 "${pid}" 2>/dev/null || true
    done
  fi
else
  echo "[dev:restart] No running next dev process found."
fi

if [ -f "${LOCK_FILE}" ]; then
  echo "[dev:restart] Removing lock file: ${LOCK_FILE}"
  rm -f "${LOCK_FILE}"
fi

echo "[dev:restart] Starting next dev on http://localhost:3000 ..."
cd "${ROOT_DIR}"
exec next dev --webpack
