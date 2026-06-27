#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import warnings
from pathlib import Path
from typing import Any

import joblib
import polars as pl
import psycopg
from psycopg.rows import dict_row

from common import (
    DEFAULT_MODEL_DIR,
    REPO_ROOT,
    SHADOW_DECISION_POLICY_VERSION,
    coerce_feature_frame,
    compute_labels,
    ensure_dir,
    extract_payload_features,
    load_json,
    normalize_direction,
    now_tag,
    predict_positive,
    score_to_shadow_decision,
    write_json,
)
from telegram_notify import outcome_check_message, send_telegram_message

SHADOW_ENTRY_STAKE_USDC = 5.0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Evaluate ML shadow decisions after market outcomes are known.")
    parser.add_argument("--database-url", default=os.getenv("DATABASE_URL"))
    parser.add_argument("--model-dir", type=Path, default=DEFAULT_MODEL_DIR)
    parser.add_argument("--definition-id", type=int, default=4328)
    parser.add_argument("--asset", default="btc")
    parser.add_argument("--timeframe", default="5m", choices=["5m", "15m"])
    parser.add_argument("--hours", type=int, default=24)
    parser.add_argument("--out-dir", type=Path)
    parser.add_argument("--stop-loss-gap-usd", type=float, default=1.0)
    parser.add_argument("--no-replay-missing-ml", action="store_true")
    parser.add_argument("--telegram", action="store_true")
    parser.add_argument("--telegram-user-id", type=int, default=int(os.getenv("ML_ENTRY_QUALITY_TELEGRAM_USER_ID", "1")))
    return parser.parse_args()


def classify_shadow_outcome(
    ml_shadow_decision: str | None,
    entry_quality_label: int | None,
    stop_loss_risk_label: int | None,
) -> str:
    if entry_quality_label is None or stop_loss_risk_label is None:
        return "unresolved"
    decision = (ml_shadow_decision or "").strip().lower()
    if decision == "caution":
        return "neutral_caution"
    if decision == "allow_like":
        return "correct" if entry_quality_label == 1 and stop_loss_risk_label == 0 else "wrong"
    if decision == "avoid_like":
        return "correct" if entry_quality_label == 0 or stop_loss_risk_label == 1 else "wrong"
    return "unknown_decision"


def classify_trade_pnl_outcome(ml_shadow_decision: str | None, pnl_usdc: float | None) -> str:
    if pnl_usdc is None:
        return "no_actual_trade"
    decision = (ml_shadow_decision or "").strip().lower()
    if decision == "caution":
        return "neutral_caution"
    if decision == "allow_like":
        return "correct" if pnl_usdc > 0 else "wrong"
    if decision == "avoid_like":
        return "correct" if pnl_usdc <= 0 else "wrong"
    return "unknown_decision"


def wrong_reason(entry_quality_label: int | None, stop_loss_risk_label: int | None, pnl_usdc: float | None = None) -> str:
    if pnl_usdc is not None:
        return "lost" if pnl_usdc <= 0 else "won"
    if stop_loss_risk_label == 1:
        return "stop_loss_risk_hit"
    if entry_quality_label == 0:
        return "entry_quality_label_0"
    return "label_mismatch"


def maybe_float(value: Any) -> float | None:
    if value is None:
        return None
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        return None
    return parsed if parsed == parsed else None


def fetch_entry_logs(args: argparse.Namespace) -> list[dict[str, Any]]:
    if not args.database_url:
        raise SystemExit("DATABASE_URL or --database-url is required")
    sql = """
SELECT event_ts, flow_definition_id, market_slug, outcome, order_id, root_order_id, payload
FROM bot_decision_logs
WHERE event_type = 'ENTRY_EVALUATED'
  AND flow_definition_id = %(definition_id)s::TEXT
  AND event_ts >= now() - (%(hours)s::TEXT || ' hours')::INTERVAL
  AND lower(COALESCE(asset, %(asset)s)) = lower(%(asset)s)
  AND market_slug ILIKE %(timeframe_pattern)s
ORDER BY event_ts ASC
"""
    params = {
        "definition_id": args.definition_id,
        "hours": args.hours,
        "asset": args.asset,
        "timeframe_pattern": f"%{args.timeframe}%",
    }
    with psycopg.connect(args.database_url, row_factory=dict_row) as conn:
        with conn.cursor() as cur:
            cur.execute(sql, params)
            return [dict(row) for row in cur.fetchall()]


