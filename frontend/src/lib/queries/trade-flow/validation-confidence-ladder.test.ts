import assert from 'node:assert/strict';
import test from 'node:test';

import { validateTradeFlowGraph } from '@/lib/queries/trade-flow/validation';
import type { TradeFlowGraph } from '@/lib/types';

function confidenceLadderGraph(confidenceLadder: Record<string, unknown>): TradeFlowGraph {
  return {
    context: {},
    nodes: [
      {
        key: 'trigger_confidence_ladder',
        type: 'trigger.market_price',
        positionX: 0,
        positionY: 0,
        config: {
          marketMode: 'auto_scope',
          marketScope: 'btc_5m_updown',
          bindingMode: 'confidence_ladder_only',
          repeatMode: 'loop',
          priceMode: 'composite',
          outcomeConditions: [],
          priceToBeatTriggerEnabled: false,
        },
      },
      {
        key: 'action_confidence_ladder',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          mode: 'confidence_ladder_hedge_lock_v1',
          side: 'buy',
          executionMode: 'market',
          tpEnabled: false,
          slEnabled: false,
          ptbStopLossEnabled: false,
          confidenceLadder,
        },
      },
    ],
    edges: [
      {
        key: 'edge_confidence_ladder',
        source: 'trigger_confidence_ladder',
        target: 'action_confidence_ladder',
        type: 'default',
        condition: null,
      },
    ],
  };
}

function errorCodes(graph: TradeFlowGraph): string[] {
  return validateTradeFlowGraph(graph)
    .issues.filter((issue) => issue.severity === 'error')
    .map((issue) => issue.code);
}

test('confidence ladder validation accepts BTC 5m scoped hedge-lock graph', () => {
  assert.deepEqual(
    errorCodes(
      confidenceLadderGraph({
        baseProbeShares: 2,
        maxLossPerMarketUsdc: 3,
        maxTotalCostPerMarketUsdc: 25,
        hardNoChaseAbove: 0.93,
        hedge: {
          damageControlPriceMin: 0.35,
          damageControlPriceMax: 0.60,
        },
      }),
    ),
    [],
  );
});

test('confidence ladder validation rejects invalid risk ranges', () => {
  const codes = errorCodes(
    confidenceLadderGraph({
      baseProbeShares: 0,
      maxLossPerMarketUsdc: -1,
      maxSpread: 1.2,
      entryWindowStartSec: 280,
      entryWindowEndSec: 30,
      hedge: {
        damageControlPriceMin: 0.7,
        damageControlPriceMax: 0.6,
      },
      stop: {
        maxDirectionFlips: -1,
      },
    }),
  );

  assert.ok(codes.includes('invalid_confidence_ladder_base_probe'));
  assert.ok(codes.includes('invalid_confidence_ladder_max_loss'));
  assert.ok(codes.includes('invalid_confidence_ladder_max_spread'));
  assert.ok(codes.includes('invalid_confidence_ladder_entry_window'));
  assert.ok(codes.includes('invalid_confidence_ladder_damage_control_range'));
  assert.ok(codes.includes('invalid_confidence_ladder_max_direction_flips'));
});
