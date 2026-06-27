from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

MODEL_VERSION = "ml_entry_quality_v1"
FEATURES_VERSION = "features_v1"
SHADOW_DECISION_POLICY_VERSION = "static_balanced_v3"
SHADOW_ALLOW_ENTRY_MIN = 0.60
SHADOW_ALLOW_RISK_MAX = 0.90
SHADOW_AVOID_ENTRY_MAX = 0.30
SHADOW_AVOID_RISK_MIN = 0.95

NUMERIC_FEATURES = [
    "remaining_sec",
    "elapsed_sec",
    "entry_ask",
    "best_bid",
    "spread_cent",
    "ask_depth_usdc",
    "directional_gap",
    "gap_abs",
    "gap_velocity_3s",
    "gap_velocity_10s",
    "drawdown_from_peak_30s",
    "seconds_since_peak_30s",
    "iv_edge",
    "adjusted_margin",
    "gap_strength",
    "gap_strength_margin",
    "q_final",
    "expected_move_eff",
    "chainlink_age_ms",
    "cex_blocking",
    "recent_notional_10s",
    "recent_notional_30s",
    "recent_trade_count_30s",
]

CATEGORICAL_FEATURES = [
    "direction",
    "asset",
    "timeframe",
    "matched_rule",
    "selected_entry_current_source",
    "cex_status",
    "volume_regime",
]

LABEL_COLUMNS = ["entry_quality_label", "stop_loss_risk_label"]
FEATURE_COLUMNS = NUMERIC_FEATURES + CATEGORICAL_FEATURES

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MODEL_DIR = REPO_ROOT / "artifacts" / "ml_entry_quality" / "v1"


def now_tag() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%d%H%M%S")


def ensure_dir(path: Path) -> Path:
    path.mkdir(parents=True, exist_ok=True)
    return path


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def build_feature_schema() -> dict[str, Any]:
    return {
        "model_version": MODEL_VERSION,
        "features_version": FEATURES_VERSION,
        "numeric_features": NUMERIC_FEATURES,
        "categorical_features": CATEGORICAL_FEATURES,
        "feature_columns": FEATURE_COLUMNS,
        "label_columns": LABEL_COLUMNS,
    }


def coerce_feature_frame(frame: Any, schema: dict[str, Any] | None = None):
    import polars as pl

    schema = schema or build_feature_schema()
    if not isinstance(frame, pl.DataFrame):
        frame = pl.DataFrame(frame)

    for name in schema["numeric_features"]:
        if name not in frame.columns:
            frame = frame.with_columns(pl.lit(None).alias(name))
        frame = frame.with_columns(pl.col(name).cast(pl.Float64, strict=False))

    for name in schema["categorical_features"]:
        if name not in frame.columns:
            frame = frame.with_columns(pl.lit("unknown").alias(name))
        frame = frame.with_columns(
            pl.col(name)
            .cast(pl.Utf8, strict=False)
            .fill_null("unknown")
            .replace("", "unknown")
        )

    return frame.select(schema["feature_columns"])


def get_path(value: Any, path: str, default: Any = None) -> Any:
    current = value
    for key in path.split("."):
        if isinstance(current, dict) and key in current:
            current = current[key]
        else:
            return default
    return current


def first_non_null(*values: Any) -> Any:
    for value in values:
        if value is not None:
            return value
    return None


def as_float(value: Any, default: float | None = None) -> float | None:
    if value is None:
        return default
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        return default
    return parsed if parsed == parsed else default


def as_bool_float(value: Any, default: float = 0.0) -> float:
    if isinstance(value, bool):
        return 1.0 if value else 0.0
    if isinstance(value, (int, float)):
        return 1.0 if value else 0.0
    if isinstance(value, str):
        return 1.0 if value.lower() in {"1", "true", "yes", "on"} else 0.0
    return default


def find_nested_key(value: Any, key: str) -> Any:
    if isinstance(value, dict):
        if key in value:
            return value[key]
        for child in value.values():
            found = find_nested_key(child, key)
            if found is not None:
                return found
    elif isinstance(value, list):
        for child in value:
            found = find_nested_key(child, key)
            if found is not None:
                return found
    return None


def normalize_direction(value: Any) -> str:
    text = str(value or "").strip().lower()
    if text in {"yes", "up", "long", "bull"}:
        return "up"
    if text in {"no", "down", "short", "bear"}:
        return "down"
    return "unknown"


def timeframe_from_slug(market_slug: str | None) -> str:
    slug = market_slug or ""
    if "15m" in slug:
        return "15m"
    if "5m" in slug:
        return "5m"
    return "unknown"


def score_to_shadow_decision(entry_quality: float | None, stop_loss_risk: float | None) -> str:
    if entry_quality is None or stop_loss_risk is None:
        return "caution"
    if entry_quality >= SHADOW_ALLOW_ENTRY_MIN and stop_loss_risk <= SHADOW_ALLOW_RISK_MAX:
        return "allow_like"
    if entry_quality < SHADOW_AVOID_ENTRY_MAX or stop_loss_risk >= SHADOW_AVOID_RISK_MIN:
        return "avoid_like"
    return "caution"


