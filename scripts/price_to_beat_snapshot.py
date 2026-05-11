#!/usr/bin/env python3
"""
Anlik dinamik PTB ozeti.

Varsayilan davranis:
- `journalctl -u dextrabot -o cat` loglarindan son aktif marketi bulur.
- Secilen market icin son 3 tamamlanmis market penceresinin
  open/high/low/close ve up/down excursion ortalamasini hesaplar.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Iterable

SUPPORTED_MARKETS: dict[tuple[str, str], int] = {
    ("btc", "5m"): 300,
    ("btc", "15m"): 900,
    ("eth", "5m"): 300,
    ("eth", "15m"): 900,
    ("sol", "5m"): 300,
    ("sol", "15m"): 900,
    ("xrp", "5m"): 300,
    ("xrp", "15m"): 900,
}
AUTO_DETECT_TAIL_LINES = 1500
MAX_BOUNDARY_GAP_MS = 5_000
MARKET_RE = re.compile(r"^(btc|eth|sol|xrp)-updown-(5m|15m)-(\d+)$", re.IGNORECASE)
MARKET_FRAGMENT_RE = re.compile(r"(btc|eth|sol|xrp)-updown-(5m|15m)-(\d+)", re.IGNORECASE)
JSON_RE = re.compile(r"(\{.*\})")


@dataclass(frozen=True)
class TickSample:
    provider_timestamp_ms: int
    value: float


@dataclass(frozen=True)
class WindowStats:
    start_ts: int
    sample_count: int
    open_price: float
    high_price: float
    low_price: float
    close_price: float

    @property
    def up_excursion(self) -> float:
        return max(self.high_price - self.open_price, 0.0)

    @property
    def down_excursion(self) -> float:
        return max(self.open_price - self.low_price, 0.0)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Dextrabot journal loglarindan dinamik PTB snapshot'i hesapla."
    )
    parser.add_argument(
        "--market-slug",
        help="Auto-detect yerine belirli bir market slug kullan.",
    )
    parser.add_argument(
        "--service",
        default="dextrabot",
        help="journalctl servis adi. Varsayilan: dextrabot",
    )
    return parser.parse_args()


def load_journal(service: str, *, tail_lines: int | None = None, since: str | None = None, until: str | None = None) -> list[str]:
    cmd = ["journalctl", "-u", service, "--no-pager", "-o", "cat"]
    if tail_lines is not None:
        cmd.extend(["-n", str(tail_lines)])
    if since is not None:
        cmd.extend(["--since", since])
    if until is not None:
        cmd.extend(["--until", until])
    completed = subprocess.run(cmd, check=True, capture_output=True, text=True)
    return completed.stdout.splitlines()


def extract_json(line: str) -> dict | None:
    line = line.strip()
    if not line:
        return None
    try:
        obj = json.loads(line)
        if isinstance(obj, dict):
            return obj
    except json.JSONDecodeError:
        match = JSON_RE.search(line)
        if match:
            try:
                obj = json.loads(match.group(1))
                if isinstance(obj, dict):
                    return obj
            except json.JSONDecodeError:
                return None
    return None


def parse_market_slug(raw: str) -> tuple[str, str, int]:
    match = MARKET_RE.match(raw.strip())
    if not match:
        raise SystemExit(
            f"HATA: desteklenmeyen market slug: {raw!r}. Beklenen format: btc|eth|sol|xrp-updown-5m|15m-<start_ts>"
        )
    asset, timeframe, start_ts_raw = match.groups()
    return asset.lower(), timeframe.lower(), int(start_ts_raw)


def detect_active_market_slug(service: str) -> str:
    lines = load_journal(service, tail_lines=AUTO_DETECT_TAIL_LINES)
    latest_slug: str | None = None
    for line in lines:
        if "TRIGGER_WS_TARGET_SELECTED" not in line:
            continue
        if "resolved_market_slug" not in line:
            continue
        obj = extract_json(line)
        if obj is None:
            continue
        fields = obj.get("fields", {})
        resolved = fields.get("resolved_market_slug")
        if not isinstance(resolved, str):
            continue
        match = MARKET_FRAGMENT_RE.search(resolved)
        if match:
            latest_slug = match.group(0)
    if latest_slug is None:
        raise SystemExit(
            f"HATA: {service} journal loglarinda aktif supported market bulunamadi."
        )
    return latest_slug


def journal_time(ts_seconds: int) -> str:
    return datetime.fromtimestamp(ts_seconds, tz=timezone.utc).strftime("%Y-%m-%d %H:%M:%S")


def iso_time(ts_seconds: int) -> str:
    return datetime.fromtimestamp(ts_seconds, tz=timezone.utc).isoformat()


def collect_tick_samples(service: str, asset: str, start_ts: int, end_ts: int) -> list[TickSample]:
    symbol = f"{asset}/usd"
    lines = load_journal(
        service,
        since=journal_time(start_ts),
        until=journal_time(end_ts),
    )
    samples: list[TickSample] = []
    for line in lines:
        if "CHAINLINK_LIVE_DATA_WS_TICK" not in line or f'"symbol":"{symbol}"' not in line:
            continue
        obj = extract_json(line)
        if obj is None:
            continue
        fields = obj.get("fields", {})
        try:
            provider_timestamp_ms = int(fields["provider_timestamp_ms"])
            value = float(fields["value"])
        except (KeyError, TypeError, ValueError):
            continue
        if start_ts * 1000 <= provider_timestamp_ms < end_ts * 1000:
            samples.append(TickSample(provider_timestamp_ms=provider_timestamp_ms, value=value))
    samples.sort(key=lambda item: item.provider_timestamp_ms)
    return samples


def validate_window_coverage(samples: list[TickSample], start_ts: int, end_ts: int) -> None:
    if not samples:
        raise SystemExit(
            f"HATA: {iso_time(start_ts)} penceresi icin hic tick bulunamadi."
        )
    start_gap_ms = samples[0].provider_timestamp_ms - start_ts * 1000
    end_gap_ms = end_ts * 1000 - samples[-1].provider_timestamp_ms
    if start_gap_ms > MAX_BOUNDARY_GAP_MS:
        raise SystemExit(
            f"HATA: {iso_time(start_ts)} penceresinde open coverage eksik; ilk tick {start_gap_ms}ms gec geliyor."
        )
    if end_gap_ms > MAX_BOUNDARY_GAP_MS:
        raise SystemExit(
            f"HATA: {iso_time(start_ts)} penceresinde close coverage eksik; son tick pencere sonundan {end_gap_ms}ms once kaliyor."
        )


def build_window_stats(start_ts: int, samples: list[TickSample]) -> WindowStats:
    validate_window_coverage(samples, start_ts, start_ts + infer_window_secs(start_ts, samples))
    values = [sample.value for sample in samples]
    return WindowStats(
        start_ts=start_ts,
        sample_count=len(samples),
        open_price=samples[0].value,
        high_price=max(values),
        low_price=min(values),
        close_price=samples[-1].value,
    )


def infer_window_secs(start_ts: int, samples: list[TickSample]) -> int:
    if len(samples) < 2:
        raise SystemExit(
            f"HATA: {iso_time(start_ts)} penceresinde yeterli tick yok; en az 2 tick gerekli."
        )
    return max(1, (samples[-1].provider_timestamp_ms - samples[0].provider_timestamp_ms) // 1000)


def calculate_window_stats(service: str, asset: str, timeframe: str, current_start_ts: int) -> list[WindowStats]:
    window_secs = SUPPORTED_MARKETS[(asset, timeframe)]
    stats: list[WindowStats] = []
    for offset in (3, 2, 1):
        start_ts = current_start_ts - window_secs * offset
        end_ts = start_ts + window_secs
        samples = collect_tick_samples(service, asset, start_ts, end_ts)
        validate_window_coverage(samples, start_ts, end_ts)
        values = [sample.value for sample in samples]
        stats.append(
            WindowStats(
                start_ts=start_ts,
                sample_count=len(samples),
                open_price=samples[0].value,
                high_price=max(values),
                low_price=min(values),
                close_price=samples[-1].value,
            )
        )
    return stats


def avg(values: Iterable[float]) -> float:
    values = list(values)
    if not values:
        raise SystemExit("HATA: ortalama icin bos deger listesi geldi.")
    return sum(values) / len(values)


def print_snapshot(market_slug: str, window_stats: list[WindowStats]) -> None:
    asset, timeframe, current_start_ts = parse_market_slug(market_slug)
    avg_up = avg(item.up_excursion for item in window_stats)
    avg_down = avg(item.down_excursion for item in window_stats)
    previous_close = window_stats[-1].close_price
    effective_up = previous_close + avg_up
    effective_down = previous_close - avg_down

    print(f"Market: {market_slug}")
    print(f"Pencere Baslangici: {iso_time(current_start_ts)}")
    print(f"Previous Close: {previous_close:.8f}")
    print(f"Up Threshold: {avg_up:.8f} USD")
    print(f"Down Threshold: {avg_down:.8f} USD")
    print(f"Efektif Up Seviye: {effective_up:.8f}")
    print(f"Efektif Down Seviye: {effective_down:.8f}")
    print("")
    print("Son 3 Pencere")
    for item in window_stats:
        print(
            f"- {iso_time(item.start_ts)} | O {item.open_price:.8f} | H {item.high_price:.8f} | "
            f"L {item.low_price:.8f} | C {item.close_price:.8f} | "
            f"Up {item.up_excursion:.8f} | Down {item.down_excursion:.8f} | samples {item.sample_count}"
        )


def main() -> None:
    args = parse_args()
    market_slug = args.market_slug or detect_active_market_slug(args.service)
    asset, timeframe, current_start_ts = parse_market_slug(market_slug)
    if (asset, timeframe) not in SUPPORTED_MARKETS:
        raise SystemExit(
            f"HATA: desteklenmeyen market kombinasyonu: asset={asset}, timeframe={timeframe}"
        )
    stats = calculate_window_stats(args.service, asset, timeframe, current_start_ts)
    print_snapshot(market_slug, stats)


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as exc:
        sys.exit(f"HATA: journalctl komutu basarisiz oldu: {exc}")
