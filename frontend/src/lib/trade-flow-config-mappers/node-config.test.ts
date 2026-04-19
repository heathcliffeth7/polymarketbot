import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildNodeConfigFromForm,
  parseNodeConfigToForm,
} from '@/lib/trade-flow-config-mappers';

test('action.place_order reentry price fields round-trip through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
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
    reentryMaxAttempts: 3,
    reentryMinPriceCent: 60,
    reentryMaxPriceCent: 85,
  });

  assert.equal(form.fields.reentryMinPriceCent, '60');
  assert.equal(form.fields.reentryMaxPriceCent, '85');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.reentryMinPriceCent, 60);
  assert.equal(rebuilt.reentryMaxPriceCent, 85);
  assert.equal(rebuilt.reentryMaxAttempts, 3);
});

test('action.place_order execution floor override round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'nba-lal-orl-2026-03-21',
    tokenId: 'magic-token',
    outcomeLabel: 'Moneyline: Magic',
    executionFloorGuardEnabled: true,
    executionFloorPriceCent: 82,
    retryOnExecutionFloorGuardBlock: true,
  });

  assert.equal(form.fields.executionFloorPriceCent, '82');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.executionFloorPriceCent, 82);
  assert.equal(rebuilt.executionFloorGuardEnabled, true);
  assert.equal(rebuilt.retryOnExecutionFloorGuardBlock, true);
});

test('action.place_order hard and staged exits round-trip together', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'nba-lal-orl-2026-03-21',
    tokenId: 'magic-token',
    outcomeLabel: 'Moneyline: Magic',
    tpEnabled: true,
    tpPriceCent: 92,
    tpRules: [
      { priceCent: 65, sizePct: 40 },
      { priceCent: 75, sizePct: 60 },
    ],
    slEnabled: true,
    slPriceCent: 38,
    slRules: [
      { priceCent: 48, sizePct: 25 },
      { priceCent: 42, sizePct: 75 },
    ],
  });

  assert.equal(form.fields.tpPriceCent, '92');
  assert.equal(form.fields.slPriceCent, '38');
  assert.equal(form.tpRuleRows.length, 2);
  assert.equal(form.slRuleRows.length, 2);

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.tpPriceCent, 92);
  assert.equal(rebuilt.slPriceCent, 38);
  assert.deepEqual(rebuilt.tpRules, [
    { priceCent: 65, sizePct: 40 },
    { priceCent: 75, sizePct: 60 },
  ]);
  assert.deepEqual(rebuilt.slRules, [
    { priceCent: 48, sizePct: 25 },
    { priceCent: 42, sizePct: 75 },
  ]);
});

test('action.place_order staged sl behavior round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'nba-lal-orl-2026-03-21',
    tokenId: 'magic-token',
    outcomeLabel: 'Moneyline: Magic',
    slEnabled: true,
    slRules: [
      { priceCent: 48, sizePct: 25 },
      { priceCent: 42, sizePct: 75 },
    ],
    reenterOnSlHit: true,
    reentryMaxAttempts: 2,
    stagedSlReentryOnlyAfterAllStages: true,
  });

  assert.equal(form.fields.stagedSlReentryOnlyAfterAllStages, 'true');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.stagedSlReentryOnlyAfterAllStages, true);
});

test('action.place_order ptb stop-loss round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    ptbStopLossEnabled: true,
    ptbStopLossGapUsd: 0,
    notifyOnSlHit: true,
    reenterOnSlHit: true,
    reentryMaxAttempts: 2,
  });

  assert.equal(form.fields.ptbStopLossEnabled, 'true');
  assert.equal(form.ptbStopLossRuleRows.length, 1);
  assert.equal(form.ptbStopLossRuleRows[0]?.gapUsd, '0');
  assert.equal(form.ptbStopLossRuleRows[0]?.sizePct, '100');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.ptbStopLossEnabled, true);
  assert.deepEqual(rebuilt.ptbStopLossRules, [{ gapUsd: 0, sizePct: 100 }]);
  assert.equal('ptbStopLossGapUsd' in rebuilt, false);
  assert.equal(rebuilt.notifyOnSlHit, true);
  assert.equal(rebuilt.reenterOnSlHit, true);
});

