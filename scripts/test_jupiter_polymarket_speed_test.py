#!/usr/bin/env python3

from __future__ import annotations

import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import jupiter_polymarket_speed_test as speed


class JupiterPolymarketSpeedTest(unittest.TestCase):
    def test_build_polymarket_spec_parses_gamma_json_strings(self) -> None:
        spec = speed.build_polymarket_spec(
            {
                "slug": "btc-updown-5m-1780431900",
                "question": "Bitcoin Up or Down",
                "conditionId": "0xabc",
                "clobTokenIds": '["tok-up", "tok-down"]',
                "outcomes": '["Up", "Down"]',
            }
        )

        self.assertIsNotNone(spec)
        assert spec is not None
        self.assertEqual(spec.asset, "btc")
        self.assertEqual(spec.timeframe, "5m")
        self.assertEqual(spec.start_ts, 1780431900)
        self.assertEqual(spec.end_ts, 1780432200)
        self.assertEqual(spec.token_by_side, {"up": "tok-up", "down": "tok-down"})

    def test_select_active_polymarket_prefers_current_window(self) -> None:
        markets = [
            {
                "slug": "btc-updown-5m-1000",
                "clobTokenIds": '["old-up", "old-down"]',
                "outcomes": '["Up", "Down"]',
            },
            {
                "slug": "btc-updown-5m-1300",
                "clobTokenIds": '["cur-up", "cur-down"]',
                "outcomes": '["Up", "Down"]',
            },
        ]

        spec = speed.select_active_polymarket(markets, "btc", 1310)

        self.assertEqual(spec.slug, "btc-updown-5m-1300")
        self.assertEqual(spec.token_by_side["up"], "cur-up")

    def test_jupiter_event_parser_normalizes_micro_usd_prices(self) -> None:
        spec = speed.JupiterSpec(
            event_id="POLY-1",
            slug="btc-updown-5m-1000",
            market_by_side={"up": "m-up", "down": "m-down"},
        )
        snapshots = speed.snapshots_from_jupiter_event(
            {
                "eventId": "POLY-1",
                "markets": [
                    {
                        "id": "m-up",
                        "title": "Up",
                        "status": "open",
                        "pricing": {
                            "buyYesPriceUsd": 505000,
                            "sellYesPriceUsd": 495000,
                            "buyNoPriceUsd": 505000,
                            "sellNoPriceUsd": 495000,
                        },
                    }
                ],
            },
            spec,
            "btc-updown-5m-1000",
            123,
            "4",
        )

        self.assertEqual(len(snapshots), 1)
        self.assertEqual(snapshots[0].side, "up")
        self.assertAlmostEqual(snapshots[0].bid or 0, 0.495)
        self.assertAlmostEqual(snapshots[0].ask or 0, 0.505)
        self.assertAlmostEqual(snapshots[0].mid or 0, 0.5)

    def test_summarize_changes_reports_platform_delta(self) -> None:
        changes = [
            speed.ChangeEvent(1000, "polymarket", "up", 0.50, 0.51, 1, "ws", None),
            speed.ChangeEvent(1250, "jupiter", "up", 0.50, 0.51, 1, "event", None),
        ]

        summary = speed.summarize_changes(changes)

        self.assertEqual(summary["first_change_delta_ms"]["up"], 250)
        self.assertEqual(summary["first_matched_direction_delta"]["delta_ms"], 250)


if __name__ == "__main__":
    unittest.main()
