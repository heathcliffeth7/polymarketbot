#!/usr/bin/env python3
"""
PTB/CEX drift diagnostic for recent trade_flow_events.

Reads Postgres through psql, normalizes the rich PTB guard payloads, and reports
why Up/Down entries are being blocked without changing any runtime setting.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import os
import re
import subprocess
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

MARKET_RE = re.compile(r"^(btc|eth|sol|xrp)-updown-(5m|15m)-(\d+)$", re.IGNORECASE)
EVENT_TYPES = (
    "price_to_beat_iv_mismatch_edge_decision",
    "pre_order_price_to_beat_blocked",
)
DEFAULT_ENV_FILE = "/etc/dextrabot/dextrabot.env"
DEFAULT_DIFF_BPS_SUSPECT = 5.0
DEFAULT_STALE_MS = 5_000
DEFAULT_OPEN_TOLERANCE_MS = 1_500
ASSET_CHOP_USD = {
    "btc": 5.0,
    "eth": 0.25,
    "sol": 0.02,
    "xrp": 0.001,
}


@dataclass
class VenueGap:
    venue: str
    gap: float | None
    opposite_gap: float | None
    passed: bool | None
    opposite_passed: bool | None
    stale: bool | None
    open_timestamp_ms: int | None
    current_timestamp_ms: int | None
    open_mid: float | None
    current_mid: float | None


@dataclass
class NormalizedEvent:
    event_id: int
    event_type: str
    created_at: str
    asset: str
    market_slug: str
    outcome: str
    reason: str
    cex_reason: str
    classification: str
    classification_notes: list[str] = field(default_factory=list)
    decision_gap_source: str = ""
    chainlink_signed_gap: float | None = None
    conservative_cex_gap: float | None = None
    effective_gap: float | None = None
    chainlink_cex_diff_usd: float | None = None
    chainlink_cex_diff_bps: float | None = None
    gap_strength: float | None = None
    required_gap_strength: float | None = None
    q_final: float | None = None
    edge: float | None = None
    threshold: float | None = None
    cost: float | None = None
    execution_vwap_cent: float | None = None
    execution_vwap_edge_margin: float | None = None
    venues: list[VenueGap] = field(default_factory=list)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Son PTB/CEX bloklarini read-only trade_flow_events uzerinden teshis et."
    )
    parser.add_argument("--minutes", type=int, default=30, help="Lookback dakikasi. Varsayilan: 30")
    parser.add_argument("--asset", choices=["btc", "eth", "sol", "xrp"], help="Opsiyonel asset filtresi")
    parser.add_argument("--market", help="Opsiyonel market slug filtresi")
    parser.add_argument("--database-url", default=os.getenv("DATABASE_URL"), help="Postgres DATABASE_URL")
    parser.add_argument(
        "--env-file",
        default=DEFAULT_ENV_FILE,
        help=f"DATABASE_URL yoksa okunacak env dosyasi. Varsayilan: {DEFAULT_ENV_FILE}",
    )
    parser.add_argument("--limit", type=int, default=20_000, help="Maksimum event satiri. Varsayilan: 20000")
    parser.add_argument("--json", dest="json_path", help="Normalize eventleri JSON dosyasina yaz")
    parser.add_argument("--csv", dest="csv_path", help="Normalize eventleri CSV dosyasina yaz")
    parser.add_argument("--details", type=int, default=8, help="Ornek detay sayisi. Varsayilan: 8")
    parser.add_argument("--stale-ms", type=int, default=DEFAULT_STALE_MS)
    parser.add_argument("--open-tolerance-ms", type=int, default=DEFAULT_OPEN_TOLERANCE_MS)
    parser.add_argument("--diff-bps-suspect", type=float, default=DEFAULT_DIFF_BPS_SUSPECT)
    args = parser.parse_args()
    if args.minutes <= 0:
        raise SystemExit("HATA: --minutes pozitif olmali")
    if args.limit <= 0:
        raise SystemExit("HATA: --limit pozitif olmali")
    if args.market and not MARKET_RE.match(args.market):
        raise SystemExit("HATA: --market beklenen formatta degil: btc|eth|sol|xrp-updown-5m|15m-<start_ts>")
    return args


def load_env_database_url(env_file: str) -> str | None:
    if not env_file:
        return None
    path = Path(env_file)
    if not path.exists() or not os.access(path, os.R_OK):
        return None
    for line in path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or "=" not in stripped:
            continue
        key, value = stripped.split("=", 1)
        if key.strip() == "DATABASE_URL":
            return value.strip().strip('"').strip("'")
    return None


def sql_literal(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def run_psql(database_url: str, sql: str) -> list[dict[str, Any]]:
    cmd = ["psql", database_url, "-X", "-v", "ON_ERROR_STOP=1", "-tA", "-c", sql]
    try:
        completed = subprocess.run(cmd, check=True, capture_output=True, text=True)
    except FileNotFoundError:
        raise SystemExit("HATA: psql bulunamadi; postgresql-client gerekli.") from None
    except subprocess.CalledProcessError as exc:
        stderr = exc.stderr.strip()
        raise SystemExit(f"HATA: psql sorgusu basarisiz: {stderr}") from None

    rows: list[dict[str, Any]] = []
    for line in completed.stdout.splitlines():
        if not line:
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as exc:
            raise SystemExit(f"HATA: psql JSON satiri parse edilemedi: {exc}") from None
        if isinstance(row, dict):
            rows.append(row)
    return rows


def fetch_events(args: argparse.Namespace, database_url: str) -> list[dict[str, Any]]:
    where = [
        "created_at >= NOW() - make_interval(mins => %d)" % args.minutes,
        "event_type IN (%s)" % ", ".join(sql_literal(item) for item in EVENT_TYPES),
    ]
    if args.market:
        where.append("payload_json->>'market_slug' = %s" % sql_literal(args.market))
    elif args.asset:
        where.append("payload_json->>'market_slug' ILIKE %s" % sql_literal(f"{args.asset}-%"))

    sql = """
