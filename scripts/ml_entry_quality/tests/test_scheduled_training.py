import argparse
import json
from pathlib import Path

import pytest

pytest.importorskip("polars")

from run_scheduled_training import (
    gate_decision,
    nonblocking_lock,
    promote_candidate,
)


MODEL_FILES = [
    "entry_quality_model.joblib",
    "stop_loss_risk_model.joblib",
    "feature_schema.json",
    "training_metrics.json",
]


def write_artifact(path: Path, marker: str, entry_auc: float = 0.7, risk_auc: float = 0.7):
    path.mkdir(parents=True, exist_ok=True)
    for name in MODEL_FILES:
        target = path / name
        if name == "training_metrics.json":
            target.write_text(
                json.dumps(
                    {
                        "entry_quality": {"test": {"auc": entry_auc}},
                        "stop_loss_risk": {"test": {"auc": risk_auc}},
                    }
                ),
                encoding="utf-8",
            )
        else:
            target.write_text(f"{marker}:{name}", encoding="utf-8")


def test_metric_gate_promotes_better_candidate():
    decision = gate_decision(
        candidate_metrics={
            "entry_quality": {"test": {"auc": 0.72}},
            "stop_loss_risk": {"test": {"auc": 0.71}},
        },
        current={
            "entry_quality": {"test": {"auc": 0.70}},
            "stop_loss_risk": {"test": {"auc": 0.70}},
        },
        row_count=1000,
        min_rows=1000,
        auc_floor=0.60,
        tolerance=0.02,
    )

    assert decision["passed"]


def test_metric_gate_rejects_regression():
    decision = gate_decision(
        candidate_metrics={
            "entry_quality": {"test": {"auc": 0.67}},
            "stop_loss_risk": {"test": {"auc": 0.71}},
        },
        current={
            "entry_quality": {"test": {"auc": 0.72}},
            "stop_loss_risk": {"test": {"auc": 0.70}},
        },
        row_count=1000,
        min_rows=1000,
        auc_floor=0.60,
        tolerance=0.02,
    )

    assert not decision["passed"]
    assert "entry_quality_auc_below_gate" in decision["reasons"]


def test_metric_gate_allows_first_artifact_above_floor():
    decision = gate_decision(
        candidate_metrics={
            "entry_quality": {"test": {"auc": 0.61}},
            "stop_loss_risk": {"test": {"auc": 0.62}},
        },
        current=None,
        row_count=1000,
        min_rows=1000,
        auc_floor=0.60,
        tolerance=0.02,
    )

    assert decision["passed"]


def test_nonblocking_lock_reports_second_holder_as_skipped(tmp_path):
    lock_file = tmp_path / "train.lock"
    with nonblocking_lock(lock_file) as first:
        assert first
        with nonblocking_lock(lock_file) as second:
            assert not second


def test_promote_candidate_archives_current_and_replaces_atomically(tmp_path):
    model_dir = tmp_path / "artifacts" / "ml_entry_quality" / "v1"
    candidate_dir = tmp_path / "candidate"
    write_artifact(model_dir, "old", 0.7, 0.7)
    write_artifact(candidate_dir, "new", 0.8, 0.8)

    result = promote_candidate(candidate_dir, model_dir, "20260603000000")

    assert (model_dir / "entry_quality_model.joblib").read_text(encoding="utf-8").startswith("new:")
    archive = Path(result["archive_dir"])
    assert (archive / "entry_quality_model.joblib").read_text(encoding="utf-8").startswith("old:")
