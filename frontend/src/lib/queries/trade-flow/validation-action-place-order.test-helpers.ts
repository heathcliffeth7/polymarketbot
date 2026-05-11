import assert from 'node:assert/strict';

import { validateActionPlaceOrderConfig } from '@/lib/queries/trade-flow/validation-action-place-order';
import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';

export function buildGenericPresetAction(
  nodeKey: string,
  refKey = 'preset_place_order'
): TradeFlowNode {
  return {
    key: nodeKey,
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
      marketSlug: 'nba-lal-orl-2026-03-21',
      tokenId: 'stale-token',
      outcomeLabel: 'Stale Outcome',
    },
  };
}

export function buildFixedTrigger(
  key: string,
  tokenId: string,
  outcomeLabel: string,
  triggerPriceCent: number
): TradeFlowNode {
  return {
    key,
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
          tokenId,
          outcomeLabel,
          triggerCondition: 'level_above',
          triggerPriceCent,
        },
      ],
    },
  };
}

export function buildAutoScopeTrigger(
  key: string,
  marketScope = 'btc_5m_updown'
): TradeFlowNode {
  return {
    key,
    type: 'trigger.market_price',
    positionX: 0,
    positionY: 0,
    config: {
      marketMode: 'auto_scope',
      marketScope,
      marketSelection: 'latest_by_slug',
      priceMode: 'composite',
      repeatMode: 'once',
      outcomeConditions: [
        {
          triggerCondition: 'level_above',
          triggerPriceCent: 30,
        },
      ],
    },
  };
}

export function collectActionIssues(
  graph: TradeFlowGraph,
  nodeKey: string
): TradeFlowValidationIssue[] {
  const node = graph.nodes.find((item) => item.key === nodeKey);
  assert.ok(node, `node ${nodeKey} should exist`);
  const issues: TradeFlowValidationIssue[] = [];
  validateActionPlaceOrderConfig(issues, node, graph);
  return issues;
}

export function buildReentryAction(
  key: string,
  overrides: Record<string, unknown> = {}
): TradeFlowNode {
  return {
    key,
    type: 'action.place_order',
    positionX: 200,
    positionY: 0,
    config: {
      side: 'buy',
      executionMode: 'market',
      sizeMode: 'usdc',
      sizeUsdc: 10,
      marketSlug: 'nba-lal-orl-2026-03-21',
      tokenId: 'magic-token',
      outcomeLabel: 'Moneyline: Magic',
      slEnabled: true,
      slPriceCent: 45,
      reenterOnSlHit: true,
      reentryMaxAttempts: 2,
      ...overrides,
    },
  };
}