test('action.place_order negative ptb stop-loss round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    ptbStopLossEnabled: true,
    ptbStopLossGapUsd: -20,
  });

  assert.equal(form.fields.ptbStopLossEnabled, 'true');
  assert.equal(form.ptbStopLossRuleRows.length, 1);
  assert.equal(form.ptbStopLossRuleRows[0]?.gapUsd, '-20');
  assert.equal(form.ptbStopLossRuleRows[0]?.sizePct, '100');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.ptbStopLossEnabled, true);
  assert.deepEqual(rebuilt.ptbStopLossRules, [{ gapUsd: -20, sizePct: 100 }]);
  assert.equal('ptbStopLossGapUsd' in rebuilt, false);
});

test('action.place_order PTB stop-loss bump round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'manual',
    priceToBeatMaxDiff: 80,
    priceToBeatMaxDiffUnit: 'cent',
    priceToBeatStopLossBumpEnabled: true,
    priceToBeatStopLossBumpAmount: 10,
    priceToBeatStopLossBumpMaxValue: 30,
    priceToBeatStopLossBumpUnit: 'cent',
  });

  assert.equal(form.fields.priceToBeatStopLossBumpEnabled, 'true');
  assert.equal(form.fields.priceToBeatStopLossBumpAmount, '10');
  assert.equal(form.fields.priceToBeatStopLossBumpMaxValue, '30');
  assert.equal(form.fields.priceToBeatStopLossBumpUnit, 'cent');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.priceToBeatStopLossBumpEnabled, true);
  assert.equal(rebuilt.priceToBeatStopLossBumpAmount, 10);
  assert.equal(rebuilt.priceToBeatStopLossBumpMaxValue, 30);
  assert.equal(rebuilt.priceToBeatStopLossBumpUnit, 'cent');
});

test('action.place_order PTB stop-loss bump is preserved in auto PTB modes', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'auto_vol_pct',
    priceToBeatStopLossBumpEnabled: true,
    priceToBeatStopLossBumpAmount: 10,
    priceToBeatStopLossBumpUnit: 'cent',
  });

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.priceToBeatStopLossBumpEnabled, true);
  assert.equal(rebuilt.priceToBeatStopLossBumpAmount, 10);
  assert.equal(rebuilt.priceToBeatStopLossBumpUnit, 'cent');
});

test('action.place_order auto PTB relax config round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'auto_vol_pct',
    priceToBeatMaxPriceRelaxMissCount: 3,
    priceToBeatMaxPriceRelaxHistoryCount: 4,
    priceToBeatMaxPriceRelaxMinValue: 15,
    priceToBeatMaxPriceRelaxMinUnit: 'cent',
    priceToBeatMaxPriceRelaxStepMode: 'absolute',
    priceToBeatMaxPriceRelaxStepValue: 20,
    priceToBeatMaxPriceRelaxStepUnit: 'cent',
  });

  assert.equal(form.fields.priceToBeatMaxPriceRelaxMissCount, '3');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxHistoryCount, '4');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxMinValue, '15');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxMinUnit, 'cent');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxStepMode, 'absolute');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxStepValue, '20');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxStepUnit, 'cent');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMissCount, 3);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxHistoryCount, 4);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMinValue, 15);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMinUnit, 'cent');
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepMode, 'absolute');
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepValue, 20);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepUnit, 'cent');
});

test('action.place_order manual PTB relax config round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'manual',
    priceToBeatMaxDiff: 80,
    priceToBeatMaxDiffUnit: 'cent',
    priceToBeatMaxPriceRelaxMissCount: 3,
    priceToBeatMaxPriceRelaxHistoryCount: 4,
    priceToBeatMaxPriceRelaxMinValue: 15,
    priceToBeatMaxPriceRelaxMinUnit: 'cent',
    priceToBeatMaxPriceRelaxStepMode: 'percent',
    priceToBeatMaxPriceRelaxStepValue: 10,
  });

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMissCount, 3);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxHistoryCount, 4);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMinValue, 15);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMinUnit, 'cent');
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepMode, 'percent');
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepValue, 10);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepUnit, undefined);
});

test('action.place_order PTB relax defaults to percent 25 in form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'auto_vol_pct',
  });

  assert.equal(form.fields.priceToBeatMaxPriceRelaxStepMode, 'percent');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxStepValue, '25');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepMode, 'percent');
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepValue, 25);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepUnit, undefined);
});

