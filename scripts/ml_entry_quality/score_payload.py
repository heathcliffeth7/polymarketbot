#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import joblib
import polars as pl

from common import (
    DEFAULT_MODEL_DIR,
    SHADOW_DECISION_POLICY_VERSION,
    extract_payload_features,
    coerce_feature_frame,
    load_json,
    predict_positive,
    score_to_shadow_decision,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Score one ENTRY_EVALUATED payload for ML shadow fields.")
    parser.add_argument("--model-dir", type=Path, default=DEFAULT_MODEL_DIR)
    return parser.parse_args()


def score_payload(payload: dict, model_dir: Path) -> dict:
    schema = load_json(model_dir / "feature_schema.json")
    entry_model = joblib.load(model_dir / "entry_quality_model.joblib")
    risk_model = joblib.load(model_dir / "stop_loss_risk_model.joblib")
    row = extract_payload_features(payload)
    features = coerce_feature_frame(pl.DataFrame([row]), schema).to_pandas()
    entry_score = predict_positive(entry_model, features)[0]
    risk_score = predict_positive(risk_model, features)[0]
    return {
        "ml_entry_quality_score": entry_score,
        "ml_stop_loss_risk": risk_score,
        "ml_model_version": schema.get("model_version", "unknown"),
        "ml_features_version": schema.get("features_version", "unknown"),
        "ml_shadow_policy_version": SHADOW_DECISION_POLICY_VERSION,
        "ml_shadow_decision": score_to_shadow_decision(entry_score, risk_score),
    }


def main() -> None:
    args = parse_args()
    payload = json.loads(sys.stdin.read())
    print(json.dumps(score_payload(payload, args.model_dir), sort_keys=True))


if __name__ == "__main__":
    main()
