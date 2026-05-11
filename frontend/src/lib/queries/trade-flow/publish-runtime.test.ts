import assert from 'node:assert/strict';
import test from 'node:test';

import { normalizeTradeFlowGraph } from './graph';
import { normalizeTradeFlowContextForPublish } from './publish-runtime';

test('normalizeTradeFlowContextForPublish clears carried reentry state on publish cutover', () => {
  const graph = normalizeTradeFlowGraph({
    context: {
      marketSlug: 'btc-updown-5m-1774698300',
    },
    nodes: [
      {
        key: 'trigger_reentry',
        type: 'trigger.market_price',
        positionX: 0,
        positionY: 0,
        config: {
          marketMode: 'auto_scope',
          marketScope: 'btc_5m_updown',
          marketSelection: 'latest_by_slug',
          priceMode: 'composite',
          repeatMode: 'once',
          onceScope: 'run',
          outcomeConditions: [
            {
              triggerCondition: 'level_above',
              triggerPriceCent: 45,
            },
          ],
        },
      },
      {
        key: 'action_buy',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          slEnabled: true,
          slPriceCent: 45,
          reenterOnSlHit: true,
          reentryMaxAttempts: 2,
        },
      },
    ],
    edges: [
      {
        key: 'edge_1',
        source: 'trigger_reentry',
        target: 'action_buy',
        type: 'default',
        condition: null,
      },
    ],
  });

  const result = normalizeTradeFlowContextForPublish(
    graph,
    {
      flowContext: {
        marketSlug: 'btc-updown-5m-1774698300',
      },
      vars: {},
      state: {},
      refs: {},
      nodeState: {
        action_buy: {
          reentry_attempts_used: 2,
          reentry_market_slug: 'btc-updown-5m-1774698300',
        },
        trigger_reentry: {
          reentry_generation: 2,
          once_fired: true,
          once_fired_market_slug: 'btc-updown-5m-1774698300',
          once_blocked_logged: true,
        },
      },
    },
    'publish-marker-1'
  );

  const actionState = result.context.nodeState.action_buy as Record<string, unknown>;
  const triggerState = result.context.nodeState.trigger_reentry as Record<string, unknown>;

  assert.equal('reentry_attempts_used' in actionState, false);
  assert.equal('reentry_market_slug' in actionState, false);
  assert.equal('reentry_generation' in triggerState, false);
  assert.deepEqual(result.resetReentryActionNodeKeys, ['action_buy']);
  assert.deepEqual(result.resetReentryTriggerNodeKeys, ['trigger_reentry']);
});