test('action.place_order staged ptb stop-loss round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    ptbStopLossRules: [
      { gapUsd: 12.5, sizePct: 25 },
      { gapUsd: 3, sizePct: 75 },
    ],
    reenterOnSlHit: true,
    stagedSlReentryOnlyAfterAllStages: true,
  });

  assert.equal(form.fields.ptbStopLossEnabled, 'true');
  assert.equal(form.ptbStopLossRuleRows.length, 2);
  assert.equal(form.ptbStopLossRuleRows[0]?.gapUsd, '12.5');
  assert.equal(form.ptbStopLossRuleRows[1]?.sizePct, '75');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.deepEqual(rebuilt.ptbStopLossRules, [
    { gapUsd: 12.5, sizePct: 25 },
    { gapUsd: 3, sizePct: 75 },
  ]);
  assert.equal(rebuilt.ptbStopLossEnabled, true);
  assert.equal(rebuilt.stagedSlReentryOnlyAfterAllStages, true);
});

test('action.place_order disables ptb config when master toggle is off', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    ptbStopLossEnabled: true,
    ptbStopLossGapUsd: 0,
    ptbStopLossRules: [{ gapUsd: 3, sizePct: 100 }],
  });

  form.fields.ptbStopLossEnabled = 'false';

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal('ptbStopLossGapUsd' in rebuilt, false);
  assert.equal('ptbStopLossRules' in rebuilt, false);
});

test('trigger.market_price custom_range round-trips exact start/end values', () => {
  const form = parseNodeConfigToForm('trigger.market_price', {
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
  });

  assert.equal(form.fields.cycleWindowMode, 'custom_range');
  assert.equal(form.fields.cycleWindowStartSec, '240');
  assert.equal(form.fields.cycleWindowEndSec, '285');
  assert.equal(form.fields.autoSellOnWindowEnd, 'true');

  const rebuilt = buildNodeConfigFromForm('trigger.market_price', form);
  assert.equal(rebuilt.cycleWindowMode, 'custom_range');
  assert.equal(rebuilt.cycleWindowStartSec, 240);
  assert.equal(rebuilt.cycleWindowEndSec, 285);
  assert.equal(rebuilt.autoSellOnWindowEnd, true);
});

test('trigger.market_price auto_vol_pct round-trips and drops manual PTB fields', () => {
  const form = parseNodeConfigToForm('trigger.market_price', {
    marketMode: 'auto_scope',
    marketScope: 'eth_5m_updown',
    marketSelection: 'latest_by_slug',
    priceMode: 'composite',
    repeatMode: 'once',
    priceToBeatTriggerEnabled: true,
    priceToBeatMode: 'auto_vol_pct',
    priceToBeatTriggerMinGap: 2,
    priceToBeatTriggerUnit: 'usd',
  });

  assert.equal(form.fields.priceToBeatMode, 'auto_vol_pct');

  const rebuilt = buildNodeConfigFromForm('trigger.market_price', form);
  assert.equal(rebuilt.priceToBeatMode, 'auto_vol_pct');
  assert.equal('priceToBeatTriggerMinGap' in rebuilt, false);
  assert.equal('priceToBeatTriggerUnit' in rebuilt, false);
});

test('trigger.market_price bindingMode round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('trigger.market_price', {
    marketMode: 'auto_scope',
    marketScope: 'btc_5m_updown',
    marketSelection: 'latest_by_slug',
    priceMode: 'composite',
    repeatMode: 'once',
    bindingMode: 'pair_lock_only',
    outcomeConditions: [
      {
        tokenId: 'btc-up-token',
        outcomeLabel: 'Up',
        triggerCondition: 'level_above',
        triggerPriceCent: 70,
      },
    ],
  });

  assert.equal(form.fields.bindingMode, 'pair_lock_only');

  const rebuilt = buildNodeConfigFromForm('trigger.market_price', form);
  assert.equal(rebuilt.bindingMode, 'pair_lock_only');
});

test('trigger.market_price pair_lock_only strips outcome and PTB trigger config on rebuild', () => {
  const form = parseNodeConfigToForm('trigger.market_price', {
    marketMode: 'auto_scope',
    marketScope: 'btc_5m_updown',
    marketSelection: 'latest_by_slug',
    repeatMode: 'once',
    bindingMode: 'pair_lock_only',
    priceToBeatTriggerEnabled: true,
    priceToBeatMode: 'manual',
    priceToBeatTriggerMinGap: 10,
    outcomeConditions: [
      {
        tokenId: 'btc-up-token',
        outcomeLabel: 'Up',
        triggerCondition: 'level_above',
        triggerPriceCent: 70,
      },
    ],
  });

  const rebuilt = buildNodeConfigFromForm('trigger.market_price', form);
  assert.equal(rebuilt.bindingMode, 'pair_lock_only');
  assert.equal('outcomeConditions' in rebuilt, false);
  assert.equal('priceToBeatTriggerEnabled' in rebuilt, false);
  assert.equal('priceToBeatMode' in rebuilt, false);
  assert.equal('priceToBeatTriggerMinGap' in rebuilt, false);
});

