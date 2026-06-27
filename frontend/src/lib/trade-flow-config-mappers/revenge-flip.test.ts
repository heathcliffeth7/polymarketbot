import assert from 'node:assert/strict';
import test from 'node:test';

import { buildNodeConfigFromForm, parseNodeConfigToForm } from '@/lib/trade-flow-config-mappers';
import {
  REVENGE_FLIP_CLASSIC_STOP_LOSS_ENABLED_FIELD,
  REVENGE_FLIP_ENTRY_PTB_RULES_FIELD,
  REVENGE_FLIP_INITIAL_ORDER_USDC_FIELD,
  REVENGE_FLIP_MODE,
  REVENGE_FLIP_MIN_REENTRY_SHARES_FIELD,
  REVENGE_FLIP_POST_STOP_LOSS_IV_MISMATCH_ENABLED_FIELD,
  REVENGE_FLIP_PROFIT_TARGET_USDC_FIELD,
  REVENGE_FLIP_PTB_STOP_LOSS_CURRENT_SOURCE_FIELD,
  REVENGE_FLIP_PTB_STOP_LOSS_ENABLED_FIELD,
  REVENGE_FLIP_PTB_STOP_LOSS_GAP_FIELD,
  REVENGE_FLIP_REENTRY_SIDE_MODE_FIELD,
  REVENGE_FLIP_STOP_LOSS_RULES_FIELD,
  REVENGE_FLIP_TIME_RULES_FIELD,
} from './revenge-flip';

