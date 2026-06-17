#!/usr/bin/env python3

from __future__ import annotations

import argparse
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import diagnose_ptb_cex_drift as diag


def default_args() -> argparse.Namespace:
    return argparse.Namespace(
        stale_ms=5_000,
        open_tolerance_ms=1_500,
        diff_bps_suspect=5.0,
    )


class DiagnosePtbCexDriftTest(unittest.TestCase):
    def test_normalize_blocked_event_extracts_venues(self) -> None:
        row = {
            "id": 1,
            "event_type": "pre_order_price_to_beat_blocked",
            "created_at": "2026-06-17T15:37:00.000000+00:00",
            "payload": {
                "market_slug": "sol-updown-5m-1781710500",
                "outcome_label": "Up",
                "price_to_beat_guard": {
                    "reason_code": "open_gap_opposite_venue_detected",
                    "market_slug": "sol-updown-5m-1781710500",
                    "cex_entry_consensus_result": {
                        "reason": "open_gap_opposite_venue_detected",
                        "venues": [
                            {
                                "venue": "binance",
                                "own_5m_open": 73.03,
                                "current_mid": 73.025,
                                "gap": -0.005,
                                "opposite_gap": 0.005,
                                "pass": False,
                                "opposite_pass": True,
                                "stale": False,
                                "open_timestamp_ms": 1781710500000,
                                "current_timestamp_ms": 1781710620000,
                            }
                        ],
                    },
                },
            },
        }

        event = diag.normalize_event(row, default_args())

        self.assertIsNotNone(event)
        assert event is not None
        self.assertEqual(event.asset, "sol")
        self.assertEqual(event.outcome, "up")
        self.assertEqual(event.reason, "open_gap_opposite_venue_detected")
        self.assertEqual(event.classification, "ambiguous_open_chop")
        self.assertEqual(event.venues[0].venue, "binance")

    def test_execution_vwap_block_is_execution_too_expensive(self) -> None:
        row = {
            "id": 2,
            "event_type": "price_to_beat_iv_mismatch_edge_decision",
            "created_at": "2026-06-17T15:37:00.000000+00:00",
            "payload": {
                "market_slug": "btc-updown-5m-1781710500",
                "outcome_label": "Up",
                "reason_code": "blocked_execution_vwap_edge_below_threshold",
                "block_summary": {
                    "execution_vwap_edge_margin": -8.0,
                    "required_gap_strength": 2.5,
                },
                "iv_mismatch_edge": {
                    "q_final": 0.57,
                    "cost": 0.60,
                    "gap_strength": 0.37,
                    "chainlink_cex_diff_bps": 0.18,
                },
            },
        }

        event = diag.normalize_event(row, default_args())

        self.assertIsNotNone(event)
        assert event is not None
        self.assertEqual(event.classification, "execution_too_expensive")
        self.assertAlmostEqual(event.q_final or 0, 0.57)

    def test_large_chainlink_cex_bps_marks_data_suspect(self) -> None:
        row = {
            "id": 3,
            "event_type": "price_to_beat_iv_mismatch_edge_decision",
            "created_at": "2026-06-17T15:37:00.000000+00:00",
            "payload": {
                "market_slug": "eth-updown-5m-1781710500",
                "outcome_label": "Down",
                "reason_code": "blocked_chainlink_cex_mixed_gap_fail",
                "iv_mismatch_edge": {
                    "decision_gap_source": "min_chainlink_cex",
                    "chainlink_cex_diff_bps": 8.2,
                },
            },
        }

        event = diag.normalize_event(row, default_args())

        self.assertIsNotNone(event)
        assert event is not None
        self.assertEqual(event.classification, "data_suspect")
        self.assertIn("chainlink_cex_diff_bps=8.20", event.classification_notes)


if __name__ == "__main__":
    unittest.main()