test('action.place_order auto_vol_pct round-trips and drops manual PTB fields', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'auto_vol_pct',
    priceToBeatMaxDiff: 2,
    priceToBeatMaxDiffUnit: 'usd',
  });

  assert.equal(form.fields.priceToBeatMode, 'auto_vol_pct');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.priceToBeatMode, 'auto_vol_pct');
  assert.equal('priceToBeatMaxDiff' in rebuilt, false);
  assert.equal('priceToBeatMaxDiffUnit' in rebuilt, false);
});

test('action.place_order manual reentry PTB override round-trips explicit unit through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    slEnabled: true,
    reenterOnSlHit: true,
    reentryMaxAttempts: 1,
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'manual',
    priceToBeatMaxDiff: 10,
    priceToBeatMaxDiffUnit: 'cent',
    reentryPriceToBeatMaxDiff: 5,
    reentryPriceToBeatMaxDiffUnit: 'usd',
  });

  assert.equal(form.fields.reentryPriceToBeatMaxDiff, '5');
  assert.equal(form.fields.reentryPriceToBeatMaxDiffUnit, 'usd');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.reentryPriceToBeatMaxDiff, 5);
  assert.equal(rebuilt.reentryPriceToBeatMaxDiffUnit, 'usd');
});

test('action.place_order manual reentry PTB override keeps unit optional when absent', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    slEnabled: true,
    reenterOnSlHit: true,
    reentryMaxAttempts: 1,
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'manual',
    priceToBeatMaxDiff: 10,
    priceToBeatMaxDiffUnit: 'cent',
    reentryPriceToBeatMaxDiff: 5,
  });

  assert.equal(form.fields.reentryPriceToBeatMaxDiff, '5');
  assert.equal(form.fields.reentryPriceToBeatMaxDiffUnit, '');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.reentryPriceToBeatMaxDiff, 5);
  assert.equal('reentryPriceToBeatMaxDiffUnit' in rebuilt, false);
});

test('action.place_order auto PTB reentry override round-trips unit through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    slEnabled: true,
    reenterOnSlHit: true,
    reentryMaxAttempts: 1,
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'auto_vol_pct',
    reentryPriceToBeatMaxDiff: 3,
    reentryPriceToBeatMaxDiffUnit: 'usd',
  });

  assert.equal(form.fields.reentryPriceToBeatMaxDiff, '3');
  assert.equal(form.fields.reentryPriceToBeatMaxDiffUnit, 'usd');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.reentryPriceToBeatMaxDiff, 3);
  assert.equal(rebuilt.reentryPriceToBeatMaxDiffUnit, 'usd');
});

