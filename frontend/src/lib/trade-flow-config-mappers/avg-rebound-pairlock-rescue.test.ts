import assert from 'node:assert/strict';
import test from 'node:test';

import {
  MICRO_AVG_REBOUND_PAIRLOCK_RESCUE_23USDC_CONFIG,
  buildNodeConfigFromForm,
  parseNodeConfigToForm,
} from '@/lib/trade-flow-config-mappers';

test('action.place_order avg rebound pairlock rescue config round-trips', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'avg_rebound_pairlock_rescue_v1',
    side: 'buy',
    executionMode: 'limit',
    orderType: 'FOK',
    avgReboundPairlockRescue: {
      sessionBudgetUsdc: '60',
      reservedBudgetBufferUsdc: '1.00',
      primaryOutcomeLabel: 'No',
      primaryLadder: [{ id: 'p49', priceCap: '0.49', qty: '9' }],
      stages: [
        {
          id: 'stage_49',
          requiredPrimaryTierIds: ['p49'],
          profitLegs: [{ id: 'profit_44', oppositeVwapCap: '0.44', qty: '9' }],
          givebackGuard: { trigger: '0.48', maxExecutionVwap: '0.49' },
        },
      ],
      rescue: {
        enabledOnlyAfterFullLadder: true,
        normalVwapCap: '0.78',
        emergencyVwapCap: '0.81',
        hardMaxVwapCap: '0.81',
      },
    },
  });

  assert.equal(form.fields.mode, 'avg_rebound_pairlock_rescue_v1');
  assert.match(form.fields.avgReboundPairlockRescue, /sessionBudgetUsdc/);

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  const strategy = rebuilt.avgReboundPairlockRescue as Record<string, unknown>;
  const primaryLadder = strategy.primaryLadder as Array<Record<string, unknown>>;

  assert.equal(rebuilt.mode, 'avg_rebound_pairlock_rescue_v1');
  assert.equal(rebuilt.side, 'buy');
  assert.equal(rebuilt.executionMode, 'limit');
  assert.equal(rebuilt.orderType, 'FOK');
  assert.equal('tpEnabled' in rebuilt, false);
  assert.equal('slEnabled' in rebuilt, false);
  assert.equal(strategy.sessionBudgetUsdc, '60');
  assert.equal(primaryLadder[0].id, 'p49');
});

test('action.place_order avg rebound default config uses ladder continuation rescue defaults', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'avg_rebound_pairlock_rescue_v1',
  });
  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  const strategy = rebuilt.avgReboundPairlockRescue as Record<string, unknown>;
  const rescue = strategy.rescue as Record<string, unknown>;

  assert.equal(strategy.allowPrimaryAfterPartialProfit, true);
  assert.equal(strategy.preFullGivebackGuardEnabled, false);
  assert.equal(strategy.fullGivebackGuardEnabled, true);
  assert.equal(strategy.primaryOutcomeLabel, 'auto');
  assert.equal(strategy.primarySideSelection, 'cheapest_eligible');
  assert.equal(rescue.emergencyVwapCap, '0.81');
  assert.equal(rescue.hardMaxVwapCap, '0.81');
});

test('action.place_order avg rebound micro config uses 23 usdc auto cheapest preset', () => {
  const strategy = MICRO_AVG_REBOUND_PAIRLOCK_RESCUE_23USDC_CONFIG;
  const rescue = strategy.rescue;

  assert.equal(strategy.sessionBudgetUsdc, '23');
  assert.equal(strategy.reservedBudgetBufferUsdc, '0.25');
  assert.equal(strategy.targetProfitUsdc, '0.10');
  assert.equal(strategy.primaryOutcomeLabel, 'auto');
  assert.equal(strategy.oppositeOutcomeLabel, 'opposite');
  assert.equal(strategy.primarySideSelection, 'cheapest_eligible');
  assert.deepEqual(strategy.primaryLadder, [
    { id: 'p50', priceCap: '0.50', qty: '4' },
    { id: 'p30', priceCap: '0.30', qty: '5' },
    { id: 'p10', priceCap: '0.10', qty: '10' },
  ]);
  assert.equal(rescue.normalVwapCap, '0.770');
  assert.equal(rescue.emergencyVwapCap, '0.800');
  assert.equal(rescue.hardMaxVwapCap, '0.800');
  assert.equal(rescue.lastChanceVwapCap, '0.850');
});

test('trigger.market_price avg rebound binding round-trips and strips outcome rows', () => {
  const form = parseNodeConfigToForm('trigger.market_price', {
    marketMode: 'auto_scope',
    marketScope: 'btc_5m_updown',
    bindingMode: 'avg_rebound_pairlock_rescue_only',
    tokenId: 'strip-me',
    outcomeConditions: [{ tokenId: 'tok', outcomeLabel: 'Up' }],
  });

  assert.equal(form.fields.bindingMode, 'avg_rebound_pairlock_rescue_only');
  assert.equal(form.outcomeConditionRows.length, 0);

  const rebuilt = buildNodeConfigFromForm('trigger.market_price', form);
  assert.equal(rebuilt.bindingMode, 'avg_rebound_pairlock_rescue_only');
  assert.equal('tokenId' in rebuilt, false);
  assert.equal('outcomeConditions' in rebuilt, false);
});
