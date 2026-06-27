#!/usr/bin/env bash
# Rebuild production frontend and restart dextrabot-frontend.service.
# Requires sudo (same as setup_frontend_service.sh).
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
exec bash "$ROOT_DIR/scripts/setup_frontend_service.sh"