def load_models(model_dir: Path):
    schema = load_json(model_dir / "feature_schema.json")
    entry_model = joblib.load(model_dir / "entry_quality_model.joblib")
    risk_model = joblib.load(model_dir / "stop_loss_risk_model.joblib")
    return schema, entry_model, risk_model


def replay_shadow_decision(payload: dict[str, Any], models) -> dict[str, Any]:
    schema, entry_model, risk_model = models
    row = extract_payload_features(payload)
    features = coerce_feature_frame(pl.DataFrame([row]), schema).to_pandas()
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", UserWarning)
        entry_score = predict_positive(entry_model, features)[0]
        risk_score = predict_positive(risk_model, features)[0]
    return {
        "ml_entry_quality_score": entry_score,
        "ml_stop_loss_risk": risk_score,
        "ml_shadow_policy_version": SHADOW_DECISION_POLICY_VERSION,
        "ml_shadow_decision": score_to_shadow_decision(entry_score, risk_score),
    }


def snapshot_outcome(conn, market_slug: str, event_ts, direction: str, stop_loss_gap_usd: float) -> dict[str, Any]:
    sql = """
WITH market AS (
  SELECT max(window_end) AS window_end
  FROM market_price_second_snapshots
  WHERE market_slug = %(market_slug)s
),
future AS (
  SELECT
    second_ts,
    window_end,
    CASE
      WHEN %(direction)s = 'up' THEN chainlink_price - ptb_ref_price
      WHEN %(direction)s = 'down' THEN ptb_ref_price - chainlink_price
    END AS directional_gap,
    CASE WHEN %(direction)s = 'up' THEN yes_best_ask ELSE no_best_ask END AS snapshot_entry_ask,
    CASE WHEN %(direction)s = 'up' THEN yes_best_bid ELSE no_best_bid END AS snapshot_exit_bid
  FROM market_price_second_snapshots
  WHERE market_slug = %(market_slug)s
    AND second_ts >= date_trunc('second', %(event_ts)s::timestamptz)
    AND second_ts <= window_end
    AND ptb_ref_price IS NOT NULL
    AND chainlink_price IS NOT NULL
),
future_enriched AS (
  SELECT
    *,
    first_value(directional_gap) OVER (ORDER BY second_ts ASC) AS entry_directional_gap
  FROM future
),
agg AS (
  SELECT
    count(*) AS sample_count,
    min(directional_gap) AS future_min_gap,
    (array_agg(directional_gap ORDER BY second_ts ASC))[1] AS directional_gap,
    (array_agg(directional_gap ORDER BY second_ts DESC))[1] AS terminal_directional_gap,
    (array_agg(snapshot_entry_ask ORDER BY second_ts ASC))[1] AS snapshot_entry_ask
  FROM future_enriched
),
first_stop_loss_hit AS (
  SELECT
    second_ts AS stop_loss_hit_ts,
    snapshot_exit_bid AS stop_loss_hit_bid
  FROM future_enriched
  WHERE directional_gap <= %(stop_loss_gap_usd)s
     OR entry_directional_gap - directional_gap >= %(stop_loss_gap_usd)s
  ORDER BY second_ts ASC
  LIMIT 1
),
stop_loss_exit_fallback AS (
  SELECT future_enriched.snapshot_exit_bid AS stop_loss_fallback_bid
  FROM future_enriched
  CROSS JOIN first_stop_loss_hit
  WHERE future_enriched.second_ts >= first_stop_loss_hit.stop_loss_hit_ts
    AND future_enriched.snapshot_exit_bid IS NOT NULL
  ORDER BY future_enriched.second_ts ASC
  LIMIT 1
)
SELECT
  market.window_end,
  (market.window_end < now() - interval '30 seconds') AS resolved,
  agg.sample_count,
  agg.future_min_gap,
  agg.directional_gap,
  agg.terminal_directional_gap,
  agg.snapshot_entry_ask,
  first_stop_loss_hit.stop_loss_hit_ts,
  first_stop_loss_hit.stop_loss_hit_bid,
  stop_loss_exit_fallback.stop_loss_fallback_bid
FROM market
CROSS JOIN agg
LEFT JOIN first_stop_loss_hit ON TRUE
LEFT JOIN stop_loss_exit_fallback ON TRUE
"""
    with conn.cursor() as cur:
        cur.execute(
            sql,
            {
                "market_slug": market_slug,
                "event_ts": event_ts,
                "direction": direction,
                "stop_loss_gap_usd": stop_loss_gap_usd,
            },
        )
        row = cur.fetchone()
    if not row or row["window_end"] is None:
        return {"snapshot_status": "unresolved_or_missing_snapshot"}
    if not row["resolved"]:
        return {"snapshot_status": "unresolved_market", "window_end": row["window_end"].isoformat()}
    if not row["sample_count"]:
        return {"snapshot_status": "unresolved_or_missing_snapshot", "window_end": row["window_end"].isoformat()}
    return {
        "snapshot_status": "resolved",
        "window_end": row["window_end"].isoformat(),
        "future_min_gap": row["future_min_gap"],
        "directional_gap": row["directional_gap"],
        "terminal_directional_gap": row["terminal_directional_gap"],
        "snapshot_entry_ask": row["snapshot_entry_ask"],
        "stop_loss_hit_ts": row["stop_loss_hit_ts"].isoformat() if row["stop_loss_hit_ts"] else None,
        "stop_loss_hit_bid": row["stop_loss_hit_bid"],
        "stop_loss_fallback_bid": row["stop_loss_fallback_bid"],
        "stop_loss_gap_usd": stop_loss_gap_usd,
    }


