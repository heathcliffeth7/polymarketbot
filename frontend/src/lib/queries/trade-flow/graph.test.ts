import assert from 'node:assert/strict';
import test from 'node:test';

import { normalizeTradeFlowGraph } from '@/lib/queries/trade-flow/graph';
import type { TradeFlowGraph } from '@/lib/types';

function getNodeConfig(graph: TradeFlowGraph, nodeKey: string): Record<string, unknown> {
  const node = graph.nodes.find((item) => item.key === nodeKey);
  assert.ok(node, `node ${nodeKey} should exist`);
  return node.config;
}

function buildFixedTrigger(
  key: string,
  marketSlug: string,
  tokenId: string,
  outcomeLabel: string,
  triggerPriceCent: number
) {
  return {
    key,
    type: 'trigger.market_price',
    positionX: 0,
    positionY: 0,
    config: {
      marketMode: 'fixed',
      priceMode: 'composite',
      marketSlug,
      repeatMode: 'once',
      outcomeConditions: [
        {
          tokenId,
          outcomeLabel,
          triggerCondition: 'level_above',
          triggerPriceCent,
        },
      ],
    },
  };
}

function buildAutoScopeTrigger(key: string) {
  return {
    key,
    type: 'trigger.market_price',
    positionX: 0,
    positionY: 0,
    config: {
      marketMode: 'auto_scope',
      marketScope: 'btc_5m_updown',
      marketSelection: 'latest_by_slug',
      priceMode: 'composite',
      repeatMode: 'once',
      outcomeConditions: [
        {
          triggerCondition: 'level_above',
          triggerPriceCent: 50,
        },
      ],
    },
  };
}

function buildGenericPlaceOrder(
  key: string,
  refKey: string,
  marketSlug: string,
  tokenId: string,
  outcomeLabel: string
) {
  return {
    key,
    type: 'action.place_order',
    positionX: 200,
    positionY: 0,
    config: {
      presetKind: 'place_order',
      refKey,
      side: 'buy',
      executionMode: 'market',
      sizeMode: 'usdc',
      sizeUsdc: 5,
      marketSlug,
      tokenId,
      outcomeLabel,
    },
  };
}

test('normalizeTradeFlowGraph syncs generic preset place_order to unique upstream fixed trigger outcome', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildFixedTrigger(
        'trigger_market',
        'nba-lal-orl-2026-03-21',
        'lakers-token',
        'Moneyline: Lakers',
        50
      ),
      buildGenericPlaceOrder(
        'action_bsz3nb',
        'action_bsz3nb',
        'nba-lal-orl-2026-03-21',
        'lakers-token',
        'Moneyline: Lakers'
      ),
      buildFixedTrigger(
        'trigger_eyh0vk',
        'nba-lal-orl-2026-03-21',
        'magic-token',
        'Moneyline: Magic',
        30
      ),
      buildGenericPlaceOrder(
        'action_k2p4un',
        'action_bsz3nb',
        'nba-lal-orl-2026-03-21',
        'lakers-token',
        'Moneyline: Lakers'
      ),
    ],
    edges: [
      { key: 'edge_1', source: 'trigger_market', target: 'action_bsz3nb', type: 'default', condition: null },
      { key: 'edge_2', source: 'trigger_eyh0vk', target: 'action_k2p4un', type: 'default', condition: null },
    ],
  });

  const actionConfig = getNodeConfig(graph, 'action_k2p4un');
  assert.equal(actionConfig.marketSlug, 'nba-lal-orl-2026-03-21');
  assert.equal(actionConfig.tokenId, 'magic-token');
  assert.equal(actionConfig.outcomeLabel, 'Moneyline: Magic');
  assert.equal(actionConfig.refKey, 'action_k2p4un');
});

test('normalizeTradeFlowGraph rewrites generic preset refKey when it points at another node key', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildGenericPlaceOrder(
        'action_source',
        'action_source',
        'nba-lal-orl-2026-03-21',
        'lakers-token',
        'Moneyline: Lakers'
      ),
      buildGenericPlaceOrder(
        'action_target',
        'action_source',
        'nba-lal-orl-2026-03-21',
        'lakers-token',
        'Moneyline: Lakers'
      ),
    ],
    edges: [],
  });

  const actionConfig = getNodeConfig(graph, 'action_target');
  assert.equal(actionConfig.refKey, 'action_target');
});

