#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
from pathlib import Path

import joblib
import polars as pl
from sklearn.calibration import CalibratedClassifierCV
from sklearn.compose import ColumnTransformer
from sklearn.dummy import DummyClassifier
from sklearn.ensemble import HistGradientBoostingClassifier
from sklearn.impute import SimpleImputer
from sklearn.metrics import log_loss, roc_auc_score
from sklearn.pipeline import Pipeline
from sklearn.preprocessing import OneHotEncoder

from common import (
    DEFAULT_MODEL_DIR,
    build_feature_schema,
    coerce_feature_frame,
    ensure_dir,
    now_tag,
    predict_positive,
    write_json,
)
from telegram_notify import send_telegram_message, training_message


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Train RevengeFlip ML entry-quality shadow models.")
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=DEFAULT_MODEL_DIR)
    parser.add_argument("--min-rows", type=int, default=1000)
    parser.add_argument("--telegram", action="store_true", help="Send a Telegram summary after completion.")
    parser.add_argument("--database-url", default=os.getenv("DATABASE_URL"))
    parser.add_argument("--telegram-user-id", type=int, default=int(os.getenv("ML_ENTRY_QUALITY_TELEGRAM_USER_ID", "1")))
    return parser.parse_args()


def one_hot_encoder() -> OneHotEncoder:
    try:
        return OneHotEncoder(handle_unknown="ignore", sparse_output=False)
    except TypeError:
        return OneHotEncoder(handle_unknown="ignore", sparse=False)


def make_pipeline(schema: dict, y_train) -> Pipeline:
    if len(set(int(value) for value in y_train)) < 2:
        classifier = DummyClassifier(strategy="most_frequent")
    else:
        classifier = HistGradientBoostingClassifier(
            max_iter=200,
            learning_rate=0.05,
            max_leaf_nodes=31,
            random_state=42,
        )
    preprocessor = ColumnTransformer(
        transformers=[
            ("num", SimpleImputer(strategy="median"), schema["numeric_features"]),
            (
                "cat",
                Pipeline(
                    steps=[
                        ("imputer", SimpleImputer(strategy="most_frequent")),
                        ("onehot", one_hot_encoder()),
                    ]
                ),
                schema["categorical_features"],
            ),
        ]
    )
    return Pipeline(steps=[("features", preprocessor), ("classifier", classifier)])


def split_frame(frame: pl.DataFrame) -> tuple[pl.DataFrame, pl.DataFrame, pl.DataFrame]:
    frame = frame.sort("second_ts") if "second_ts" in frame.columns else frame
    n_rows = frame.height
    train_end = max(1, int(n_rows * 0.70))
    valid_end = max(train_end + 1, int(n_rows * 0.85))
    return frame[:train_end], frame[train_end:valid_end], frame[valid_end:]


def model_metrics(model, x_frame, y_values) -> dict:
    if len(y_values) == 0:
        return {"rows": 0, "auc": None, "logloss": None}
    probabilities = predict_positive(model, x_frame)
    classes = sorted(set(int(value) for value in y_values))
    auc = roc_auc_score(y_values, probabilities) if len(classes) == 2 else None
    loss = log_loss(y_values, probabilities, labels=[0, 1]) if len(classes) == 2 else None
    return {"rows": len(y_values), "auc": auc, "logloss": loss}


def calibrate_if_possible(model, x_valid, y_valid):
    classes = sorted(set(int(value) for value in y_valid))
    if len(classes) < 2 or len(y_valid) < 20:
        return model
    try:
        from sklearn.frozen import FrozenEstimator

        calibrated = CalibratedClassifierCV(FrozenEstimator(model), method="sigmoid")
        calibrated.fit(x_valid, y_valid)
        return calibrated
    except (ImportError, ValueError):
        pass
    try:
        calibrated = CalibratedClassifierCV(model, method="sigmoid", cv="prefit")
        calibrated.fit(x_valid, y_valid)
        return calibrated
    except TypeError:
        calibrated = CalibratedClassifierCV(estimator=model, method="sigmoid", cv="prefit")
        calibrated.fit(x_valid, y_valid)
        return calibrated
    except ValueError:
        return model


def train_one(label: str, train: pl.DataFrame, valid: pl.DataFrame, test: pl.DataFrame, schema: dict):
    x_train = coerce_feature_frame(train, schema).to_pandas()
    x_valid = coerce_feature_frame(valid, schema).to_pandas()
    x_test = coerce_feature_frame(test, schema).to_pandas()
    y_train = train[label].to_list()
    y_valid = valid[label].to_list()
    y_test = test[label].to_list()

    model = make_pipeline(schema, y_train)
    model.fit(x_train, y_train)
    model = calibrate_if_possible(model, x_valid, y_valid)
    return model, {
        "train": model_metrics(model, x_train, y_train),
        "validation": model_metrics(model, x_valid, y_valid),
        "test": model_metrics(model, x_test, y_test),
    }


def train_models(args: argparse.Namespace) -> dict:
    frame = pl.read_parquet(args.dataset)
    if frame.height < args.min_rows:
        raise SystemExit(f"dataset has {frame.height} rows; need at least {args.min_rows}")

    schema = build_feature_schema()
    missing = [label for label in schema["label_columns"] if label not in frame.columns]
    if missing:
        raise SystemExit(f"dataset missing labels: {', '.join(missing)}")

    train, valid, test = split_frame(frame)
    out_dir = ensure_dir(args.out)
    entry_model, entry_metrics = train_one("entry_quality_label", train, valid, test, schema)
    risk_model, risk_metrics = train_one("stop_loss_risk_label", train, valid, test, schema)

    joblib.dump(entry_model, out_dir / "entry_quality_model.joblib")
    joblib.dump(risk_model, out_dir / "stop_loss_risk_model.joblib")
    schema = {
        **schema,
        "trained_at": now_tag(),
        "dataset_path": str(args.dataset),
        "train_rows": train.height,
        "validation_rows": valid.height,
        "test_rows": test.height,
    }
    write_json(out_dir / "feature_schema.json", schema)
    metrics = {
        "entry_quality": entry_metrics,
        "stop_loss_risk": risk_metrics,
        "rows": {"train": train.height, "validation": valid.height, "test": test.height},
    }
    write_json(out_dir / "training_metrics.json", metrics)
    return metrics


def main() -> None:
    args = parse_args()
    metrics = train_models(args)
    notify = send_telegram_message(
        training_message(metrics, str(args.out)),
        enabled=args.telegram,
        database_url=args.database_url,
        user_id=args.telegram_user_id,
    )
    print(f"trained models: entry_auc={metrics['entry_quality']['test']['auc']} risk_auc={metrics['stop_loss_risk']['test']['auc']}")
    if args.telegram:
        print(f"telegram: {notify.reason}")


if __name__ == "__main__":
    main()