test('action.place_order pair_lock fields round-trip through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'pair_lock',
    pairSizingMode: 'manual',
    side: 'buy',
    executionMode: 'limit',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    marketSlug: 'btc-updown-5m-1774013100',
    tokenId: 'btc-yes-token',
    outcomeLabel: 'Up',
    pairTargetTotalCent: 90,
    pairOrphanGraceMs: 1500,
    notifyOnPairLocked: true,
    counterLegEnabled: true,
    counterLegSizeUsdc: 5,
    counterLegOutcomeLabel: 'opposite',
    counterLegTriggerCondition: 'level_below',
    counterLegTriggerPriceCent: 20,
    counterLegMaxPriceCent: 42,
    counterLegPriceToBeatGuardEnabled: true,
    counterLegPriceToBeatMode: 'manual',
    counterLegPriceToBeatMaxDiff: 10,
    counterLegPriceToBeatMaxDiffUnit: 'usd',
    counterLegExecutionFloorGuardEnabled: true,
    counterLegExecutionFloorPriceCent: 18,
    counterLegRetryOnPriceToBeatGuardBlock: true,
    slEnabled: true,
    slPriceCent: 45,
    slTriggerPriceMode: 'composite_safe',
    ptbStopLossEnabled: true,
    ptbStopLossGapUsd: 0,
    ptbStopLossTimeDecayMode: 'relax',
    notifyOnSlHit: true,
    reenterOnSlHit: true,
    reentryMaxAttempts: 2,
    reentryCooldownSec: 15,
  });

  assert.equal(form.fields.mode, 'pair_lock');
  assert.equal(form.fields.pairSizingMode, 'manual');
  assert.equal(form.fields.counterLegOutcomeLabel, 'opposite');
  assert.equal(form.fields.pairMaxTotalCent, '90');
  assert.equal(form.fields.counterLegPriceToBeatMaxDiffUnit, 'usd');
  assert.equal(form.fields.slPriceCent, '45');
  assert.equal(form.fields.ptbStopLossGapUsd, '0');
  assert.equal(form.ptbStopLossRuleRows.length, 0);

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.mode, 'pair_lock');
  assert.equal(rebuilt.pairMaxTotalCent, 90);
  assert.equal('pairTargetTotalCent' in rebuilt, false);
  assert.equal(rebuilt.pairSizingMode, 'manual');
  assert.equal(rebuilt.counterLegEnabled, true);
  assert.equal(rebuilt.counterLegSizeUsdc, 5);
  assert.equal(rebuilt.counterLegOutcomeLabel, 'opposite');
  assert.equal(rebuilt.counterLegTriggerCondition, 'level_below');
  assert.equal(rebuilt.counterLegTriggerPriceCent, 20);
  assert.equal(rebuilt.counterLegMaxPriceCent, 42);
  assert.equal(rebuilt.counterLegPriceToBeatGuardEnabled, true);
  assert.equal(rebuilt.counterLegPriceToBeatMode, 'manual');
  assert.equal(rebuilt.counterLegPriceToBeatMaxDiff, 10);
  assert.equal(rebuilt.counterLegPriceToBeatMaxDiffUnit, 'usd');
  assert.equal(rebuilt.counterLegExecutionFloorGuardEnabled, true);
  assert.equal(rebuilt.counterLegExecutionFloorPriceCent, 18);
  assert.equal(rebuilt.counterLegRetryOnPriceToBeatGuardBlock, true);
  assert.equal(rebuilt.slEnabled, true);
  assert.equal(rebuilt.slPriceCent, 45);
  assert.equal(rebuilt.slTriggerPriceMode, 'composite_safe');
  assert.equal(rebuilt.ptbStopLossEnabled, true);
  assert.equal(rebuilt.ptbStopLossGapUsd, 0);
  assert.equal(rebuilt.ptbStopLossTimeDecayMode, 'relax');
  assert.equal(rebuilt.notifyOnSlHit, true);
  assert.equal(rebuilt.reenterOnSlHit, true);
  assert.equal(rebuilt.reentryMaxAttempts, 2);
  assert.equal(rebuilt.reentryCooldownSec, 15);
});

test('action.place_order pair_lock auto remaining budget round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'pair_lock',
    pairSizingMode: 'auto_remaining_budget',
    pairTotalBudgetUsdc: 14,
    side: 'buy',
    executionMode: 'limit',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    marketSlug: 'btc-updown-5m-1774013100',
    tokenId: 'btc-yes-token',
    outcomeLabel: 'Up',
    pairMaxTotalCent: 90,
    pairOrphanGraceMs: 1500,
    notifyOnPairLocked: true,
    counterLegEnabled: true,
    counterLegOutcomeLabel: 'opposite',
  });

  assert.equal(form.fields.pairSizingMode, 'auto_remaining_budget');
  assert.equal(form.fields.pairTotalBudgetUsdc, '14');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.pairSizingMode, 'auto_remaining_budget');
  assert.equal(rebuilt.pairTotalBudgetUsdc, 14);
  assert.equal('counterLegSizeUsdc' in rebuilt, false);
  assert.equal('pairMinNetProfitUsdc' in rebuilt, false);
  assert.equal('pairProfitSafetyBufferUsdc' in rebuilt, false);
  assert.equal('notifyOnPairNoEdge' in rebuilt, false);
});

test('action.place_order pair_lock defaults invalid counter PTB unit to usd', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'pair_lock',
    side: 'buy',
    executionMode: 'limit',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    counterLegEnabled: true,
    counterLegPriceToBeatGuardEnabled: true,
    counterLegPriceToBeatMode: 'manual',
    counterLegPriceToBeatMaxDiff: 10,
    counterLegPriceToBeatMaxDiffUnit: 'ticks',
  });

  assert.equal(form.fields.counterLegPriceToBeatMode, 'manual');
  assert.equal(form.fields.counterLegPriceToBeatMaxDiffUnit, 'usd');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.counterLegPriceToBeatMode, 'manual');
  assert.equal(rebuilt.counterLegPriceToBeatMaxDiffUnit, 'usd');
});