def trade_outcome(conn, order_id: str | None, terminal_directional_gap: float | None) -> dict[str, Any]:
    if not order_id:
        return {"actual_trade_status": "no_actual_trade"}
    try:
        builder_order_id = int(order_id)
    except (TypeError, ValueError):
        return {"actual_trade_status": "no_actual_trade"}
    sql = """
WITH buy AS (
  SELECT builder_order_id, created_at, quantity, notional_usdc
  FROM trade_builder_revenge_flip_fills
  WHERE builder_order_id = %(builder_order_id)s
    AND order_side = 'buy'
  LIMIT 1
),
sell AS (
  SELECT
    COALESCE(sum(quantity), 0.0) AS quantity,
    COALESCE(sum(notional_usdc), 0.0) AS notional_usdc,
    min(created_at) AS first_sell_at
  FROM trade_builder_revenge_flip_fills
  WHERE parent_builder_order_id = %(builder_order_id)s
    AND order_side = 'sell'
)
SELECT
  buy.builder_order_id,
  buy.created_at AS buy_at,
  buy.quantity AS buy_qty,
  buy.notional_usdc AS buy_notional,
  sell.quantity AS sell_qty,
  sell.notional_usdc AS sell_notional,
  sell.first_sell_at
FROM buy
CROSS JOIN sell
"""
    with conn.cursor() as cur:
        cur.execute(sql, {"builder_order_id": builder_order_id})
        row = cur.fetchone()
    if not row:
        return {"actual_trade_status": "no_actual_trade"}
    buy_qty = float(row["buy_qty"] or 0.0)
    buy_notional = float(row["buy_notional"] or 0.0)
    sell_qty = float(row["sell_qty"] or 0.0)
    sell_notional = float(row["sell_notional"] or 0.0)
    sold_ratio = min(1.0, sell_qty / buy_qty) if buy_qty > 0 else 0.0
    realized = sell_notional - (buy_notional * sold_ratio)
    remaining_qty = max(0.0, buy_qty - sell_qty)
    expiry_value = 0.0
    status = "realized"
    if remaining_qty > 0 and terminal_directional_gap is not None:
        expiry_value = remaining_qty if terminal_directional_gap > 0 else 0.0
        realized += expiry_value - (buy_notional * (remaining_qty / buy_qty if buy_qty > 0 else 0.0))
        status = "realized_plus_expiry_estimate" if sell_qty > 0 else "expiry_estimate"
    elif remaining_qty > 0:
        status = "open_or_unresolved"
    return {
        "actual_trade_status": status,
        "actual_pnl_usdc": realized if status != "open_or_unresolved" else None,
        "buy_qty": buy_qty,
        "buy_notional": buy_notional,
        "sell_qty": sell_qty,
        "sell_notional": sell_notional,
        "expiry_value_usdc": expiry_value,
    }


