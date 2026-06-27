#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
from pathlib import Path

import polars as pl
import psycopg
from psycopg.rows import dict_row

from common import (
    REPO_ROOT,
    build_feature_schema,
    compute_labels,
    ensure_dir,
    now_tag,
    write_json,
)
from telegram_notify import dataset_message, send_telegram_message


def timeframe_seconds(timeframe: str) -> int:
    if timeframe == "15m":
        return 900
    if timeframe == "5m":
        return 300
    raise ValueError(f"unsupported timeframe: {timeframe}")


def default_out_path() -> Path:
    return REPO_ROOT / "analysis" / f"ml_entry_quality_{now_tag()}" / "dataset.parquet"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build RevengeFlip ML entry-quality dataset.")
    parser.add_argument("--database-url", default=os.getenv("DATABASE_URL"))
    parser.add_argument("--asset", default="btc")
    parser.add_argument("--timeframe", default="5m", choices=["5m", "15m"])
    parser.add_argument("--since")
    parser.add_argument("--until")
    parser.add_argument("--out", type=Path, default=None)
    parser.add_argument("--limit-markets", type=int)
    parser.add_argument("--sample-every-sec", type=int, default=1)
    parser.add_argument("--min-remaining-sec", type=int, default=12)
    parser.add_argument("--max-remaining-sec", type=int, default=180)
    parser.add_argument("--stop-loss-gap-usd", type=float, default=1.0)
    parser.add_argument("--telegram", action="store_true", help="Send a Telegram summary after completion.")
    parser.add_argument("--telegram-user-id", type=int, default=int(os.getenv("ML_ENTRY_QUALITY_TELEGRAM_USER_ID", "1")))
    return parser.parse_args()


def fetch_candidate_rows(args: argparse.Namespace) -> list[dict]:
    if not args.database_url:
        raise SystemExit("DATABASE_URL or --database-url is required")

    duration = timeframe_seconds(args.timeframe)
    market_limit = args.limit_markets or 2_147_483_647
    sql = """
WITH base AS (
  SELECT
    market_slug,
    asset,
    window_start,
    window_end,
    second_ts,
    EXTRACT(EPOCH FROM (window_end - second_ts))::DOUBLE PRECISION AS remaining_sec,
    EXTRACT(EPOCH FROM (second_ts - window_start))::DOUBLE PRECISION AS elapsed_sec,
    ptb_ref_price,
    chainlink_price,
    yes_best_bid,
    yes_best_ask,
    yes_ask_depth_usdc,
    no_best_bid,
    no_best_ask,
    no_ask_depth_usdc
  FROM market_price_second_snapshots
  WHERE lower(asset) = lower(%(asset)s)
    AND second_ts <= window_end
    AND EXTRACT(EPOCH FROM (window_end - window_start)) BETWEEN %(duration_min)s AND %(duration_max)s
    AND ptb_ref_price IS NOT NULL
    AND chainlink_price IS NOT NULL
    AND (%(since)s::timestamptz IS NULL OR second_ts >= %(since)s::timestamptz)
    AND (%(until)s::timestamptz IS NULL OR second_ts <= %(until)s::timestamptz)
),
selected_markets AS (
  SELECT market_slug
  FROM base
  GROUP BY market_slug
  ORDER BY min(window_start) DESC
  LIMIT %(market_limit)s
),
directional AS (
  SELECT
    market_slug,
    lower(asset) AS asset,
    window_start,
    window_end,
    second_ts,
    remaining_sec,
    elapsed_sec,
    'up'::TEXT AS direction,
    yes_best_ask AS entry_ask,
    yes_best_bid AS best_bid,
    yes_ask_depth_usdc AS ask_depth_usdc,
    CASE
      WHEN yes_best_ask IS NOT NULL AND yes_best_bid IS NOT NULL
      THEN (yes_best_ask - yes_best_bid) * 100.0
    END AS spread_cent,
    chainlink_price - ptb_ref_price AS directional_gap
  FROM base
  WHERE market_slug IN (SELECT market_slug FROM selected_markets)
  UNION ALL
  SELECT
    market_slug,
    lower(asset) AS asset,
    window_start,
    window_end,
    second_ts,
    remaining_sec,
    elapsed_sec,
    'down'::TEXT AS direction,
    no_best_ask AS entry_ask,
    no_best_bid AS best_bid,
    no_ask_depth_usdc AS ask_depth_usdc,
    CASE
      WHEN no_best_ask IS NOT NULL AND no_best_bid IS NOT NULL
      THEN (no_best_ask - no_best_bid) * 100.0
    END AS spread_cent,
    ptb_ref_price - chainlink_price AS directional_gap
  FROM base
  WHERE market_slug IN (SELECT market_slug FROM selected_markets)
),
scored AS (
  SELECT
    *,
    directional_gap - LAG(directional_gap, 3) OVER (
      PARTITION BY market_slug, direction ORDER BY second_ts ASC
    ) AS gap_velocity_3s,
    directional_gap - LAG(directional_gap, 10) OVER (
      PARTITION BY market_slug, direction ORDER BY second_ts ASC
    ) AS gap_velocity_10s,
    MAX(directional_gap) OVER (
      PARTITION BY market_slug, direction ORDER BY second_ts ASC
      ROWS BETWEEN 30 PRECEDING AND CURRENT ROW
    ) AS peak_gap_30s,
    MIN(directional_gap) OVER (
      PARTITION BY market_slug, direction ORDER BY second_ts ASC
      ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING
    ) AS future_min_gap,
    LAST_VALUE(directional_gap) OVER (
      PARTITION BY market_slug, direction ORDER BY second_ts ASC
      ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING
    ) AS terminal_directional_gap
  FROM directional
)
SELECT
  market_slug,
  asset,
  %(timeframe)s::TEXT AS timeframe,
  window_start,
  window_end,
  second_ts,
  remaining_sec,
  elapsed_sec,
  direction,
  entry_ask,
  best_bid,
  spread_cent,
  ask_depth_usdc,
  directional_gap,
  abs(directional_gap) AS gap_abs,
  gap_velocity_3s,
  gap_velocity_10s,
  GREATEST(0.0, peak_gap_30s - directional_gap) AS drawdown_from_peak_30s,
  NULL::DOUBLE PRECISION AS seconds_since_peak_30s,
  future_min_gap,
  terminal_directional_gap
FROM scored
WHERE remaining_sec >= %(min_remaining_sec)s
  AND remaining_sec <= %(max_remaining_sec)s
  AND entry_ask IS NOT NULL
  AND (%(sample_every_sec)s <= 1 OR mod(floor(EXTRACT(EPOCH FROM second_ts))::BIGINT, %(sample_every_sec)s) = 0)
ORDER BY window_start ASC, second_ts ASC, direction ASC
"""
    params = {
        "asset": args.asset,
        "timeframe": args.timeframe,
        "duration_min": duration - 10,
        "duration_max": duration + 10,
        "since": args.since,
        "until": args.until,
        "market_limit": market_limit,
        "min_remaining_sec": args.min_remaining_sec,
        "max_remaining_sec": args.max_remaining_sec,
        "sample_every_sec": max(1, args.sample_every_sec),
    }
    with psycopg.connect(args.database_url, row_factory=dict_row) as conn:
        with conn.cursor() as cur:
            cur.execute(sql, params)
            return list(cur.fetchall())


