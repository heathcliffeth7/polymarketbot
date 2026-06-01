#!/usr/bin/env python3
"""
Multi-venue underlying feed study for Polymarket BTC Up/Down timing.

Example:
  python3 -m venv .venv-feed-study
  .venv-feed-study/bin/pip install websockets requests
  .venv-feed-study/bin/python scripts/multi_venue_feed_study.py --duration-min 30 --asset btc --venues all
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
    sys.exit(f"ERROR: pip install websockets requests - {exc}")


POLYMARKET_WS = os.environ.get(
    "POLYMARKET_LIVE_DATA_WS_URL", "wss://ws-live-data.polymarket.com"
)
BINANCE_WS_BASE = os.environ.get(
    "EARLY_STALE_BINANCE_WS_URL", "wss://stream.binance.com:9443/stream"
).rstrip("/")
HYPERLIQUID_WS = os.environ.get(
    "EARLY_STALE_HYPERLIQUID_WS_URL", "wss://api.hyperliquid.xyz/ws"
)
KRAKEN_WS = os.environ.get("KRAKEN_WS_URL", "wss://ws.kraken.com/v2")
COINBASE_WS = os.environ.get(
    "COINBASE_ADVANCED_WS_URL", "wss://advanced-trade-ws.coinbase.com"
)
BYBIT_WS = os.environ.get("BYBIT_SPOT_WS_URL", "wss://stream.bybit.com/v5/public/spot")
LIGHTER_BASE_URL = os.environ.get(
    "LIGHTER_BASE_URL", "https://mainnet.zklighter.elliot.ai"
).rstrip("/")
LIGHTER_WS = os.environ.get(
    "LIGHTER_WS_URL", "wss://mainnet.zklighter.elliot.ai/stream?readonly=true"
)
HL_INFO_URL = os.environ.get("HL_INFO_URL", "https://api.hyperliquid.xyz/info")

FLAT_USD = 0.01
EVENT_MOVE_USD = 1.0
WINDOW_SECONDS = 300
CROSS_CORR_LAGS = list(range(-5, 6))

FEED_CHAINLINK = "chainlink"
FEED_BINANCE = "binance"
FEED_HL_SPOT = "hyperliquid_spot"
FEED_HL_PERP = "hyperliquid_perp"
FEED_KRAKEN = "kraken"
FEED_COINBASE = "coinbase"
FEED_BYBIT = "bybit"
FEED_LIGHTER = "lighter"

DEFAULT_FEEDS = [
    FEED_CHAINLINK,
    FEED_BINANCE,
    FEED_HL_SPOT,
    FEED_HL_PERP,
    FEED_KRAKEN,
    FEED_COINBASE,
    FEED_BYBIT,
    FEED_LIGHTER,
]

ASSETS = {
    "btc": {
        "chainlink": "btc/usd",
        "binance": "btcusdt",
        "kraken": "BTC/USD",
        "coinbase": "BTC-USD",
        "bybit": "BTCUSDT",
        "hl_perp": "BTC",
        "lighter": "BTC",
    }
}


@dataclass
class FeedTick:
    price: float
    received_at_ms: int
    provider_ts_ms: int | None = None
    bid: float | None = None
    ask: float | None = None


@dataclass
class StudyState:
    asset: str
    feeds: list[str]
    latest: dict[str, FeedTick] = field(default_factory=dict)
    last_logged: dict[str, float] = field(default_factory=dict)
    errors: dict[str, int] = field(default_factory=dict)
    lock: asyncio.Lock = field(default_factory=asyncio.Lock)

    async def update(
        self,
        feed: str,
        price: float,
        received_at_ms: int,
        provider_ts_ms: int | None,
        log_events: bool,
        events_writer: csv.DictWriter | None,
        bid: float | None = None,
        ask: float | None = None,
    ) -> None:
        tick = FeedTick(
            price=price,
            received_at_ms=received_at_ms,
            provider_ts_ms=provider_ts_ms,
            bid=bid,
            ask=ask,
        )
        async with self.lock:
            self.latest[feed] = tick
            prev = self.last_logged.get(feed)
            changed = prev is None or abs(price - prev) >= 1e-9
            if log_events and events_writer is not None and changed:
                events_writer.writerow(
                    {
                        "received_at_ms": received_at_ms,
                        "provider_ts_ms": provider_ts_ms if provider_ts_ms is not None else "",
                        "feed": feed,
                        "price": price,
                        "bid": bid if bid is not None else "",
                        "ask": ask if ask is not None else "",
                    }
                )
                self.last_logged[feed] = price

    async def record_error(
        self,
        feed: str,
        message: str,
        errors_writer: csv.DictWriter | None,
    ) -> None:
        async with self.lock:
            self.errors[feed] = self.errors.get(feed, 0) + 1
            if errors_writer is not None:
                errors_writer.writerow(
                    {
                        "ts_ms": int(time.time() * 1000),
                        "feed": feed,
                        "message": message[:500],
                    }
                )

    async def snapshot(self, now_ms: int) -> dict[str, Any]:
        async with self.lock:
            latest = dict(self.latest)
            errors = dict(self.errors)
        prices = {feed: latest.get(feed).price if latest.get(feed) else None for feed in self.feeds}
        age_ms = {}
        provider_age_ms = {}
        for feed in self.feeds:
            tick = latest.get(feed)
            age_ms[feed] = now_ms - tick.received_at_ms if tick else None
            provider_age_ms[feed] = (
                now_ms - tick.provider_ts_ms if tick and tick.provider_ts_ms is not None else None
            )
        return {
            "prices": prices,
            "age_ms": age_ms,
            "provider_age_ms": provider_age_ms,
            "errors": errors,
        }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Multi-venue underlying feed study")
    parser.add_argument("--duration-min", type=float, default=30.0)
    parser.add_argument("--interval-ms", type=int, default=1000)
    parser.add_argument("--asset", default="btc", choices=sorted(ASSETS))
    parser.add_argument("--venues", default="all")
    parser.add_argument("--output-dir", default="")
    parser.add_argument("--log-events", action=argparse.BooleanOptionalAction, default=True)
    return parser.parse_args()


def normalize_feeds(raw: str) -> list[str]:
    aliases = {
        "all": DEFAULT_FEEDS,
        "chainlink": [FEED_CHAINLINK],
        "cl": [FEED_CHAINLINK],
        "binance": [FEED_BINANCE],
        "hyperliquid": [FEED_HL_SPOT, FEED_HL_PERP],
        "hl": [FEED_HL_SPOT, FEED_HL_PERP],
        "hyperliquid_spot": [FEED_HL_SPOT],
        "hl_spot": [FEED_HL_SPOT],
        "hyperliquid_perp": [FEED_HL_PERP],
        "hl_perp": [FEED_HL_PERP],
        "kraken": [FEED_KRAKEN],
        "coinbase": [FEED_COINBASE],
        "bybit": [FEED_BYBIT],
        "lighter": [FEED_LIGHTER],
    }
    feeds: list[str] = []
    for token in raw.split(","):
        key = token.strip().lower()
        if not key:
            continue
        selected = aliases.get(key)
        if selected is None:
            raise SystemExit(f"Unknown venue: {token}")
        for feed in selected:
            if feed not in feeds:
                feeds.append(feed)
    if FEED_CHAINLINK not in feeds:
        feeds.insert(0, FEED_CHAINLINK)
    return feeds


def resolve_output_dir(arg: str) -> Path:
    if arg:
        out = Path(arg)
    else:
        stamp = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
        out = Path(__file__).resolve().parent.parent / "analysis" / f"feed_study_multi_{stamp}"
    out.mkdir(parents=True, exist_ok=True)
    return out


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
        return int(float(value))
    except (TypeError, ValueError):
        return None


def epoch_to_ms(value: Any) -> int | None:
    raw = parse_i64(value)
    if raw is None:
        return None
    if raw > 10_000_000_000_000:
        return raw // 1000
    if raw < 10_000_000_000:
        return raw * 1000
    return raw


def parse_rfc3339_ms(value: Any) -> int | None:
    if not isinstance(value, str) or not value:
        return None
    raw = value.replace("Z", "+00:00")
    try:
        return int(datetime.fromisoformat(raw).astimezone(timezone.utc).timestamp() * 1000)
    except ValueError:
        return None


def mid(bid: float, ask: float) -> float:
    return (bid + ask) / 2.0


def valid_bid_ask(bid: float | None, ask: float | None) -> tuple[float, float] | None:
    if bid is None or ask is None:
        return None
    if not (bid > 0 and ask > 0 and bid <= ask):
        return None
    return bid, ask


def direction_delta(delta: float | None, threshold: float = FLAT_USD) -> int:
    if delta is None or abs(delta) < threshold:
        return 0
    return 1 if delta > 0 else -1


def csv_value(value: Any) -> Any:
    return "" if value is None else value


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


def stats(values: list[float]) -> dict[str, float | None]:
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


def pearson(xs: list[float], ys: list[float]) -> float | None:
    n = min(len(xs), len(ys))
    if n < 3:
        return None
    xs = xs[:n]
    ys = ys[:n]
    mx = statistics.mean(xs)
    my = statistics.mean(ys)
    num = sum((xs[i] - mx) * (ys[i] - my) for i in range(n))
    den_x = math.sqrt(sum((x - mx) ** 2 for x in xs))
    den_y = math.sqrt(sum((y - my) ** 2 for y in ys))
    if den_x == 0 or den_y == 0:
        return None
    return num / (den_x * den_y)


def cross_corr_best_lag(base: list[float], other: list[float]) -> dict[str, Any]:
    best_lag = 0
    best_corr: float | None = None
    by_lag: dict[str, float | None] = {}
    for lag in CROSS_CORR_LAGS:
        if lag >= 0:
            xs = other[lag:]
            ys = base[: len(xs)]
        else:
            xs = other[: len(other) + lag]
            ys = base[-lag:]
        n = min(len(xs), len(ys))
        corr = pearson(xs[:n], ys[:n]) if n >= 5 else None
        by_lag[str(lag)] = corr
        if corr is not None and (best_corr is None or abs(corr) > abs(best_corr)):
            best_corr = corr
            best_lag = lag
    return {"best_lag_sec": best_lag, "best_corr": best_corr, "by_lag": by_lag}


def best_level(rows: Any, side: str) -> float | None:
    if not isinstance(rows, list):
        return None
    prices = []
    for row in rows:
        value = None
        if isinstance(row, dict):
            value = row.get("price") or row.get("px")
        elif isinstance(row, list) and row:
            value = row[0]
        price = parse_f64(value)
        if price is not None:
            prices.append(price)
    if not prices:
        return None
    return max(prices) if side == "bid" else min(prices)


def write_csv_headers(path: Path, fieldnames: list[str]) -> csv.DictWriter:
    fh = path.open("w", newline="", encoding="utf-8")
    writer = csv.DictWriter(fh, fieldnames=fieldnames)
    writer.writeheader()
    writer._fh = fh  # type: ignore[attr-defined]
    return writer


def close_writer(writer: csv.DictWriter | None) -> None:
    if writer is None:
        return
    fh = getattr(writer, "_fh", None)
    if fh:
        fh.close()


async def chainlink_loop(
    state: StudyState,
    stop: asyncio.Event,
    log_events: bool,
    events_writer: csv.DictWriter | None,
    errors_writer: csv.DictWriter | None,
) -> None:
    symbol = ASSETS[state.asset]["chainlink"]
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
                    payload = msg.get("payload") or {}
                    if str(payload.get("symbol", "")).lower() != symbol:
                        continue
                    price = parse_f64(payload.get("value"))
                    if price is None:
                        continue
                    now_ms = int(time.time() * 1000)
                    provider_ts = epoch_to_ms(payload.get("timestamp")) or now_ms
                    await state.update(
                        FEED_CHAINLINK,
                        price,
                        now_ms,
                        provider_ts,
                        log_events,
                        events_writer,
                    )
        except asyncio.CancelledError:
            raise
        except Exception as exc:
            message = f"{type(exc).__name__}: {exc}"
            print(f"[chainlink] reconnect: {message}", flush=True)
            await state.record_error(FEED_CHAINLINK, message, errors_writer)
            await asyncio.sleep(2)


async def binance_loop(
    state: StudyState,
    stop: asyncio.Event,
    log_events: bool,
    events_writer: csv.DictWriter | None,
    errors_writer: csv.DictWriter | None,
) -> None:
    symbol = ASSETS[state.asset]["binance"]
    url = f"{BINANCE_WS_BASE}?streams={symbol}@bookTicker/{symbol}@depth5@100ms"
    while not stop.is_set():
        try:
            async with websockets.connect(url, ping_interval=20) as ws:
                while not stop.is_set():
                    raw = await asyncio.wait_for(ws.recv(), timeout=30)
                    msg = json.loads(raw)
                    data = msg.get("data") or msg
                    stream = str(msg.get("stream", ""))
                    provider_ts = epoch_to_ms(data.get("E"))
                    bid = ask = None
                    if "bookTicker" in stream or (data.get("b") and data.get("a")):
                        bid = parse_f64(data.get("b"))
                        ask = parse_f64(data.get("a"))
                    elif "depth" in stream:
                        bid = best_level(data.get("b") or data.get("bids"), "bid")
                        ask = best_level(data.get("a") or data.get("asks"), "ask")
                    pair = valid_bid_ask(bid, ask)
                    if pair is None:
                        continue
                    bid, ask = pair
                    now_ms = int(time.time() * 1000)
                    await state.update(
                        FEED_BINANCE,
                        mid(bid, ask),
                        now_ms,
                        provider_ts,
                        log_events,
                        events_writer,
                        bid,
                        ask,
                    )
        except asyncio.CancelledError:
            raise
        except Exception as exc:
            message = f"{type(exc).__name__}: {exc}"
            print(f"[binance] reconnect: {message}", flush=True)
            await state.record_error(FEED_BINANCE, message, errors_writer)
            await asyncio.sleep(2)


def parse_hyperliquid_book(payload: dict[str, Any]) -> tuple[float, float] | None:
    levels = payload.get("levels")
    if not isinstance(levels, list) or len(levels) < 2:
        return None
    bid = best_level(levels[0], "bid")
    ask = best_level(levels[1], "ask")
    return valid_bid_ask(bid, ask)


def resolve_hl_spot_coin(asset: str) -> str | None:
    override = os.environ.get("HL_SPOT_COIN", "").strip()
    if override:
        return override
    try:
        resp = requests.post(HL_INFO_URL, json={"type": "spotMeta"}, timeout=15)
        resp.raise_for_status()
        meta = resp.json()
    except Exception as exc:
        print(f"[hyperliquid_spot] spotMeta unavailable: {exc}", flush=True)
        return None
    token_names = {
        int(t["index"]): str(t.get("name", ""))
        for t in meta.get("tokens", [])
        if t.get("index") is not None
    }
    candidates: list[tuple[str, str]] = []
    asset_key = asset.lower()
    for entry in meta.get("universe", []):
        idx = entry.get("index")
        tokens = entry.get("tokens") or []
        if idx is None or len(tokens) < 2:
            continue
        base = token_names.get(int(tokens[0]), "")
        quote = token_names.get(int(tokens[1]), "")
        label = f"{base}/{quote}".upper()
        if asset_key == "btc" and ("BTC" in label or "UBTC" in label) and "USDC" in label:
            candidates.append((f"@{idx}", label))
    if not candidates:
        print(f"[hyperliquid_spot] no spot pair for asset={asset}", flush=True)
        return None
    candidates.sort(key=lambda item: (0 if item[1].endswith("/USDC") else 1, item[1]))
    coin, label = candidates[0]
    print(f"[hyperliquid_spot] resolved {coin} ({label})", flush=True)
    return coin


async def hyperliquid_loop(
    state: StudyState,
    stop: asyncio.Event,
    log_events: bool,
    events_writer: csv.DictWriter | None,
    errors_writer: csv.DictWriter | None,
    hl_spot_coin: str | None,
) -> None:
    perp_coin = ASSETS[state.asset]["hl_perp"]
    coin_map: dict[str, str] = {}
    subs = []
    if FEED_HL_PERP in state.feeds:
        coin_map[perp_coin] = FEED_HL_PERP
        subs.append({"method": "subscribe", "subscription": {"type": "l2Book", "coin": perp_coin}})
    if FEED_HL_SPOT in state.feeds and hl_spot_coin:
        coin_map[hl_spot_coin] = FEED_HL_SPOT
        subs.append(
            {"method": "subscribe", "subscription": {"type": "l2Book", "coin": hl_spot_coin}}
        )
    if not subs:
        return
    while not stop.is_set():
        try:
            async with websockets.connect(HYPERLIQUID_WS, ping_interval=20) as ws:
                for sub in subs:
                    await ws.send(json.dumps(sub))
                while not stop.is_set():
                    raw = await asyncio.wait_for(ws.recv(), timeout=30)
                    msg = json.loads(raw)
                    data = msg.get("data") or msg
                    if msg.get("channel") != "l2Book" and data.get("channel") != "l2Book":
                        continue
                    if msg.get("channel") == "l2Book":
                        data = msg.get("data") or {}
                    feed = coin_map.get(str(data.get("coin", "")))
                    if not feed:
                        continue
                    pair = parse_hyperliquid_book(data)
                    if pair is None:
                        continue
                    bid, ask = pair
                    now_ms = int(time.time() * 1000)
                    provider_ts = epoch_to_ms(data.get("time")) or now_ms
                    await state.update(
                        feed,
                        mid(bid, ask),
                        now_ms,
                        provider_ts,
                        log_events,
                        events_writer,
                        bid,
                        ask,
                    )
        except asyncio.CancelledError:
            raise
        except Exception as exc:
            message = f"{type(exc).__name__}: {exc}"
            print(f"[hyperliquid] reconnect: {message}", flush=True)
            await state.record_error("hyperliquid", message, errors_writer)
            await asyncio.sleep(2)


async def kraken_loop(
    state: StudyState,
    stop: asyncio.Event,
    log_events: bool,
    events_writer: csv.DictWriter | None,
    errors_writer: csv.DictWriter | None,
) -> None:
    symbol = ASSETS[state.asset]["kraken"]
    sub = {
        "method": "subscribe",
        "params": {
            "channel": "ticker",
            "symbol": [symbol],
            "event_trigger": "bbo",
            "snapshot": True,
        },
    }
    while not stop.is_set():
        try:
            async with websockets.connect(KRAKEN_WS, ping_interval=20) as ws:
                await ws.send(json.dumps(sub))
                while not stop.is_set():
                    raw = await asyncio.wait_for(ws.recv(), timeout=30)
                    msg = json.loads(raw)
                    if msg.get("channel") != "ticker":
                        continue
                    for item in msg.get("data") or []:
                        if str(item.get("symbol", "")).upper() != symbol:
                            continue
                        bid = parse_f64(item.get("bid"))
                        ask = parse_f64(item.get("ask"))
                        pair = valid_bid_ask(bid, ask)
                        if pair is None:
                            continue
                        bid, ask = pair
                        now_ms = int(time.time() * 1000)
                        await state.update(
                            FEED_KRAKEN,
                            mid(bid, ask),
                            now_ms,
                            None,
                            log_events,
                            events_writer,
                            bid,
                            ask,
                        )
        except asyncio.CancelledError:
            raise
        except Exception as exc:
            message = f"{type(exc).__name__}: {exc}"
            print(f"[kraken] reconnect: {message}", flush=True)
            await state.record_error(FEED_KRAKEN, message, errors_writer)
            await asyncio.sleep(2)


async def coinbase_loop(
    state: StudyState,
    stop: asyncio.Event,
    log_events: bool,
    events_writer: csv.DictWriter | None,
    errors_writer: csv.DictWriter | None,
) -> None:
    product = ASSETS[state.asset]["coinbase"]
    subs = [
        {"type": "subscribe", "product_ids": [product], "channel": "ticker"},
        {"type": "subscribe", "product_ids": [product], "channel": "heartbeats"},
    ]
    while not stop.is_set():
        try:
            async with websockets.connect(COINBASE_WS, ping_interval=20) as ws:
                for sub in subs:
                    await ws.send(json.dumps(sub))
                while not stop.is_set():
                    raw = await asyncio.wait_for(ws.recv(), timeout=30)
                    msg = json.loads(raw)
                    if msg.get("channel") != "ticker":
                        continue
                    for event in msg.get("events") or []:
                        for ticker in event.get("tickers") or []:
                            if ticker.get("product_id") != product:
                                continue
                            bid = parse_f64(
                                ticker.get("best_bid") or ticker.get("best_bid_price")
                            )
                            ask = parse_f64(
                                ticker.get("best_ask") or ticker.get("best_ask_price")
                            )
                            pair = valid_bid_ask(bid, ask)
                            price = parse_f64(ticker.get("price"))
                            if pair is not None:
                                bid, ask = pair
                                price = mid(bid, ask)
                            if price is None:
                                continue
                            now_ms = int(time.time() * 1000)
                            provider_ts = parse_rfc3339_ms(
                                ticker.get("time") or event.get("event_time")
                            )
                            await state.update(
                                FEED_COINBASE,
                                price,
                                now_ms,
                                provider_ts,
                                log_events,
                                events_writer,
                                bid,
                                ask,
                            )
        except asyncio.CancelledError:
            raise
        except Exception as exc:
            message = f"{type(exc).__name__}: {exc}"
            print(f"[coinbase] reconnect: {message}", flush=True)
            await state.record_error(FEED_COINBASE, message, errors_writer)
            await asyncio.sleep(2)


async def bybit_loop(
    state: StudyState,
    stop: asyncio.Event,
    log_events: bool,
    events_writer: csv.DictWriter | None,
    errors_writer: csv.DictWriter | None,
) -> None:
    symbol = ASSETS[state.asset]["bybit"]
    topic = f"orderbook.1.{symbol}"
    sub = {"op": "subscribe", "args": [topic]}
    while not stop.is_set():
        try:
            async with websockets.connect(BYBIT_WS, ping_interval=20) as ws:
                await ws.send(json.dumps(sub))
                while not stop.is_set():
                    try:
                        raw = await asyncio.wait_for(ws.recv(), timeout=30)
                    except asyncio.TimeoutError:
                        await ws.send(json.dumps({"op": "ping"}))
                        continue
                    msg = json.loads(raw)
                    if msg.get("topic") != topic:
                        continue
                    data = msg.get("data") or {}
                    bid = best_level(data.get("b"), "bid")
                    ask = best_level(data.get("a"), "ask")
                    pair = valid_bid_ask(bid, ask)
                    if pair is None:
                        continue
                    bid, ask = pair
                    now_ms = int(time.time() * 1000)
                    provider_ts = epoch_to_ms(msg.get("cts") or msg.get("ts"))
                    await state.update(
                        FEED_BYBIT,
                        mid(bid, ask),
                        now_ms,
                        provider_ts,
                        log_events,
                        events_writer,
                        bid,
                        ask,
                    )
        except asyncio.CancelledError:
            raise
        except Exception as exc:
            message = f"{type(exc).__name__}: {exc}"
            print(f"[bybit] reconnect: {message}", flush=True)
            await state.record_error(FEED_BYBIT, message, errors_writer)
            await asyncio.sleep(2)


def resolve_lighter_market_index(asset: str) -> int | None:
    specific_env = f"LIGHTER_{asset.upper()}_MARKET_INDEX"
    override = os.environ.get(specific_env) or os.environ.get("LIGHTER_MARKET_INDEX")
    if override:
        parsed = parse_i64(override)
        if parsed is not None:
            return parsed
    symbol = ASSETS[asset]["lighter"].upper()
    try:
        resp = requests.get(f"{LIGHTER_BASE_URL}/api/v1/orderBookDetails", timeout=15)
        resp.raise_for_status()
        payload = resp.json()
    except Exception as exc:
        print(f"[lighter] orderBookDetails unavailable: {exc}", flush=True)
        return None
    books = payload.get("order_book_details") or payload.get("orderBookDetails") or []
    candidates = [
        item
        for item in books
        if str(item.get("symbol", "")).upper() == symbol
        and str(item.get("status", "")).lower() in ("", "active")
    ]
    if not candidates:
        print(f"[lighter] no market index for symbol={symbol}", flush=True)
        return None
    candidates.sort(key=lambda item: 0 if str(item.get("market_type", "")).lower() == "perp" else 1)
    market_id = parse_i64(candidates[0].get("market_id"))
    if market_id is not None:
        print(f"[lighter] resolved market_index={market_id} symbol={symbol}", flush=True)
    return market_id


async def lighter_loop(
    state: StudyState,
    stop: asyncio.Event,
    log_events: bool,
    events_writer: csv.DictWriter | None,
    errors_writer: csv.DictWriter | None,
    market_index: int | None,
) -> None:
    if market_index is None:
        await state.record_error(FEED_LIGHTER, "market_index_unresolved", errors_writer)
        return
    sub = {"type": "subscribe", "channel": f"ticker/{market_index}"}
    expected_channel = f"ticker:{market_index}"
    while not stop.is_set():
        try:
            async with websockets.connect(LIGHTER_WS, ping_interval=20) as ws:
                await ws.send(json.dumps(sub))
                while not stop.is_set():
                    raw = await asyncio.wait_for(ws.recv(), timeout=30)
                    msg = json.loads(raw)
                    if msg.get("channel") != expected_channel or msg.get("type") != "update/ticker":
                        continue
                    ticker = msg.get("ticker") or {}
                    bid = parse_f64((ticker.get("b") or {}).get("price"))
                    ask = parse_f64((ticker.get("a") or {}).get("price"))
                    pair = valid_bid_ask(bid, ask)
                    if pair is None:
                        continue
                    bid, ask = pair
                    provider_ts = (
                        epoch_to_ms(ticker.get("last_updated_at"))
                        or epoch_to_ms(msg.get("last_updated_at"))
                        or epoch_to_ms(msg.get("timestamp"))
                    )
                    now_ms = int(time.time() * 1000)
                    await state.update(
                        FEED_LIGHTER,
                        mid(bid, ask),
                        now_ms,
                        provider_ts,
                        log_events,
                        events_writer,
                        bid,
                        ask,
                    )
        except asyncio.CancelledError:
            raise
        except Exception as exc:
            message = f"{type(exc).__name__}: {exc}"
            print(f"[lighter] reconnect: {message}", flush=True)
            await state.record_error(FEED_LIGHTER, message, errors_writer)
            await asyncio.sleep(2)


def tick_fieldnames(feeds: list[str]) -> list[str]:
    fields = ["ts_ms", "second_index", "window_index"]
    for feed in feeds:
        fields.extend([feed, f"{feed}_age_ms", f"{feed}_provider_age_ms", f"{feed}_ok"])
    fields.extend(["delta_chainlink", "window_delta_chainlink", "dir_chainlink"])
    for feed in feeds:
        if feed == FEED_CHAINLINK:
            continue
        fields.extend(
            [
                f"delta_{feed}",
                f"window_delta_{feed}",
                f"dir_{feed}",
                f"gap_{feed}_cl",
                f"adjusted_{feed}",
                f"adjusted_error_{feed}",
                f"adjusted_agrees_cl_{feed}",
                f"adjusted_conflicts_cl_{feed}",
            ]
        )
    fields.extend(["consensus_up_count", "consensus_down_count", "consensus_net", "consensus_total"])
    return fields


def build_tasks(
    state: StudyState,
    stop: asyncio.Event,
    log_events: bool,
    events_writer: csv.DictWriter | None,
    errors_writer: csv.DictWriter | None,
    hl_spot_coin: str | None,
    lighter_market_index: int | None,
) -> list[asyncio.Task]:
    tasks = []
    if FEED_CHAINLINK in state.feeds:
        tasks.append(asyncio.create_task(chainlink_loop(state, stop, log_events, events_writer, errors_writer)))
    if FEED_BINANCE in state.feeds:
        tasks.append(asyncio.create_task(binance_loop(state, stop, log_events, events_writer, errors_writer)))
    if FEED_HL_SPOT in state.feeds or FEED_HL_PERP in state.feeds:
        tasks.append(
            asyncio.create_task(
                hyperliquid_loop(state, stop, log_events, events_writer, errors_writer, hl_spot_coin)
            )
        )
    if FEED_KRAKEN in state.feeds:
        tasks.append(asyncio.create_task(kraken_loop(state, stop, log_events, events_writer, errors_writer)))
    if FEED_COINBASE in state.feeds:
        tasks.append(asyncio.create_task(coinbase_loop(state, stop, log_events, events_writer, errors_writer)))
    if FEED_BYBIT in state.feeds:
        tasks.append(asyncio.create_task(bybit_loop(state, stop, log_events, events_writer, errors_writer)))
    if FEED_LIGHTER in state.feeds:
        tasks.append(
            asyncio.create_task(
                lighter_loop(state, stop, log_events, events_writer, errors_writer, lighter_market_index)
            )
        )
    return tasks


async def run_collection(
    state: StudyState,
    out_dir: Path,
    duration_sec: float,
    interval_ms: int,
    log_events: bool,
    hl_spot_coin: str | None,
    lighter_market_index: int | None,
) -> list[dict[str, Any]]:
    stop = asyncio.Event()
    ticks_writer = write_csv_headers(out_dir / "ticks.csv", tick_fieldnames(state.feeds))
    events_writer = (
        write_csv_headers(
            out_dir / "events.csv",
            ["received_at_ms", "provider_ts_ms", "feed", "price", "bid", "ask"],
        )
        if log_events
        else None
    )
    errors_writer = write_csv_headers(out_dir / "errors.csv", ["ts_ms", "feed", "message"])
    tasks = build_tasks(
        state,
        stop,
        log_events,
        events_writer,
        errors_writer,
        hl_spot_coin,
        lighter_market_index,
    )
    started = time.monotonic()
    started_wall_ms = int(time.time() * 1000)
    target_samples = int(duration_sec * 1000 / interval_ms)
    print(
        f"Collecting {duration_sec:.0f}s @ {interval_ms}ms across {','.join(state.feeds)}",
        flush=True,
    )

    rows: list[dict[str, Any]] = []
    prev_prices: dict[str, float | None] = {feed: None for feed in state.feeds}
    window_opens: dict[int, dict[str, float]] = {}
    second_index = 0
    try:
        while time.monotonic() - started < duration_sec:
            loop_start = time.monotonic()
            now_ms = int(time.time() * 1000)
            window_index = max(0, int((now_ms - started_wall_ms) / (WINDOW_SECONDS * 1000)))
            snap = await state.snapshot(now_ms)
            prices: dict[str, float | None] = snap["prices"]
            age_ms: dict[str, int | None] = snap["age_ms"]
            provider_age_ms: dict[str, int | None] = snap["provider_age_ms"]
            opens = window_opens.setdefault(window_index, {})
            for feed, price in prices.items():
                if price is not None and feed not in opens:
                    opens[feed] = price

            cl = prices.get(FEED_CHAINLINK)
            cl_open = opens.get(FEED_CHAINLINK)
            delta_cl = (
                cl - prev_prices[FEED_CHAINLINK]
                if cl is not None and prev_prices.get(FEED_CHAINLINK) is not None
                else None
            )
            window_delta_cl = cl - cl_open if cl is not None and cl_open is not None else None
            dir_cl = direction_delta(window_delta_cl)

            row: dict[str, Any] = {
                "ts_ms": now_ms,
                "second_index": second_index,
                "window_index": window_index,
                "delta_chainlink": csv_value(delta_cl),
                "window_delta_chainlink": csv_value(window_delta_cl),
                "dir_chainlink": dir_cl,
            }
            for feed in state.feeds:
                price = prices.get(feed)
                row[feed] = csv_value(price)
                row[f"{feed}_age_ms"] = csv_value(age_ms.get(feed))
                row[f"{feed}_provider_age_ms"] = csv_value(provider_age_ms.get(feed))
                row[f"{feed}_ok"] = int(price is not None)

            consensus_up = 0
            consensus_down = 0
            consensus_total = 0
            for feed in state.feeds:
                if feed == FEED_CHAINLINK:
                    continue
                price = prices.get(feed)
                prev = prev_prices.get(feed)
                feed_open = opens.get(feed)
                delta_feed = price - prev if price is not None and prev is not None else None
                window_delta_feed = (
                    price - feed_open if price is not None and feed_open is not None else None
                )
                dir_feed = direction_delta(window_delta_feed)
                gap = price - cl if price is not None and cl is not None else None
                adjusted = (
                    cl_open + window_delta_feed
                    if cl_open is not None and window_delta_feed is not None
                    else None
                )
                adjusted_error = adjusted - cl if adjusted is not None and cl is not None else None
                agrees = int(dir_feed != 0 and dir_cl != 0 and dir_feed == dir_cl)
                conflicts = int(dir_feed != 0 and dir_cl != 0 and dir_feed != dir_cl)
                row[f"delta_{feed}"] = csv_value(delta_feed)
                row[f"window_delta_{feed}"] = csv_value(window_delta_feed)
                row[f"dir_{feed}"] = dir_feed
                row[f"gap_{feed}_cl"] = csv_value(gap)
                row[f"adjusted_{feed}"] = csv_value(adjusted)
                row[f"adjusted_error_{feed}"] = csv_value(adjusted_error)
                row[f"adjusted_agrees_cl_{feed}"] = agrees
                row[f"adjusted_conflicts_cl_{feed}"] = conflicts
                if dir_feed > 0:
                    consensus_up += 1
                    consensus_total += 1
                elif dir_feed < 0:
                    consensus_down += 1
                    consensus_total += 1
            row["consensus_up_count"] = consensus_up
            row["consensus_down_count"] = consensus_down
            row["consensus_net"] = consensus_up - consensus_down
            row["consensus_total"] = consensus_total

            ticks_writer.writerow(row)
            rows.append(row)
            prev_prices = dict(prices)
            second_index += 1
            sleep_s = max(0.0, interval_ms / 1000.0 - (time.monotonic() - loop_start))
            await asyncio.sleep(sleep_s)
    finally:
        stop.set()
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        close_writer(ticks_writer)
        close_writer(events_writer)
        close_writer(errors_writer)

    run_meta = {
        "duration_sec": duration_sec,
        "interval_ms": interval_ms,
        "samples_written": second_index,
        "target_samples": target_samples,
        "feeds": state.feeds,
        "hl_spot_coin": hl_spot_coin,
        "lighter_market_index": lighter_market_index,
        "window_seconds": WINDOW_SECONDS,
    }
    (out_dir / "run_meta.json").write_text(json.dumps(run_meta, indent=2), encoding="utf-8")
    return rows


def load_csv(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    with path.open(encoding="utf-8") as fh:
        return list(csv.DictReader(fh))


def row_float(row: dict[str, Any], key: str) -> float | None:
    value = row.get(key, "")
    if value == "":
        return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def analyze_availability(rows: list[dict[str, Any]], feeds: list[str]) -> dict[str, Any]:
    result = {}
    total = len(rows)
    for feed in feeds:
        ok = sum(1 for row in rows if row.get(f"{feed}_ok") == "1" or row.get(f"{feed}_ok") == 1)
        ages = [v for row in rows if (v := row_float(row, f"{feed}_age_ms")) is not None]
        provider_ages = [
            v for row in rows if (v := row_float(row, f"{feed}_provider_age_ms")) is not None
        ]
        result[feed] = {
            "coverage_pct": 100.0 * ok / total if total else None,
            "ok_samples": ok,
            "age_ms": stats(ages),
            "provider_age_ms": stats(provider_ages),
        }
    return result


def pct_bool(rows: list[dict[str, Any]], key: str) -> float | None:
    vals = []
    for row in rows:
        value = row.get(key, "")
        if value == "":
            continue
        vals.append(bool(int(value)))
    if not vals:
        return None
    return 100.0 * sum(vals) / len(vals)


def analyze_adjusted(rows: list[dict[str, Any]], feeds: list[str]) -> dict[str, Any]:
    result = {}
    for feed in feeds:
        if feed == FEED_CHAINLINK:
            continue
        errors = [v for row in rows if (v := row_float(row, f"adjusted_error_{feed}")) is not None]
        abs_errors = [abs(v) for v in errors]
        gaps = [v for row in rows if (v := row_float(row, f"gap_{feed}_cl")) is not None]
        block: dict[str, Any] = {
            "agreement_pct": pct_bool(rows, f"adjusted_agrees_cl_{feed}"),
            "conflict_pct": pct_bool(rows, f"adjusted_conflicts_cl_{feed}"),
            "adjusted_error": stats(errors),
            "adjusted_abs_error": stats(abs_errors),
            "raw_gap_vs_chainlink": stats(gaps),
        }
        result[feed] = block
    return result


def analyze_lead_lag(rows: list[dict[str, Any]], events: list[dict[str, Any]], feeds: list[str]) -> dict[str, Any]:
    d_cl = [v for row in rows if (v := row_float(row, "delta_chainlink")) is not None]
    pearsons = {}
    cross_corr = {}
    for feed in feeds:
        if feed == FEED_CHAINLINK:
            continue
        d_feed = [v for row in rows if (v := row_float(row, f"delta_{feed}")) is not None]
        n = min(len(d_cl), len(d_feed))
        pearsons[f"{feed}_vs_chainlink"] = pearson(d_feed[:n], d_cl[:n]) if n else None
        cross_corr[f"{feed}_vs_chainlink"] = cross_corr_best_lag(d_cl, d_feed)
    return {
        "pearson": pearsons,
        "cross_corr_lag": cross_corr,
        "event_median_lead_ms": {
            f"{feed}_to_chainlink": event_median_lead_ms(events, feed)
            for feed in feeds
            if feed != FEED_CHAINLINK
        },
    }


def event_median_lead_ms(events: list[dict[str, Any]], feed: str) -> float | None:
    by_feed: dict[str, list[dict[str, Any]]] = {}
    for event in events:
        by_feed.setdefault(str(event.get("feed", "")), []).append(event)
    for items in by_feed.values():
        items.sort(key=lambda item: int(item["received_at_ms"]))

    def moves(name: str) -> list[tuple[int, int]]:
        out = []
        prev_price: float | None = None
        for event in by_feed.get(name, []):
            price = parse_f64(event.get("price"))
            if price is None:
                continue
            if prev_price is not None:
                sign = direction_delta(price - prev_price, EVENT_MOVE_USD)
                if sign:
                    out.append((int(event["received_at_ms"]), sign))
            prev_price = price
        return out

    leader_moves = moves(feed)
    cl_moves = moves(FEED_CHAINLINK)
    if not leader_moves or not cl_moves:
        return None
    leads = []
    for t0, sign in leader_moves:
        for t1, cl_sign in cl_moves:
            if t1 < t0:
                continue
            if t1 - t0 > 3_000:
                break
            if cl_sign == sign:
                leads.append(t1 - t0)
                break
    return statistics.median(leads) if leads else None


def write_window_report(out_dir: Path, rows: list[dict[str, Any]], feeds: list[str]) -> list[dict[str, Any]]:
    fieldnames = ["window_index", "feed", "samples", "chainlink_open", "venue_open", "agreement_pct", "conflict_pct", "median_adjusted_error", "median_abs_adjusted_error", "first_venue_signal_ms", "first_chainlink_signal_ms", "lead_ms"]
    by_window: dict[int, list[dict[str, Any]]] = {}
    for row in rows:
        by_window.setdefault(int(row["window_index"]), []).append(row)
    report_rows = []
    for window_index, window_rows in sorted(by_window.items()):
        cl_open = first_window_price(window_rows, FEED_CHAINLINK)
        first_cl_signal = first_window_signal_ms(window_rows, "window_delta_chainlink")
        for feed in feeds:
            if feed == FEED_CHAINLINK:
                continue
            venue_open = first_window_price(window_rows, feed)
            errors = [
                v
                for row in window_rows
                if (v := row_float(row, f"adjusted_error_{feed}")) is not None
            ]
            abs_errors = [abs(v) for v in errors]
            first_venue_signal = first_window_signal_ms(window_rows, f"window_delta_{feed}")
            report_row = {
                "window_index": window_index,
                "feed": feed,
                "samples": len(window_rows),
                "chainlink_open": csv_value(cl_open),
                "venue_open": csv_value(venue_open),
                "agreement_pct": csv_value(pct_bool(window_rows, f"adjusted_agrees_cl_{feed}")),
                "conflict_pct": csv_value(pct_bool(window_rows, f"adjusted_conflicts_cl_{feed}")),
                "median_adjusted_error": csv_value(statistics.median(errors) if errors else None),
                "median_abs_adjusted_error": csv_value(
                    statistics.median(abs_errors) if abs_errors else None
                ),
                "first_venue_signal_ms": csv_value(first_venue_signal),
                "first_chainlink_signal_ms": csv_value(first_cl_signal),
                "lead_ms": csv_value(
                    first_cl_signal - first_venue_signal
                    if first_cl_signal is not None and first_venue_signal is not None
                    else None
                ),
            }
            report_rows.append(report_row)
    writer = write_csv_headers(out_dir / "window_report.csv", fieldnames)
    try:
        for row in report_rows:
            writer.writerow(row)
    finally:
        close_writer(writer)
    return report_rows


def first_window_price(rows: list[dict[str, Any]], feed: str) -> float | None:
    for row in rows:
        value = row_float(row, feed)
        if value is not None:
            return value
    return None

def first_window_signal_ms(rows: list[dict[str, Any]], column: str) -> int | None:
    if not rows:
        return None
    start_ms = int(rows[0]["ts_ms"])
    for row in rows:
        delta = row_float(row, column)
        if delta is not None and abs(delta) >= EVENT_MOVE_USD:
            return int(row["ts_ms"]) - start_ms
    return None


def best_adjusted_feed(summary: dict[str, Any]) -> dict[str, Any] | None:
    candidates = []
    for feed, block in summary.get("adjusted", {}).items():
        agreement = block.get("agreement_pct")
        error = block.get("adjusted_abs_error", {}).get("median")
        if agreement is not None and error is not None:
            candidates.append((feed, float(agreement), float(error)))
    if not candidates:
        return None
    feed, agreement, error = sorted(candidates, key=lambda item: (-item[1], item[2]))[0]
    return {"feed": feed, "agreement_pct": agreement, "median_abs_error": error}


def fmt_pct(value: Any) -> str:
    if value is None or value == "":
        return "n/a"
    return f"{float(value):.1f}%"


def fmt_num(value: Any, digits: int = 4) -> str:
    if value is None or value == "":
        return "n/a"
    return f"{float(value):.{digits}f}"


def render_summary_md(summary: dict[str, Any]) -> str:
    meta = summary["meta"]
    lines = [
        f"## {meta['asset'].upper()} multi-venue feed study",
        "",
        f"- Samples: {meta['samples']} @ {meta['interval_ms']}ms",
        f"- Duration: {meta['duration_min']:.2f} min",
        f"- Feeds: {', '.join(meta['feeds'])}",
        f"- Output: `{meta['output_dir']}`",
        "",
        "### Availability",
    ]
    for feed, block in summary["availability"].items():
        age = block.get("age_ms", {}).get("median")
        p95 = block.get("age_ms", {}).get("p95")
        lines.append(
            f"- {feed}: coverage {fmt_pct(block.get('coverage_pct'))}, "
            f"age median {fmt_num(age, 0)}ms p95 {fmt_num(p95, 0)}ms"
        )
    lines.extend(["", "### Delta-adjusted predictor"])
    best = best_adjusted_feed(summary)
    if best:
        lines.append(
            f"- Best adjusted feed: **{best['feed']}** "
            f"agreement {fmt_pct(best['agreement_pct'])}, "
            f"median abs error {fmt_num(best['median_abs_error'])} USD"
        )
    for feed, block in summary["adjusted"].items():
        lines.append(
            f"- {feed}: agreement {fmt_pct(block.get('agreement_pct'))}, "
            f"conflict {fmt_pct(block.get('conflict_pct'))}, "
            f"median abs error {fmt_num(block.get('adjusted_abs_error', {}).get('median'))} USD"
        )
    lines.extend(["", "### Lead-lag"])
    for feed, lead in summary["lead_lag"].get("event_median_lead_ms", {}).items():
        lines.append(f"- Event median lead {feed}: {fmt_num(lead, 0)} ms")
    for pair, block in summary["lead_lag"].get("cross_corr_lag", {}).items():
        lines.append(
            f"- Cross-corr {pair}: best_lag={block.get('best_lag_sec')}s "
            f"corr={fmt_num(block.get('best_corr'))}"
        )
    lines.extend(["", "### Files", "- `ticks.csv`", "- `events.csv`", "- `window_report.csv`", "- `summary.json`"])
    return "\n".join(lines) + "\n"


def run_analysis(out_dir: Path, rows: list[dict[str, Any]], meta: dict[str, Any]) -> dict[str, Any]:
    events = load_csv(out_dir / "events.csv")
    if not rows:
        rows = load_csv(out_dir / "ticks.csv")
    window_rows = write_window_report(out_dir, rows, meta["feeds"])
    summary = {
        "meta": {**meta, "samples": len(rows)},
        "availability": analyze_availability(rows, meta["feeds"]),
        "adjusted": analyze_adjusted(rows, meta["feeds"]),
        "lead_lag": analyze_lead_lag(rows, events, meta["feeds"]),
        "window_report_rows": len(window_rows),
    }
    (out_dir / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    (out_dir / "summary.md").write_text(render_summary_md(summary), encoding="utf-8")
    return summary


async def main_async() -> None:
    args = parse_args()
    feeds = normalize_feeds(args.venues)
    out_dir = resolve_output_dir(args.output_dir)
    hl_spot_coin = resolve_hl_spot_coin(args.asset) if FEED_HL_SPOT in feeds else None
    lighter_market_index = (
        resolve_lighter_market_index(args.asset) if FEED_LIGHTER in feeds else None
    )
    state = StudyState(asset=args.asset, feeds=feeds)
    duration_sec = args.duration_min * 60.0
    rows = await run_collection(
        state,
        out_dir,
        duration_sec,
        args.interval_ms,
        args.log_events,
        hl_spot_coin,
        lighter_market_index,
    )
    meta = {
        "asset": args.asset,
        "duration_min": args.duration_min,
        "interval_ms": args.interval_ms,
        "output_dir": str(out_dir),
        "feeds": feeds,
        "log_events": args.log_events,
        "hl_spot_coin": hl_spot_coin,
        "lighter_market_index": lighter_market_index,
        "window_seconds": WINDOW_SECONDS,
    }
    summary = run_analysis(out_dir, rows, meta)
    print(render_summary_md(summary), flush=True)
    print(f"Wrote {out_dir}", flush=True)


def main() -> None:
    asyncio.run(main_async())


if __name__ == "__main__":
    main()
