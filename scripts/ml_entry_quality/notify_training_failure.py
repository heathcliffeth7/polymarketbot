#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os

from telegram_notify import scheduled_training_systemd_failure_message, send_telegram_message


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Send a Telegram alert when scheduled ML training fails in systemd.")
    parser.add_argument("--database-url", default=os.getenv("DATABASE_URL"))
    parser.add_argument("--telegram-user-id", type=int, default=int(os.getenv("ML_ENTRY_QUALITY_TELEGRAM_USER_ID", "1")))
    parser.add_argument("--unit", default="dextrabot-ml-entry-quality-train.service")
    parser.add_argument("--reason", default="systemd unit failed before trainer could report")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    result = send_telegram_message(
        scheduled_training_systemd_failure_message(args.unit, args.reason),
        enabled=True,
        database_url=args.database_url,
        user_id=args.telegram_user_id,
    )
    print(f"telegram: {result.reason}")


if __name__ == "__main__":
    main()