test('action.place_order revenge_flip_v1 mapper round-trips nested config', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: REVENGE_FLIP_MODE,
    side: 'buy',
    executionMode: 'market',
    revengeFlip: {
      initialOrderUsdc: 9,
      profitTargetUsdc: 0.6,
      classicStopLossEnabled: true,
      stopLossPct: 0.18,
      stopLossRules: [
        { minFlip: 0, maxFlip: 0, stopLossPct: 0.2 },
        { minFlip: 1, stopLossPct: 0.15 },
      ],
      reentrySideMode: 'rule_match',
      entryPtbRules: [
        {
          minFlip: 0,
          maxFlip: 0,
          sideMode: 'up',
          minRemainingSec: 0,
          maxRemainingSec: 300,
          priceToBeatMaxDiff: 5,
          priceToBeatMaxDiffUnit: 'cent',
          maxPriceCent: 80,
        },
        { minFlip: 1, sideMode: 'any', priceToBeatMaxDiff: 2, priceToBeatMaxDiffUnit: 'cent', maxPriceCent: 90 },
      ],
      maxFlip: 3,
      minReentryShares: 5,
      postStopLossIvMismatchEnabled: false,
      lotLimitPct: 0.7,
      closeOnlySec: 12,
      timeRules: [{ minRemainingSec: 20, maxRemainingSec: 80, priceToBeatMaxDiff: 1, priceToBeatMaxDiffUnit: 'cent' }],
      ptbStopLossEnabled: true,
      ptbStopLossGapUsd: -4,
      ptbStopLossGapUnit: 'cent',
      ptbStopLossCurrentPriceSource: 'chainlink_cex_consensus',
      ptbStopLossTimeDecayMode: 'none',
    },
    triggerPrice: { enabled: true, minCent: 45, maxCent: 60 },
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'iv_mismatch_edge',
    priceToBeatCurrentPriceSource: 'chainlink',
    priceToBeatIvTimeRules: [
      {
        startRemainingSec: 45,
        endRemainingSec: 30,
        minEdge: 0.03,
        minGapStrength: 0.5,
        maxPriceCent: 92,
      },
      {
        startRemainingSec: 30,
        endRemainingSec: 15,
        minEdge: 0.05,
        minGapStrength: 0.75,
        maxPriceCent: 92,
      },
      {
        startRemainingSec: 15,
        endRemainingSec: 8,
        minEdge: 0.07,
        minGapStrength: 1,
        maxPriceCent: 92,
      },
    ],
    priceToBeatIvEntryQualityPolicy: true,
    priceToBeatIvEntryQualityChainlinkMaxAgeMs: 2500,
    priceToBeatIvEntryQualityHighRiskAskCent: 85,
    cexDirectionGuardEnabled: true,
    cexDirectionGuardMode: 'bybit_plus_one',
    cexDirectionGuardFailClosed: false,
    priceToBeatMaxDiff: 0.02,
    priceToBeatMaxDiffUnit: 'usd',
  });

  assert.equal(form.fields.mode, REVENGE_FLIP_MODE);
  assert.equal(form.fields[REVENGE_FLIP_INITIAL_ORDER_USDC_FIELD], '9');
  assert.equal(form.fields[REVENGE_FLIP_PROFIT_TARGET_USDC_FIELD], '0.6');
  assert.equal(form.fields[REVENGE_FLIP_CLASSIC_STOP_LOSS_ENABLED_FIELD], 'true');
  assert.match(form.fields[REVENGE_FLIP_STOP_LOSS_RULES_FIELD], /stopLossPct/);
  assert.match(form.fields[REVENGE_FLIP_ENTRY_PTB_RULES_FIELD], /priceToBeatMinDiff/);
  assert.match(form.fields[REVENGE_FLIP_TIME_RULES_FIELD], /minRemainingSec/);
  assert.equal(form.fields[REVENGE_FLIP_REENTRY_SIDE_MODE_FIELD], 'rule_match');
  assert.equal(form.fields[REVENGE_FLIP_MIN_REENTRY_SHARES_FIELD], '5');
  assert.equal(form.fields[REVENGE_FLIP_POST_STOP_LOSS_IV_MISMATCH_ENABLED_FIELD], 'false');
  assert.equal(form.fields[REVENGE_FLIP_PTB_STOP_LOSS_ENABLED_FIELD], 'true');
  assert.equal(form.fields[REVENGE_FLIP_PTB_STOP_LOSS_GAP_FIELD], '-4');
  assert.equal(form.fields[REVENGE_FLIP_PTB_STOP_LOSS_CURRENT_SOURCE_FIELD], 'chainlink_cex_consensus');
  assert.equal(form.fields.priceToBeatMode, 'iv_mismatch_edge');
  assert.equal(form.fields.priceToBeatCurrentPriceSource, 'chainlink');
  assert.equal(form.fields.cexDirectionGuardEnabled, 'true');
  assert.equal(form.fields.cexDirectionGuardMode, 'bybit_plus_one');
  assert.equal(form.fields.cexDirectionGuardFailClosed, 'false');
  assert.equal(form.fields.priceToBeatIvEntryQualityPolicy, 'true');
  assert.equal(form.fields.priceToBeatIvNormalMaxPriceCent, '94');
  assert.equal(form.fields.priceToBeatIvPremiumMaxPriceCent, '96');
  assert.equal(form.fields.priceToBeatIvMinExpectedMoveBps, '2');
  assert.equal(form.fields.priceToBeatIvPremiumBufferRetain5s, '0.9');
  assert.equal(form.fields.priceToBeatIvCexAlignMaxBps, '5');
  assert.equal(form.fields.priceToBeatIvEntryQualityChainlinkMaxAgeMs, '2500');
  assert.equal(form.ptbIvTimeRuleRows?.length, 3);

  form.fields[REVENGE_FLIP_INITIAL_ORDER_USDC_FIELD] = '11';
  form.fields[REVENGE_FLIP_PROFIT_TARGET_USDC_FIELD] = '-1';
  form.fields[REVENGE_FLIP_MIN_REENTRY_SHARES_FIELD] = '6.5';
  const rebuilt = buildNodeConfigFromForm('action.place_order', form) as Record<string, unknown>;
  assert.equal(rebuilt.mode, REVENGE_FLIP_MODE);
  assert.equal((rebuilt.revengeFlip as Record<string, unknown>).initialOrderUsdc, 11);
  assert.equal((rebuilt.revengeFlip as Record<string, unknown>).profitTargetUsdc, -1);
  assert.equal((rebuilt.revengeFlip as Record<string, unknown>).classicStopLossEnabled, true);
  assert.deepEqual((rebuilt.revengeFlip as Record<string, unknown>).stopLossRules, [
    { minFlip: 0, maxFlip: 0, stopLossPct: 0.2 },
    { minFlip: 1, stopLossPct: 0.15 },
  ]);
  assert.deepEqual((rebuilt.revengeFlip as Record<string, unknown>).entryPtbRules, [
    {
      minFlip: 0,
      maxFlip: 0,
      sideMode: 'up',
      minRemainingSec: 0,
      maxRemainingSec: 300,
      priceToBeatMinDiff: 5,
      priceToBeatMinDiffUnit: 'cent',
      maxPriceCent: 80,
    },
    { minFlip: 1, sideMode: 'any', priceToBeatMinDiff: 2, priceToBeatMinDiffUnit: 'cent', maxPriceCent: 90 },
  ]);
  assert.equal((rebuilt.revengeFlip as Record<string, unknown>).reentrySideMode, 'rule_match');
  assert.equal((rebuilt.revengeFlip as Record<string, unknown>).minReentryShares, 6.5);
  assert.equal(
    (rebuilt.revengeFlip as Record<string, unknown>).postStopLossIvMismatchEnabled,
    false
  );
  assert.equal((rebuilt.revengeFlip as Record<string, unknown>).ptbStopLossEnabled, true);
  assert.equal((rebuilt.revengeFlip as Record<string, unknown>).ptbStopLossGapUsd, -4);
  assert.equal(
    (rebuilt.revengeFlip as Record<string, unknown>).ptbStopLossCurrentPriceSource,
    'chainlink_cex_consensus'
  );
  assert.equal((rebuilt.triggerPrice as Record<string, unknown>).enabled, true);
  assert.equal(rebuilt.priceToBeatGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatMode, 'iv_mismatch_edge');
  assert.equal(rebuilt.priceToBeatCurrentPriceSource, 'chainlink');
  assert.equal(rebuilt.cexDirectionGuardEnabled, true);
  assert.equal(rebuilt.cexDirectionGuardMode, 'bybit_plus_one');
  assert.equal(rebuilt.cexDirectionGuardFailClosed, false);
  assert.equal(rebuilt.priceToBeatIvEntryQualityPolicy, true);
  assert.equal(rebuilt.priceToBeatIvNormalMaxPriceCent, 94);
  assert.equal(rebuilt.priceToBeatIvPremiumMaxPriceCent, 96);
  assert.equal(rebuilt.priceToBeatIvMinExpectedMoveBps, 2);
  assert.equal(rebuilt.priceToBeatIvPremiumBufferRetain5s, 0.9);
  assert.equal(rebuilt.priceToBeatIvCexAlignMaxBps, 5);
  assert.equal(rebuilt.priceToBeatIvEntryQualityChainlinkMaxAgeMs, 2500);
  assert.equal(rebuilt.priceToBeatIvEntryQualityHighRiskAskCent, 85);
  assert.deepEqual(rebuilt.priceToBeatIvTimeRules, [
    {
      startRemainingSec: 45,
      endRemainingSec: 30,
      maxPriceCent: 92,
      minEdge: 0.03,
      minGapStrength: 0.5,
    },
    {
      startRemainingSec: 30,
      endRemainingSec: 15,
      maxPriceCent: 92,
      minEdge: 0.05,
      minGapStrength: 0.75,
    },
    {
      startRemainingSec: 15,
      endRemainingSec: 8,
      maxPriceCent: 92,
      minEdge: 0.07,
      minGapStrength: 1,
    },
  ]);
});