def compute_labels(
    *,
    entry_ask: float | None,
    terminal_directional_gap: float | None,
    directional_gap: float | None,
    future_min_gap: float | None,
    stop_loss_gap_usd: float,
) -> tuple[int, int]:
    entry_ok = entry_ask is not None and entry_ask <= 0.98
    quality = int(entry_ok and terminal_directional_gap is not None and terminal_directional_gap > 0.0)
    adverse = None
    if directional_gap is not None and future_min_gap is not None:
        adverse = max(0.0, directional_gap - future_min_gap)
    risk = int(
        future_min_gap is not None
        and (
            future_min_gap <= stop_loss_gap_usd
            or (adverse is not None and adverse >= stop_loss_gap_usd)
        )
    )
    return quality, risk


def extract_payload_features(payload: dict[str, Any]) -> dict[str, Any]:
    entry_debug = find_nested_key(payload, "entry_quality_debug") or {}
    iv_edge = entry_debug.get("iv_edge", {}) if isinstance(entry_debug, dict) else {}
    cex = entry_debug.get("cex_direction_guard", {}) if isinstance(entry_debug, dict) else {}
    source = entry_debug.get("source", {}) if isinstance(entry_debug, dict) else {}
    ptb = payload.get("ptb", {}) if isinstance(payload.get("ptb"), dict) else {}
    price = payload.get("price", {}) if isinstance(payload.get("price"), dict) else {}
    market = payload.get("market", {}) if isinstance(payload.get("market"), dict) else {}
    volume = get_path(payload, "volume.polymarket", {}) or {}

    outcome = first_non_null(payload.get("outcome_norm"), payload.get("outcome"), market.get("outcome"))
    ask = first_non_null(price.get("best_ask"), price.get("token_ask"), price.get("estimated_avg_fill"))
    gap_now = ptb.get("gap_now")
    peak_drawdown = first_non_null(ptb.get("drawdown_from_peak"), ptb.get("drawdown_from_peak_30s"))

    return {
        "remaining_sec": as_float(market.get("remaining_s")),
        "elapsed_sec": as_float(market.get("market_elapsed_s")),
        "direction": normalize_direction(outcome),
        "asset": str(first_non_null(market.get("asset"), payload.get("asset"), "unknown")).lower(),
        "timeframe": timeframe_from_slug(first_non_null(payload.get("market_slug"), market.get("market_slug"))),
        "entry_ask": as_float(ask),
        "best_bid": as_float(price.get("best_bid")),
        "spread_cent": as_float(price.get("spread_cent")),
        "ask_depth_usdc": as_float(price.get("ask_depth_usdc")),
        "directional_gap": as_float(gap_now),
        "gap_abs": abs(as_float(gap_now, 0.0) or 0.0),
        "gap_velocity_3s": as_float(first_non_null(ptb.get("slope_3s"), ptb.get("gap_velocity_3s"))),
        "gap_velocity_10s": as_float(first_non_null(ptb.get("slope_10s"), ptb.get("gap_velocity_10s"))),
        "drawdown_from_peak_30s": as_float(peak_drawdown),
        "seconds_since_peak_30s": as_float(first_non_null(ptb.get("seconds_since_peak"), ptb.get("seconds_since_peak_30s"))),
        "iv_edge": as_float(first_non_null(iv_edge.get("edge"), entry_debug.get("edge"))),
        "adjusted_margin": as_float(first_non_null(iv_edge.get("adjustedMargin"), iv_edge.get("adjusted_margin"))),
        "gap_strength": as_float(first_non_null(iv_edge.get("gapStrength"), iv_edge.get("gap_strength"))),
        "gap_strength_margin": as_float(first_non_null(iv_edge.get("gapStrengthMargin"), iv_edge.get("gap_strength_margin"))),
        "q_final": as_float(first_non_null(iv_edge.get("qFinal"), iv_edge.get("q_final"))),
        "expected_move_eff": as_float(first_non_null(iv_edge.get("expectedMoveEff"), iv_edge.get("expected_move_eff"))),
        "matched_rule": str(first_non_null(iv_edge.get("matchedRule"), iv_edge.get("matched_rule"), "unknown")),
        "chainlink_age_ms": as_float(first_non_null(source.get("chainlinkAgeMs"), source.get("chainlink_age_ms"))),
        "selected_entry_current_source": str(
            first_non_null(
                payload.get("selected_entry_current_source"),
                source.get("ptbCurrentPriceSource"),
                source.get("ptb_current_price_source"),
                "unknown",
            )
        ),
        "cex_status": str(first_non_null(cex.get("status"), "unknown")),
        "cex_blocking": as_bool_float(first_non_null(cex.get("blocking"), payload.get("cex_blocking"))),
        "recent_notional_10s": as_float(volume.get("recent_notional_10s")),
        "recent_notional_30s": as_float(volume.get("recent_notional_30s")),
        "recent_trade_count_30s": as_float(volume.get("recent_trade_count_30s")),
        "volume_regime": str(first_non_null(volume.get("regime"), "unknown")),
    }


def predict_positive(model: Any, features: Any) -> list[float]:
    if hasattr(model, "predict_proba"):
        probabilities = model.predict_proba(features)
        if getattr(probabilities, "shape", (0, 0))[1] >= 2:
            return [float(value) for value in probabilities[:, 1]]
        return [float(value) for value in probabilities[:, 0]]
    predictions = model.predict(features)
    return [float(value) for value in predictions]