def simulate_shadow_pnl_5usdc(
    *,
    entry_ask: Any,
    terminal_directional_gap: Any,
    stop_loss_hit_ts: Any = None,
    stop_loss_hit_bid: Any = None,
    stop_loss_fallback_bid: Any = None,
    stake_usdc: float = SHADOW_ENTRY_STAKE_USDC,
) -> dict[str, Any]:
    entry_price = maybe_float(entry_ask)
    base = {
        "shadow_entry_stake_usdc": stake_usdc,
        "shadow_entry_ask": entry_price,
        "shadow_shares": None,
        "shadow_exit_kind": "unresolved",
        "shadow_exit_price": None,
        "shadow_pnl_5usdc": None,
        "shadow_pnl_status": "unresolved",
    }
    if entry_price is None or entry_price <= 0.0:
        return {**base, "shadow_pnl_status": "bad_entry_ask"}

    shares = stake_usdc / entry_price
    base["shadow_shares"] = shares
    if stop_loss_hit_ts:
        hit_bid = maybe_float(stop_loss_hit_bid)
        fallback_bid = maybe_float(stop_loss_fallback_bid)
        if hit_bid is not None:
            exit_price = hit_bid
            status = "ok"
        elif fallback_bid is not None:
            exit_price = fallback_bid
            status = "fallback_after_stop_loss"
        else:
            exit_price = 0.0
            status = "fallback_zero_after_stop_loss"
        return {
            **base,
            "shadow_exit_kind": "stop_loss",
            "shadow_exit_price": exit_price,
            "shadow_pnl_5usdc": (shares * exit_price) - stake_usdc,
            "shadow_pnl_status": status,
        }

    terminal_gap = maybe_float(terminal_directional_gap)
    if terminal_gap is None:
        return base
    if terminal_gap > 0.0:
        return {
            **base,
            "shadow_exit_kind": "expiry_win",
            "shadow_exit_price": 1.0,
            "shadow_pnl_5usdc": shares - stake_usdc,
            "shadow_pnl_status": "ok",
        }
    return {
        **base,
        "shadow_exit_kind": "expiry_loss",
        "shadow_exit_price": 0.0,
        "shadow_pnl_5usdc": -stake_usdc,
        "shadow_pnl_status": "ok",
    }


def score_source_for_payload(payload: dict[str, Any]) -> str:
    return "live_shadow" if payload.get("ml_shadow_decision") else "current_model_replay"


def evaluate_rows(args: argparse.Namespace, rows: list[dict[str, Any]], models=None) -> list[dict[str, Any]]:
    evaluated: list[dict[str, Any]] = []
    with psycopg.connect(args.database_url, row_factory=dict_row) as conn:
        for row in rows:
            payload = row["payload"] or {}
            source = score_source_for_payload(payload)
            if source == "current_model_replay":
                if args.no_replay_missing_ml or models is None:
                    continue
                payload = {**payload, **replay_shadow_decision(payload, models)}

            direction = normalize_direction(row.get("outcome") or payload.get("outcome_norm") or payload.get("outcome"))
            entry_ask = (
                payload.get("price", {}).get("best_ask")
                if isinstance(payload.get("price"), dict)
                else None
            )
            if entry_ask is None and isinstance(payload.get("price"), dict):
                entry_ask = payload["price"].get("token_ask") or payload["price"].get("estimated_avg_fill")
            snapshot = snapshot_outcome(conn, row["market_slug"], row["event_ts"], direction, args.stop_loss_gap_usd)
            resolved_entry_ask = entry_ask if entry_ask is not None else snapshot.get("snapshot_entry_ask")
            if snapshot.get("snapshot_status") == "resolved":
                labels = compute_labels(
                    entry_ask=resolved_entry_ask,
                    terminal_directional_gap=snapshot.get("terminal_directional_gap"),
                    directional_gap=snapshot.get("directional_gap"),
                    future_min_gap=snapshot.get("future_min_gap"),
                    stop_loss_gap_usd=args.stop_loss_gap_usd,
                )
                entry_label, risk_label = labels
            else:
                entry_label, risk_label = None, None
            ml_decision = payload.get("ml_shadow_decision")
            counterfactual = classify_shadow_outcome(ml_decision, entry_label, risk_label)
            trade = trade_outcome(conn, row.get("order_id"), snapshot.get("terminal_directional_gap"))
            trade_correctness = classify_trade_pnl_outcome(ml_decision, trade.get("actual_pnl_usdc"))
            shadow_pnl = simulate_shadow_pnl_5usdc(
                entry_ask=resolved_entry_ask,
                terminal_directional_gap=snapshot.get("terminal_directional_gap"),
                stop_loss_hit_ts=snapshot.get("stop_loss_hit_ts"),
                stop_loss_hit_bid=snapshot.get("stop_loss_hit_bid"),
                stop_loss_fallback_bid=snapshot.get("stop_loss_fallback_bid"),
            )
            evaluated.append(
                {
                    "event_ts": row["event_ts"].isoformat(),
                    "flow_definition_id": row["flow_definition_id"],
                    "market_slug": row["market_slug"],
                    "order_id": row["order_id"],
                    "root_order_id": row["root_order_id"],
                    "direction": direction,
                    "score_source": source,
                    "ml_shadow_decision": ml_decision,
                    "ml_entry_quality_score": payload.get("ml_entry_quality_score"),
                    "ml_stop_loss_risk": payload.get("ml_stop_loss_risk"),
                    "entry_quality_label": entry_label,
                    "stop_loss_risk_label": risk_label,
                    "counterfactual_correctness": counterfactual,
                    "wrong_reason": wrong_reason(entry_label, risk_label)
                    if counterfactual == "wrong"
                    else None,
                    **snapshot,
                    **shadow_pnl,
                    **trade,
                    "trade_pnl_correctness": trade_correctness,
                    "trade_wrong_reason": wrong_reason(entry_label, risk_label, trade.get("actual_pnl_usdc"))
                    if trade_correctness == "wrong"
                    else None,
                }
            )
    return evaluated