def build_dataset(args: argparse.Namespace) -> tuple[Path, dict]:
    rows = fetch_candidate_rows(args)
    if rows:
        frame = pl.DataFrame(rows)
    else:
        frame = pl.DataFrame()

    if frame.is_empty():
        frame = pl.DataFrame({"entry_quality_label": [], "stop_loss_risk_label": []})
    else:
        labels = [
            compute_labels(
                entry_ask=row.get("entry_ask"),
                terminal_directional_gap=row.get("terminal_directional_gap"),
                directional_gap=row.get("directional_gap"),
                future_min_gap=row.get("future_min_gap"),
                stop_loss_gap_usd=args.stop_loss_gap_usd,
            )
            for row in rows
        ]
        frame = frame.with_columns(
            pl.Series("entry_quality_label", [label[0] for label in labels]).cast(pl.Int8),
            pl.Series("stop_loss_risk_label", [label[1] for label in labels]).cast(pl.Int8),
            pl.lit(None).cast(pl.Float64).alias("iv_edge"),
            pl.lit(None).cast(pl.Float64).alias("adjusted_margin"),
            pl.lit(None).cast(pl.Float64).alias("gap_strength"),
            pl.lit(None).cast(pl.Float64).alias("gap_strength_margin"),
            pl.lit(None).cast(pl.Float64).alias("q_final"),
            pl.lit(None).cast(pl.Float64).alias("expected_move_eff"),
            pl.lit(None).cast(pl.Float64).alias("chainlink_age_ms"),
            pl.lit(0.0).alias("cex_blocking"),
            pl.lit(None).cast(pl.Float64).alias("recent_notional_10s"),
            pl.lit(None).cast(pl.Float64).alias("recent_notional_30s"),
            pl.lit(None).cast(pl.Float64).alias("recent_trade_count_30s"),
            pl.lit("snapshot_counterfactual").alias("matched_rule"),
            pl.lit("chainlink").alias("selected_entry_current_source"),
            pl.lit("unknown").alias("cex_status"),
            pl.lit("unknown").alias("volume_regime"),
        )

    out_path = args.out or default_out_path()
    ensure_dir(out_path.parent)
    frame.write_parquet(out_path)

    meta = {
        "generated_at": now_tag(),
        "dataset_path": str(out_path),
        "row_count": frame.height,
        "market_count": frame.select(pl.col("market_slug").n_unique()).item() if "market_slug" in frame.columns and frame.height else 0,
        "asset": args.asset,
        "timeframe": args.timeframe,
        "stop_loss_gap_usd": args.stop_loss_gap_usd,
        "feature_schema": build_feature_schema(),
    }
    write_json(out_path.parent / "dataset_meta.json", meta)
    return out_path, meta


def main() -> None:
    args = parse_args()
    out_path, meta = build_dataset(args)
    notify = send_telegram_message(
        dataset_message(meta),
        enabled=args.telegram,
        database_url=args.database_url,
        user_id=args.telegram_user_id,
    )
    print(f"wrote {meta['row_count']} rows to {out_path}")
    if args.telegram:
        print(f"telegram: {notify.reason}")


if __name__ == "__main__":
    main()