test('normalizeTradeFlowGraph clears stale preset market fields when upstream trigger is auto_scope', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_auto'),
      buildGenericPlaceOrder(
        'action_auto',
        'preset_place_order',
        'stale-market',
        'stale-token',
        'Stale Outcome'
      ),
    ],
    edges: [{ key: 'edge_1', source: 'trigger_auto', target: 'action_auto', type: 'default', condition: null }],
  });

  const actionConfig = getNodeConfig(graph, 'action_auto');
  assert.equal('marketSlug' in actionConfig, false);
  assert.equal('tokenId' in actionConfig, false);
  assert.equal('outcomeLabel' in actionConfig, false);
  assert.equal(actionConfig.refKey, 'action_auto');
});

test('normalizeTradeFlowGraph clears stale outcome fields when upstream fixed trigger market is unique but outcome is not', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      {
        key: 'trigger_market',
        type: 'trigger.market_price',
        positionX: 0,
        positionY: 0,
        config: {
          marketMode: 'fixed',
          priceMode: 'composite',
          marketSlug: 'nba-lal-orl-2026-03-21',
          repeatMode: 'once',
          outcomeConditions: [
            {
              tokenId: 'lakers-token',
              outcomeLabel: 'Moneyline: Lakers',
              triggerCondition: 'level_above',
              triggerPriceCent: 50,
            },
            {
              tokenId: 'magic-token',
              outcomeLabel: 'Moneyline: Magic',
              triggerCondition: 'level_above',
              triggerPriceCent: 30,
            },
          ],
        },
      },
      buildGenericPlaceOrder(
        'action_k2p4un',
        'preset_place_order',
        'stale-market',
        'stale-token',
        'Stale Outcome'
      ),
    ],
    edges: [{ key: 'edge_1', source: 'trigger_market', target: 'action_k2p4un', type: 'default', condition: null }],
  });

  const actionConfig = getNodeConfig(graph, 'action_k2p4un');
  assert.equal(actionConfig.marketSlug, 'nba-lal-orl-2026-03-21');
  assert.equal('tokenId' in actionConfig, false);
  assert.equal('outcomeLabel' in actionConfig, false);
  assert.equal(actionConfig.refKey, 'action_k2p4un');
});

test('normalizeTradeFlowGraph leaves quick preset buy/sell place_order refs untouched', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      {
        key: 'action_buy_current_position',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          presetKind: 'buy_current_position',
          refKey: 'preset_buy_current_position',
          side: 'buy',
          executionMode: 'market',
          marketSlug: 'stale-market',
          tokenId: 'stale-token',
          outcomeLabel: 'Stale Outcome',
        },
      },
    ],
    edges: [],
  });

  const actionConfig = getNodeConfig(graph, 'action_buy_current_position');
  assert.equal(actionConfig.refKey, 'preset_buy_current_position');
  assert.equal(actionConfig.marketSlug, 'stale-market');
  assert.equal(actionConfig.tokenId, 'stale-token');
  assert.equal(actionConfig.outcomeLabel, 'Stale Outcome');
});

test('normalizeTradeFlowGraph preserves trigger.market_price custom_range values', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      {
        key: 'trigger_market',
        type: 'trigger.market_price',
        positionX: 0,
        positionY: 0,
        config: {
          marketMode: 'auto_scope',
          marketScope: 'eth_5m_updown',
          marketSelection: 'latest_by_slug',
          priceMode: 'composite',
          repeatMode: 'once',
          cycleWindowMode: 'custom_range',
          cycleWindowStartSec: 240,
          cycleWindowEndSec: 285,
          autoSellOnWindowEnd: true,
          outcomeConditions: [
            {
              tokenId: 'down-token',
              outcomeLabel: 'Down',
              triggerCondition: 'level_above',
              triggerPriceCent: 45,
            },
          ],
        },
      },
    ],
    edges: [],
  });

  const config = getNodeConfig(graph, 'trigger_market');
  assert.equal(config.cycleWindowMode, 'custom_range');
  assert.equal(config.cycleWindowStartSec, 240);
  assert.equal(config.cycleWindowEndSec, 285);
  assert.equal(config.autoSellOnWindowEnd, true);
});
