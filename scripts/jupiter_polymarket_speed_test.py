#!/usr/bin/env python3
"""
Read-only Jupiter vs Polymarket BTC 5m price speed study.

Example:
  python3 scripts/jupiter_polymarket_speed_test.py --duration-min 10 --asset btc

The script compares tradable Up/Down market prices for the same active
btc-updown-5m-* window. It never places orders or uses wallet credentials.
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
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


GAMMA_BASE_URL = os.environ.get("GAMMA_BASE_URL", "https://gamma-api.polymarket.com").rstrip("/")
POLYMARKET_CLOB_BASE_URL = os.environ.get(
    "POLYMARKET_CLOB_BASE_URL", "https://clob.polymarket.com"
).rstrip("/")
POLYMARKET_CLOB_WS_URL = os.environ.get(
    "POLYMARKET_CLOB_WS_URL", "wss://ws-subscriptions-clob.polymarket.com/ws/market"
)
JUPITER_PREDICTION_BASE_URL = os.environ.get(
    "JUPITER_PREDICTION_BASE_URL", "https://api.jup.ag/prediction/v1"
).rstrip("/")
JUPITER_API_KEY_ENV = "JUPITER_PREDICTION_API_KEY"

WINDOW_SECONDS = {"5m": 300}
PRICE_EPSILON = 1e-9
USER_AGENT = "polymarketbot-jupiter-speed-test/1.0"


@dataclass(frozen=True)
class HttpJsonResponse:
    status: int
    payload: Any
    headers: dict[str, str]


@dataclass(frozen=True)
class PolymarketSpec:
    slug: str
    question: str
    asset: str
    timeframe: str
    start_ts: int
    end_ts: int
    condition_id: str | None
    token_by_side: dict[str, str]


@dataclass(frozen=True)
class JupiterSpec:
    event_id: str
    slug: str
    market_by_side: dict[str, str]


@dataclass
class PriceSnapshot:
    platform: str
    side: str
    market_slug: str
    market_id: str = ""
    token_id: str = ""
    event_id: str = ""
    bid: float | None = None
    bid_size: float | None = None
    ask: float | None = None
    ask_size: float | None = None
    mid: float | None = None
    buy_yes: float | None = None
    sell_yes: float | None = None
    buy_no: float | None = None
    sell_no: float | None = None
    status: str = ""
    source: str = ""
    received_at_ms: int = 0
    provider_ts_ms: int | None = None
    rate_limit_remaining: str = ""

    def source_age_ms(self) -> int | None:
        if self.provider_ts_ms is None:
            return None
        return max(0, self.received_at_ms - self.provider_ts_ms)


@dataclass(frozen=True)
class ChangeEvent:
    ts_ms: int
    platform: str
    side: str
    old_mid: float
    new_mid: float
    direction: int
    source: str
    source_age_ms: int | None


@dataclass
class RateLimitStats:
    jupiter_429_count: int = 0
    jupiter_http_error_count: int = 0
    polymarket_http_error_count: int = 0
    jupiter_last_remaining: str = ""
    jupiter_last_reset: str = ""


@dataclass
class StudyRecorder:
    out_dir: Path
    market_slug: str
    event_id: str
    low_confidence: bool
    rate_limits: RateLimitStats
    ticks_writer: csv.DictWriter
    events_writer: csv.DictWriter
    previous_mid: dict[tuple[str, str], float] = field(default_factory=dict)
    tick_counts: dict[str, int] = field(default_factory=dict)
    first_tick_ms: dict[str, int] = field(default_factory=dict)
    last_tick_ms: dict[str, int] = field(default_factory=dict)
    changes: list[ChangeEvent] = field(default_factory=list)
    lock: asyncio.Lock = field(default_factory=asyncio.Lock)

    async def record_snapshot(self, snapshot: PriceSnapshot) -> None:
        if snapshot.mid is None or not math.isfinite(snapshot.mid):
            return
        async with self.lock:
            key = (snapshot.platform, snapshot.side)
            previous = self.previous_mid.get(key)
            self.previous_mid[key] = snapshot.mid
            self.tick_counts[snapshot.platform] = self.tick_counts.get(snapshot.platform, 0) + 1
            self.first_tick_ms.setdefault(snapshot.platform, snapshot.received_at_ms)
            self.last_tick_ms[snapshot.platform] = snapshot.received_at_ms
            self.ticks_writer.writerow(snapshot_to_row(snapshot))
            if previous is None or abs(snapshot.mid - previous) < PRICE_EPSILON:
                return
            direction = 1 if snapshot.mid > previous else -1
            event = ChangeEvent(
                ts_ms=snapshot.received_at_ms,
                platform=snapshot.platform,
                side=snapshot.side,
                old_mid=previous,
                new_mid=snapshot.mid,
                direction=direction,
                source=snapshot.source,
                source_age_ms=snapshot.source_age_ms(),
            )
            self.changes.append(event)
            self.events_writer.writerow(change_to_row(event))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Jupiter vs Polymarket BTC 5m speed test")
    parser.add_argument("--duration-min", type=float, default=10.0)
    parser.add_argument("--asset", default="btc", choices=["btc"])
    parser.add_argument("--output-dir", default="")
    parser.add_argument("--dry-run-discovery", action="store_true")
    parser.add_argument("--jupiter-interval-ms", type=int, default=None)
    parser.add_argument("--polymarket-rest-interval-ms", type=int, default=500)
    parser.add_argument(
        "--polymarket-mode",
        choices=["auto", "ws", "rest"],
        default="auto",
        help="auto uses CLOB websocket when websockets is installed, plus REST fallback.",
    )
    parser.add_argument("--http-timeout-sec", type=float, default=10.0)
    return parser.parse_args()


def now_ms() -> int:
    return int(time.time() * 1000)


def iso_ms(ts_ms: int) -> str:
    return datetime.fromtimestamp(ts_ms / 1000.0, tz=timezone.utc).isoformat()


def resolve_output_dir(raw: str) -> Path:
    if raw:
        out_dir = Path(raw)
    else:
        stamp = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
        out_dir = Path(__file__).resolve().parent.parent / "analysis" / f"jupiter_poly_speed_{stamp}"
    out_dir.mkdir(parents=True, exist_ok=True)
    return out_dir


def parse_f64(value: Any) -> float | None:
    if value is None or value == "":
        return None
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        return None
    if not math.isfinite(parsed):
        return None
    return parsed


def parse_i64(value: Any) -> int | None:
    if value is None or value == "":
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


def parse_json_list(value: Any) -> list[Any]:
    if isinstance(value, list):
        return value
    if isinstance(value, str):
        try:
            parsed = json.loads(value)
        except json.JSONDecodeError:
            return []
        return parsed if isinstance(parsed, list) else []
    return []


def market_slug_parts(slug: str) -> tuple[str, str, int] | None:
    parts = slug.strip().lower().split("-")
    if len(parts) != 4 or parts[1] != "updown":
        return None
    asset, timeframe, start_raw = parts[0], parts[2], parts[3]
    if timeframe not in WINDOW_SECONDS:
        return None
    start_ts = parse_i64(start_raw)
    if start_ts is None:
        return None
    return asset, timeframe, start_ts


def request_headers(jupiter_api_key: str | None = None) -> dict[str, str]:
    headers = {"User-Agent": USER_AGENT, "Accept": "application/json"}
    if jupiter_api_key:
        headers["x-api-key"] = jupiter_api_key
    return headers


def http_get_json(
    url: str,
    *,
    timeout_sec: float,
    headers: dict[str, str] | None = None,
) -> HttpJsonResponse:
    req = urllib.request.Request(url, headers=headers or request_headers())
    try:
        with urllib.request.urlopen(req, timeout=timeout_sec) as resp:
            body = resp.read().decode("utf-8")
            payload = json.loads(body) if body else None
            return HttpJsonResponse(
                status=resp.status,
                payload=payload,
                headers={k.lower(): v for k, v in resp.headers.items()},
            )
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        try:
            payload = json.loads(body) if body else None
        except json.JSONDecodeError:
            payload = {"error": body[:500]}
        return HttpJsonResponse(
            status=exc.code,
            payload=payload,
            headers={k.lower(): v for k, v in exc.headers.items()},
        )


def build_polymarket_spec(raw: dict[str, Any]) -> PolymarketSpec | None:
    slug = str(raw.get("slug") or "").strip().lower()
    parts = market_slug_parts(slug)
    if parts is None:
        return None
    asset, timeframe, start_ts = parts
    outcomes = [str(item).strip().lower() for item in parse_json_list(raw.get("outcomes"))]
    tokens = [str(item).strip() for item in parse_json_list(raw.get("clobTokenIds"))]
    token_by_side: dict[str, str] = {}
    for outcome, token in zip(outcomes, tokens):
        if outcome in ("up", "down") and token:
            token_by_side[outcome] = token
    if set(token_by_side) != {"up", "down"}:
        return None
    return PolymarketSpec(
        slug=slug,
        question=str(raw.get("question") or ""),
        asset=asset,
        timeframe=timeframe,
        start_ts=start_ts,
        end_ts=start_ts + WINDOW_SECONDS[timeframe],
        condition_id=raw.get("conditionId") or raw.get("condition_id"),
        token_by_side=token_by_side,
    )


def select_active_polymarket(markets: list[Any], asset: str, now_seconds: int) -> PolymarketSpec:
    specs = []
    for item in markets:
        if not isinstance(item, dict):
            continue
        spec = build_polymarket_spec(item)
        if spec and spec.asset == asset:
            specs.append(spec)
    if not specs:
        raise RuntimeError(f"No active {asset} 5m Polymarket market found")

    in_window = [spec for spec in specs if spec.start_ts <= now_seconds < spec.end_ts]
    if in_window:
        return sorted(in_window, key=lambda spec: spec.start_ts)[-1]

    future = [spec for spec in specs if spec.start_ts > now_seconds]
    if future:
        return sorted(future, key=lambda spec: spec.start_ts)[0]
    return sorted(specs, key=lambda spec: spec.start_ts)[-1]


def discover_polymarket_spec(
    asset: str,
    timeout_sec: float,
    jupiter_api_key: str | None = None,
) -> PolymarketSpec:
    url = f"{GAMMA_BASE_URL}/markets?active=true&closed=false&limit=1000"
    response = http_get_json(url, timeout_sec=timeout_sec)
    if response.status >= 400:
        raise RuntimeError(f"Polymarket Gamma discovery failed: HTTP {response.status}")
    markets = response.payload if isinstance(response.payload, list) else []
    try:
        return select_active_polymarket(markets, asset, int(time.time()))
    except RuntimeError:
        slug = discover_slug_from_jupiter_search(asset, timeout_sec, jupiter_api_key)
        slug_url = f"{GAMMA_BASE_URL}/markets?slug={urllib.parse.quote(slug)}"
        slug_response = http_get_json(slug_url, timeout_sec=timeout_sec)
        if slug_response.status >= 400:
            raise RuntimeError(f"Polymarket Gamma slug lookup failed: HTTP {slug_response.status}")
        slug_markets = slug_response.payload if isinstance(slug_response.payload, list) else []
        return select_active_polymarket(slug_markets, asset, int(time.time()))


def discover_slug_from_jupiter_search(
    asset: str,
    timeout_sec: float,
    jupiter_api_key: str | None,
) -> str:
    query = urllib.parse.urlencode({"query": "bitcoin", "limit": "20"})
    search_url = f"{JUPITER_PREDICTION_BASE_URL}/events/search?{query}"
    response = http_get_json(
        search_url,
        timeout_sec=timeout_sec,
        headers=request_headers(jupiter_api_key),
    )
    if response.status >= 400:
        raise RuntimeError(f"Jupiter fallback search failed: HTTP {response.status}")
    candidates = []
    current = int(time.time())
    for item in jupiter_search_items(response.payload):
        metadata = item.get("metadata") if isinstance(item.get("metadata"), dict) else {}
        slug = str(metadata.get("slug") or "").strip().lower()
        parts = market_slug_parts(slug)
        if parts is None:
            continue
        item_asset, timeframe, start_ts = parts
        if item_asset != asset or timeframe != "5m":
            continue
        if item.get("isActive") is False:
            continue
        candidates.append((start_ts <= current < start_ts + WINDOW_SECONDS[timeframe], start_ts, slug))
    if not candidates:
        raise RuntimeError(f"No Jupiter fallback slug found for active {asset} 5m market")
    in_window = [item for item in candidates if item[0]]
    selected = sorted(in_window or candidates, key=lambda item: item[1])[0 if not in_window else -1]
    return selected[2]


def jupiter_search_items(payload: Any) -> list[dict[str, Any]]:
    data = payload.get("data") if isinstance(payload, dict) else payload
    if isinstance(data, list):
        return [item for item in data if isinstance(item, dict)]
    if isinstance(data, dict):
        events = data.get("events")
        if isinstance(events, list):
            return [item for item in events if isinstance(item, dict)]
    return []


def market_side_from_title(value: Any) -> str | None:
    side = str(value or "").strip().lower()
    if side in ("up", "down"):
        return side
    return None


def build_jupiter_spec(event: dict[str, Any], slug: str) -> JupiterSpec | None:
    event_id = str(event.get("eventId") or event.get("id") or "").strip()
    if not event_id:
        return None
    market_by_side: dict[str, str] = {}
    for market in event.get("markets") or []:
        if not isinstance(market, dict):
            continue
        side = market_side_from_title(market.get("title") or (market.get("metadata") or {}).get("title"))
        market_id = str(market.get("marketId") or market.get("id") or "").strip()
        if side and market_id:
            market_by_side[side] = market_id
    return JupiterSpec(event_id=event_id, slug=slug, market_by_side=market_by_side)


def discover_jupiter_spec(
    slug: str,
    timeout_sec: float,
    jupiter_api_key: str | None,
) -> tuple[JupiterSpec, dict[str, str]]:
    query = urllib.parse.urlencode({"query": "bitcoin", "limit": "20"})
    search_url = f"{JUPITER_PREDICTION_BASE_URL}/events/search?{query}"
    headers = request_headers(jupiter_api_key)
    response = http_get_json(search_url, timeout_sec=timeout_sec, headers=headers)
    if response.status >= 400:
        raise RuntimeError(f"Jupiter event search failed: HTTP {response.status}")
    matched_event_id = None
    for item in jupiter_search_items(response.payload):
        metadata = item.get("metadata") if isinstance(item.get("metadata"), dict) else {}
        if str(metadata.get("slug") or "").strip().lower() == slug:
            matched_event_id = str(item.get("eventId") or item.get("id") or "").strip()
            break
    if not matched_event_id:
        raise RuntimeError(f"No Jupiter event matched metadata.slug={slug}")

    event_url = f"{JUPITER_PREDICTION_BASE_URL}/events/{matched_event_id}"
    event_response = http_get_json(event_url, timeout_sec=timeout_sec, headers=headers)
    if event_response.status >= 400:
        raise RuntimeError(f"Jupiter event fetch failed: HTTP {event_response.status}")
    event = event_response.payload.get("data") if isinstance(event_response.payload, dict) else None
    if not isinstance(event, dict):
        event = event_response.payload if isinstance(event_response.payload, dict) else {}
    spec = build_jupiter_spec(event, slug)
    if spec is None:
        raise RuntimeError(f"Jupiter event {matched_event_id} did not include usable markets")
    return spec, event_response.headers


def normalize_jupiter_price(value: Any) -> float | None:
    raw = parse_f64(value)
    if raw is None:
        return None
    if raw < 0:
        return None
    if raw <= 1:
        return raw
    if raw <= 100:
        return raw / 100.0
    return raw / 1_000_000.0


def first_number_from_levels(levels: Any, prefer: str) -> float | None:
    if not isinstance(levels, list):
        return None
    prices = []
    for level in levels:
        value = None
        if isinstance(level, dict):
            value = level.get("price") or level.get("px") or level.get("p")
        elif isinstance(level, list) and level:
            value = level[0]
        price = normalize_jupiter_price(value)
        if price is not None and price > 0:
            prices.append(price)
    if not prices:
        return None
    return max(prices) if prefer == "bid" else min(prices)


def jupiter_snapshot_from_market(
    raw_market: dict[str, Any],
    spec: JupiterSpec,
    market_slug: str,
    received_at_ms: int,
    rate_limit_remaining: str = "",
) -> PriceSnapshot | None:
    side = market_side_from_title(raw_market.get("title") or (raw_market.get("metadata") or {}).get("title"))
    market_id = str(raw_market.get("marketId") or raw_market.get("id") or "").strip()
    if side is None:
        for known_side, known_market_id in spec.market_by_side.items():
            if market_id == known_market_id:
                side = known_side
                break
    if side is None or not market_id:
        return None

    pricing = raw_market.get("pricing") if isinstance(raw_market.get("pricing"), dict) else {}
    buy_yes = normalize_jupiter_price(pricing.get("buyYesPriceUsd"))
    sell_yes = normalize_jupiter_price(pricing.get("sellYesPriceUsd"))
    buy_no = normalize_jupiter_price(pricing.get("buyNoPriceUsd"))
    sell_no = normalize_jupiter_price(pricing.get("sellNoPriceUsd"))
    bid = sell_yes
    ask = buy_yes
    mid = midpoint_or_single(bid, ask)
    provider_ts = epoch_to_ms(
        raw_market.get("updatedAt") or raw_market.get("updated_at") or raw_market.get("timestamp")
    )
    return PriceSnapshot(
        platform="jupiter",
        side=side,
        market_slug=market_slug,
        market_id=market_id,
        event_id=spec.event_id,
        bid=bid,
        ask=ask,
        mid=mid,
        buy_yes=buy_yes,
        sell_yes=sell_yes,
        buy_no=buy_no,
        sell_no=sell_no,
        status=str(raw_market.get("status") or ""),
        source="jupiter_event",
        received_at_ms=received_at_ms,
        provider_ts_ms=provider_ts,
        rate_limit_remaining=rate_limit_remaining,
    )


def snapshots_from_jupiter_event(
    payload: Any,
    spec: JupiterSpec,
    market_slug: str,
    received_at_ms: int,
    rate_limit_remaining: str = "",
) -> list[PriceSnapshot]:
    event = payload.get("data") if isinstance(payload, dict) else None
    if not isinstance(event, dict):
        event = payload if isinstance(payload, dict) else {}
    snapshots = []
    for raw_market in event.get("markets") or []:
        if isinstance(raw_market, dict):
            snapshot = jupiter_snapshot_from_market(
                raw_market,
                spec,
                market_slug,
                received_at_ms,
                rate_limit_remaining,
            )
            if snapshot and snapshot.mid is not None:
                snapshots.append(snapshot)
    return snapshots


def update_snapshot_from_jupiter_orderbook(snapshot: PriceSnapshot, orderbook: Any) -> PriceSnapshot:
    if not isinstance(orderbook, dict):
        return snapshot
    bid = first_number_from_levels(
        orderbook.get("yes_dollars") or orderbook.get("yes") or orderbook.get("bids"),
        "bid",
    )
    ask = first_number_from_levels(
        orderbook.get("no_dollars") or orderbook.get("asks"),
        "ask",
    )
    if bid is None and ask is None:
        return snapshot
    snapshot.bid = bid if bid is not None else snapshot.bid
    snapshot.ask = ask if ask is not None else snapshot.ask
    snapshot.mid = midpoint_or_single(snapshot.bid, snapshot.ask)
    snapshot.source = "jupiter_orderbook"
    return snapshot


def midpoint_or_single(bid: float | None, ask: float | None) -> float | None:
    if bid is not None and ask is not None and bid > 0 and ask > 0:
        return (bid + ask) / 2.0
    if ask is not None and ask > 0:
        return ask
    if bid is not None and bid > 0:
        return bid
    return None


def best_book_level(raw_levels: Any, side: str) -> tuple[float | None, float | None]:
    if not isinstance(raw_levels, list):
        return None, None
    candidates = []
    for row in raw_levels:
        price = size = None
        if isinstance(row, dict):
            price = parse_f64(row.get("price") or row.get("px") or row.get("p"))
            size = parse_f64(row.get("size") or row.get("sz") or row.get("s") or row.get("amount"))
        elif isinstance(row, list) and row:
            price = parse_f64(row[0])
            size = parse_f64(row[1] if len(row) > 1 else None)
        if price is not None and price > 0:
            candidates.append((price, size))
    if not candidates:
        return None, None
    best = (max if side == "bid" else min)(candidates, key=lambda item: item[0])
    return best


def polymarket_snapshot_from_book(
    book: dict[str, Any],
    side: str,
    token_id: str,
    market_slug: str,
    received_at_ms: int,
    source: str,
) -> PriceSnapshot:
    bid, bid_size = best_book_level(book.get("bids"), "bid")
    ask, ask_size = best_book_level(book.get("asks"), "ask")
    return PriceSnapshot(
        platform="polymarket",
        side=side,
        market_slug=market_slug,
        token_id=token_id,
        bid=bid,
        bid_size=bid_size,
        ask=ask,
        ask_size=ask_size,
        mid=midpoint_or_single(bid, ask),
        source=source,
        received_at_ms=received_at_ms,
        provider_ts_ms=epoch_to_ms(book.get("timestamp") or book.get("ts")),
    )


def extract_polymarket_ws_snapshots(payload: Any, spec: PolymarketSpec) -> list[PriceSnapshot]:
    side_by_token = {token: side for side, token in spec.token_by_side.items()}
    items = payload if isinstance(payload, list) else [payload]
    out = []
    fallback_ts = now_ms()
    for item in items:
        if not isinstance(item, dict):
            continue
        nested = item.get("price_changes")
        if isinstance(nested, list):
            out.extend(extract_polymarket_ws_snapshots(nested, spec))
            continue
        token_id = str(item.get("asset_id") or item.get("assetId") or item.get("market") or "").strip()
        side = side_by_token.get(token_id)
        if not side:
            continue
        bid = parse_f64(item.get("best_bid") or item.get("bestBid") or item.get("bid"))
        ask = parse_f64(item.get("best_ask") or item.get("bestAsk") or item.get("ask"))
        book_bid, book_bid_size = best_book_level(item.get("bids"), "bid")
        book_ask, book_ask_size = best_book_level(item.get("asks"), "ask")
        bid = bid if bid is not None else book_bid
        ask = ask if ask is not None else book_ask
        received = now_ms()
        out.append(
            PriceSnapshot(
                platform="polymarket",
                side=side,
                market_slug=spec.slug,
                token_id=token_id,
                bid=bid,
                bid_size=book_bid_size,
                ask=ask,
                ask_size=book_ask_size,
                mid=midpoint_or_single(bid, ask),
                source=str(item.get("event_type") or item.get("type") or "polymarket_ws"),
                received_at_ms=received,
                provider_ts_ms=epoch_to_ms(item.get("timestamp") or item.get("ts")) or fallback_ts,
            )
        )
    return out


def tick_fieldnames() -> list[str]:
    return [
        "ts_ms",
        "received_iso",
        "platform",
        "source",
        "market_slug",
        "event_id",
        "market_id",
        "side",
        "token_id",
        "bid",
        "bid_size",
        "ask",
        "ask_size",
        "mid",
        "buy_yes",
        "sell_yes",
        "buy_no",
        "sell_no",
        "status",
        "source_age_ms",
        "rate_limit_remaining",
    ]


def event_fieldnames() -> list[str]:
    return [
        "ts_ms",
        "received_iso",
        "platform",
        "side",
        "old_mid",
        "new_mid",
        "direction",
        "source",
        "source_age_ms",
    ]


def csv_value(value: Any) -> Any:
    return "" if value is None else value


def snapshot_to_row(snapshot: PriceSnapshot) -> dict[str, Any]:
    return {
        "ts_ms": snapshot.received_at_ms,
        "received_iso": iso_ms(snapshot.received_at_ms),
        "platform": snapshot.platform,
        "source": snapshot.source,
        "market_slug": snapshot.market_slug,
        "event_id": snapshot.event_id,
        "market_id": snapshot.market_id,
        "side": snapshot.side,
        "token_id": snapshot.token_id,
        "bid": csv_value(snapshot.bid),
        "bid_size": csv_value(snapshot.bid_size),
        "ask": csv_value(snapshot.ask),
        "ask_size": csv_value(snapshot.ask_size),
        "mid": csv_value(snapshot.mid),
        "buy_yes": csv_value(snapshot.buy_yes),
        "sell_yes": csv_value(snapshot.sell_yes),
        "buy_no": csv_value(snapshot.buy_no),
        "sell_no": csv_value(snapshot.sell_no),
        "status": snapshot.status,
        "source_age_ms": csv_value(snapshot.source_age_ms()),
        "rate_limit_remaining": snapshot.rate_limit_remaining,
    }


def change_to_row(change: ChangeEvent) -> dict[str, Any]:
    return {
        "ts_ms": change.ts_ms,
        "received_iso": iso_ms(change.ts_ms),
        "platform": change.platform,
        "side": change.side,
        "old_mid": change.old_mid,
        "new_mid": change.new_mid,
        "direction": change.direction,
        "source": change.source,
        "source_age_ms": csv_value(change.source_age_ms),
    }


def write_csv(path: Path, fieldnames: list[str]) -> csv.DictWriter:
    handle = path.open("w", newline="", encoding="utf-8")
    writer = csv.DictWriter(handle, fieldnames=fieldnames)
    writer.writeheader()
    writer._handle = handle  # type: ignore[attr-defined]
    return writer


def close_writer(writer: csv.DictWriter) -> None:
    handle = getattr(writer, "_handle", None)
    if handle:
        handle.close()


async def jupiter_loop(
    spec: JupiterSpec,
    market_slug: str,
    recorder: StudyRecorder,
    stop: asyncio.Event,
    interval_ms: int,
    timeout_sec: float,
    jupiter_api_key: str | None,
) -> None:
    headers = request_headers(jupiter_api_key)
    event_url = f"{JUPITER_PREDICTION_BASE_URL}/events/{spec.event_id}"
    while not stop.is_set():
        loop_started = time.monotonic()
        response = await asyncio.to_thread(
            http_get_json,
            event_url,
            timeout_sec=timeout_sec,
            headers=headers,
        )
        recorder.rate_limits.jupiter_last_remaining = response.headers.get("x-ratelimit-remaining", "")
        recorder.rate_limits.jupiter_last_reset = response.headers.get("x-ratelimit-reset", "")
        if response.status == 429:
            recorder.rate_limits.jupiter_429_count += 1
        elif response.status >= 400:
            recorder.rate_limits.jupiter_http_error_count += 1
        else:
            received = now_ms()
            snapshots = snapshots_from_jupiter_event(
                response.payload,
                spec,
                market_slug,
                received,
                recorder.rate_limits.jupiter_last_remaining,
            )
            if not snapshots:
                snapshots = await fetch_jupiter_market_fallbacks(
                    spec,
                    market_slug,
                    timeout_sec,
                    headers,
                    recorder,
                )
            for snapshot in snapshots:
                await recorder.record_snapshot(snapshot)
        sleep_s = max(0.0, interval_ms / 1000.0 - (time.monotonic() - loop_started))
        await asyncio.sleep(sleep_s)


async def fetch_jupiter_market_fallbacks(
    spec: JupiterSpec,
    market_slug: str,
    timeout_sec: float,
    headers: dict[str, str],
    recorder: StudyRecorder,
) -> list[PriceSnapshot]:
    snapshots = []
    for side, market_id in spec.market_by_side.items():
        market_url = f"{JUPITER_PREDICTION_BASE_URL}/markets/{market_id}"
        response = await asyncio.to_thread(
            http_get_json,
            market_url,
            timeout_sec=timeout_sec,
            headers=headers,
        )
        recorder.rate_limits.jupiter_last_remaining = response.headers.get("x-ratelimit-remaining", "")
        if response.status == 429:
            recorder.rate_limits.jupiter_429_count += 1
            continue
        if response.status >= 400:
            recorder.rate_limits.jupiter_http_error_count += 1
            continue
        raw_market = response.payload.get("data") if isinstance(response.payload, dict) else None
        if not isinstance(raw_market, dict):
            raw_market = response.payload if isinstance(response.payload, dict) else {}
        raw_market.setdefault("marketId", market_id)
        raw_market.setdefault("title", side.capitalize())
        snapshot = jupiter_snapshot_from_market(
            raw_market,
            spec,
            market_slug,
            now_ms(),
            recorder.rate_limits.jupiter_last_remaining,
        )
        if snapshot is None or snapshot.mid is None:
            orderbook_url = f"{JUPITER_PREDICTION_BASE_URL}/orderbook/{market_id}"
            orderbook_response = await asyncio.to_thread(
                http_get_json,
                orderbook_url,
                timeout_sec=timeout_sec,
                headers=headers,
            )
            if orderbook_response.status == 429:
                recorder.rate_limits.jupiter_429_count += 1
                continue
            if orderbook_response.status >= 400:
                recorder.rate_limits.jupiter_http_error_count += 1
                continue
            if snapshot is None:
                snapshot = PriceSnapshot(
                    platform="jupiter",
                    side=side,
                    market_slug=market_slug,
                    market_id=market_id,
                    event_id=spec.event_id,
                    received_at_ms=now_ms(),
                    source="jupiter_orderbook",
                )
            snapshot = update_snapshot_from_jupiter_orderbook(snapshot, orderbook_response.payload)
        if snapshot and snapshot.mid is not None:
            snapshots.append(snapshot)
    return snapshots


async def polymarket_rest_loop(
    spec: PolymarketSpec,
    recorder: StudyRecorder,
    stop: asyncio.Event,
    interval_ms: int,
    timeout_sec: float,
) -> None:
    token_items = list(spec.token_by_side.items())
    while not stop.is_set():
        loop_started = time.monotonic()
        for side, token_id in token_items:
            url = f"{POLYMARKET_CLOB_BASE_URL}/book?token_id={urllib.parse.quote(token_id)}"
            response = await asyncio.to_thread(http_get_json, url, timeout_sec=timeout_sec)
            if response.status >= 400 or not isinstance(response.payload, dict):
                recorder.rate_limits.polymarket_http_error_count += 1
                continue
            snapshot = polymarket_snapshot_from_book(
                response.payload,
                side,
                token_id,
                spec.slug,
                now_ms(),
                "polymarket_rest_book",
            )
            await recorder.record_snapshot(snapshot)
        sleep_s = max(0.0, interval_ms / 1000.0 - (time.monotonic() - loop_started))
        await asyncio.sleep(sleep_s)


async def polymarket_ws_loop(
    spec: PolymarketSpec,
    recorder: StudyRecorder,
    stop: asyncio.Event,
) -> None:
    try:
        import websockets
    except ImportError:
        return

    sub_msg = json.dumps(
        {
            "type": "market",
            "assets_ids": list(spec.token_by_side.values()),
            "initial_dump": True,
            "custom_feature_enabled": True,
        }
    )
    while not stop.is_set():
        try:
            async with websockets.connect(POLYMARKET_CLOB_WS_URL, ping_interval=20) as ws:
                await ws.send(sub_msg)
                while not stop.is_set():
                    raw = await asyncio.wait_for(ws.recv(), timeout=30)
                    if isinstance(raw, bytes):
                        raw = raw.decode("utf-8", errors="replace")
                    payload = json.loads(raw)
                    snapshots = extract_polymarket_ws_snapshots(payload, spec)
                    for snapshot in snapshots:
                        await recorder.record_snapshot(snapshot)
        except asyncio.CancelledError:
            raise
        except Exception:
            await asyncio.sleep(1.0)


def summarize_changes(changes: list[ChangeEvent]) -> dict[str, Any]:
    first_by_platform_side: dict[str, dict[str, Any]] = {}
    for change in sorted(changes, key=lambda item: item.ts_ms):
        key = f"{change.platform}:{change.side}"
        first_by_platform_side.setdefault(key, change_to_summary(change))

    deltas = {}
    for side in ("up", "down"):
        jupiter = first_by_platform_side.get(f"jupiter:{side}")
        poly = first_by_platform_side.get(f"polymarket:{side}")
        if jupiter and poly:
            deltas[side] = int(jupiter["ts_ms"]) - int(poly["ts_ms"])
        else:
            deltas[side] = None

    matched_direction = None
    for side in ("up", "down"):
        for direction in (1, -1):
            j = first_change(changes, "jupiter", side, direction)
            p = first_change(changes, "polymarket", side, direction)
            if j and p:
                candidate = {
                    "side": side,
                    "direction": direction,
                    "delta_ms": j.ts_ms - p.ts_ms,
                    "jupiter_ts_ms": j.ts_ms,
                    "polymarket_ts_ms": p.ts_ms,
                }
                if matched_direction is None or abs(candidate["delta_ms"]) < abs(matched_direction["delta_ms"]):
                    matched_direction = candidate

    return {
        "first_by_platform_side": first_by_platform_side,
        "first_change_delta_ms": deltas,
        "first_matched_direction_delta": matched_direction,
    }


def change_to_summary(change: ChangeEvent) -> dict[str, Any]:
    return {
        "ts_ms": change.ts_ms,
        "received_iso": iso_ms(change.ts_ms),
        "old_mid": change.old_mid,
        "new_mid": change.new_mid,
        "direction": change.direction,
        "source": change.source,
        "source_age_ms": change.source_age_ms,
    }


def first_change(
    changes: list[ChangeEvent],
    platform: str,
    side: str,
    direction: int,
) -> ChangeEvent | None:
    for change in sorted(changes, key=lambda item: item.ts_ms):
        if change.platform == platform and change.side == side and change.direction == direction:
            return change
    return None


def render_summary(summary: dict[str, Any]) -> str:
    meta = summary["meta"]
    lines = [
        "## Jupiter vs Polymarket BTC 5m speed test",
        "",
        f"- Market: `{meta['market_slug']}`",
        f"- Jupiter event: `{meta['jupiter_event_id']}`",
        f"- Duration: {meta['duration_min']:.2f} min",
        f"- Low confidence: {str(meta['low_confidence']).lower()}",
        "",
        "### First change delta",
    ]
    deltas = summary["changes"]["first_change_delta_ms"]
    for side in ("up", "down"):
        value = deltas.get(side)
        if value is None:
            lines.append(f"- {side}: n/a")
        elif value < 0:
            lines.append(f"- {side}: Jupiter first by {abs(value)}ms")
        elif value > 0:
            lines.append(f"- {side}: Polymarket first by {value}ms")
        else:
            lines.append(f"- {side}: same millisecond")
    matched = summary["changes"].get("first_matched_direction_delta")
    if matched:
        delta = matched["delta_ms"]
        leader = "Jupiter" if delta < 0 else "Polymarket" if delta > 0 else "Tie"
        lines.append(
            f"- matched direction {matched['side']} dir={matched['direction']}: "
            f"{leader} delta_ms={delta}"
        )
    lines.extend(["", "### Coverage"])
    for platform, block in summary["coverage"].items():
        lines.append(
            f"- {platform}: ticks={block['ticks']} "
            f"first={block.get('first_iso', 'n/a')} last={block.get('last_iso', 'n/a')}"
        )
    lines.extend(
        [
            "",
            "### Rate limits",
            f"- Jupiter 429: {summary['rate_limits']['jupiter_429_count']}",
            f"- Jupiter HTTP errors: {summary['rate_limits']['jupiter_http_error_count']}",
            f"- Polymarket HTTP errors: {summary['rate_limits']['polymarket_http_error_count']}",
            f"- Jupiter last remaining: {summary['rate_limits']['jupiter_last_remaining'] or 'n/a'}",
            "",
            "### Files",
            "- `ticks.csv`",
            "- `events.csv`",
            "- `summary.json`",
        ]
    )
    return "\n".join(lines) + "\n"


def build_summary(
    recorder: StudyRecorder,
    duration_min: float,
    jupiter_interval_ms: int,
    polymarket_mode: str,
) -> dict[str, Any]:
    coverage = {}
    for platform in ("jupiter", "polymarket"):
        first = recorder.first_tick_ms.get(platform)
        last = recorder.last_tick_ms.get(platform)
        coverage[platform] = {
            "ticks": recorder.tick_counts.get(platform, 0),
            "first_ts_ms": first,
            "last_ts_ms": last,
            "first_iso": iso_ms(first) if first else None,
            "last_iso": iso_ms(last) if last else None,
        }
    return {
        "meta": {
            "market_slug": recorder.market_slug,
            "jupiter_event_id": recorder.event_id,
            "duration_min": duration_min,
            "jupiter_interval_ms": jupiter_interval_ms,
            "polymarket_mode": polymarket_mode,
            "low_confidence": recorder.low_confidence,
            "output_dir": str(recorder.out_dir),
        },
        "coverage": coverage,
        "changes": summarize_changes(recorder.changes),
        "rate_limits": recorder.rate_limits.__dict__,
    }


async def run(args: argparse.Namespace) -> None:
    api_key = os.environ.get(JUPITER_API_KEY_ENV, "").strip() or None
    jupiter_interval_ms = args.jupiter_interval_ms
    if jupiter_interval_ms is None:
        jupiter_interval_ms = 500 if api_key else 2000
    low_confidence = api_key is None or jupiter_interval_ms > 1000

    poly_spec = discover_polymarket_spec(args.asset, args.http_timeout_sec, api_key)
    jup_spec, jupiter_headers = discover_jupiter_spec(
        poly_spec.slug,
        args.http_timeout_sec,
        api_key,
    )
    if not jup_spec.market_by_side:
        raise RuntimeError("Jupiter discovery did not return Up/Down market ids")

    discovery = {
        "polymarket": poly_spec.__dict__,
        "jupiter": jup_spec.__dict__,
        "jupiter_rate_limit_remaining": jupiter_headers.get("x-ratelimit-remaining", ""),
        "low_confidence": low_confidence,
        "jupiter_interval_ms": jupiter_interval_ms,
    }
    if args.dry_run_discovery:
        print(json.dumps(discovery, indent=2), flush=True)
        return

    out_dir = resolve_output_dir(args.output_dir)
    (out_dir / "discovery.json").write_text(json.dumps(discovery, indent=2), encoding="utf-8")
    ticks_writer = write_csv(out_dir / "ticks.csv", tick_fieldnames())
    events_writer = write_csv(out_dir / "events.csv", event_fieldnames())
    recorder = StudyRecorder(
        out_dir=out_dir,
        market_slug=poly_spec.slug,
        event_id=jup_spec.event_id,
        low_confidence=low_confidence,
        rate_limits=RateLimitStats(jupiter_last_remaining=jupiter_headers.get("x-ratelimit-remaining", "")),
        ticks_writer=ticks_writer,
        events_writer=events_writer,
    )

    stop = asyncio.Event()
    tasks = [
        asyncio.create_task(
            jupiter_loop(
                jup_spec,
                poly_spec.slug,
                recorder,
                stop,
                jupiter_interval_ms,
                args.http_timeout_sec,
                api_key,
            )
        )
    ]
    use_ws = args.polymarket_mode in ("auto", "ws")
    if use_ws:
        tasks.append(asyncio.create_task(polymarket_ws_loop(poly_spec, recorder, stop)))
    if args.polymarket_mode in ("auto", "rest"):
        tasks.append(
            asyncio.create_task(
                polymarket_rest_loop(
                    poly_spec,
                    recorder,
                    stop,
                    args.polymarket_rest_interval_ms,
                    args.http_timeout_sec,
                )
            )
        )

    try:
        await asyncio.sleep(max(0.0, args.duration_min * 60.0))
    finally:
        stop.set()
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        close_writer(ticks_writer)
        close_writer(events_writer)

    summary = build_summary(recorder, args.duration_min, jupiter_interval_ms, args.polymarket_mode)
    (out_dir / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    (out_dir / "summary.md").write_text(render_summary(summary), encoding="utf-8")
    print(render_summary(summary), flush=True)
    print(f"Wrote {out_dir}", flush=True)


def main() -> None:
    args = parse_args()
    try:
        asyncio.run(run(args))
    except KeyboardInterrupt:
        raise
    except Exception as exc:
        sys.exit(f"ERROR: {type(exc).__name__}: {exc}")


if __name__ == "__main__":
    main()
