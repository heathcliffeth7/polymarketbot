#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FRONTEND_DIR="$ROOT_DIR/frontend"
LOCAL_ENV_FILE="$FRONTEND_DIR/.env.local"

ENV_DIR="/etc/dextrabot"
ENV_FILE="$ENV_DIR/dextrabot-frontend.env"
ENV_EXAMPLE="$ROOT_DIR/deploy/systemd/dextrabot-frontend.env.example"

UNIT_SRC="$ROOT_DIR/deploy/systemd/dextrabot-frontend.service"
UNIT_DST="/etc/systemd/system/dextrabot-frontend.service"
SUDOERS_FILE="/etc/sudoers.d/dextrabot-bot-systemctl"

step() { echo "[STEP] $*"; }
ok() { echo "[OK] $*"; }
fail() { echo "[FAIL] $*"; exit 1; }

install_unit_file() {
  local tmp_file
  tmp_file="$(mktemp)"
  sed "s|__DEXTRABOT_ROOT__|$ROOT_DIR|g" "$UNIT_SRC" >"$tmp_file"
  sudo cp "$tmp_file" "$UNIT_DST"
  rm -f "$tmp_file"
}

sanitize_service_name() {
  local value="$1"
  if [[ "$value" =~ ^[a-zA-Z0-9_.@-]+$ ]]; then
    printf '%s' "$value"
    return
  fi
  fail "Invalid BOT_SERVICE_NAME '$value' (allowed: [a-zA-Z0-9_.@-])"
}

read_env_value() {
  local key="$1"
  local file="$2"
  sed -n "s/^${key}=//p" "$file" | tail -n1
}

escape_sed_replacement() {
  printf '%s' "$1" | sed -e 's/[&|]/\\&/g'
}

set_or_append_env_value() {
  local key="$1"
  local value="$2"
  local escaped
  escaped="$(escape_sed_replacement "$value")"

  if sudo grep -q "^${key}=" "$ENV_FILE"; then
    sudo sed -i "s|^${key}=.*|${key}=${escaped}|" "$ENV_FILE"
  else
    printf '%s=%s\n' "$key" "$value" | sudo tee -a "$ENV_FILE" >/dev/null
  fi
}

require_non_placeholder() {
  local key="$1"
  local value
  value="$(sudo sed -n "s/^${key}=//p" "$ENV_FILE" | tail -n1)"
  if [[ -z "$value" ]]; then
    fail "Missing required env key in $ENV_FILE: $key"
  fi
  if [[ "$value" == *"CHANGE_ME"* ]]; then
    fail "Replace placeholder value for $key in $ENV_FILE before starting the service"
  fi
}

