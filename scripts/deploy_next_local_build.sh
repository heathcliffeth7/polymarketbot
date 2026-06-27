#!/usr/bin/env bash
# Deploy a build produced with NEXT_DIST_DIR=.next-local into production .next.
# Requires sudo to replace dextrabot-owned .next and restart the service.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FRONTEND_DIR="$ROOT_DIR/frontend"
LOCAL_BUILD="$FRONTEND_DIR/.next-local"
PROD_BUILD="$FRONTEND_DIR/.next"

if [[ ! -d "$LOCAL_BUILD" ]]; then
  echo "Missing $LOCAL_BUILD — run: cd frontend && NEXT_DIST_DIR=.next-local npm run build"
  exit 1
fi

echo "[1/4] Stopping dextrabot-frontend..."
sudo systemctl stop dextrabot-frontend.service

echo "[2/4] Replacing production .next from .next-local..."
sudo rm -rf "$PROD_BUILD"
sudo mv "$LOCAL_BUILD" "$PROD_BUILD"
sudo chown -R dextrabot:dextrabot "$PROD_BUILD"
sudo find "$PROD_BUILD" -type d -exec chmod 750 {} +
sudo find "$PROD_BUILD" -type f -exec chmod 640 {} +

echo "[3/4] Starting dextrabot-frontend..."
sudo systemctl start dextrabot-frontend.service

echo "[4/4] Status:"
sudo systemctl --no-pager -l status dextrabot-frontend.service | head -15 || true

echo "Done. Hard refresh the browser (Ctrl+Shift+R) and open workflow #4329."