def empty_group() -> dict[str, Any]:
    return {
        "resolved": 0,
        "allow_correct": 0,
        "allow_total": 0,
        "avoid_correct": 0,
        "avoid_total": 0,
        "caution": 0,
        "wrong": 0,
    }


def add_group_row(group: dict[str, Any], row: dict[str, Any]) -> None:
    if row["counterfactual_correctness"] not in {"correct", "wrong", "neutral_caution"}:
        return
    group["resolved"] += 1
    decision = row["ml_shadow_decision"]
    if decision == "caution":
        group["caution"] += 1
        return
    if decision == "allow_like":
        group["allow_total"] += 1
        if row["counterfactual_correctness"] == "correct":
            group["allow_correct"] += 1
    elif decision == "avoid_like":
        group["avoid_total"] += 1
        if row["counterfactual_correctness"] == "correct":
            group["avoid_correct"] += 1
    if row["counterfactual_correctness"] == "wrong":
        group["wrong"] += 1


def summarize_outcomes(rows: list[dict[str, Any]]) -> dict[str, Any]:
    live = empty_group()
    replay = empty_group()
    actual = {"trades": 0, "model_right": 0, "wrong": 0}
    last_wrong = None
    for row in rows:
        add_group_row(live if row["score_source"] == "live_shadow" else replay, row)
        if row["trade_pnl_correctness"] in {"correct", "wrong"}:
            actual["trades"] += 1
            if row["trade_pnl_correctness"] == "correct":
                actual["model_right"] += 1
            else:
                actual["wrong"] += 1
        if row["counterfactual_correctness"] == "wrong" or row["trade_pnl_correctness"] == "wrong":
            last_wrong = {
                "market_slug": row["market_slug"],
                "direction": row["direction"],
                "ml_shadow_decision": row["ml_shadow_decision"],
                "score_source": row["score_source"],
                "reason": row.get("trade_wrong_reason") or row.get("wrong_reason"),
            }
    return {
        "rows": len(rows),
        "live_shadow": live,
        "replay_current_model": replay,
        "actual_trades": actual,
        "optimized_threshold_backtest": optimize_trade_pnl_thresholds(rows),
        "shadow_pnl_backtest": optimize_shadow_pnl_thresholds(rows),
        "last_wrong": last_wrong,
    }


def threshold_values(start: int, stop: int) -> list[float]:
    return [round(value / 100.0, 2) for value in range(start, stop + 1, 5)]


def threshold_decision(row: dict[str, Any], entry_min: float, risk_max: float) -> str | None:
    entry_score = row.get("ml_entry_quality_score")
    risk_score = row.get("ml_stop_loss_risk")
    if entry_score is None or risk_score is None:
        return None
    return "allow_like" if entry_score >= entry_min and risk_score <= risk_max else "avoid_like"


def is_trade_decision_right(decision: str, pnl_usdc: float) -> bool:
    if decision == "allow_like":
        return pnl_usdc > 0
    return pnl_usdc <= 0


def pnl_for_shadow_decision(decision: str | None, pnl_usdc: float) -> float:
    return pnl_usdc if decision == "allow_like" else 0.0


def first_entry_key(row: dict[str, Any]) -> tuple[str, str]:
    return (str(row.get("market_slug") or ""), str(row.get("direction") or ""))


def first_entry_rows(rows: list[dict[str, Any]], decision_fn) -> list[dict[str, Any]]:
    selected: dict[tuple[str, str], dict[str, Any]] = {}
    for row in sorted(rows, key=lambda item: str(item.get("event_ts") or "")):
        if decision_fn(row) != "allow_like":
            continue
        key = first_entry_key(row)
        if key not in selected:
            selected[key] = row
    return list(selected.values())