require_min_length() {
  local key="$1"
  local minimum="$2"
  local value
  value="$(sudo sed -n "s/^${key}=//p" "$ENV_FILE" | tail -n1)"
  if (( ${#value} < minimum )); then
    fail "$key in $ENV_FILE must be at least $minimum characters"
  fi
}

install_bot_systemctl_sudoers() {
  local bot_service_name="$1"
  local tmp_file
  tmp_file="$(mktemp)"
  cat >"$tmp_file" <<EOF
# Managed by scripts/setup_frontend_service.sh
dextrabot ALL=(root) NOPASSWD: /usr/bin/systemctl start ${bot_service_name}, /usr/bin/systemctl stop ${bot_service_name}, /usr/bin/systemctl restart ${bot_service_name}, /usr/bin/systemctl is-active ${bot_service_name}
EOF
  chmod 0440 "$tmp_file"
  sudo visudo -cf "$tmp_file" >/dev/null || {
    rm -f "$tmp_file"
    fail "Generated sudoers rule is invalid"
  }
  sudo install -o root -g root -m 0440 "$tmp_file" "$SUDOERS_FILE"
  rm -f "$tmp_file"
}

ensure_frontend_build_permissions() {
  local build_dir="$FRONTEND_DIR/.next"
  if [[ ! -d "$build_dir" ]]; then
    fail "Frontend build output not found at $build_dir"
  fi

  sudo chgrp -R dextrabot "$build_dir"
  sudo find "$build_dir" -type d -exec chmod 750 {} +
  sudo find "$build_dir" -type f -exec chmod 640 {} +
}

command -v sudo >/dev/null 2>&1 || fail "sudo is required"
command -v systemctl >/dev/null 2>&1 || fail "systemctl is required"
command -v visudo >/dev/null 2>&1 || fail "visudo is required"
command -v npm >/dev/null 2>&1 || fail "npm is required"
command -v node >/dev/null 2>&1 || fail "node is required"

if [[ "${SKIP_FRONTEND_BUILD:-false}" == "true" ]]; then
  step "Skipping frontend build (SKIP_FRONTEND_BUILD=true)"
else
  step "Build frontend production bundle"
  (
    cd "$FRONTEND_DIR"
    npm run build
  )
  ok "Frontend build complete"
fi

step "Ensure dextrabot system user"
if ! id dextrabot >/dev/null 2>&1; then
  sudo useradd --system --create-home --shell /usr/sbin/nologin dextrabot
fi
ok "System user ready"

step "Ensure frontend build permissions"
ensure_frontend_build_permissions
ok "Frontend build output readable by dextrabot"

step "Install frontend env file"
sudo mkdir -p "$ENV_DIR"
if [[ ! -f "$ENV_FILE" ]]; then
  sudo cp "$ENV_EXAMPLE" "$ENV_FILE"
  sudo sed -i "s|^BOT_CONFIG_DIR=.*|BOT_CONFIG_DIR=${ROOT_DIR}/config|" "$ENV_FILE"
fi
if sudo grep -q "^BOT_CONFIG_DIR=__DEXTRABOT_ROOT__/config$" "$ENV_FILE"; then
  sudo sed -i "s|^BOT_CONFIG_DIR=.*|BOT_CONFIG_DIR=${ROOT_DIR}/config|" "$ENV_FILE"
fi

if [[ -n "${DATABASE_URL:-}" ]]; then
  set_or_append_env_value "DATABASE_URL" "$DATABASE_URL"
elif [[ -f "$LOCAL_ENV_FILE" ]]; then
  local_db_url="$(read_env_value "DATABASE_URL" "$LOCAL_ENV_FILE")"
  [[ -n "$local_db_url" ]] && set_or_append_env_value "DATABASE_URL" "$local_db_url"
fi

if [[ -f "$LOCAL_ENV_FILE" ]]; then
  local_bot_config_dir="$(read_env_value "BOT_CONFIG_DIR" "$LOCAL_ENV_FILE")"
  local_auth_secret="$(read_env_value "AUTH_SECRET" "$LOCAL_ENV_FILE")"
  local_config_key="$(read_env_value "CONFIG_ENCRYPTION_KEY" "$LOCAL_ENV_FILE")"
  [[ -n "$local_bot_config_dir" ]] && set_or_append_env_value "BOT_CONFIG_DIR" "$local_bot_config_dir"
  [[ -n "$local_auth_secret" ]] && set_or_append_env_value "AUTH_SECRET" "$local_auth_secret"
  [[ -n "$local_config_key" ]] && set_or_append_env_value "CONFIG_ENCRYPTION_KEY" "$local_config_key"
fi

if [[ -n "${BOT_CONFIG_DIR:-}" ]]; then
  set_or_append_env_value "BOT_CONFIG_DIR" "$BOT_CONFIG_DIR"
fi
if ! sudo grep -q "^BOT_CONFIG_DIR=" "$ENV_FILE"; then
  set_or_append_env_value "BOT_CONFIG_DIR" "$ROOT_DIR/config"
fi

configured_bot_config_dir="$(sudo sed -n 's/^BOT_CONFIG_DIR=//p' "$ENV_FILE" | tail -n1)"
configured_bot_config_dir="${configured_bot_config_dir:-$ROOT_DIR/config}"

step "Ensure dextrabot can write config directory"
"$ROOT_DIR/scripts/ensure_config_permissions.sh" "$configured_bot_config_dir"
ok "Config directory permissions ready at $configured_bot_config_dir"

if [[ -n "${SYSTEMD_CONTROL_ENABLED:-}" ]]; then
  set_or_append_env_value "SYSTEMD_CONTROL_ENABLED" "$SYSTEMD_CONTROL_ENABLED"
elif ! sudo grep -q "^SYSTEMD_CONTROL_ENABLED=" "$ENV_FILE"; then
  set_or_append_env_value "SYSTEMD_CONTROL_ENABLED" "true"
fi

if [[ -n "${BOT_SERVICE_NAME:-}" ]]; then
  sanitized_bot_service_name="$(sanitize_service_name "$BOT_SERVICE_NAME")"
  set_or_append_env_value "BOT_SERVICE_NAME" "$sanitized_bot_service_name"
elif ! sudo grep -q "^BOT_SERVICE_NAME=" "$ENV_FILE"; then
  set_or_append_env_value "BOT_SERVICE_NAME" "dextrabot"
fi

if [[ -n "${AUTH_COOKIE_SECURE:-}" ]]; then
  set_or_append_env_value "AUTH_COOKIE_SECURE" "$AUTH_COOKIE_SECURE"
elif ! sudo grep -q "^AUTH_COOKIE_SECURE=" "$ENV_FILE"; then
  set_or_append_env_value "AUTH_COOKIE_SECURE" "false"
fi

set_or_append_env_value "FRONTEND_NODE_BIN" "${FRONTEND_NODE_BIN:-$(command -v node)}"

sudo chown root:dextrabot "$ENV_FILE"
sudo chmod 0640 "$ENV_FILE"
ok "Frontend env installed at $ENV_FILE"

step "Install sudoers rule for frontend bot control"
configured_service_name="$(sudo sed -n 's/^BOT_SERVICE_NAME=//p' "$ENV_FILE" | tail -n1)"
configured_service_name="${configured_service_name:-dextrabot}"
configured_service_name="$(sanitize_service_name "$configured_service_name")"
install_bot_systemctl_sudoers "$configured_service_name"
ok "sudoers installed at $SUDOERS_FILE for service '$configured_service_name'"

step "Validate required env values"
require_non_placeholder "DATABASE_URL"
require_non_placeholder "AUTH_SECRET"
require_non_placeholder "CONFIG_ENCRYPTION_KEY"
require_min_length "AUTH_SECRET" 32
ok "Required env values look valid"

if pgrep -f "$FRONTEND_DIR/node_modules/.bin/next dev --webpack" >/dev/null 2>&1; then
  fail "Port 3000 is occupied by next dev. Stop dev mode (or disable user service polymarket-frontend.service) before starting dextrabot-frontend."
fi

step "Install and restart frontend systemd service"
install_unit_file
sudo systemctl daemon-reload
sudo systemctl enable dextrabot-frontend
if sudo systemctl is-active --quiet dextrabot-frontend; then
  sudo systemctl restart dextrabot-frontend
  ok "dextrabot-frontend enabled and restarted"
else
  sudo systemctl start dextrabot-frontend
  ok "dextrabot-frontend enabled and started"
fi

step "Print service status"
sudo systemctl --no-pager -l status dextrabot-frontend || true

ok "Frontend service setup completed"
echo "Open: http://<SERVER_IP>:3000"
