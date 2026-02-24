#!/usr/bin/env bash
set -euo pipefail

ENV_FILE="${1:-$HOME/.dextrabot/dextrabot.env}"

match_line() {
  local pattern="$1"
  local file="$2"
  if command -v rg >/dev/null 2>&1; then
    rg -n "$pattern" "$file" >/dev/null
  else
    grep -nE "$pattern" "$file" >/dev/null
  fi
}

if [[ ! -f "$ENV_FILE" ]]; then
  echo "missing env file: $ENV_FILE"
  exit 1
fi

required=(
  DATABASE_URL
  BOT_CONFIG_DIR
  CONFIG_ENCRYPTION_KEY
)

missing=0
for key in "${required[@]}"; do
  if ! match_line "^${key}=" "$ENV_FILE"; then
    echo "missing key: $key"
    missing=1
  fi
done

if [[ $missing -eq 1 ]]; then
  exit 1
fi

legacy_keys=(
  PM_POLY_ADDRESS
  PM_API_KEY
  PM_API_SECRET
  PM_API_PASSPHRASE
)

for key in "${legacy_keys[@]}"; do
  if match_line "^${key}=" "$ENV_FILE"; then
    echo "remove legacy plaintext key from env file: $key"
    missing=1
  fi
done

if [[ $missing -eq 1 ]]; then
  exit 1
fi

config_dir="$(sed -n 's/^BOT_CONFIG_DIR=//p' "$ENV_FILE" | tail -n1)"
config_dir="${config_dir%\"}"
config_dir="${config_dir#\"}"

if [[ -z "$config_dir" ]]; then
  echo "BOT_CONFIG_DIR is empty in env file: $ENV_FILE"
  exit 1
fi

exchange_file="$config_dir/exchange.toml"
if [[ ! -f "$exchange_file" ]]; then
  echo "missing exchange config file: $exchange_file"
  exit 1
fi

plaintext_detected=0
for key in api_address api_key api_secret api_passphrase; do
  if match_line "^${key}\\s*=\\s*\"enc:v1:" "$exchange_file"; then
    continue
  fi
  if match_line "^${key}\\s*=\\s*\"[^\"]+\"" "$exchange_file"; then
    echo "warning: plaintext credential field in exchange.toml: $key"
    plaintext_detected=1
    continue
  fi
  echo "missing credential field in exchange.toml: $key"
  missing=1
done

if [[ $missing -eq 1 ]]; then
  exit 1
fi

if [[ $plaintext_detected -eq 1 ]]; then
  echo "warning: exchange.toml contains plaintext credentials (recommended: enc:v1 encryption)"
fi

echo "env + exchange credentials look complete: $ENV_FILE"