def first_entry_decisions(rows: list[dict[str, Any]], decision_fn) -> list[tuple[str, dict[str, Any]]]:
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for row in sorted(rows, key=lambda item: str(item.get("event_ts") or "")):
        grouped.setdefault(first_entry_key(row), []).append(row)

    decisions: list[tuple[str, dict[str, Any]]] = []
    for group_rows in grouped.values():
        allow_row = next((row for row in group_rows if decision_fn(row) == "allow_like"), None)
        if allow_row is not None:
            decisions.append(("allow_like", allow_row))
            continue
        decided_row = next(
            (row for row in group_rows if decision_fn(row) in {"avoid_like", "allow_like"}),
            None,
        )
        if decided_row is not None:
            decisions.append((str(decision_fn(decided_row)), decided_row))
    return decisions


def optimize_trade_pnl_thresholds(rows: list[dict[str, Any]]) -> dict[str, Any]:
    eligible = [
        row
        for row in rows
        if row.get("trade_pnl_correctness") in {"correct", "wrong"}
        and row.get("actual_pnl_usdc") is not None
        and row.get("ml_entry_quality_score") is not None
        and row.get("ml_stop_loss_risk") is not None
    ]
    actual_trades = len(eligible)
    current_right = sum(1 for row in eligible if row.get("trade_pnl_correctness") == "correct")
    candidates: list[dict[str, Any]] = []
    for entry_min in threshold_values(35, 85):
        for risk_max in threshold_values(45, 95):
            allow_count = 0
            model_right = 0
            for row in eligible:
                decision = threshold_decision(row, entry_min, risk_max)
                if decision == "allow_like":
                    allow_count += 1
                if decision and is_trade_decision_right(decision, float(row["actual_pnl_usdc"])):
                    model_right += 1
            candidates.append(
                {
                    "entry_quality_min": entry_min,
                    "stop_loss_risk_max": risk_max,
                    "model_right": model_right,
                    "actual_trades": actual_trades,
                    "allow_count": allow_count,
                    "avoid_count": actual_trades - allow_count,
                }
            )
    candidates.sort(
        key=lambda row: (
            row["model_right"],
            row["allow_count"],
            -row["entry_quality_min"],
            row["stop_loss_risk_max"],
        ),
        reverse=True,
    )
    best = candidates[0] if candidates else {}
    return {
        "current_model_right": current_right,
        "best_model_right": best.get("model_right", 0),
        "actual_trades": actual_trades,
        "best_entry_quality_min": best.get("entry_quality_min"),
        "best_stop_loss_risk_max": best.get("stop_loss_risk_max"),
        "allow_count": best.get("allow_count", 0),
        "avoid_count": best.get("avoid_count", 0),
        "low_sample": actual_trades < 8,
        "top_candidates": candidates[:5],
    }