test('action.place_order revenge_flip_v1 mapper round-trips PTB-only stop-loss', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: REVENGE_FLIP_MODE,
    revengeFlip: {
      initialOrderUsdc: 2,
      profitTargetUsdc: 0.25,
      classicStopLossEnabled: false,
      ptbStopLossEnabled: true,
      ptbStopLossGapUsd: 1,
      ptbStopLossGapUnit: 'usd',
      ptbStopLossTimeDecayMode: 'none',
    },
    priceToBeatCurrentPriceSource: 'chainlink',
  });

  assert.equal(form.fields[REVENGE_FLIP_CLASSIC_STOP_LOSS_ENABLED_FIELD], 'false');
  assert.equal(form.fields[REVENGE_FLIP_PTB_STOP_LOSS_ENABLED_FIELD], 'true');
  assert.equal(form.fields[REVENGE_FLIP_PTB_STOP_LOSS_GAP_FIELD], '1');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form) as Record<string, unknown>;
  const revenge = rebuilt.revengeFlip as Record<string, unknown>;
  assert.equal(revenge.classicStopLossEnabled, false);
  assert.equal(revenge.ptbStopLossEnabled, true);
  assert.equal(revenge.ptbStopLossGapUsd, 1);
  assert.equal(revenge.ptbStopLossGapUnit, 'usd');
  assert.equal(revenge.ptbStopLossTimeDecayMode, 'none');
  assert.equal(revenge.postStopLossIvMismatchEnabled, true);
});

test('trigger.market_price revenge_flip_only binding round-trips and strips trigger rows', () => {
  const form = parseNodeConfigToForm('trigger.market_price', {
    marketMode: 'auto_scope',
    bindingMode: 'revenge_flip_only',
    outcomeConditions: [{ triggerCondition: 'level_above', triggerPriceCent: 50 }],
    priceToBeatTriggerEnabled: true,
  });

  assert.equal(form.fields.bindingMode, 'revenge_flip_only');
  assert.equal(form.outcomeConditionRows.length, 0);

  const rebuilt = buildNodeConfigFromForm('trigger.market_price', form) as Record<string, unknown>;
  assert.equal(rebuilt.bindingMode, 'revenge_flip_only');
  assert.equal(rebuilt.outcomeConditions, undefined);
  assert.equal(rebuilt.priceToBeatTriggerEnabled, undefined);
});
