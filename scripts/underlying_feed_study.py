#!/usr/bin/env python3
"""
30 dk underlying feed study: Chainlink, Binance, Hyperliquid spot/perp.

Ornek:
  python3 -m venv .venv-feed-study && .venv-feed-study/bin/pip install websockets requests
  .venv-feed-study/bin/python scripts/underlying_feed_study.py --duration-min 30 --asset btc
"""

from __future__ import annotations

import argparse
import asyncio
import csv
import json
import math
import os
import statistics
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    import requests
    import websockets
except ImportError as exc:
    sys.exit(f"HATA: pip install websockets requests — {exc}")

POLYMARKET_WS = os.environ.get(
    "POLYMARKET_LIVE_DATA_WS_URL", "wss://ws-live-data.polymarket.com"
)
BINANCE_WS_BASE = os.environ.get(
    "EARLY_STALE_BINANCE_WS_URL", "wss://stream.binance.com:9443/stream"
).rstrip("/")
HYPERLIQUID_WS = os.environ.get(
    "EARLY_STALE_HYPERLIQUID_WS_URL", "wss://api.hyperliquid.xyz/ws"
)
HL_INFO_URL = "https://api.hyperliquid.xyz/info"

FLAT_USD = 0.01
BASELINE_SECONDS = 60
CROSS_CORR_LAGS = list(range(-5, 6))
SIGNIFICANT_D_CL = (1.0, 3.0, 5.0)

ASSET_SYMBOL = {
    "btc": ("btc/usd", "btcusdt"),
    "eth": ("eth/usd", "ethusdt"),
    "sol": ("sol/usd", "solusdt"),
    "xrp": ("xrp/usd", "xrpusdt"),
}
HL_PERP_COIN = {"btc": "BTC", "eth": "ETH", "sol": "SOL", "xrp": "XRP"}


@dataclass
class FeedTick:
    price: float
    received_at_ms: int
    provider_ts_ms: int | None = None