def optimize_shadow_pnl_thresholds(rows: list[dict[str, Any]]) -> dict[str, Any]:
    eligible = [
        row
        for row in rows
        if row.get("shadow_pnl_5usdc") is not None
        and row.get("ml_entry_quality_score") is not None
        and row.get("ml_stop_loss_risk") is not None
    ]
    candidate_rows = len(eligible)
    entry_groups = len({first_entry_key(row) for row in eligible})
    row_level_current_total = sum(
        pnl_for_shadow_decision(row.get("ml_shadow_decision"), float(row["shadow_pnl_5usdc"]))
        for row in eligible
    )
    current_allow_rows = sum(1 for row in eligible if row.get("ml_shadow_decision") == "allow_like")
    current_first_entries = first_entry_rows(eligible, lambda row: row.get("ml_shadow_decision"))
    current_total = sum(float(row["shadow_pnl_5usdc"]) for row in current_first_entries)
    current_group_decisions = first_entry_decisions(eligible, lambda row: row.get("ml_shadow_decision"))
    current_model_right = sum(
        1
        for decision, row in current_group_decisions
        if is_trade_decision_right(decision, float(row["shadow_pnl_5usdc"]))
    )
    candidates: list[dict[str, Any]] = []
    for entry_min in threshold_values(35, 85):
        for risk_max in threshold_values(45, 95):
            row_level_allow_count = 0
            row_level_total_pnl = 0.0
            for row in eligible:
                pnl = float(row["shadow_pnl_5usdc"])
                decision = threshold_decision(row, entry_min, risk_max)
                if decision == "allow_like":
                    row_level_allow_count += 1
                    row_level_total_pnl += pnl
            group_decisions = first_entry_decisions(
                eligible,
                lambda row: threshold_decision(row, entry_min, risk_max),
            )
            selected = [row for decision, row in group_decisions if decision == "allow_like"]
            model_right = sum(
                1
                for decision, row in group_decisions
                if is_trade_decision_right(decision, float(row["shadow_pnl_5usdc"]))
            )
            total_pnl = sum(float(row["shadow_pnl_5usdc"]) for row in selected)
            allow_count = len(selected)
            candidates.append(
                {
                    "entry_quality_min": entry_min,
                    "stop_loss_risk_max": risk_max,
                    "total_shadow_pnl_5usdc": round(total_pnl, 4),
                    "row_level_total_shadow_pnl_5usdc": round(row_level_total_pnl, 4),
                    "model_right": model_right,
                    "sample_count": entry_groups,
                    "allow_count": allow_count,
                    "avoid_count": entry_groups - allow_count,
                    "row_level_allow_count": row_level_allow_count,
                }
            )
    candidates.sort(
        key=lambda row: (
            row["total_shadow_pnl_5usdc"],
            row["model_right"],
            row["allow_count"],
            -row["entry_quality_min"],
            row["stop_loss_risk_max"],
        ),
        reverse=True,
    )
    best = candidates[0] if candidates else {}
    return {
        "current_total_pnl_5usdc": round(current_total, 4),
        "best_total_pnl_5usdc": best.get("total_shadow_pnl_5usdc", 0.0),
        "sample_count": entry_groups,
        "candidate_rows": candidate_rows,
        "entry_groups": entry_groups,
        "current_allow_count": len(current_first_entries),
        "current_avoid_count": entry_groups - len(current_first_entries),
        "current_allow_rows": current_allow_rows,
        "current_first_entry_count": len(current_first_entries),
        "current_model_right": current_model_right,
        "best_model_right": best.get("model_right", 0),
        "best_entry_quality_min": best.get("entry_quality_min"),
        "best_stop_loss_risk_max": best.get("stop_loss_risk_max"),
        "allow_count": best.get("allow_count", 0),
        "avoid_count": best.get("avoid_count", 0),
        "row_level_current_total_pnl_5usdc": round(row_level_current_total, 4),
        "row_level_best_total_pnl_5usdc": best.get("row_level_total_shadow_pnl_5usdc", 0.0),
        "row_level_allow_count": best.get("row_level_allow_count", 0),
        "low_sample": entry_groups < 8,
        "top_candidates": candidates[:5],
        "exit_counts": shadow_exit_counts(eligible),
    }


def shadow_exit_counts(rows: list[dict[str, Any]]) -> dict[str, int]:
    counts = {
        "stop_loss": 0,
        "expiry_win": 0,
        "expiry_loss": 0,
        "fallback_bid": 0,
    }
    for row in rows:
        exit_kind = row.get("shadow_exit_kind")
        if exit_kind in {"stop_loss", "expiry_win", "expiry_loss"}:
            counts[exit_kind] += 1
        if str(row.get("shadow_pnl_status") or "").startswith("fallback"):
            counts["fallback_bid"] += 1
    return counts


