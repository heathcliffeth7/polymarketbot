#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
from pathlib import Path

import joblib
import polars as pl
import psycopg
from psycopg.rows import dict_row

from common import (
    DEFAULT_MODEL_DIR,
    REPO_ROOT,
    coerce_feature_frame,
    ensure_dir,
    extract_payload_features,
    load_json,
    now_tag,
    predict_positive,
    score_to_shadow_decision,
    write_json,
)
from telegram_notify import recent_shadow_message, send_telegram_message


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Score recent ENTRY_EVALUATED decision logs offline.")
    parser.add_argument("--database-url", default=os.getenv("DATABASE_URL"))
    parser.add_argument("--model-dir", type=Path, default=DEFAULT_MODEL_DIR)
    parser.add_argument("--definition-id", type=int, default=4328)
    parser.add_argument("--hours", type=int, default=24)
    parser.add_argument("--out-dir", type=Path)
    parser.add_argument("--telegram", action="store_true", help="Send a Telegram summary after completion.")
    parser.add_argument("--telegram-user-id", type=int, default=int(os.getenv("ML_ENTRY_QUALITY_TELEGRAM_USER_ID", "1")))
    return parser.parse_args()


def fetch_recent_payloads(args: argparse.Namespace) -> list[dict]:
    if not args.database_url:
        raise SystemExit("DATABASE_URL or --database-url is required")
    sql = """
SELECT event_ts, market_slug, payload
FROM bot_decision_logs
WHERE event_type = 'ENTRY_EVALUATED'
  AND (%(definition_id)s::TEXT IS NULL OR flow_definition_id = %(definition_id)s::TEXT)
  AND event_ts >= now() - (%(hours)s::TEXT || ' hours')::INTERVAL
ORDER BY event_ts ASC
"""
    with psycopg.connect(args.database_url, row_factory=dict_row) as conn:
        with conn.cursor() as cur:
            cur.execute(sql, {"definition_id": args.definition_id, "hours": args.hours})
            return list(cur.fetchall())


def shadow_score_recent(args: argparse.Namespace) -> dict:
    rows = fetch_recent_payloads(args)
    out_dir = ensure_dir(args.out_dir or (REPO_ROOT / "analysis" / f"ml_entry_quality_recent_{now_tag()}"))
    if not rows:
        report = {"rows": 0, "reason": "no_recent_entry_evaluated_logs"}
        write_json(out_dir / "metrics.json", report)
        return report

    schema = load_json(args.model_dir / "feature_schema.json")
    entry_model = joblib.load(args.model_dir / "entry_quality_model.joblib")
    risk_model = joblib.load(args.model_dir / "stop_loss_risk_model.joblib")
    feature_rows = [extract_payload_features(row["payload"]) for row in rows]
    features = coerce_feature_frame(pl.DataFrame(feature_rows), schema).to_pandas()
    entry_scores = predict_positive(entry_model, features)
    risk_scores = predict_positive(risk_model, features)
    scored_rows = []
    for source, entry_score, risk_score in zip(rows, entry_scores, risk_scores):
        scored_rows.append(
            {
                "event_ts": source["event_ts"].isoformat(),
                "market_slug": source["market_slug"],
                "ml_entry_quality_score": entry_score,
                "ml_stop_loss_risk": risk_score,
                "ml_shadow_decision": score_to_shadow_decision(entry_score, risk_score),
            }
        )

    frame = pl.DataFrame(scored_rows)
    frame.write_parquet(out_dir / "recent_scores.parquet")
    metrics = {
        "rows": len(scored_rows),
        "avg_entry_quality_score": sum(entry_scores) / len(entry_scores),
        "avg_stop_loss_risk": sum(risk_scores) / len(risk_scores),
        "shadow_decisions": frame.group_by("ml_shadow_decision").len().to_dicts(),
    }
    write_json(out_dir / "metrics.json", metrics)
    lines = [
        "# Recent ML Entry Quality Shadow Scores",
        "",
        f"- Rows: {metrics['rows']}",
        f"- Avg entry quality: {metrics['avg_entry_quality_score']:.4f}",
        f"- Avg stop-loss risk: {metrics['avg_stop_loss_risk']:.4f}",
        "",
        "## Decisions",
        "",
    ]
    for row in metrics["shadow_decisions"]:
        lines.append(f"- {row['ml_shadow_decision']}: {row['len']}")
    (out_dir / "report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return metrics


def main() -> None:
    args = parse_args()
    metrics = shadow_score_recent(args)
    notify = send_telegram_message(
        recent_shadow_message(metrics),
        enabled=args.telegram,
        database_url=args.database_url,
        user_id=args.telegram_user_id,
    )
    print(f"scored {metrics['rows']} recent entry logs")
    if args.telegram:
        print(f"telegram: {notify.reason}")


if __name__ == "__main__":
    main()
