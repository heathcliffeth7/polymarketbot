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
count=0
if [ -n "${pids}" ]; then
  count="$(wc -w <<<"${pids}" | tr -d ' ')"
fi

status_ok=true

if [ "${count}" -gt 1 ]; then
  echo "[dev:preflight] Multiple next dev processes detected for ${ROOT_DIR}: ${pids}"
  status_ok=false
elif [ "${count}" -eq 1 ]; then
  echo "[dev:preflight] next dev is running (pid: ${pids})."
else
  echo "[dev:preflight] next dev is not running."
fi

if [ -f "${LOCK_FILE}" ]; then
  if [ "${count}" -gt 0 ]; then
    echo "[dev:preflight] Lock file is present and expected: ${LOCK_FILE}"
  else
    echo "[dev:preflight] Stale lock file detected: ${LOCK_FILE}"
    echo "[dev:preflight] Run 'npm run dev' (auto-cleans stale lock) or 'npm run dev:restart'."
    status_ok=false
  fi
else
  echo "[dev:preflight] No lock file found."
fi

if [ "${status_ok}" = false ]; then
  exit 1
fi

echo "[dev:preflight] OK."
