from __future__ import annotations

import json
import os
import urllib.parse
import urllib.request
import base64
from dataclasses import dataclass
from typing import Any

CONFIG_ENC_PREFIX = "enc:v1:"
CONFIG_ENC_NONCE_LEN = 12


@dataclass
class TelegramNotifyResult:
    sent: bool
    reason: str
    status_code: int | None = None


def decrypt_config_value(raw: str) -> str:
    trimmed = raw.strip()
    if not trimmed.startswith(CONFIG_ENC_PREFIX):
        return trimmed
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM

    encoded_key = os.getenv("CONFIG_ENCRYPTION_KEY", "").strip()
    if not encoded_key:
        raise RuntimeError("CONFIG_ENCRYPTION_KEY is required for encrypted Telegram config")
    key = base64.b64decode(encoded_key)
    decoded = base64.b64decode(trimmed[len(CONFIG_ENC_PREFIX) :])
    nonce = decoded[:CONFIG_ENC_NONCE_LEN]
    ciphertext = decoded[CONFIG_ENC_NONCE_LEN:]
    return AESGCM(key).decrypt(nonce, ciphertext, None).decode("utf-8").strip()


def load_db_telegram_credentials(database_url: str | None, user_id: int | None) -> tuple[str | None, str | None]:
    if not database_url or user_id is None:
        return None, None
    try:
        import psycopg
        from psycopg.rows import dict_row

        with psycopg.connect(database_url, row_factory=dict_row) as conn:
            with conn.cursor() as cur:
                cur.execute(
                    """
                    SELECT payload_json
                    FROM user_settings
                    WHERE user_id = %s AND config_name = 'telegram'
                    LIMIT 1
                    """,
                    (user_id,),
                )
                row = cur.fetchone()
        if not row:
            return None, None
        payload = row["payload_json"] or {}
        token = payload.get("bot_token")
        chat_id = payload.get("chat_id")
        if token:
            token = decrypt_config_value(str(token))
        return (str(token).strip() if token else None), (str(chat_id).strip() if chat_id else None)
    except Exception:
        return None, None


def telegram_credentials(
    database_url: str | None = None,
    user_id: int | None = None,
) -> tuple[str | None, str | None]:
    token = os.getenv("ML_ENTRY_QUALITY_TELEGRAM_BOT_TOKEN") or os.getenv("TELEGRAM_BOT_TOKEN")
    chat_id = os.getenv("ML_ENTRY_QUALITY_TELEGRAM_CHAT_ID") or os.getenv("TELEGRAM_CHAT_ID")
    token = token.strip() if token else None
    chat_id = chat_id.strip() if chat_id else None
    if not token or not chat_id:
        db_token, db_chat_id = load_db_telegram_credentials(database_url, user_id)
        token = token or db_token
        chat_id = chat_id or db_chat_id
    return token or None, chat_id or None