test('action.place_order pair_lock drops counter PTB manual fields in auto mode', () => {
  const rebuilt = buildNodeConfigFromForm('action.place_order', {
    fields: {
      mode: 'pair_lock',
      side: 'buy',
      executionMode: 'limit',
      sizeMode: 'usdc',
      sizeUsdc: '5',
      counterLegEnabled: 'true',
      counterLegPriceToBeatGuardEnabled: 'true',
      counterLegPriceToBeatMode: 'auto_vol_pct',
      counterLegPriceToBeatMaxDiff: '10',
      counterLegPriceToBeatMaxDiffUnit: 'cent',
    },
    advancedJson: '',
    triggerSizeRows: [],
    keyValueDrafts: [],
    expressionRows: [],
    outcomeConditionRows: [],
    tpRuleRows: [],
    slRuleRows: [],
    ptbStopLossRuleRows: [],
    timeExitRuleRows: [],
    drawdownRuleRows: [],
  });

  assert.equal(rebuilt.counterLegPriceToBeatMode, 'auto_vol_pct');
  assert.equal('counterLegPriceToBeatMaxDiff' in rebuilt, false);
  assert.equal('counterLegPriceToBeatMaxDiffUnit' in rebuilt, false);
});

test('action.place_order pair_lock drops unsupported exit and advanced reentry fields', () => {
  const rebuilt = buildNodeConfigFromForm('action.place_order', {
    fields: {
      mode: 'pair_lock',
      side: 'buy',
      executionMode: 'limit',
      sizeMode: 'usdc',
      sizeUsdc: '5',
      pairMaxTotalCent: '90',
      counterLegEnabled: 'true',
      counterLegSizeUsdc: '5',
      slEnabled: 'true',
      slPriceCent: '45',
      ptbStopLossEnabled: 'true',
      ptbStopLossGapUsd: '0',
      reenterOnSlHit: 'true',
      reentryMaxAttempts: '2',
      reentryCooldownSec: '15',
      tpEnabled: 'true',
      tpPriceCent: '95',
      reentryMinPriceCent: '40',
      reentryPriceToBeatMaxDiff: '3',
      reentryPriceToBeatMaxDiffUnit: 'usd',
    },
    advancedJson: '',
    triggerSizeRows: [],
    keyValueDrafts: [],
    expressionRows: [],
    outcomeConditionRows: [],
    tpRuleRows: [{ id: 'tp-1', priceCent: '95', sizePct: '100' }],
    slRuleRows: [{ id: 'sl-1', priceCent: '40', sizePct: '100' }],
    ptbStopLossRuleRows: [{ id: 'ptb-1', gapUsd: '0', sizePct: '100' }],
    timeExitRuleRows: [{ id: 'time-1', elapsedMinutes: '5', remainingPct: '100' }],
    drawdownRuleRows: [],
    expressionJoin: 'and',
    expressionSupported: false,
    nestedExprMode: false,
    nestedExprGroup: null,
    statePatchRows: [],
  });

  assert.equal(rebuilt.slEnabled, true);
  assert.equal(rebuilt.slPriceCent, 45);
  assert.equal(rebuilt.ptbStopLossEnabled, true);
  assert.equal(rebuilt.ptbStopLossGapUsd, 0);
  assert.equal(rebuilt.reenterOnSlHit, true);
  assert.equal(rebuilt.reentryMaxAttempts, 2);
  assert.equal(rebuilt.reentryCooldownSec, 15);
  assert.equal('tpEnabled' in rebuilt, false);
  assert.equal('tpPriceCent' in rebuilt, false);
  assert.equal('tpRules' in rebuilt, false);
  assert.equal('slRules' in rebuilt, false);
  assert.equal('ptbStopLossRules' in rebuilt, false);
  assert.equal('timeExitRules' in rebuilt, false);
  assert.equal('reentryMinPriceCent' in rebuilt, false);
  assert.equal('reentryPriceToBeatMaxDiff' in rebuilt, false);
  assert.equal('reentryPriceToBeatMaxDiffUnit' in rebuilt, false);
});
