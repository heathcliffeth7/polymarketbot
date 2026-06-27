#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
from pathlib import Path

import duckdb
import joblib
import polars as pl
from sklearn.metrics import log_loss, roc_auc_score

from common import (
    DEFAULT_MODEL_DIR,
    REPO_ROOT,
    coerce_feature_frame,
    ensure_dir,
    load_json,
    now_tag,
    predict_positive,
    score_to_shadow_decision,
    write_json,
)
from telegram_notify import evaluation_message, send_telegram_message


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Evaluate RevengeFlip ML entry-quality shadow models.")
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--model-dir", type=Path, default=DEFAULT_MODEL_DIR)
    parser.add_argument("--out-dir", type=Path)
    parser.add_argument("--telegram", action="store_true", help="Send a Telegram summary after completion.")
    parser.add_argument("--database-url", default=os.getenv("DATABASE_URL"))
    parser.add_argument("--telegram-user-id", type=int, default=int(os.getenv("ML_ENTRY_QUALITY_TELEGRAM_USER_ID", "1")))
    return parser.parse_args()


def metric_block(labels: list[int], scores: list[float]) -> dict:
    classes = sorted(set(int(value) for value in labels))
    return {
        "rows": len(labels),
        "positive_rate": sum(labels) / len(labels) if labels else None,
        "auc": roc_auc_score(labels, scores) if len(classes) == 2 else None,
        "logloss": log_loss(labels, scores, labels=[0, 1]) if len(classes) == 2 else None,
    }


def bucket_table(frame) -> list[dict]:
    con = duckdb.connect(":memory:")
    con.register("scores", frame)
    return con.execute(
        """
        SELECT
          floor(entry_score * 10) / 10 AS entry_bucket,
          count(*) AS rows,
          avg(entry_quality_label) AS entry_quality_rate,
          avg(stop_loss_risk_label) AS stop_loss_risk_rate,
          avg(stop_loss_score) AS avg_stop_loss_score
        FROM scores
        GROUP BY 1
        ORDER BY 1
        """
    ).fetchdf().to_dict(orient="records")


def write_report(path: Path, metrics: dict, buckets: list[dict]) -> None:
    lines = [
        "# RevengeFlip ML Entry Quality V1 Evaluation",
        "",
        f"- Rows: {metrics['rows']}",
        f"- Entry quality AUC: {metrics['entry_quality']['auc']}",
        f"- Stop-loss risk AUC: {metrics['stop_loss_risk']['auc']}",
        "",
        "## Score Buckets",
        "",
        "| entry_bucket | rows | entry_quality_rate | stop_loss_risk_rate | avg_stop_loss_score |",
        "| --- | ---: | ---: | ---: | ---: |",
    ]
    for row in buckets:
        lines.append(
            "| {entry_bucket} | {rows} | {entry_quality_rate:.4f} | {stop_loss_risk_rate:.4f} | {avg_stop_loss_score:.4f} |".format(
                entry_bucket=row["entry_bucket"],
                rows=row["rows"],
                entry_quality_rate=row["entry_quality_rate"] or 0.0,
                stop_loss_risk_rate=row["stop_loss_risk_rate"] or 0.0,
                avg_stop_loss_score=row["avg_stop_loss_score"] or 0.0,
            )
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def evaluate(args: argparse.Namespace) -> dict:
    out_dir = ensure_dir(args.out_dir or (REPO_ROOT / "analysis" / f"ml_entry_quality_{now_tag()}"))
    schema = load_json(args.model_dir / "feature_schema.json")
    entry_model = joblib.load(args.model_dir / "entry_quality_model.joblib")
    risk_model = joblib.load(args.model_dir / "stop_loss_risk_model.joblib")
    frame = pl.read_parquet(args.dataset)
    features = coerce_feature_frame(frame, schema).to_pandas()

    entry_scores = predict_positive(entry_model, features)
    risk_scores = predict_positive(risk_model, features)
    labels_entry = [int(value) for value in frame["entry_quality_label"].to_list()]
    labels_risk = [int(value) for value in frame["stop_loss_risk_label"].to_list()]
    scored = frame.select(["entry_quality_label", "stop_loss_risk_label"]).to_pandas()
    scored["entry_score"] = entry_scores
    scored["stop_loss_score"] = risk_scores
    scored["shadow_decision"] = [
        score_to_shadow_decision(entry, risk) for entry, risk in zip(entry_scores, risk_scores)
    ]
    buckets = bucket_table(scored)
    metrics = {
        "rows": frame.height,
        "report_dir": str(out_dir),
        "entry_quality": metric_block(labels_entry, entry_scores),
        "stop_loss_risk": metric_block(labels_risk, risk_scores),
        "buckets": buckets,
    }
    write_json(out_dir / "metrics.json", metrics)
    write_report(out_dir / "report.md", metrics, buckets)
    scored.to_parquet(out_dir / "scored_rows.parquet")
    return metrics


def main() -> None:
    args = parse_args()
    metrics = evaluate(args)
    notify = send_telegram_message(
        evaluation_message(metrics, metrics["report_dir"]),
        enabled=args.telegram,
        database_url=args.database_url,
        user_id=args.telegram_user_id,
    )
    print(f"wrote evaluation for {metrics['rows']} rows")
    if args.telegram:
        print(f"telegram: {notify.reason}")


if __name__ == "__main__":
    main()