def send_telegram_message(
    text: str,
    *,
    enabled: bool,
    database_url: str | None = None,
    user_id: int | None = None,
    timeout_sec: float = 8.0,
) -> TelegramNotifyResult:
    if not enabled:
        return TelegramNotifyResult(sent=False, reason="disabled")
    token, chat_id = telegram_credentials(database_url, user_id)
    if not token or not chat_id:
        return TelegramNotifyResult(sent=False, reason="missing_credentials")

    data = urllib.parse.urlencode(
        {
            "chat_id": chat_id,
            "text": text[:3900],
            "disable_web_page_preview": "true",
        }
    ).encode("utf-8")
    request = urllib.request.Request(
        f"https://api.telegram.org/bot{token}/sendMessage",
        data=data,
        headers={"Content-Type": "application/x-www-form-urlencoded"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout_sec) as response:
            body = response.read().decode("utf-8", errors="replace")
            ok = json.loads(body).get("ok", False)
            return TelegramNotifyResult(
                sent=bool(ok),
                reason="sent" if ok else "api_not_ok",
                status_code=response.status,
            )
    except Exception as exc:
        return TelegramNotifyResult(sent=False, reason=f"send_failed:{type(exc).__name__}")


def fmt_metric(value: Any) -> str:
    if value is None:
        return "n/a"
    if isinstance(value, float):
        return f"{value:.4f}"
    return str(value)


def training_message(metrics: dict[str, Any], model_dir: str) -> str:
    entry = metrics.get("entry_quality", {}).get("test", {})
    risk = metrics.get("stop_loss_risk", {}).get("test", {})
    rows = metrics.get("rows", {})
    return "\n".join(
        [
            "ML Entry Quality V1 trained",
            f"Model dir: {model_dir}",
            f"Rows: train={rows.get('train')} val={rows.get('validation')} test={rows.get('test')}",
            f"Entry AUC: {fmt_metric(entry.get('auc'))} logloss={fmt_metric(entry.get('logloss'))}",
            f"Stop-loss AUC: {fmt_metric(risk.get('auc'))} logloss={fmt_metric(risk.get('logloss'))}",
        ]
    )


def evaluation_message(metrics: dict[str, Any], out_dir: str) -> str:
    return "\n".join(
        [
            "ML Entry Quality V1 evaluated",
            f"Report dir: {out_dir}",
            f"Rows: {metrics.get('rows')}",
            f"Entry AUC: {fmt_metric(metrics.get('entry_quality', {}).get('auc'))}",
            f"Stop-loss AUC: {fmt_metric(metrics.get('stop_loss_risk', {}).get('auc'))}",
        ]
    )


def dataset_message(meta: dict[str, Any]) -> str:
    return "\n".join(
        [
            "ML Entry Quality dataset built",
            f"Dataset: {meta.get('dataset_path')}",
            f"Rows: {meta.get('row_count')} markets={meta.get('market_count')}",
            f"Scope: {meta.get('asset')} {meta.get('timeframe')}",
        ]
    )


def recent_shadow_message(metrics: dict[str, Any]) -> str:
    decisions = metrics.get("shadow_decisions") or []
    decision_text = ", ".join(f"{row.get('ml_shadow_decision')}={row.get('len')}" for row in decisions) or "n/a"
    return "\n".join(
        [
            "ML Entry Quality recent shadow scored",
            f"Rows: {metrics.get('rows')}",
            f"Avg entry quality: {fmt_metric(metrics.get('avg_entry_quality_score'))}",
            f"Avg stop-loss risk: {fmt_metric(metrics.get('avg_stop_loss_risk'))}",
            f"Decisions: {decision_text}",
        ]
    )


def fmt_count_pair(done: Any, total: Any) -> str:
    return f"{done or 0}/{total or 0}"


def fmt_signed_usdc(value: Any) -> str:
    if value is None:
        return "n/a"
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        return "n/a"
    return f"{parsed:+.2f}"


def outcome_check_message(metrics: dict[str, Any]) -> str:
    live = metrics.get("live_shadow") or {}
    replay = metrics.get("replay_current_model") or {}
    actual = metrics.get("actual_trades") or {}
    lines = ["ML Outcome Check:"]
    if live.get("resolved", 0):
        lines.extend(
            [
                f"Resolved shadow: {live.get('resolved')}",
                f"Allow correct: {fmt_count_pair(live.get('allow_correct'), live.get('allow_total'))}",
                f"Avoid correct: {fmt_count_pair(live.get('avoid_correct'), live.get('avoid_total'))}",
                f"Caution: {live.get('caution', 0)}",
            ]
        )
    else:
        lines.append("Live shadow: no_resolved_entries_yet")
    if replay.get("resolved", 0):
        lines.append(
            "Replay current model: "
            f"{replay.get('resolved')} resolved, "
            f"allow_correct={fmt_count_pair(replay.get('allow_correct'), replay.get('allow_total'))}, "
            f"avoid_correct={fmt_count_pair(replay.get('avoid_correct'), replay.get('avoid_total'))}, "
            f"caution={replay.get('caution', 0)}"
        )
    if actual.get("trades", 0):
        lines.extend(
            [
                f"Actual trades: {actual.get('trades')}",
                f"Trade PnL model-right: {fmt_count_pair(actual.get('model_right'), actual.get('trades'))}",
            ]
        )
    optimizer = metrics.get("optimized_threshold_backtest") or {}
    if optimizer.get("actual_trades", 0):
        lines.append(
            "Threshold backtest: "
            f"current={fmt_count_pair(optimizer.get('current_model_right'), optimizer.get('actual_trades'))} "
            f"best={fmt_count_pair(optimizer.get('best_model_right'), optimizer.get('actual_trades'))} "
            f"entry>={fmt_metric(optimizer.get('best_entry_quality_min'))} "
            f"risk<={fmt_metric(optimizer.get('best_stop_loss_risk_max'))}"
        )
        if optimizer.get("low_sample"):
            lines.append("Threshold sample: low")
    shadow_pnl = metrics.get("shadow_pnl_backtest") or {}
    if shadow_pnl.get("sample_count", 0):
        lines.append(
            "5USDC first-entry PnL: "
            f"current={fmt_signed_usdc(shadow_pnl.get('current_total_pnl_5usdc'))} "
            f"best={fmt_signed_usdc(shadow_pnl.get('best_total_pnl_5usdc'))} "
            f"entry>={fmt_metric(shadow_pnl.get('best_entry_quality_min'))} "
            f"risk<={fmt_metric(shadow_pnl.get('best_stop_loss_risk_max'))} "
            f"entries={shadow_pnl.get('allow_count', 0)}/{shadow_pnl.get('sample_count', 0)} "
            f"groups={shadow_pnl.get('entry_groups', shadow_pnl.get('sample_count', 0))}"
        )
        if shadow_pnl.get("low_sample"):
            lines.append("5USDC sample: low")
    last_wrong = metrics.get("last_wrong")
    if isinstance(last_wrong, dict):
        direction = str(last_wrong.get("direction") or "").upper()
        lines.append(
            "Last wrong: "
            f"{last_wrong.get('market_slug')} {direction} "
            f"{last_wrong.get('ml_shadow_decision')} -> {last_wrong.get('reason')}"
        )
    return "\n".join(lines)


def scheduled_training_message(result: dict[str, Any]) -> str:
    gate = result.get("gate") or {}
    candidate = gate.get("candidate") or {}
    required = gate.get("required") or {}
    lines = [
        "ML Entry Quality scheduled training",
        f"Status: {result.get('status')}",
        f"Run: {result.get('run_tag')}",
        f"Rows: {candidate.get('rows')}",
        f"Entry AUC: {fmt_metric(candidate.get('entry_quality_auc'))} required={fmt_metric(required.get('entry_quality_auc'))}",
        f"Stop-loss AUC: {fmt_metric(candidate.get('stop_loss_risk_auc'))} required={fmt_metric(required.get('stop_loss_risk_auc'))}",
    ]
    if result.get("error"):
        lines.append(f"Error: {result.get('error')}")
    reasons = gate.get("reasons") or []
    if reasons:
        lines.append(f"Reasons: {', '.join(reasons)}")
    outcome_metrics = result.get("outcome_metrics")
    if isinstance(outcome_metrics, dict):
        lines.append(outcome_check_message(outcome_metrics))
    return "\n".join(lines)


def scheduled_training_systemd_failure_message(unit: str, reason: str | None = None) -> str:
    lines = [
        "ML Entry Quality scheduled training",
        "Status: systemd_failed",
        f"Unit: {unit}",
    ]
    if reason:
        lines.append(f"Reason: {reason}")
    lines.append("Check: journalctl -u dextrabot-ml-entry-quality-train.service")
    return "\n".join(lines)
