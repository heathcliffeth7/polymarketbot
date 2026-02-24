#!/usr/bin/env bash
set -euo pipefail

SRC="${1:-$HOME/.dextrabot/dextrabot.env}"
DST_DIR="/etc/dextrabot"
DST="$DST_DIR/dextrabot.env"

if [[ ! -f "$SRC" ]]; then
  echo "source env file not found: $SRC"
  exit 1
fi

sudo mkdir -p "$DST_DIR"
sudo cp "$SRC" "$DST"
sudo chown root:root "$DST"
sudo chmod 600 "$DST"

echo "installed: $DST"
echo "next: sudo systemctl daemon-reload && sudo systemctl restart dextrabot && sudo systemctl status dextrabot --no-pager -l"
