from telegram_notify import (
    dataset_message,
    load_db_telegram_credentials,
    outcome_check_message,
    send_telegram_message,
    scheduled_training_systemd_failure_message,
    training_message,
)


def test_telegram_disabled_does_not_require_credentials():
    result = send_telegram_message("hello", enabled=False)

    assert not result.sent
    assert result.reason == "disabled"


def test_training_message_contains_core_metrics():
    text = training_message(
        {
            "rows": {"train": 70, "validation": 15, "test": 15},
            "entry_quality": {"test": {"auc": 0.71, "logloss": 0.42}},
            "stop_loss_risk": {"test": {"auc": 0.66, "logloss": 0.51}},
        },
        "artifacts/ml_entry_quality/v1",
    )

    assert "ML Entry Quality V1 trained" in text
    assert "Entry AUC: 0.7100" in text
    assert "Stop-loss AUC: 0.6600" in text


def test_dataset_message_contains_scope_and_rows():
    text = dataset_message(
        {
            "dataset_path": "analysis/ml_entry_quality_x/dataset.parquet",
            "row_count": 120,
            "market_count": 3,
            "asset": "btc",
            "timeframe": "5m",
        }
    )

    assert "Rows: 120 markets=3" in text
    assert "Scope: btc 5m" in text


def test_db_credentials_are_empty_without_database_url():
    assert load_db_telegram_credentials(None, 1) == (None, None)


def test_systemd_failure_message_names_unit_and_reason():
    text = scheduled_training_systemd_failure_message("trainer.service", "oom-kill")

    assert "Status: systemd_failed" in text
    assert "Unit: trainer.service" in text
    assert "Reason: oom-kill" in text


def test_outcome_check_message_reports_live_and_replay():
    text = outcome_check_message(
        {
            "live_shadow": {
                "resolved": 2,
                "allow_correct": 1,
                "allow_total": 1,
                "avoid_correct": 0,
                "avoid_total": 1,
                "caution": 0,
            },
            "replay_current_model": {
                "resolved": 3,
                "allow_correct": 2,
                "allow_total": 2,
                "avoid_correct": 1,
                "avoid_total": 1,
                "caution": 0,
            },
            "actual_trades": {"trades": 1, "model_right": 1},
            "optimized_threshold_backtest": {
                "current_model_right": 5,
                "best_model_right": 8,
                "actual_trades": 11,
                "best_entry_quality_min": 0.60,
                "best_stop_loss_risk_max": 0.85,
                "low_sample": False,
            },
            "shadow_pnl_backtest": {
                "current_total_pnl_5usdc": 0.0,
                "best_total_pnl_5usdc": 7.42,
                "best_entry_quality_min": 0.60,
                "best_stop_loss_risk_max": 0.95,
                "allow_count": 6,
                "sample_count": 25,
                "entry_groups": 25,
                "low_sample": False,
            },
            "last_wrong": {
                "market_slug": "btc-updown-5m-x",
                "direction": "down",
                "ml_shadow_decision": "allow_like",
                "reason": "stop_loss_risk_hit",
            },
        }
    )

    assert "ML Outcome Check:" in text
    assert "Resolved shadow: 2" in text
    assert "Replay current model: 3 resolved, allow_correct=2/2, avoid_correct=1/1, caution=0" in text
    assert "Trade PnL model-right: 1/1" in text
    assert "Threshold backtest: current=5/11 best=8/11 entry>=0.6000 risk<=0.8500" in text
    assert "5USDC first-entry PnL: current=+0.00 best=+7.42 entry>=0.6000 risk<=0.9500 entries=6/25 groups=25" in text
    assert "Last wrong: btc-updown-5m-x DOWN allow_like -> stop_loss_risk_hit" in text
