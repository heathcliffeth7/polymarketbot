#!/usr/bin/env bash
# IV sigma/blend/cap telemetri izleyici.
# trade_flow_events tablosundan son price_to_beat_iv_mismatch_edge_decision event'lerini
# ceker ve sigma_eff_source, cex_sigma, sigma_eff, expected_move, required_gap_usd,
# required_gap_usd_capped alanlarini tablo olarak gosterir.
#
# Kullanim:
#   scripts/monitor_iv_sigma.sh              # son 20 dakika, 30 satir
#   scripts/monitor_iv_sigma.sh 60 50        # son 60 dakika, 50 satir
#   scripts/monitor_iv_sigma.sh --asset sol  # yalnizca SOL (sol/eth/btc/bnb/hype)
#
# DB parolasi /etc/dextrabot/dextrabot.env icinden okunur (sudo gerekir).
set -euo pipefail

MINUTES="${IV_SIGMA_MINUTES:-20}"
LIMIT="${IV_SIGMA_LIMIT:-30}"
ASSET_FILTER=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --asset) ASSET_FILTER="${2:-}"; shift 2;;
    -h|--help)
      sed -n '2,12p' "$0"; exit 0;;
    *)
      if [[ "$1" =~ ^[0-9]+$ ]]; then
        if [[ -z "${FIRST_SET:-}" ]]; then MINUTES="$1"; FIRST_SET=1
        else LIMIT="$1"; fi
      fi
      shift;;
  esac
done

if ! command -v psql >/dev/null 2>&1; then
  echo "FAIL: psql gerekli" >&2; exit 1
fi

DATABASE_URL="$(printf '2100100\n' | sudo -S grep '^DATABASE_URL=' /etc/dextrabot/dextrabot.env 2>/dev/null | cut -d= -f2- || true)"
if [[ -z "$DATABASE_URL" ]]; then
  echo "FAIL: /etc/dextrabot/dextrabot.env icinden DATABASE_URL okunamadi (sudo gerekir)" >&2
  exit 1
fi

ASSET_CLAUSE=""
if [[ -n "$ASSET_FILTER" ]]; then
  ASSET_CLAUSE="AND payload_json::json->>'market_slug' ILIKE '%${ASSET_FILTER}%'"
fi

# Her event icin market, asset, source, cex/chainlink sigma, expected_move, required_gap,
# gap, cap durumu. sigma_eff_source null ise (early return) '-' gosterir.
psql "$DATABASE_URL" -P pager=off -c "
SELECT
  to_char(created_at AT TIME ZONE 'UTC', 'HH24:MI:SS') AS ts,
  split_part(payload_json::json->>'market_slug','-',1) AS asset,
  COALESCE(payload_json::json->'iv_mismatch_edge'->>'sigma_eff_source','-') AS src,
  payload_json::json->'iv_mismatch_edge'->>'cex_sigma' AS cex_sig,
  payload_json::json->'iv_mismatch_edge'->>'sigma_eff' AS sig_eff,
  payload_json::json->'iv_mismatch_edge'->>'sigma_15' AS sig_15,
  payload_json::json->'iv_mismatch_edge'->>'expected_move_eff' AS exp_mv,
  payload_json::json->'iv_mismatch_edge'->>'required_gap_usd' AS req_gap,
  payload_json::json->'iv_mismatch_edge'->>'gap_strength' AS gap_str,
  COALESCE(payload_json::json->'iv_mismatch_edge'->>'required_gap_usd_capped','-') AS capped,
  LEFT(payload_json::json->>'reason_code', 34) AS reason
FROM trade_flow_events
WHERE event_type='price_to_beat_iv_mismatch_edge_decision'
  AND created_at > now() - interval '${MINUTES} minutes'
  ${ASSET_CLAUSE}
ORDER BY created_at DESC
LIMIT ${LIMIT};
"
