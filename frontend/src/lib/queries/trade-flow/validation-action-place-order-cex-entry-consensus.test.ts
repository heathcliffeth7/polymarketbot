import assert from 'node:assert/strict';
import test from 'node:test';

import { normalizeTradeFlowGraph } from '@/lib/queries/trade-flow/graph';
import {
  buildAutoScopeTrigger,
  collectActionIssues,
} from './validation-action-place-order.test-helpers';

function buildCexConsensusGraph(overrides: Record<string, unknown>) {
  return normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_auto'),
      {
        key: 'cex_consensus_buy',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'btc-updown-5m-1774013100',
          tokenId: 'btc-down-token',
          outcomeLabel: 'Down',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'manual',
          priceToBeatMaxDiff: 0.3,
          priceToBeatMaxDiffUnit: 'usd',
          priceToBeatCurrentPriceSource: 'chainlink_cex_consensus',
          ...overrides,
        },
      },
    ],
    edges: [
      {
        key: 'edge_1',
        source: 'trigger_auto',
        target: 'cex_consensus_buy',
        type: 'default',
        condition: null,
      },
    ],
  });
}

test('validateActionPlaceOrderConfig rejects invalid CEX entry consensus fields', () => {
  const graph = buildCexConsensusGraph({
    cexEntryConsensusBasis: 'own_gap_typo',
    cexEntryConsensusMode: 'okx_plus_one_or_clean_pair',
    cexEntryOpenGapThresholdUsd: 0,
    cexEntryOpenGapMinVenues: 4,
    cexEntryOpenGapRatioMin: 1.5,
    cexEntryOpenGapSpreadFloorUsd: -0.01,
    cexEntryOpenGapSpreadExpectedMoveMult: 0,
    cexEntryOpenGapAllowCleanPairWithoutAnchor: 'maybe',
    cexEntryChainlinkSanityCheck: 'maybe',
  });

  const codes = collectActionIssues(graph, 'cex_consensus_buy').map((issue) => issue.code);

  assert.ok(codes.includes('invalid_cex_entry_consensus_basis'));
  assert.ok(codes.includes('invalid_cex_entry_consensus_mode'));
  assert.ok(codes.includes('invalid_cex_entry_open_gap_threshold_usd'));
  assert.ok(codes.includes('invalid_cex_entry_open_gap_min_venues'));
  assert.ok(codes.includes('invalid_cex_entry_open_gap_ratio_min'));
  assert.ok(codes.includes('invalid_cex_entry_open_gap_spread_floor_usd'));
  assert.ok(codes.includes('invalid_cex_entry_open_gap_spread_expected_move_mult'));
  assert.ok(codes.includes('invalid_cex_entry_open_gap_allow_clean_pair_without_anchor'));
  assert.ok(codes.includes('invalid_cex_entry_chainlink_sanity_check'));
});

test('validateActionPlaceOrderConfig accepts Binance Coinbase CEX entry mode', () => {
  const graph = buildCexConsensusGraph({
    cexEntryConsensusBasis: 'own_open_gap',
    cexEntryConsensusMode: 'binance_coinbase',
  });

  const codes = collectActionIssues(graph, 'cex_consensus_buy').map((issue) => issue.code);

  assert.equal(codes.includes('invalid_cex_entry_consensus_mode'), false);
});

test('validateActionPlaceOrderConfig accepts asset-auto current-price CEX entry mode', () => {
  const graph = buildCexConsensusGraph({
    cexEntryConsensusBasis: 'current_price',
    cexEntryConsensusMode: 'asset_auto_plus_one_or_clean_pair',
  });

  const codes = collectActionIssues(graph, 'cex_consensus_buy').map((issue) => issue.code);

  assert.equal(codes.includes('invalid_cex_entry_consensus_mode'), false);
});

test('validateActionPlaceOrderConfig scopes CEX entry consensus validation to chainlink CEX source', () => {
  const graph = buildCexConsensusGraph({
    priceToBeatCurrentPriceSource: 'chainlink',
    cexEntryConsensusBasis: 'own_gap_typo',
    cexEntryConsensusMode: 'bad',
    cexEntryOpenGapThresholdUsd: 0,
  });

  const codes = collectActionIssues(graph, 'cex_consensus_buy').map((issue) => issue.code);

  assert.equal(codes.some((code) => code.startsWith('invalid_cex_entry_')), false);
});
