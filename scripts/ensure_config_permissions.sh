#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CONFIG_DIR="${1:-${BOT_CONFIG_DIR:-$ROOT_DIR/config}}"
SERVICE_USER="${DEXTRABOT_CONFIG_USER:-dextrabot}"

step() { echo "[STEP] $*"; }
ok() { echo "[OK] $*"; }
fail() { echo "[FAIL] $*"; exit 1; }

command -v setfacl >/dev/null 2>&1 || fail "setfacl is required"

if [[ "${EUID}" -eq 0 ]]; then
  SUDO=()
else
  command -v sudo >/dev/null 2>&1 || fail "sudo is required"
  SUDO=(sudo)
fi

step "Ensure config directory exists at $CONFIG_DIR"
"${SUDO[@]}" mkdir -p "$CONFIG_DIR"

step "Grant $SERVICE_USER write access to config directory"
"${SUDO[@]}" setfacl -Rm "u:${SERVICE_USER}:rwX" "$CONFIG_DIR"
"${SUDO[@]}" setfacl -dRm "u:${SERVICE_USER}:rwX" "$CONFIG_DIR"

ok "Config ACL updated for $SERVICE_USER at $CONFIG_DIR"