def write_report(out_dir: Path, metrics: dict[str, Any]) -> None:
    lines = ["# ML Shadow Outcome Check", "", f"- Rows: {metrics.get('rows', 0)}"]
    for key, title in [("live_shadow", "Live shadow"), ("replay_current_model", "Replay current model")]:
        group = metrics.get(key, {})
        lines.extend(
            [
                "",
                f"## {title}",
                f"- Resolved: {group.get('resolved', 0)}",
                f"- Allow correct: {group.get('allow_correct', 0)}/{group.get('allow_total', 0)}",
                f"- Avoid correct: {group.get('avoid_correct', 0)}/{group.get('avoid_total', 0)}",
                f"- Caution: {group.get('caution', 0)}",
            ]
        )
    actual = metrics.get("actual_trades", {})
    optimizer = metrics.get("optimized_threshold_backtest") or {}
    shadow_pnl = metrics.get("shadow_pnl_backtest") or {}
    lines.extend(
        [
            "",
            "## Actual trades",
            f"- Trades: {actual.get('trades', 0)}",
            f"- Model right: {actual.get('model_right', 0)}/{actual.get('trades', 0)}",
        ]
    )
    if optimizer:
        lines.extend(
            [
                "",
                "## Threshold backtest",
                f"- Current: {optimizer.get('current_model_right', 0)}/{optimizer.get('actual_trades', 0)}",
                f"- Best: {optimizer.get('best_model_right', 0)}/{optimizer.get('actual_trades', 0)}",
                f"- Suggested: entry>={optimizer.get('best_entry_quality_min')} risk<={optimizer.get('best_stop_loss_risk_max')}",
                f"- Best coverage: allow={optimizer.get('allow_count', 0)} avoid={optimizer.get('avoid_count', 0)}",
                f"- Low sample: {optimizer.get('low_sample', False)}",
                "",
                "### Top threshold candidates",
            ]
        )
        for row in optimizer.get("top_candidates", []):
            lines.append(
                "- "
                f"{row.get('model_right', 0)}/{row.get('actual_trades', 0)} "
                f"entry>={row.get('entry_quality_min')} "
                f"risk<={row.get('stop_loss_risk_max')} "
                f"allow={row.get('allow_count', 0)} avoid={row.get('avoid_count', 0)}"
            )
    if shadow_pnl:
        exit_counts = shadow_pnl.get("exit_counts") or {}
        lines.extend(
            [
                "",
                "## 5 USDC first-entry shadow PnL backtest",
                f"- Current PnL: {shadow_pnl.get('current_total_pnl_5usdc', 0.0)}",
                f"- Best PnL: {shadow_pnl.get('best_total_pnl_5usdc', 0.0)}",
                f"- Suggested: entry>={shadow_pnl.get('best_entry_quality_min')} risk<={shadow_pnl.get('best_stop_loss_risk_max')}",
                f"- Best first entries: allow={shadow_pnl.get('allow_count', 0)} avoid={shadow_pnl.get('avoid_count', 0)} groups={shadow_pnl.get('entry_groups', 0)}",
                f"- Current first entries: allow={shadow_pnl.get('current_first_entry_count', 0)} avoid={shadow_pnl.get('current_avoid_count', 0)}",
                f"- Row-level diagnostic: current={shadow_pnl.get('row_level_current_total_pnl_5usdc', 0.0)} best={shadow_pnl.get('row_level_best_total_pnl_5usdc', 0.0)} current_allow_rows={shadow_pnl.get('current_allow_rows', 0)} best_allow_rows={shadow_pnl.get('row_level_allow_count', 0)} candidate_rows={shadow_pnl.get('candidate_rows', 0)}",
                f"- Exit counts: stop_loss={exit_counts.get('stop_loss', 0)} expiry_win={exit_counts.get('expiry_win', 0)} expiry_loss={exit_counts.get('expiry_loss', 0)} fallback_bid={exit_counts.get('fallback_bid', 0)}",
                f"- Low sample: {shadow_pnl.get('low_sample', False)}",
                "",
                "### Top 5 USDC first-entry PnL threshold candidates",
            ]
        )
        for row in shadow_pnl.get("top_candidates", []):
            lines.append(
                "- "
                f"pnl={row.get('total_shadow_pnl_5usdc', 0.0)} "
                f"right={row.get('model_right', 0)}/{row.get('sample_count', 0)} "
                f"entry>={row.get('entry_quality_min')} "
                f"risk<={row.get('stop_loss_risk_max')} "
                f"entries={row.get('allow_count', 0)} row_allow={row.get('row_level_allow_count', 0)}"
            )
    last_wrong = metrics.get("last_wrong")
    if last_wrong:
        lines.extend(["", "## Last wrong", f"- {last_wrong}"])
    (out_dir / "report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")


def evaluate_shadow_outcomes(args: argparse.Namespace) -> dict[str, Any]:
    out_dir = ensure_dir(args.out_dir or (REPO_ROOT / "analysis" / f"ml_entry_quality_outcomes_{now_tag()}"))
    rows = fetch_entry_logs(args)
    models = None
    if not args.no_replay_missing_ml:
        try:
            models = load_models(args.model_dir)
        except Exception:
            models = None
    evaluated = evaluate_rows(args, rows, models)
    if evaluated:
        pl.DataFrame(evaluated).write_parquet(out_dir / "outcomes.parquet")
    else:
        pl.DataFrame({"event_ts": []}).write_parquet(out_dir / "outcomes.parquet")
    metrics = summarize_outcomes(evaluated)
    metrics.update(
        {
            "out_dir": str(out_dir),
            "definition_id": args.definition_id,
            "asset": args.asset,
            "timeframe": args.timeframe,
            "hours": args.hours,
        }
    )
    write_json(out_dir / "metrics.json", metrics)
    write_report(out_dir, metrics)
    return metrics


def main() -> None:
    args = parse_args()
    metrics = evaluate_shadow_outcomes(args)
    notify = send_telegram_message(
        outcome_check_message(metrics),
        enabled=args.telegram,
        database_url=args.database_url,
        user_id=args.telegram_user_id,
    )
    print(f"evaluated {metrics['rows']} shadow outcome rows")
    if args.telegram:
        print(f"telegram: {notify.reason}")


if __name__ == "__main__":
    main()