SELECT jsonb_build_object(
  'id', id,
  'event_type', event_type,
  'created_at', created_at,
  'payload', payload_json
)::text
FROM trade_flow_events
WHERE {where}
ORDER BY created_at DESC
LIMIT {limit};
""".format(where=" AND ".join(where), limit=args.limit)
    return run_psql(database_url, sql)


def as_float(value: Any) -> float | None:
    if value is None:
        return None
    try:
        out = float(value)
    except (TypeError, ValueError):
        return None
    return out if math.isfinite(out) else None


def as_int(value: Any) -> int | None:
    if value is None:
        return None
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def as_bool(value: Any) -> bool | None:
    if isinstance(value, bool):
        return value
    return None


def nested(obj: dict[str, Any], *keys: str) -> Any:
    cur: Any = obj
    for key in keys:
        if not isinstance(cur, dict):
            return None
        cur = cur.get(key)
    return cur


def market_parts(slug: str) -> tuple[str, str, int | None]:
    match = MARKET_RE.match(slug or "")
    if not match:
        return "", "", None
    asset, timeframe, start_ts = match.groups()
    return asset.lower(), timeframe.lower(), int(start_ts)


def normalize_venue(raw: dict[str, Any]) -> VenueGap:
    return VenueGap(
        venue=str(raw.get("venue") or ""),
        gap=as_float(raw.get("gap")),
        opposite_gap=as_float(raw.get("opposite_gap")),
        passed=as_bool(raw.get("pass")),
        opposite_passed=as_bool(raw.get("opposite_pass")),
        stale=as_bool(raw.get("stale")),
        open_timestamp_ms=as_int(raw.get("open_timestamp_ms")),
        current_timestamp_ms=as_int(raw.get("current_timestamp_ms")),
        open_mid=as_float(raw.get("own_5m_open")),
        current_mid=as_float(raw.get("current_mid")),
    )


def normalize_event(row: dict[str, Any], args: argparse.Namespace) -> NormalizedEvent | None:
    payload = row.get("payload")
    if not isinstance(payload, dict):
        return None

    event_type = str(row.get("event_type") or "")
    guard = payload.get("price_to_beat_guard") if isinstance(payload.get("price_to_beat_guard"), dict) else {}
    source = guard if guard else payload
    iv = source.get("iv_mismatch_edge") if isinstance(source.get("iv_mismatch_edge"), dict) else {}
    block_summary = payload.get("block_summary") if isinstance(payload.get("block_summary"), dict) else {}
    cex_result = source.get("cex_entry_consensus_result")
    if not isinstance(cex_result, dict):
        cex_result = iv.get("cex_entry_consensus_result") if isinstance(iv.get("cex_entry_consensus_result"), dict) else {}

    market_slug = str(source.get("market_slug") or payload.get("market_slug") or "")
    asset, _, _ = market_parts(market_slug)
    outcome = str(source.get("direction") or payload.get("outcome_label") or source.get("outcome_label") or "")
    if not outcome and payload.get("node_key"):
        node_key = str(payload.get("node_key"))
        outcome = "Down" if node_key.lower().endswith("_down") else "Up" if node_key.lower().endswith("_up") else ""
    outcome = outcome.lower()

    reason = str(source.get("reason_code") or payload.get("reason_code") or block_summary.get("primary_reason") or "")
    cex_reason = str(cex_result.get("reason") or "")
    venues = [
        normalize_venue(item)
        for item in cex_result.get("venues", [])
        if isinstance(item, dict)
    ]

    event = NormalizedEvent(
        event_id=int(row.get("id") or 0),
        event_type=event_type,
        created_at=str(row.get("created_at") or ""),
        asset=asset,
        market_slug=market_slug,
        outcome=outcome,
        reason=reason or cex_reason or "unknown",
        cex_reason=cex_reason,
        classification="",
        decision_gap_source=str(iv.get("decision_gap_source") or ""),
        chainlink_signed_gap=as_float(iv.get("chainlink_signed_gap")),
        conservative_cex_gap=as_float(iv.get("conservative_cex_gap")),
        effective_gap=as_float(iv.get("effective_consensus_gap_usd")),
        chainlink_cex_diff_usd=as_float(iv.get("chainlink_cex_diff_usd")),
        chainlink_cex_diff_bps=as_float(iv.get("chainlink_cex_diff_bps")),
        gap_strength=as_float(iv.get("gap_strength")),
        required_gap_strength=as_float(iv.get("required_gap_strength") or block_summary.get("required_gap_strength")),
        q_final=as_float(iv.get("q_final") or block_summary.get("q_final")),
        edge=as_float(iv.get("edge")),
        threshold=as_float(iv.get("threshold")),
        cost=as_float(iv.get("cost") or iv.get("decision_cost")),
        execution_vwap_cent=as_float(block_summary.get("execution_vwap_cent") or iv.get("execution_vwap_cent")),
        execution_vwap_edge_margin=as_float(
            block_summary.get("execution_vwap_edge_margin") or iv.get("execution_vwap_edge_margin")
        ),
        venues=venues,
    )
    event.classification, event.classification_notes = classify_event(event, args)
    return event


def classify_event(event: NormalizedEvent, args: argparse.Namespace) -> tuple[str, list[str]]:
    notes: list[str] = []
    _, _, start_ts = market_parts(event.market_slug)
    expected_open_ms = start_ts * 1000 if start_ts is not None else None

    for venue in event.venues:
        if venue.stale:
            notes.append(f"{venue.venue}:stale")
        if expected_open_ms is not None and venue.open_timestamp_ms is not None:
            drift_ms = abs(venue.open_timestamp_ms - expected_open_ms)
            if drift_ms > args.open_tolerance_ms:
                notes.append(f"{venue.venue}:open_drift_ms={drift_ms}")
        if venue.current_timestamp_ms is not None and event.created_at:
            created_ms = parse_created_at_ms(event.created_at)
            if created_ms is not None:
                age_ms = created_ms - venue.current_timestamp_ms
                if age_ms > args.stale_ms:
                    notes.append(f"{venue.venue}:current_age_ms={age_ms}")

    if event.chainlink_cex_diff_bps is not None and abs(event.chainlink_cex_diff_bps) >= args.diff_bps_suspect:
        notes.append(f"chainlink_cex_diff_bps={event.chainlink_cex_diff_bps:.2f}")

    reason_text = " ".join([event.reason, event.cex_reason]).lower()
    if any("open_drift_ms" in item or "current_age_ms" in item or item.endswith(":stale") for item in notes):
        return "data_suspect", notes
    if "execution_vwap" in reason_text or event.execution_vwap_edge_margin is not None:
        return "execution_too_expensive", notes
    if "opposite_venue" in reason_text or "no_clean_pair" in reason_text or venue_signs_disagree(event.venues):
        if event.asset and all_venue_gaps_near_open(event.asset, event.venues):
            notes.append("near_open_chop")
        return "ambiguous_open_chop", notes
    if notes:
        return "data_suspect", notes
    if event.decision_gap_source == "min_chainlink_cex" or (
        event.gap_strength is not None
        and event.required_gap_strength is not None
        and event.gap_strength < event.required_gap_strength
    ):
        return "conservative_guard", notes
    return "other", notes


def parse_created_at_ms(raw: str) -> int | None:
    try:
        normalized = raw.replace("Z", "+00:00")
        dt = datetime.fromisoformat(normalized)
        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=timezone.utc)
        return int(dt.timestamp() * 1000)
    except ValueError:
        return None


def venue_signs_disagree(venues: list[VenueGap]) -> bool:
    signs = set()
    for venue in venues:
        if venue.gap is None:
            continue
        if abs(venue.gap) < 1e-12:
            signs.add(0)
        else:
            signs.add(1 if venue.gap > 0 else -1)
    return len(signs - {0}) > 1


def all_venue_gaps_near_open(asset: str, venues: list[VenueGap]) -> bool:
    threshold = ASSET_CHOP_USD.get(asset, 0.01)
    gaps = [abs(venue.gap) for venue in venues if venue.gap is not None]
    return bool(gaps) and max(gaps) <= threshold


def mean(values: Iterable[float | None]) -> float | None:
    clean = [value for value in values if value is not None and math.isfinite(value)]
    if not clean:
        return None
    return sum(clean) / len(clean)


def max_abs(values: Iterable[float | None]) -> float | None:
    clean = [abs(value) for value in values if value is not None and math.isfinite(value)]
    return max(clean) if clean else None


def fmt(value: Any, digits: int = 4) -> str:
    if value is None:
        return "-"
    if isinstance(value, float):
        return f"{value:.{digits}f}"
    return str(value)


def print_table(title: str, headers: list[str], rows: list[list[Any]]) -> None:
    print(f"\n{title}")
    if not rows:
        print("  veri yok")
        return
    widths = [len(header) for header in headers]
    rendered = [[fmt(cell) for cell in row] for row in rows]
    for row in rendered:
        for idx, cell in enumerate(row):
            widths[idx] = max(widths[idx], len(cell))
    print("  " + "  ".join(header.ljust(widths[idx]) for idx, header in enumerate(headers)))
    print("  " + "  ".join("-" * width for width in widths))
    for row in rendered:
        print("  " + "  ".join(cell.ljust(widths[idx]) for idx, cell in enumerate(row)))


def summarize(events: list[NormalizedEvent], detail_count: int) -> None:
    print(f"PTB/CEX drift diagnostic: events={len(events)}")
    if not events:
        return

    reason_counter = Counter((event.asset, event.outcome, event.reason) for event in events)
    reason_rows = [
        [asset, outcome, reason, count]
        for (asset, outcome, reason), count in reason_counter.most_common(15)
    ]
    print_table("En cok bloklayan reason", ["asset", "outcome", "reason", "count"], reason_rows)

    class_counter = Counter(event.classification for event in events)
    print_table(
        "Teshis siniflari",
        ["classification", "count"],
        [[name, count] for name, count in class_counter.most_common()],
    )

    note_counter = Counter(note.split("=", 1)[0] for event in events for note in event.classification_notes)
    print_table(
        "Veri kalite bayraklari",
        ["flag", "count"],
        [[name, count] for name, count in note_counter.most_common(12)],
    )

    source_counter = Counter(event.decision_gap_source or "-" for event in events if event.event_type.endswith("_decision"))
    print_table(
        "Decision gap source",
        ["source", "count"],
        [[name, count] for name, count in source_counter.most_common(8)],
    )

    grouped: dict[tuple[str, str, str], list[NormalizedEvent]] = defaultdict(list)
    for event in events:
        grouped[(event.asset, event.outcome, event.reason)].append(event)
    metric_rows = []
    for (asset, outcome, reason), group in sorted(grouped.items(), key=lambda item: len(item[1]), reverse=True)[:15]:
        metric_rows.append(
            [
                asset,
                outcome,
                reason,
                len(group),
                mean(event.chainlink_cex_diff_usd for event in group),
                max_abs(event.chainlink_cex_diff_usd for event in group),
                mean(event.chainlink_cex_diff_bps for event in group),
                mean(event.gap_strength for event in group),
                mean(event.required_gap_strength for event in group),
            ]
        )
    print_table(
        "Chainlink/CEX ve gap metrikleri",
        ["asset", "outcome", "reason", "n", "avg_diff_usd", "max_abs_diff", "avg_bps", "avg_gap", "avg_req"],
        metric_rows,
    )

    venue_counter: Counter[tuple[str, str, str, str, bool | None, bool | None]] = Counter()
    for event in events:
        for venue in event.venues:
            venue_counter[(event.asset, event.outcome, event.cex_reason, venue.venue, venue.passed, venue.opposite_passed)] += 1
    venue_rows = [
        [asset, outcome, cex_reason, venue, passed, opposite, count]
        for (asset, outcome, cex_reason, venue, passed, opposite), count in venue_counter.most_common(20)
    ]
    print_table(
        "Venue pass/opposite dagilimi",
        ["asset", "outcome", "cex_reason", "venue", "pass", "opposite", "count"],
        venue_rows,
    )

    detail_events = sorted(
        events,
        key=lambda item: (
            item.classification != "data_suspect",
            item.classification != "ambiguous_open_chop",
            item.created_at,
        ),
    )[:detail_count]
    print("\nOrnek detaylar")
    if not detail_events:
        print("  veri yok")
    for event in detail_events:
        note = ",".join(event.classification_notes) if event.classification_notes else "-"
        print(
            f"  id={event.event_id} {event.created_at} {event.asset}/{event.outcome} "
            f"reason={event.reason} class={event.classification} notes={note}"
        )
        for venue in event.venues:
            print(
                "    "
                f"{venue.venue}: open={fmt(venue.open_mid, 8)} current={fmt(venue.current_mid, 8)} "
                f"gap={fmt(venue.gap, 8)} opposite_gap={fmt(venue.opposite_gap, 8)} "
                f"pass={venue.passed} opposite={venue.opposite_passed}"
            )


def write_json(path: str, events: list[NormalizedEvent]) -> None:
    data = [event_to_dict(event) for event in events]
    Path(path).write_text(json.dumps(data, indent=2, sort_keys=True), encoding="utf-8")


def write_csv(path: str, events: list[NormalizedEvent]) -> None:
    fields = [
        "event_id",
        "created_at",
        "event_type",
        "asset",
        "market_slug",
        "outcome",
        "reason",
        "cex_reason",
        "classification",
        "notes",
        "decision_gap_source",
        "chainlink_cex_diff_usd",
        "chainlink_cex_diff_bps",
        "gap_strength",
        "required_gap_strength",
        "q_final",
        "cost",
        "execution_vwap_cent",
        "execution_vwap_edge_margin",
    ]
    with Path(path).open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        for event in events:
            row = event_to_dict(event)
            row["notes"] = ",".join(event.classification_notes)
            writer.writerow({field: row.get(field) for field in fields})


def event_to_dict(event: NormalizedEvent) -> dict[str, Any]:
    return {
        "event_id": event.event_id,
        "event_type": event.event_type,
        "created_at": event.created_at,
        "asset": event.asset,
        "market_slug": event.market_slug,
        "outcome": event.outcome,
        "reason": event.reason,
        "cex_reason": event.cex_reason,
        "classification": event.classification,
        "classification_notes": event.classification_notes,
        "decision_gap_source": event.decision_gap_source,
        "chainlink_signed_gap": event.chainlink_signed_gap,
        "conservative_cex_gap": event.conservative_cex_gap,
        "effective_gap": event.effective_gap,
        "chainlink_cex_diff_usd": event.chainlink_cex_diff_usd,
        "chainlink_cex_diff_bps": event.chainlink_cex_diff_bps,
        "gap_strength": event.gap_strength,
        "required_gap_strength": event.required_gap_strength,
        "q_final": event.q_final,
        "edge": event.edge,
        "threshold": event.threshold,
        "cost": event.cost,
        "execution_vwap_cent": event.execution_vwap_cent,
        "execution_vwap_edge_margin": event.execution_vwap_edge_margin,
        "venues": [venue.__dict__ for venue in event.venues],
    }


def main() -> None:
    args = parse_args()
    database_url = args.database_url or load_env_database_url(args.env_file)
    if not database_url:
        raise SystemExit(
            "HATA: DATABASE_URL gerekli. Env olarak verin veya okunabilir --env-file kullanin."
        )

    rows = fetch_events(args, database_url)
    events = [event for row in rows if (event := normalize_event(row, args)) is not None]
    summarize(events, args.details)
    if args.json_path:
        write_json(args.json_path, events)
        print(f"\nJSON yazildi: {args.json_path}")
    if args.csv_path:
        write_csv(args.csv_path, events)
        print(f"\nCSV yazildi: {args.csv_path}")


if __name__ == "__main__":
    main()