@dataclass
class StudyState:
    asset: str
    chainlink_symbol: str
    binance_symbol: str
    hl_spot_coin: str
    lock: asyncio.Lock = field(default_factory=asyncio.Lock)
    chainlink: FeedTick | None = None
    binance: FeedTick | None = None
    hl_spot: FeedTick | None = None
    hl_perp: FeedTick | None = None
    last_logged: dict[str, float] = field(default_factory=dict)

    async def update(
        self,
        feed: str,
        price: float,
        received_at_ms: int,
        provider_ts_ms: int | None,
        log_events: bool,
        events_writer: csv.DictWriter | None,
    ) -> None:
        tick = FeedTick(price=price, received_at_ms=received_at_ms, provider_ts_ms=provider_ts_ms)
        async with self.lock:
            if feed == "chainlink":
                self.chainlink = tick
            elif feed == "binance":
                self.binance = tick
            elif feed == "hl_spot":
                self.hl_spot = tick
            elif feed == "hl_perp":
                self.hl_perp = tick
            prev = self.last_logged.get(feed)
            if log_events and events_writer is not None and (
                prev is None or abs(price - prev) >= 1e-9
            ):
                events_writer.writerow(
                    {
                        "received_at_ms": received_at_ms,
                        "provider_ts_ms": provider_ts_ms if provider_ts_ms is not None else "",
                        "feed": feed,
                        "price": price,
                    }
                )
                self.last_logged[feed] = price

    async def snapshot(self) -> dict[str, Any]:
        async with self.lock:
            cl = self.chainlink
            bn = self.binance
            sp = self.hl_spot
            pp = self.hl_perp
        return {
            "chainlink": cl.price if cl else None,
            "binance_mid": bn.price if bn else None,
            "hl_spot_mid": sp.price if sp else None,
            "hl_perp_mid": pp.price if pp else None,
            "cl_ok": cl is not None,
            "binance_ok": bn is not None,
            "perp_ok": pp is not None,
            "spot_ok": sp is not None,
        }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Underlying feed study (CL/BN/HL)")
    parser.add_argument("--duration-min", type=float, default=30.0)
    parser.add_argument("--interval-ms", type=int, default=1000)
    parser.add_argument("--asset", default="btc", choices=sorted(ASSET_SYMBOL))
    parser.add_argument("--output-dir", default="")
    parser.add_argument("--log-events", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--baseline-sec", type=int, default=BASELINE_SECONDS)
    return parser.parse_args()


def resolve_output_dir(arg: str) -> Path:
    if arg:
        out = Path(arg)
    else:
        stamp = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
        out = Path(__file__).resolve().parent.parent / "analysis" / f"feed_study_{stamp}"
    out.mkdir(parents=True, exist_ok=True)
    return out


def resolve_hl_spot_coin(asset: str) -> str:
    override = os.environ.get("HL_SPOT_COIN", "").strip()
    if override:
        return override
    resp = requests.post(HL_INFO_URL, json={"type": "spotMeta"}, timeout=15)
    resp.raise_for_status()
    meta = resp.json()
    token_names = {
        int(t["index"]): str(t.get("name", ""))
        for t in meta.get("tokens", [])
        if t.get("index") is not None
    }
    candidates: list[tuple[str, str]] = []
    asset_key = asset.lower()
    for entry in meta.get("universe", []):
        idx = entry.get("index")
        pair_tokens = entry.get("tokens") or []
        if idx is None or len(pair_tokens) < 2:
            continue
        base = token_names.get(int(pair_tokens[0]), "")
        quote = token_names.get(int(pair_tokens[1]), "")
        label = f"{base}/{quote}".upper()
        coin = f"@{idx}"
        if asset_key == "btc" and ("UBTC" in label or "BTC" in label) and "USDC" in label:
            candidates.append((coin, label))
        elif asset_key == "eth" and "ETH" in label and "USDC" in label:
            candidates.append((coin, label))
        elif asset_key == "sol" and "SOL" in label and "USDC" in label:
            candidates.append((coin, label))
        elif asset_key == "xrp" and "XRP" in label and "USDC" in label:
            candidates.append((coin, label))
    if not candidates:
        raise RuntimeError(f"HL spot pair not found for asset={asset}; set HL_SPOT_COIN")
    # Prefer USDC quote over USDH etc.
    candidates.sort(key=lambda x: (0 if x[1].endswith("/USDC") else 1, x[1]))
    coin, label = candidates[0]
    print(f"HL spot resolved: {coin} ({label})", flush=True)
    return coin


def mid(bid: float, ask: float) -> float:
    return (bid + ask) / 2.0


def parse_f64(value: Any) -> float | None:
    if value is None:
        return None
    try:
        out = float(value)
    except (TypeError, ValueError):
        return None
    return out if math.isfinite(out) and out > 0 else None


def parse_i64(value: Any) -> int | None:
    if value is None:
        return None
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def direction_delta(delta: float | None) -> int:
    if delta is None:
        return 0
    if abs(delta) < FLAT_USD:
        return 0
    return 1 if delta > 0 else -1


def pearson(xs: list[float], ys: list[float]) -> float | None:
    n = min(len(xs), len(ys))
    if n < 3:
        return None
    mx = statistics.mean(xs[:n])
    my = statistics.mean(ys[:n])
    num = sum((xs[i] - mx) * (ys[i] - my) for i in range(n))
    den_x = math.sqrt(sum((xs[i] - mx) ** 2 for i in range(n)))
    den_y = math.sqrt(sum((ys[i] - my) ** 2 for i in range(n)))
    if den_x == 0 or den_y == 0:
        return None
    return num / (den_x * den_y)


def percentile(values: list[float], pct: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    k = (len(ordered) - 1) * (pct / 100.0)
    f = math.floor(k)
    c = math.ceil(k)
    if f == c:
        return ordered[int(k)]
    return ordered[f] * (c - k) + ordered[c] * (k - f)


def gap_stats(values: list[float]) -> dict[str, float | None]:
    if not values:
        return {}
    return {
        "count": float(len(values)),
        "mean": statistics.mean(values),
        "median": statistics.median(values),
        "std": statistics.pstdev(values) if len(values) > 1 else 0.0,
        "p5": percentile(values, 5),
        "p95": percentile(values, 95),
        "min": min(values),
        "max": max(values),
    }


def cross_corr_best_lag(
    base: list[float], other: list[float], lags: list[int]
) -> dict[str, Any]:
    best_lag = 0
    best_corr: float | None = None
    results: dict[str, float | None] = {}
    for lag in lags:
        if lag >= 0:
            xs = other[lag:]
            ys = base[: len(xs)]
        else:
            xs = other[: len(other) + lag]
            ys = base[-lag:]
        n = min(len(xs), len(ys))
        if n < 5:
            results[str(lag)] = None
            continue
        c = pearson(xs[:n], ys[:n])
        results[str(lag)] = c
        if c is not None and (best_corr is None or abs(c) > abs(best_corr)):
            best_corr = c
            best_lag = lag
    return {"best_lag_sec": best_lag, "best_corr": best_corr, "by_lag": results}


async def chainlink_loop(
    state: StudyState,
    stop: asyncio.Event,
    log_events: bool,
    events_writer: csv.DictWriter | None,
) -> None:
    sub = json.dumps(
        {
            "action": "subscribe",
            "subscriptions": [
                {"topic": "crypto_prices_chainlink", "type": "*", "filters": ""}
            ],
        }
    )
    while not stop.is_set():
        try:
            async with websockets.connect(POLYMARKET_WS, ping_interval=20) as ws:
                await ws.send(sub)
                while not stop.is_set():
                    try:
                        raw = await asyncio.wait_for(ws.recv(), timeout=20)
                    except asyncio.TimeoutError:
                        await ws.send("PING")
                        continue
                    if not raw or raw == "PONG":
                        continue
                    try:
                        msg = json.loads(raw)
                    except json.JSONDecodeError:
                        continue
                    if msg.get("topic") != "crypto_prices_chainlink":
                        continue
                    if msg.get("type") not in (None, "update"):
                        continue
                    payload = msg.get("payload") or {}
                    symbol = str(payload.get("symbol", "")).lower()
                    if symbol != state.chainlink_symbol:
                        continue
                    price = parse_f64(payload.get("value"))
                    if price is None:
                        continue
                    ts = parse_i64(payload.get("timestamp")) or int(time.time() * 1000)
                    now_ms = int(time.time() * 1000)
                    await state.update(
                        "chainlink", price, now_ms, ts, log_events, events_writer
                    )
        except Exception as exc:
            print(f"[chainlink] reconnect: {exc}", flush=True)
            await asyncio.sleep(2)


async def binance_loop(
    state: StudyState, stop: asyncio.Event, log_events: bool, events_writer: csv.DictWriter | None
) -> None:
    symbol = state.binance_symbol
    url = f"{BINANCE_WS_BASE}?streams={symbol}@bookTicker/{symbol}@depth5@100ms"
    while not stop.is_set():
        try:
            async with websockets.connect(url, ping_interval=20) as ws:
                while not stop.is_set():
                    raw = await asyncio.wait_for(ws.recv(), timeout=30)
                    msg = json.loads(raw)
                    data = msg.get("data") or msg
                    stream = str(msg.get("stream", ""))
                    now_ms = int(time.time() * 1000)
                    price = None
                    provider_ts = parse_i64(data.get("E"))
                    if "bookTicker" in stream or data.get("b") and data.get("a"):
                        bid = parse_f64(data.get("b"))
                        ask = parse_f64(data.get("a"))
                        if bid and ask:
                            price = mid(bid, ask)
                    elif "depth" in stream:
                        bids = data.get("b") or data.get("bids") or []
                        asks = data.get("a") or data.get("asks") or []
                        best_bid = max(
                            (parse_f64(row[0]) for row in bids if isinstance(row, list) and row),
                            default=None,
                        )
                        best_ask = min(
                            (parse_f64(row[0]) for row in asks if isinstance(row, list) and row),
                            default=None,
                        )
                        if best_bid and best_ask:
                            price = mid(best_bid, best_ask)
                    if price is not None:
                        await state.update(
                            "binance", price, now_ms, provider_ts, log_events, events_writer
                        )
        except Exception as exc:
            print(f"[binance] reconnect: {exc}", flush=True)
            await asyncio.sleep(2)


def parse_hl_book(payload: dict) -> float | None:
    levels = payload.get("levels")
    if not isinstance(levels, list) or len(levels) < 2:
        return None
    bids, asks = levels[0], levels[1]

    def level_px(row: Any) -> float | None:
        if isinstance(row, dict):
            return parse_f64(row.get("px") or row.get("price"))
        if isinstance(row, list) and row:
            return parse_f64(row[0])
        return None

    bid_prices = [p for p in (level_px(r) for r in bids) if p]
    ask_prices = [p for p in (level_px(r) for r in asks) if p]
    if not bid_prices or not ask_prices:
        return None
    return mid(max(bid_prices), min(ask_prices))


async def hyperliquid_loop(
    state: StudyState,
    stop: asyncio.Event,
    log_events: bool,
    events_writer: csv.DictWriter | None,
) -> None:
    perp_coin = HL_PERP_COIN.get(state.asset, state.asset.upper())
    perp_sub = json.dumps(
        {"method": "subscribe", "subscription": {"type": "l2Book", "coin": perp_coin}}
    )
    spot_sub = json.dumps(
        {
            "method": "subscribe",
            "subscription": {"type": "l2Book", "coin": state.hl_spot_coin},
        }
    )
    coin_map = {perp_coin: "hl_perp", state.hl_spot_coin: "hl_spot"}

    while not stop.is_set():
        try:
            async with websockets.connect(HYPERLIQUID_WS, ping_interval=20) as ws:
                await ws.send(perp_sub)
                await ws.send(spot_sub)
                while not stop.is_set():
                    raw = await asyncio.wait_for(ws.recv(), timeout=30)
                    msg = json.loads(raw)
                    channel = msg.get("channel")
                    data = msg.get("data") or msg
                    if channel != "l2Book" and data.get("channel") != "l2Book":
                        continue
                    if channel == "l2Book":
                        data = msg.get("data") or {}
                    coin = str(data.get("coin", ""))
                    feed = coin_map.get(coin)
                    if not feed:
                        continue
                    price = parse_hl_book(data)
                    if price is None:
                        continue
                    now_ms = int(time.time() * 1000)
                    provider_ts = parse_i64(data.get("time")) or now_ms
                    await state.update(
                        feed, price, now_ms, provider_ts, log_events, events_writer
                    )
        except Exception as exc:
            print(f"[hyperliquid] reconnect: {exc}", flush=True)
            await asyncio.sleep(2)


def write_csv_headers(path: Path, fieldnames: list[str]) -> csv.DictWriter:
    fh = path.open("w", newline="", encoding="utf-8")
    writer = csv.DictWriter(fh, fieldnames=fieldnames)
    writer.writeheader()
    writer._fh = fh  # type: ignore[attr-defined]
    return writer


def close_writer(writer: csv.DictWriter) -> None:
    fh = getattr(writer, "_fh", None)
    if fh:
        fh.close()


async def run_collection(
    state: StudyState,
    out_dir: Path,
    duration_sec: float,
    interval_ms: int,
    log_events: bool,
) -> list[dict[str, Any]]:
    stop = asyncio.Event()
    ticks_path = out_dir / "ticks.csv"
    events_path = out_dir / "events.csv"
    direction_path = out_dir / "direction_report.csv"
    spread_path = out_dir / "spread_report.csv"

    tick_fields = [
        "ts_ms",
        "second_index",
        "chainlink",
        "binance_mid",
        "hl_spot_mid",
        "hl_perp_mid",
        "gap_spot_cl",
        "gap_perp_cl",
        "gap_binance_cl",
        "gap_perp_spot",
        "gap_binance_spot",
        "gap_binance_perp",
        "d_cl",
        "d_binance",
        "d_spot",
        "d_perp",
        "dir_cl",
        "dir_binance",
        "dir_spot",
        "dir_perp",
        "velocity_lead_spot",
        "velocity_lead_perp",
        "velocity_lead_binance",
        "cl_ok",
        "binance_ok",
        "perp_ok",
        "spot_ok",
    ]
    direction_fields = [
        "ts_ms",
        "d_cl",
        "d_binance",
        "d_spot",
        "d_perp",
        "binance_agrees_cl",
        "spot_agrees_cl",
        "perp_agrees_cl",
        "spot_agrees_binance",
        "perp_agrees_binance",
        "spot_leads_cl",
        "perp_leads_cl",
        "binance_leads_cl",
        "conflict_spot_vs_cl",
        "conflict_perp_vs_cl",
        "conflict_binance_vs_cl",
    ]
    spread_fields = [
        "ts_ms",
        "gap_spot_cl",
        "gap_perp_cl",
        "gap_binance_cl",
        "gap_perp_spot",
        "gap_binance_spot",
        "gap_binance_perp",
        "excess_spot_cl",
        "excess_perp_cl",
        "excess_binance_cl",
    ]
    event_fields = ["received_at_ms", "provider_ts_ms", "feed", "price"]

    ticks_writer = write_csv_headers(ticks_path, tick_fields)
    direction_writer = write_csv_headers(direction_path, direction_fields)
    spread_writer = write_csv_headers(spread_path, spread_fields)
    events_writer = (
        write_csv_headers(events_path, event_fields) if log_events else None
    )

    tasks = [
        asyncio.create_task(chainlink_loop(state, stop, log_events, events_writer)),
        asyncio.create_task(
            binance_loop(state, stop, log_events, events_writer)
        ),
        asyncio.create_task(
            hyperliquid_loop(state, stop, log_events, events_writer)
        ),
    ]

    rows: list[dict[str, Any]] = []
    prev: dict[str, float | None] = {
        "chainlink": None,
        "binance_mid": None,
        "hl_spot_mid": None,
        "hl_perp_mid": None,
    }
    baselines: dict[str, float | None] = {
        "spot_cl": None,
        "perp_cl": None,
        "binance_cl": None,
    }
    baseline_samples: dict[str, list[float]] = {
        "spot_cl": [],
        "perp_cl": [],
        "binance_cl": [],
    }

    started = time.monotonic()
    target_samples = int(duration_sec * 1000 / interval_ms)
    print(
        f"Collecting {duration_sec:.0f}s @ {interval_ms}ms (~{target_samples} samples)...",
        flush=True,
    )

    second_index = 0
    try:
        while time.monotonic() - started < duration_sec:
            loop_start = time.monotonic()
            now_ms = int(time.time() * 1000)
            snap = await state.snapshot()
            cl = snap["chainlink"]
            bn = snap["binance_mid"]
            sp = snap["hl_spot_mid"]
            pp = snap["hl_perp_mid"]

            def gap(a: float | None, b: float | None) -> float | None:
                if a is None or b is None:
                    return None
                return a - b

            g_spot_cl = gap(sp, cl)
            g_perp_cl = gap(pp, cl)
            g_binance_cl = gap(bn, cl)
            g_perp_spot = gap(pp, sp)
            g_binance_spot = gap(bn, sp)
            g_binance_perp = gap(bn, pp)

            if second_index < BASELINE_SECONDS:
                if g_spot_cl is not None:
                    baseline_samples["spot_cl"].append(g_spot_cl)
                if g_perp_cl is not None:
                    baseline_samples["perp_cl"].append(g_perp_cl)
                if g_binance_cl is not None:
                    baseline_samples["binance_cl"].append(g_binance_cl)

            if baselines["spot_cl"] is None and len(baseline_samples["spot_cl"]) >= 10:
                baselines["spot_cl"] = statistics.median(baseline_samples["spot_cl"])
                baselines["perp_cl"] = statistics.median(baseline_samples["perp_cl"])
                baselines["binance_cl"] = statistics.median(baseline_samples["binance_cl"])

            def excess(g: float | None, key: str) -> float | None:
                if g is None or baselines[key] is None:
                    return None
                return g - baselines[key]

            d_cl = (cl - prev["chainlink"]) if cl is not None and prev["chainlink"] is not None else None
            d_bn = (bn - prev["binance_mid"]) if bn is not None and prev["binance_mid"] is not None else None
            d_sp = (sp - prev["hl_spot_mid"]) if sp is not None and prev["hl_spot_mid"] is not None else None
            d_pp = (pp - prev["hl_perp_mid"]) if pp is not None and prev["hl_perp_mid"] is not None else None

            dir_cl = direction_delta(d_cl)
            dir_bn = direction_delta(d_bn)
            dir_sp = direction_delta(d_sp)
            dir_pp = direction_delta(d_pp)

            v_spot = (d_sp - d_cl) if d_sp is not None and d_cl is not None else None
            v_perp = (d_pp - d_cl) if d_pp is not None and d_cl is not None else None
            v_bn = (d_bn - d_cl) if d_bn is not None and d_cl is not None else None

            def agrees(a: int, b: int) -> bool:
                return a != 0 and b != 0 and a == b

            def leads(d_other: float | None, d_base: float | None) -> bool:
                if d_other is None or d_base is None:
                    return False
                if direction_delta(d_other) != direction_delta(d_base) or direction_delta(d_base) == 0:
                    return False
                return abs(d_other) > abs(d_base)

            def conflict(a: int, b: int) -> bool:
                return a != 0 and b != 0 and a != b

            row = {
                "ts_ms": now_ms,
                "second_index": second_index,
                "chainlink": cl if cl is not None else "",
                "binance_mid": bn if bn is not None else "",
                "hl_spot_mid": sp if sp is not None else "",
                "hl_perp_mid": pp if pp is not None else "",
                "gap_spot_cl": g_spot_cl if g_spot_cl is not None else "",
                "gap_perp_cl": g_perp_cl if g_perp_cl is not None else "",
                "gap_binance_cl": g_binance_cl if g_binance_cl is not None else "",
                "gap_perp_spot": g_perp_spot if g_perp_spot is not None else "",
                "gap_binance_spot": g_binance_spot if g_binance_spot is not None else "",
                "gap_binance_perp": g_binance_perp if g_binance_perp is not None else "",
                "d_cl": d_cl if d_cl is not None else "",
                "d_binance": d_bn if d_bn is not None else "",
                "d_spot": d_sp if d_sp is not None else "",
                "d_perp": d_pp if d_pp is not None else "",
                "dir_cl": dir_cl,
                "dir_binance": dir_bn,
                "dir_spot": dir_sp,
                "dir_perp": dir_pp,
                "velocity_lead_spot": v_spot if v_spot is not None else "",
                "velocity_lead_perp": v_perp if v_perp is not None else "",
                "velocity_lead_binance": v_bn if v_bn is not None else "",
                "cl_ok": int(snap["cl_ok"]),
                "binance_ok": int(snap["binance_ok"]),
                "perp_ok": int(snap["perp_ok"]),
                "spot_ok": int(snap["spot_ok"]),
            }
            ticks_writer.writerow(row)
            rows.append(row)

            direction_writer.writerow(
                {
                    "ts_ms": now_ms,
                    "d_cl": d_cl if d_cl is not None else "",
                    "d_binance": d_bn if d_bn is not None else "",
                    "d_spot": d_sp if d_sp is not None else "",
                    "d_perp": d_pp if d_pp is not None else "",
                    "binance_agrees_cl": int(agrees(dir_bn, dir_cl)),
                    "spot_agrees_cl": int(agrees(dir_sp, dir_cl)),
                    "perp_agrees_cl": int(agrees(dir_pp, dir_cl)),
                    "spot_agrees_binance": int(agrees(dir_sp, dir_bn)),
                    "perp_agrees_binance": int(agrees(dir_pp, dir_bn)),
                    "spot_leads_cl": int(leads(d_sp, d_cl)),
                    "perp_leads_cl": int(leads(d_pp, d_cl)),
                    "binance_leads_cl": int(leads(d_bn, d_cl)),
                    "conflict_spot_vs_cl": int(conflict(dir_sp, dir_cl)),
                    "conflict_perp_vs_cl": int(conflict(dir_pp, dir_cl)),
                    "conflict_binance_vs_cl": int(conflict(dir_bn, dir_cl)),
                }
            )
            spread_writer.writerow(
                {
                    "ts_ms": now_ms,
                    "gap_spot_cl": g_spot_cl if g_spot_cl is not None else "",
                    "gap_perp_cl": g_perp_cl if g_perp_cl is not None else "",
                    "gap_binance_cl": g_binance_cl if g_binance_cl is not None else "",
                    "gap_perp_spot": g_perp_spot if g_perp_spot is not None else "",
                    "gap_binance_spot": g_binance_spot if g_binance_spot is not None else "",
                    "gap_binance_perp": g_binance_perp if g_binance_perp is not None else "",
                    "excess_spot_cl": excess(g_spot_cl, "spot_cl") or "",
                    "excess_perp_cl": excess(g_perp_cl, "perp_cl") or "",
                    "excess_binance_cl": excess(g_binance_cl, "binance_cl") or "",
                }
            )

            prev = {
                "chainlink": cl,
                "binance_mid": bn,
                "hl_spot_mid": sp,
                "hl_perp_mid": pp,
            }
            second_index += 1

            elapsed = time.monotonic() - loop_start
            sleep_s = max(0.0, interval_ms / 1000.0 - elapsed)
            await asyncio.sleep(sleep_s)
    finally:
        stop.set()
        for t in tasks:
            t.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        close_writer(ticks_writer)
        close_writer(direction_writer)
        close_writer(spread_writer)
        if events_writer:
            close_writer(events_writer)

    meta = {
        "duration_sec": duration_sec,
        "interval_ms": interval_ms,
        "samples_written": second_index,
        "target_samples": target_samples,
        "baselines": baselines,
    }
    (out_dir / "run_meta.json").write_text(json.dumps(meta, indent=2), encoding="utf-8")
    return rows


def load_events(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    with path.open(encoding="utf-8") as fh:
        return list(csv.DictReader(fh))


def analyze_direction(rows: list[dict[str, Any]]) -> dict[str, Any]:
    def parse_bool_row(key: str) -> list[bool]:
        out = []
        for r in rows:
            v = r.get(key, "")
            if v == "":
                continue
            out.append(bool(int(v)))
        return out

    def agreement_pct(key: str, mask: list[bool] | None = None) -> float | None:
        vals = []
        for i, r in enumerate(rows):
            v = r.get(key, "")
            if v == "":
                continue
            if mask is not None and i < len(mask) and not mask[i]:
                continue
            vals.append(bool(int(v)))
        if not vals:
            return None
        return 100.0 * sum(vals) / len(vals)

    significant_masks = {}
    for thresh in SIGNIFICANT_D_CL:
        mask = []
        for r in rows:
            d = r.get("d_cl", "")
            if d == "":
                mask.append(False)
            else:
                mask.append(abs(float(d)) >= thresh)
        significant_masks[str(thresh)] = mask

    result = {
        "samples": len(rows),
        "agreement_binance_cl_pct": agreement_pct("binance_agrees_cl"),
        "agreement_spot_cl_pct": agreement_pct("spot_agrees_cl"),
        "agreement_perp_cl_pct": agreement_pct("perp_agrees_cl"),
        "agreement_spot_binance_pct": agreement_pct("spot_agrees_binance"),
        "agreement_perp_binance_pct": agreement_pct("perp_agrees_binance"),
        "spot_leads_cl_pct": agreement_pct("spot_leads_cl"),
        "perp_leads_cl_pct": agreement_pct("perp_leads_cl"),
        "binance_leads_cl_pct": agreement_pct("binance_leads_cl"),
        "conflict_spot_cl_pct": agreement_pct("conflict_spot_vs_cl"),
        "conflict_perp_cl_pct": agreement_pct("conflict_perp_vs_cl"),
        "conflict_binance_cl_pct": agreement_pct("conflict_binance_vs_cl"),
        "by_significant_d_cl": {},
    }
    for thresh, mask in significant_masks.items():
        result["by_significant_d_cl"][thresh] = {
            "agreement_spot_cl_pct": agreement_pct("spot_agrees_cl", mask),
            "agreement_perp_cl_pct": agreement_pct("perp_agrees_cl", mask),
            "agreement_binance_cl_pct": agreement_pct("binance_agrees_cl", mask),
            "samples": sum(mask),
        }
    agreements = [
        ("binance", result["agreement_binance_cl_pct"]),
        ("spot", result["agreement_spot_cl_pct"]),
        ("perp", result["agreement_perp_cl_pct"]),
    ]
    agreements = [(k, v) for k, v in agreements if v is not None]
    if agreements:
        best = max(agreements, key=lambda x: x[1])
        result["best_agreement_vs_cl"] = {"feed": best[0], "pct": best[1]}
    return result


def load_csv_rows(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    with path.open(encoding="utf-8") as fh:
        return list(csv.DictReader(fh))


def analyze_spread(rows: list[dict[str, Any]]) -> dict[str, Any]:
    col_map = {
        "spot_cl": "gap_spot_cl",
        "perp_cl": "gap_perp_cl",
        "binance_cl": "gap_binance_cl",
        "perp_spot": "gap_perp_spot",
        "binance_spot": "gap_binance_spot",
        "binance_perp": "gap_binance_perp",
        "excess_spot_cl": "excess_spot_cl",
        "excess_perp_cl": "excess_perp_cl",
        "excess_binance_cl": "excess_binance_cl",
    }
    gaps: dict[str, list[float]] = {k: [] for k in col_map}
    for r in rows:
        for key, col in col_map.items():
            v = r.get(col, "")
            if v != "":
                try:
                    gaps[key].append(float(v))
                except ValueError:
                    pass
    stats = {k: gap_stats(v) for k, v in gaps.items() if v}
    medians = {k: s.get("median") for k, s in stats.items() if s}
    cl_pairs = [
        ("binance_cl", medians.get("binance_cl")),
        ("spot_cl", medians.get("spot_cl")),
        ("perp_cl", medians.get("perp_cl")),
    ]
    cl_pairs = [(k, abs(v)) for k, v in cl_pairs if v is not None]
    if cl_pairs:
        tightest = min(cl_pairs, key=lambda x: x[1])
        stats["tightest_vs_cl_median_abs"] = {"pair": tightest[0], "abs_median": tightest[1]}
    return stats


def analyze_lead_lag(rows: list[dict[str, Any]], events: list[dict[str, Any]]) -> dict[str, Any]:
    d_cl, d_spot, d_perp, d_bn = [], [], [], []
    v_spot, v_perp, v_bn = [], [], []
    for r in rows:
        for key, dest in [
            ("d_cl", d_cl),
            ("d_spot", d_spot),
            ("d_perp", d_perp),
            ("d_binance", d_bn),
        ]:
            v = r.get(key, "")
            if v != "":
                dest.append(float(v))
        for key, dest in [
            ("velocity_lead_spot", v_spot),
            ("velocity_lead_perp", v_perp),
            ("velocity_lead_binance", v_bn),
        ]:
            v = r.get(key, "")
            if v != "":
                dest.append(float(v))

    n = min(len(d_cl), len(d_spot), len(d_perp), len(d_bn))
    result: dict[str, Any] = {
        "pearson": {
            "spot_vs_cl": pearson(d_spot[:n], d_cl[:n]),
            "perp_vs_cl": pearson(d_perp[:n], d_cl[:n]),
            "binance_vs_cl": pearson(d_bn[:n], d_cl[:n]),
        },
        "cross_corr_lag": {
            "spot_vs_cl": cross_corr_best_lag(d_cl, d_spot, CROSS_CORR_LAGS),
            "perp_vs_cl": cross_corr_best_lag(d_cl, d_perp, CROSS_CORR_LAGS),
            "binance_vs_cl": cross_corr_best_lag(d_cl, d_bn, CROSS_CORR_LAGS),
        },
        "velocity_lead": {
            "spot": gap_stats(v_spot),
            "perp": gap_stats(v_perp),
            "binance": gap_stats(v_bn),
        },
    }
    if v_spot:
        result["velocity_lead"]["spot"]["pct_bullish_gt4"] = (
            100.0 * sum(1 for x in v_spot if x > 4) / len(v_spot)
        )
        result["velocity_lead"]["spot"]["pct_bearish_lt_neg4"] = (
            100.0 * sum(1 for x in v_spot if x < -4) / len(v_spot)
        )

    # Event-based lead: median ms from feed event to next CL event (same sign move)
    by_feed: dict[str, list[dict]] = {}
    for ev in events:
        feed = ev.get("feed", "")
        by_feed.setdefault(feed, []).append(ev)
    for feed in by_feed:
        by_feed[feed].sort(key=lambda e: int(e["received_at_ms"]))

    def median_lead_ms(leader: str) -> float | None:
        leader_events = by_feed.get(leader, [])
        cl_events = by_feed.get("chainlink", [])
        if not leader_events or not cl_events:
            return None
        deltas = []
        for le in leader_events:
            t0 = int(le["received_at_ms"])
            p0 = float(le["price"])
            for ce in cl_events:
                t1 = int(ce["received_at_ms"])
                if t1 < t0:
                    continue
                if t1 - t0 > 3000:
                    break
                p1 = float(ce["price"])
                if abs(p1 - p0) < FLAT_USD:
                    continue
                deltas.append(t1 - t0)
                break
        return statistics.median(deltas) if deltas else None

    result["event_median_lead_ms"] = {
        "hl_spot_to_chainlink": median_lead_ms("hl_spot"),
        "hl_perp_to_chainlink": median_lead_ms("hl_perp"),
        "binance_to_chainlink": median_lead_ms("binance"),
    }
    result["event_count"] = {k: len(v) for k, v in by_feed.items()}
    return result


def render_summary_md(summary: dict[str, Any]) -> str:
    d = summary["direction"]
    s = summary["spread"]
    l = summary["lead_lag"]
    lines = [
        f"## {summary['meta']['asset'].upper()} — {summary['meta']['duration_min']:.0f} dk feed study",
        "",
        f"- Samples: {d.get('samples', 0)} @ {summary['meta']['interval_ms']}ms",
        f"- Output: `{summary['meta']['output_dir']}`",
        "",
        "### 1. Yon uyumlulugu",
        f"- Binance == CL: {fmt_pct(d.get('agreement_binance_cl_pct'))}",
        f"- Spot == CL: {fmt_pct(d.get('agreement_spot_cl_pct'))}",
        f"- Perp == CL: {fmt_pct(d.get('agreement_perp_cl_pct'))}",
        f"- Spot == Binance: {fmt_pct(d.get('agreement_spot_binance_pct'))}",
        f"- Conflict spot vs CL: {fmt_pct(d.get('conflict_spot_cl_pct'))}",
        f"- Conflict perp vs CL: {fmt_pct(d.get('conflict_perp_cl_pct'))}",
        f"- Spot leads CL (agrees + faster): {fmt_pct(d.get('spot_leads_cl_pct'))}",
        f"- Perp leads CL: {fmt_pct(d.get('perp_leads_cl_pct'))}",
        f"- Binance leads CL: {fmt_pct(d.get('binance_leads_cl_pct'))}",
    ]
    best = d.get("best_agreement_vs_cl")
    if best:
        lines.append(f"- En iyi CL uyumu: **{best['feed']}** ({best['pct']:.1f}%)")
    for thresh, block in d.get("by_significant_d_cl", {}).items():
        lines.append(
            f"- |d_cl|>={thresh} (n={block.get('samples',0)}): "
            f"spot {fmt_pct(block.get('agreement_spot_cl_pct'))}, "
            f"perp {fmt_pct(block.get('agreement_perp_cl_pct'))}, "
            f"binance {fmt_pct(block.get('agreement_binance_cl_pct'))}"
        )
    lines.extend(["", "### 2. Spread (median gap USD)"])
    for key in ("binance_cl", "spot_cl", "perp_cl", "perp_spot", "binance_spot"):
        st = s.get(key, {})
        if st:
            lines.append(f"- {key}: {st.get('median', 'n/a'):.4f} (p95 {fmt_num(st.get('p95'))})")
    tight = s.get("tightest_vs_cl_median_abs")
    if tight:
        lines.append(f"- CL'e en dar (median abs): **{tight['pair']}** ({tight['abs_median']:.4f})")
    lines.extend(["", "### 3. Lead-lag"])
    for pair, val in l.get("pearson", {}).items():
        lines.append(f"- Pearson {pair}: {fmt_num(val)}")
    for pair, block in l.get("cross_corr_lag", {}).items():
        lines.append(
            f"- Cross-corr {pair}: best_lag={block.get('best_lag_sec')}s corr={fmt_num(block.get('best_corr'))}"
        )
    for feed, ms in l.get("event_median_lead_ms", {}).items():
        lines.append(f"- Event median lead {feed}: {fmt_num(ms)} ms")
    vl = l.get("velocity_lead", {}).get("spot", {})
    if vl:
        lines.append(
            f"- velocity_lead spot: median {fmt_num(vl.get('median'))} "
            f"bullish>4 {fmt_pct(vl.get('pct_bullish_gt4'))}"
        )
    lines.extend(["", "### Sonuc (guard icin)", "- Detay: summary.json", ""])
    return "\n".join(lines)


def fmt_pct(v: Any) -> str:
    if v is None:
        return "n/a"
    return f"{float(v):.1f}%"


def fmt_num(v: Any) -> str:
    if v is None or v == "":
        return "n/a"
    return f"{float(v):.4f}"


def run_analysis(out_dir: Path, tick_rows: list[dict[str, Any]], meta: dict[str, Any]) -> dict[str, Any]:
    events = load_events(out_dir / "events.csv")
    direction_rows = load_csv_rows(out_dir / "direction_report.csv")
    spread_rows = load_csv_rows(out_dir / "spread_report.csv")
    if not direction_rows:
        direction_rows = tick_rows
    if not spread_rows:
        spread_rows = tick_rows
    summary = {
        "meta": meta,
        "direction": analyze_direction(direction_rows),
        "spread": analyze_spread(spread_rows),
        "lead_lag": analyze_lead_lag(tick_rows, events),
    }
    (out_dir / "summary.json").write_text(
        json.dumps(summary, indent=2, default=str), encoding="utf-8"
    )
    (out_dir / "summary.md").write_text(render_summary_md(summary), encoding="utf-8")
    return summary


async def main_async() -> None:
    args = parse_args()
    out_dir = resolve_output_dir(args.output_dir)
    cl_sym, bn_sym = ASSET_SYMBOL[args.asset]
    hl_spot = resolve_hl_spot_coin(args.asset)
    state = StudyState(
        asset=args.asset,
        chainlink_symbol=cl_sym,
        binance_symbol=bn_sym,
        hl_spot_coin=hl_spot,
    )
    duration_sec = args.duration_min * 60.0
    rows = await run_collection(
        state, out_dir, duration_sec, args.interval_ms, args.log_events
    )
    meta = {
        "asset": args.asset,
        "duration_min": args.duration_min,
        "interval_ms": args.interval_ms,
        "output_dir": str(out_dir),
        "hl_spot_coin": hl_spot,
        "log_events": args.log_events,
    }
    summary = run_analysis(out_dir, rows, meta)
    print(render_summary_md(summary), flush=True)
    print(f"\nWrote {out_dir}", flush=True)


def main() -> None:
    asyncio.run(main_async())


if __name__ == "__main__":
    main()
