import json
import subprocess
import sys

import pytest

polars = pytest.importorskip("polars")
pytest.importorskip("sklearn")
pytest.importorskip("joblib")

from common import FEATURE_COLUMNS


def test_train_and_evaluate_scripts_emit_artifacts(tmp_path):
    rows = []
    for idx in range(80):
        row = {column: 0.0 for column in FEATURE_COLUMNS}
        row.update(
            {
                "second_ts": f"2026-01-01T00:{idx // 60:02d}:{idx % 60:02d}Z",
                "direction": "up" if idx % 2 == 0 else "down",
                "asset": "btc",
                "timeframe": "5m",
                "matched_rule": "fixture",
                "selected_entry_current_source": "chainlink",
                "cex_status": "neutral",
                "volume_regime": "normal",
                "remaining_sec": float(180 - idx),
                "entry_ask": 0.55 + (idx % 20) / 100,
                "directional_gap": float(idx % 13),
                "entry_quality_label": 1 if idx % 4 in {0, 1} else 0,
                "stop_loss_risk_label": 1 if idx % 5 == 0 else 0,
            }
        )
        rows.append(row)
    dataset = tmp_path / "dataset.parquet"
    polars.DataFrame(rows).write_parquet(dataset)
    model_dir = tmp_path / "model"
    report_dir = tmp_path / "report"

    subprocess.run(
        [
            sys.executable,
            "scripts/ml_entry_quality/train_models.py",
            "--dataset",
            str(dataset),
            "--out",
            str(model_dir),
            "--min-rows",
            "10",
        ],
        check=True,
    )
    subprocess.run(
        [
            sys.executable,
            "scripts/ml_entry_quality/evaluate_models.py",
            "--dataset",
            str(dataset),
            "--model-dir",
            str(model_dir),
            "--out-dir",
            str(report_dir),
        ],
        check=True,
    )
    scored = subprocess.run(
        [
            sys.executable,
            "scripts/ml_entry_quality/score_payload.py",
            "--model-dir",
            str(model_dir),
        ],
        input=json.dumps(
            {
                "market_slug": "btc-updown-5m-1780443300",
                "outcome_norm": "UP",
                "market": {"asset": "BTC", "remaining_s": 42, "market_elapsed_s": 258},
                "price": {"best_ask": 0.72},
                "ptb": {"gap_now": 12.0, "slope_3s": 1.0, "slope_10s": 3.0},
            }
        ),
        text=True,
        capture_output=True,
        check=True,
    )
    score_payload = json.loads(scored.stdout)

    assert (model_dir / "entry_quality_model.joblib").exists()
    assert (model_dir / "stop_loss_risk_model.joblib").exists()
    assert (model_dir / "feature_schema.json").exists()
    assert (report_dir / "metrics.json").exists()
    assert (report_dir / "report.md").exists()
    assert "ml_entry_quality_score" in score_payload
    assert "ml_stop_loss_risk" in score_payload
    assert score_payload["ml_shadow_policy_version"] == "static_balanced_v3"
    assert score_payload["ml_shadow_decision"] in {"allow_like", "caution", "avoid_like"}
