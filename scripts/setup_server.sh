#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
ENV_DIR="/etc/dextrabot"
ENV_FILE="$ENV_DIR/dextrabot.env"
UNIT_SRC="$ROOT_DIR/deploy/systemd/dextrabot.service"
UNIT_DST="/etc/systemd/system/dextrabot.service"
ENV_EXAMPLE="$ROOT_DIR/deploy/systemd/dextrabot.env.example"
RETENTION_SERVICE_SRC="$ROOT_DIR/deploy/systemd/dextrabot-db-retention.service"
RETENTION_SERVICE_DST="/etc/systemd/system/dextrabot-db-retention.service"
RETENTION_TIMER_SRC="$ROOT_DIR/deploy/systemd/dextrabot-db-retention.timer"
RETENTION_TIMER_DST="/etc/systemd/system/dextrabot-db-retention.timer"
JOURNALD_SRC="$ROOT_DIR/deploy/systemd/journald-dextrabot.conf"
JOURNALD_DST="/etc/systemd/journald.conf.d/dextrabot.conf"
LOGROTATE_SRC="$ROOT_DIR/deploy/logrotate/rsyslog"
LOGROTATE_DST="/etc/logrotate.d/rsyslog"

step() { echo "[STEP] $*"; }
ok() { echo "[OK] $*"; }
fail() { echo "[FAIL] $*"; exit 1; }

ensure_local_config_files() {
  local name
  for name in bot strategy risk execution exchange claim telegram; do
    local file="$ROOT_DIR/config/${name}.toml"
    local example="$ROOT_DIR/config/${name}.toml.example"
    if [[ ! -f "$file" && -f "$example" ]]; then
      cp "$example" "$file"
    fi
  done
}

install_template_file() {
  local src="$1"
  local dst="$2"
  local tmp_file
  tmp_file="$(mktemp)"
  sed "s|__DEXTRABOT_ROOT__|$ROOT_DIR|g" "$src" >"$tmp_file"
  sudo cp "$tmp_file" "$dst"
  rm -f "$tmp_file"
}

install_unit_file() {
  install_template_file "$UNIT_SRC" "$UNIT_DST"
}

install_retention_units() {
  install_template_file "$RETENTION_SERVICE_SRC" "$RETENTION_SERVICE_DST"
  install_template_file "$RETENTION_TIMER_SRC" "$RETENTION_TIMER_DST"
}

install_log_retention_config() {
  sudo mkdir -p "$(dirname "$JOURNALD_DST")"
  sudo cp "$JOURNALD_SRC" "$JOURNALD_DST"
  sudo cp "$LOGROTATE_SRC" "$LOGROTATE_DST"
}

command -v sudo >/dev/null 2>&1 || fail "sudo is required"
command -v systemctl >/dev/null 2>&1 || fail "systemctl is required"
command -v cargo >/dev/null 2>&1 || fail "cargo is required"
command -v setfacl >/dev/null 2>&1 || fail "setfacl is required"

[[ -n "${DB_APP_PASSWORD:-}" ]] || fail "DB_APP_PASSWORD env var is required"

# Build DATABASE_URL if not explicitly provided.
if [[ -z "${DATABASE_URL:-}" ]]; then
  export DATABASE_URL="postgres://dextrabot_app:${DB_APP_PASSWORD}@127.0.0.1:5432/dextrabot"
fi

step "Ensure local config files"
ensure_local_config_files
ok "Local config files ready"

step "Install and start PostgreSQL + Redis"
"$ROOT_DIR/scripts/bootstrap_server_services.sh"
ok "Database services installed/started"

step "Bootstrap database and application role"
DB_APP_PASSWORD="$DB_APP_PASSWORD" "$ROOT_DIR/scripts/bootstrap_db.sh"
ok "Database bootstrap complete"

step "Apply SQL migrations"
DATABASE_URL="$DATABASE_URL" "$ROOT_DIR/scripts/apply_migrations.sh"
ok "Migrations applied"

step "Build release binary"
(
  cd "$ROOT_DIR"
  cargo build --release -p bot-runner
)
ok "Release binary built"

step "Ensure dextrabot system user"
if ! id dextrabot >/dev/null 2>&1; then
  sudo useradd --system --create-home --shell /usr/sbin/nologin dextrabot
fi
ok "System user ready"

step "Ensure dextrabot can execute release binary"
release_binary="$ROOT_DIR/target/release/bot-runner"
sudo chgrp dextrabot "$release_binary"
sudo chmod 750 "$release_binary"
sudo setfacl -m u:dextrabot:--x "$(dirname "$ROOT_DIR")"
sudo setfacl -m u:dextrabot:rx "$release_binary"
ok "Release binary ACL ready"

step "Ensure dextrabot can execute DB retention script"
sudo chmod 755 "$ROOT_DIR/scripts/prune_high_volume_db_tables.sh"
ok "DB retention script executable"

step "Install environment file"
sudo mkdir -p "$ENV_DIR"
if [[ ! -f "$ENV_FILE" ]]; then
  sudo cp "$ENV_EXAMPLE" "$ENV_FILE"
  sudo sed -i "s|^DATABASE_URL=.*|DATABASE_URL=${DATABASE_URL}|" "$ENV_FILE"
  sudo sed -i "s|^BOT_CONFIG_DIR=.*|BOT_CONFIG_DIR=${ROOT_DIR}/config|" "$ENV_FILE"
fi
if sudo grep -q "^BOT_CONFIG_DIR=__DEXTRABOT_ROOT__/config$" "$ENV_FILE"; then
  sudo sed -i "s|^BOT_CONFIG_DIR=.*|BOT_CONFIG_DIR=${ROOT_DIR}/config|" "$ENV_FILE"
fi
sudo chown root:dextrabot "$ENV_FILE"
sudo chmod 0640 "$ENV_FILE"
ok "Environment file installed at $ENV_FILE"

configured_bot_config_dir="$(sudo sed -n 's/^BOT_CONFIG_DIR=//p' "$ENV_FILE" | tail -n1)"
configured_bot_config_dir="${configured_bot_config_dir:-$ROOT_DIR/config}"

step "Ensure dextrabot can write config directory"
"$ROOT_DIR/scripts/ensure_config_permissions.sh" "$configured_bot_config_dir"
ok "Config directory permissions ready at $configured_bot_config_dir"

step "Install systemd services"
install_unit_file
install_retention_units
install_log_retention_config
sudo systemctl daemon-reload
sudo systemctl enable dextrabot
sudo systemctl enable --now dextrabot-db-retention.timer
sudo systemctl restart dextrabot
if ! sudo systemctl is-active --quiet dextrabot; then
  fail "dextrabot service failed to start after restart"
fi
ok "dextrabot service and DB retention timer installed"

step "Print service status"
sudo systemctl --no-pager -l status dextrabot || true

ok "Server setup completed"
echo "Run health check: ${ROOT_DIR}/scripts/check_health.sh"
