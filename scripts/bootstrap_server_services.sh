#!/usr/bin/env bash
set -euo pipefail

# This script must be run with sudo privileges on Ubuntu server.

sudo apt-get update
sudo apt-get install -y postgresql postgresql-client redis-server

sudo systemctl enable --now postgresql
sudo systemctl enable --now redis-server

echo "postgresql status: $(systemctl is-active postgresql || true)"
echo "redis status: $(systemctl is-active redis-server || true)"

echo "Installed versions:"
psql --version || true
redis-server --version || true

echo "Done. Next: run scripts/bootstrap_db.sh"
