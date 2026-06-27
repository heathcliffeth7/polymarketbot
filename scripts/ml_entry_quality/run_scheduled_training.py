#!/usr/bin/env python3
from __future__ import annotations

import argparse
import fcntl
import os
import shutil
import sys
from contextlib import contextmanager
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

from build_dataset import build_dataset
from common import REPO_ROOT, ensure_dir, load_json, now_tag, write_json
from evaluate_models import evaluate
from evaluate_shadow_outcomes import evaluate_shadow_outcomes
from telegram_notify import scheduled_training_message, send_telegram_message
from train_models import train_models

MODEL_FILES = [
    "entry_quality_model.joblib",
    "stop_loss_risk_model.joblib",
    "feature_schema.json",
    "training_metrics.json",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Scheduled ML entry-quality training with metric-gated promotion.")
    parser.add_argument("--database-url", default=os.getenv("DATABASE_URL"))
    parser.add_argument("--asset", default="btc")
    parser.add_argument("--timeframe", default="5m", choices=["5m", "15m"])
    parser.add_argument("--lookback-days", type=int, default=45)
    parser.add_argument("--model-dir", type=Path, default=REPO_ROOT / "artifacts" / "ml_entry_quality" / "v1")
    parser.add_argument("--min-rows", type=int, default=1000)
    parser.add_argument("--auc-floor", type=float, default=0.60)
    parser.add_argument("--auc-regression-tolerance", type=float, default=0.02)
    parser.add_argument("--telegram", action="store_true")
    parser.add_argument("--telegram-user-id", type=int, default=int(os.getenv("ML_ENTRY_QUALITY_TELEGRAM_USER_ID", "1")))
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--limit-markets", type=int)
    parser.add_argument("--sample-every-sec", type=int, default=1)
    parser.add_argument("--lock-file", type=Path, default=REPO_ROOT / "analysis" / "ml_entry_quality_training.lock")
    parser.add_argument("--outcome-hours", type=int, default=24)
    parser.add_argument("--skip-outcome-check", action="store_true")
    return parser.parse_args()


@contextmanager
def nonblocking_lock(path: Path):
    ensure_dir(path.parent)
    with path.open("w", encoding="utf-8") as handle:
        try:
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            yield False
            return
        yield True
        fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


def utc_since(days: int) -> str:
    return (datetime.now(timezone.utc) - timedelta(days=days)).isoformat()


def metric_auc(metrics: dict[str, Any], target: str) -> float | None:
    block = metrics.get(target, {})
    if "test" in block:
        return block.get("test", {}).get("auc")
    return block.get("auc")


def current_metrics(model_dir: Path) -> dict[str, Any] | None:
    path = model_dir / "training_metrics.json"
    if not path.exists():
        return None
    try:
        return load_json(path)
    except Exception:
        return None


def has_complete_artifact(model_dir: Path) -> bool:
    return all((model_dir / name).exists() for name in MODEL_FILES)


def gate_decision(
    *,
    candidate_metrics: dict[str, Any],
    current: dict[str, Any] | None,
    row_count: int,
    min_rows: int,
    auc_floor: float,
    tolerance: float,
) -> dict[str, Any]:
    candidate_entry = metric_auc(candidate_metrics, "entry_quality")
    candidate_risk = metric_auc(candidate_metrics, "stop_loss_risk")
    current_entry = metric_auc(current, "entry_quality") if current else None
    current_risk = metric_auc(current, "stop_loss_risk") if current else None
    required_entry = max(auc_floor, (current_entry or auc_floor) - tolerance)
    required_risk = max(auc_floor, (current_risk or auc_floor) - tolerance)
    passed = (
        row_count >= min_rows
        and candidate_entry is not None
        and candidate_risk is not None
        and candidate_entry >= required_entry
        and candidate_risk >= required_risk
    )
    reasons = []
    if row_count < min_rows:
        reasons.append("dataset_below_min_rows")
    if candidate_entry is None or candidate_entry < required_entry:
        reasons.append("entry_quality_auc_below_gate")
    if candidate_risk is None or candidate_risk < required_risk:
        reasons.append("stop_loss_risk_auc_below_gate")
    return {
        "passed": passed,
        "reasons": reasons,
        "candidate": {
            "entry_quality_auc": candidate_entry,
            "stop_loss_risk_auc": candidate_risk,
            "rows": row_count,
        },
        "current": {
            "entry_quality_auc": current_entry,
            "stop_loss_risk_auc": current_risk,
        },
        "required": {
            "entry_quality_auc": required_entry,
            "stop_loss_risk_auc": required_risk,
            "min_rows": min_rows,
        },
    }


def copy_artifact(src: Path, dst: Path) -> None:
    ensure_dir(dst)
    for name in MODEL_FILES:
        shutil.copy2(src / name, dst / name)


def promote_candidate(candidate_dir: Path, model_dir: Path, run_tag: str) -> dict[str, Any]:
    archive_dir = model_dir.parent / "archive" / run_tag
    if has_complete_artifact(model_dir):
        copy_artifact(model_dir, archive_dir)
    staging_dir = model_dir.parent / f".v1.promote-{run_tag}"
    if staging_dir.exists():
        shutil.rmtree(staging_dir)
    copy_artifact(candidate_dir, staging_dir)
    ensure_dir(model_dir)
    for name in MODEL_FILES:
        os.replace(staging_dir / name, model_dir / name)
    shutil.rmtree(staging_dir, ignore_errors=True)
    return {"archive_dir": str(archive_dir) if archive_dir.exists() else None}


def attach_outcome_metrics(args: argparse.Namespace, result: dict[str, Any], run_tag: str) -> None:
    if args.skip_outcome_check:
        return
    try:
        outcome_args = argparse.Namespace(
            database_url=args.database_url,
            model_dir=args.model_dir,
            definition_id=4328,
            asset=args.asset,
            timeframe=args.timeframe,
            hours=args.outcome_hours,
            out_dir=REPO_ROOT / "analysis" / f"ml_entry_quality_outcomes_{run_tag}",
            stop_loss_gap_usd=1.0,
            no_replay_missing_ml=False,
            telegram=False,
            telegram_user_id=args.telegram_user_id,
        )
        result["outcome_metrics"] = evaluate_shadow_outcomes(outcome_args)
    except Exception as exc:
        result["outcome_metrics_error"] = f"{type(exc).__name__}: {exc}"


def run_training(args: argparse.Namespace) -> dict[str, Any]:
    run_tag = now_tag()
    out_dir = ensure_dir(REPO_ROOT / "analysis" / f"ml_entry_quality_{run_tag}")
    candidate_dir = out_dir / "candidate_model"
    eval_dir = out_dir / "eval"
    dataset_path = out_dir / "dataset.parquet"
    status = "started"
    result: dict[str, Any] = {
        "run_tag": run_tag,
        "status": status,
        "asset": args.asset,
        "timeframe": args.timeframe,
        "model_dir": str(args.model_dir),
        "dry_run": args.dry_run,
        "telegram_sent": False,
    }
    try:
        dataset_args = argparse.Namespace(
            database_url=args.database_url,
            asset=args.asset,
            timeframe=args.timeframe,
            since=utc_since(args.lookback_days),
            until=None,
            out=dataset_path,
            limit_markets=args.limit_markets,
            sample_every_sec=args.sample_every_sec,
            min_remaining_sec=12,
            max_remaining_sec=180,
            stop_loss_gap_usd=1.0,
            telegram=False,
            telegram_user_id=args.telegram_user_id,
        )
        _, meta = build_dataset(dataset_args)
        if meta["row_count"] < args.min_rows and args.limit_markets is None:
            dataset_args.since = None
            _, meta = build_dataset(dataset_args)
            result["fallback_used"] = "all_available_history"

        train_args = argparse.Namespace(
            dataset=dataset_path,
            out=candidate_dir,
            min_rows=args.min_rows,
            telegram=False,
            database_url=args.database_url,
            telegram_user_id=args.telegram_user_id,
        )
        train_metrics = train_models(train_args)
        eval_args = argparse.Namespace(
            dataset=dataset_path,
            model_dir=candidate_dir,
            out_dir=eval_dir,
            telegram=False,
            database_url=args.database_url,
            telegram_user_id=args.telegram_user_id,
        )
        eval_metrics = evaluate(eval_args)
        gate = gate_decision(
            candidate_metrics=train_metrics,
            current=current_metrics(args.model_dir),
            row_count=meta["row_count"],
            min_rows=args.min_rows,
            auc_floor=args.auc_floor,
            tolerance=args.auc_regression_tolerance,
        )
        result.update(
            {
                "dataset": meta,
                "training_metrics": train_metrics,
                "eval_metrics": eval_metrics,
                "gate": gate,
                "candidate_model_dir": str(candidate_dir),
                "eval_dir": str(eval_dir),
            }
        )
        if gate["passed"] and not args.dry_run:
            result.update(promote_candidate(candidate_dir, args.model_dir, run_tag))
            status = "promoted"
        elif gate["passed"] and args.dry_run:
            status = "dry_run_would_promote"
        else:
            status = "metric_gate_rejected"
    except Exception as exc:
        status = "failed"
        result["error"] = f"{type(exc).__name__}: {exc}"
    result["status"] = status
    attach_outcome_metrics(args, result, run_tag)
    write_json(out_dir / "scheduled_run.json", result)
    if args.telegram and status in {"promoted", "failed", "metric_gate_rejected"}:
        notify = send_telegram_message(
            scheduled_training_message(result),
            enabled=True,
            database_url=args.database_url,
            user_id=args.telegram_user_id,
        )
        result["telegram_sent"] = notify.sent
        result["telegram_reason"] = notify.reason
        write_json(out_dir / "scheduled_run.json", result)
    return result


def main() -> None:
    args = parse_args()
    with nonblocking_lock(args.lock_file) as acquired:
        if not acquired:
            result = {
                "run_tag": now_tag(),
                "status": "skipped_already_running",
                "lock_file": str(args.lock_file),
            }
            print(result["status"])
            return
        result = run_training(args)
        print(f"{result['status']}: {result.get('run_tag')}")
        if result["status"] == "failed":
            sys.exit(1)


if __name__ == "__main__":
    main()
