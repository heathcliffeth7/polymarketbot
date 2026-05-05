import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildNodeConfigFromForm,
  parseNodeConfigToForm,
} from '@/lib/trade-flow-config-mappers';

test('action.place_order live gap collector mode round-trips with defaults', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'live_gap_collector_v1',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    marketSlug: 'btc-updown-5m-1773319200',
    tokenId: 'tok-up',
    outcomeLabel: 'Up',
  });

  assert.equal(form.fields.mode, 'live_gap_collector_v1');
  assert.equal(form.fields.liveGapCollectorWindowStartSec, '220');
  assert.equal(form.fields.liveGapCollectorHardMaxPriceCent, '93');
  assert.equal(form.fields.notifyOnLiveGapCollectorDecision, 'true');
  assert.equal(form.fields.noReversalEntryGuardEnabled, 'false');
  assert.equal(form.fields.noReversalLookbackMode, 'multi_window_adaptive');
  assert.equal(form.fields.noReversalProfileQueryTimeoutMs, '500');
  assert.equal(form.fields.noReversalSoftPassOnInsufficientData, 'true');
  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.mode, 'live_gap_collector_v1');
  assert.equal(rebuilt.side, 'buy');
  assert.equal(rebuilt.tpEnabled, true);
  assert.equal(rebuilt.tpPriceCent, 98);
  assert.equal(rebuilt.notifyOnLiveGapCollectorDecision, true);
  assert.equal(rebuilt.liveGapStopLossEntryGapRatio, 0.33);
  assert.equal(rebuilt.noReversalEntryGuardEnabled, false);
  assert.equal(rebuilt.noReversalBaselineFloorPct, 0.8);
  assert.equal(rebuilt.noReversalProfileQueryTimeoutMs, 500);
  assert.equal(rebuilt.noReversalSoftPassOnInsufficientData, true);
});

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

test('action.place_order shares sizing round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'shares',
    targetQty: 5,
    sizeUsdc: 5,
    marketSlug: 'btc-updown-5m-1774013100',
    tokenId: 'btc-down-token',
    outcomeLabel: 'Down',
  });

  assert.equal(form.fields.sizeMode, 'shares');
  assert.equal(form.fields.targetQty, '5');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.sizeMode, 'shares');
  assert.equal(rebuilt.targetQty, 5);
  assert.equal(rebuilt.sizeUsdc, undefined);
});

test('action.place_order removes PTB current source when PTB guard and PTB stop-loss are inactive', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'btc-updown-5m-1774013100',
    tokenId: 'btc-up-token',
    outcomeLabel: 'Up',
    priceToBeatCurrentPriceSource: 'binance',
  });

  assert.equal(form.fields.priceToBeatCurrentPriceSource, 'binance');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal('priceToBeatCurrentPriceSource' in rebuilt, false);
});

test('trigger.market_price entry timing profiles round-trip through mapper form state', () => {
  const form = parseNodeConfigToForm('trigger.market_price', {
    marketMode: 'auto_scope',
    marketScope: 'btc_5m_updown',
    marketSelection: 'latest_by_slug',
    repeatMode: 'once',
    priceToBeatTriggerEnabled: true,
    priceToBeatMode: 'manual',
    priceToBeatTriggerMinGap: 10,
    outcomeConditions: [{ tokenId: 'token-up', outcomeLabel: 'Up' }],
    entryTimingProfiles: [
      {
        startRemainingSec: 90,
        endRemainingSec: 45,
        maxPriceCent: 60,
        priceToBeatTriggerMinGap: 10,
        sizeUsdc: 1.5,
      },
    ],
  });

  assert.equal(form.entryTimingProfileRows?.length, 1);
  assert.equal(form.entryTimingProfileRows?.[0]?.startRemainingSec, '90');
  assert.equal(form.entryTimingProfileRows?.[0]?.sizeUsdc, '1.5');

  const rebuilt = buildNodeConfigFromForm('trigger.market_price', form);
  assert.deepEqual(rebuilt.entryTimingProfiles, [
    {
      startRemainingSec: 90,
      endRemainingSec: 45,
      maxPriceCent: 60,
      priceToBeatTriggerMinGap: 10,
      sizeUsdc: 1.5,
    },
  ]);
});

test('action.place_order buy fill lock config round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    buyFillLockEnabled: true,
    buyFillLockGroup: 'late-entry',
    releaseBuyFillLockOnStopLoss: true,
  });

  assert.equal(form.fields.buyFillLockEnabled, 'true');
  assert.equal(form.fields.buyFillLockGroup, 'late-entry');
  assert.equal(form.fields.releaseBuyFillLockOnStopLoss, 'true');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.buyFillLockEnabled, true);
  assert.equal(rebuilt.buyFillLockGroup, 'late-entry');
  assert.equal(rebuilt.releaseBuyFillLockOnStopLoss, true);
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

test('action.place_order single staged take profit row round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'nba-lal-orl-2026-03-21',
    tokenId: 'magic-token',
    outcomeLabel: 'Moneyline: Magic',
    tpRules: [{ priceCent: 99, sizePct: 100 }],
  });

  assert.equal(form.fields.tpPriceCent, '');
  assert.deepEqual(form.tpRuleRows, [
    {
      id: form.tpRuleRows[0]?.id,
      priceCent: '99',
      sizePct: '100',
    },
  ]);

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.deepEqual(rebuilt.tpRules, [{ priceCent: 99, sizePct: 100 }]);
  assert.equal('tpPriceCent' in rebuilt, false);
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
  assert.equal(form.fields.ptbStopLossGapUnit, 'usd');
  assert.equal(form.ptbStopLossRuleRows.length, 1);
  assert.equal(form.ptbStopLossRuleRows[0]?.gapUsd, '0');
  assert.equal(form.ptbStopLossRuleRows[0]?.sizePct, '100');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.ptbStopLossEnabled, true);
  assert.equal(rebuilt.ptbStopLossGapUnit, 'usd');
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
  assert.equal(form.fields.ptbStopLossGapUnit, 'usd');
  assert.equal(form.ptbStopLossRuleRows.length, 1);
  assert.equal(form.ptbStopLossRuleRows[0]?.gapUsd, '-20');
  assert.equal(form.ptbStopLossRuleRows[0]?.sizePct, '100');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.ptbStopLossEnabled, true);
  assert.equal(rebuilt.ptbStopLossGapUnit, 'usd');
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

test('action.place_order PTB stop-loss bump loss table round-trips through mapper form state', () => {
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
    priceToBeatStopLossBumpMode: 'loss_table',
    priceToBeatStopLossBumpUnit: 'cent',
    priceToBeatStopLossBumpLossRules: [
      { lossUsd: 1, bumpValue: 25 },
      { lossUsd: 2, bumpValue: 50 },
      { lossUsd: 5, bumpValue: 100 },
    ],
  });

  assert.equal(form.fields.priceToBeatStopLossBumpEnabled, 'true');
  assert.equal(form.fields.priceToBeatStopLossBumpMode, 'loss_table');
  assert.equal(form.fields.priceToBeatStopLossBumpUnit, 'cent');
  assert.equal(form.ptbStopLossBumpLossRuleRows.length, 3);
  assert.equal(form.ptbStopLossBumpLossRuleRows[0]?.lossUsd, '1');
  assert.equal(form.ptbStopLossBumpLossRuleRows[2]?.bumpValue, '100');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.priceToBeatStopLossBumpEnabled, true);
  assert.equal(rebuilt.priceToBeatStopLossBumpMode, 'loss_table');
  assert.equal(rebuilt.priceToBeatStopLossBumpUnit, 'cent');
  assert.deepEqual(rebuilt.priceToBeatStopLossBumpLossRules, [
    { lossUsd: 1, bumpValue: 25 },
    { lossUsd: 2, bumpValue: 50 },
    { lossUsd: 5, bumpValue: 100 },
  ]);
  assert.equal('priceToBeatStopLossBumpAmount' in rebuilt, false);
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

test('action.place_order explicit false PTB bump clears stale fixed config in form state', () => {
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
    priceToBeatStopLossBumpEnabled: false,
    priceToBeatStopLossBumpAmount: 55,
    priceToBeatStopLossBumpMaxValue: 300,
    priceToBeatStopLossBumpUnit: 'cent',
    priceToBeatStopLossBumpScope: 'per_scope',
    priceToBeatStopLossBumpDecayWindows: 2,
  });

  assert.equal(form.fields.priceToBeatStopLossBumpEnabled ?? '', '');
  assert.equal(form.fields.priceToBeatStopLossBumpMode ?? '', '');
  assert.equal(form.fields.priceToBeatStopLossBumpAmount ?? '', '');
  assert.equal(form.fields.priceToBeatStopLossBumpMaxValue ?? '', '');
  assert.equal(form.fields.priceToBeatStopLossBumpUnit ?? '', '');
  assert.equal(form.fields.priceToBeatStopLossBumpScope ?? '', '');
  assert.equal(form.fields.priceToBeatStopLossBumpDecayWindows ?? '', '');
  assert.equal(form.ptbStopLossBumpLossRuleRows.length, 0);
});

test('action.place_order explicit false PTB bump clears stale loss table rows in form state', () => {
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
    priceToBeatStopLossBumpEnabled: false,
    priceToBeatStopLossBumpUnit: 'cent',
    priceToBeatStopLossBumpLossRules: [
      { lossUsd: 1, bumpValue: 25 },
      { lossUsd: 2, bumpValue: 50 },
    ],
  });

  assert.equal(form.fields.priceToBeatStopLossBumpEnabled ?? '', '');
  assert.equal(form.fields.priceToBeatStopLossBumpMode ?? '', '');
  assert.equal(form.ptbStopLossBumpLossRuleRows.length, 0);
});

test('action.place_order explicit false PTB bump drops stale bump fields on rebuild', () => {
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
    priceToBeatStopLossBumpEnabled: false,
    priceToBeatStopLossBumpAmount: 55,
    priceToBeatStopLossBumpMaxValue: 300,
    priceToBeatStopLossBumpUnit: 'cent',
    priceToBeatStopLossBumpScope: 'per_scope',
    priceToBeatStopLossBumpLossRules: [{ lossUsd: 1, bumpValue: 25 }],
  });

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.priceToBeatGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatMode, 'manual');
  assert.equal(rebuilt.priceToBeatMaxDiff, 80);
  assert.equal(rebuilt.priceToBeatMaxDiffUnit, 'cent');
  assert.equal('priceToBeatStopLossBumpEnabled' in rebuilt, false);
  assert.equal('priceToBeatStopLossBumpMode' in rebuilt, false);
  assert.equal('priceToBeatStopLossBumpAmount' in rebuilt, false);
  assert.equal('priceToBeatStopLossBumpMaxValue' in rebuilt, false);
  assert.equal('priceToBeatStopLossBumpUnit' in rebuilt, false);
  assert.equal('priceToBeatStopLossBumpScope' in rebuilt, false);
  assert.equal('priceToBeatStopLossBumpLossRules' in rebuilt, false);
});

test('action.place_order infers PTB bump from legacy config when explicit toggle is absent', () => {
  const fixedForm = parseNodeConfigToForm('action.place_order', {
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
    priceToBeatStopLossBumpAmount: 10,
    priceToBeatStopLossBumpUnit: 'cent',
  });

  assert.equal(fixedForm.fields.priceToBeatStopLossBumpEnabled, 'true');
  assert.equal(fixedForm.fields.priceToBeatStopLossBumpMode, 'fixed');

  const lossTableForm = parseNodeConfigToForm('action.place_order', {
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
    priceToBeatStopLossBumpUnit: 'cent',
    priceToBeatStopLossBumpLossRules: [{ lossUsd: 1, bumpValue: 25 }],
  });

  assert.equal(lossTableForm.fields.priceToBeatStopLossBumpEnabled, 'true');
  assert.equal(lossTableForm.fields.priceToBeatStopLossBumpMode, 'loss_table');
  assert.equal(lossTableForm.ptbStopLossBumpLossRuleRows.length, 1);
});

test('action.place_order pair_lock explicit false PTB bump clears stale bump config', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'pair_lock',
    side: 'buy',
    executionMode: 'limit',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    pairMaxTotalCent: 90,
    counterLegEnabled: true,
    counterLegSizeUsdc: 5,
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'manual',
    priceToBeatMaxDiff: 10,
    priceToBeatMaxDiffUnit: 'usd',
    priceToBeatStopLossBumpEnabled: false,
    priceToBeatStopLossBumpAmount: 55,
    priceToBeatStopLossBumpUnit: 'cent',
    priceToBeatStopLossBumpScope: 'per_scope',
  });

  assert.equal(form.fields.priceToBeatStopLossBumpEnabled ?? '', '');
  assert.equal(form.fields.priceToBeatStopLossBumpAmount ?? '', '');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.mode, 'pair_lock');
  assert.equal(rebuilt.priceToBeatGuardEnabled, true);
  assert.equal('priceToBeatStopLossBumpEnabled' in rebuilt, false);
  assert.equal('priceToBeatStopLossBumpAmount' in rebuilt, false);
  assert.equal('priceToBeatStopLossBumpUnit' in rebuilt, false);
  assert.equal('priceToBeatStopLossBumpScope' in rebuilt, false);
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
    priceToBeatMaxPriceRelaxEnabled: false,
    priceToBeatMaxPriceRelaxMissCount: 3,
    priceToBeatMaxPriceRelaxHistoryCount: 4,
    priceToBeatMaxPriceRelaxMinValue: 15,
    priceToBeatMaxPriceRelaxMinUnit: 'cent',
    priceToBeatMaxPriceRelaxStepMode: 'absolute',
    priceToBeatMaxPriceRelaxStepValue: 20,
    priceToBeatMaxPriceRelaxStepUnit: 'cent',
  });

  assert.equal(form.fields.priceToBeatMaxPriceRelaxEnabled, 'false');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxMissCount, '3');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxHistoryCount, '4');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxMinValue, '15');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxMinUnit, 'cent');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxStepMode, 'absolute');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxStepValue, '20');
  assert.equal(form.fields.priceToBeatMaxPriceRelaxStepUnit, 'cent');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxEnabled, false);
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
    priceToBeatCurrentPriceSource: 'binance',
    ptbStopLossCurrentPriceSource: 'coinbase',
    ptbStopLossRules: [
      { gapUsd: 12.5, sizePct: 25 },
      { gapUsd: 3, sizePct: 75 },
    ],
    reenterOnSlHit: true,
    stagedSlReentryOnlyAfterAllStages: true,
  });

  assert.equal(form.fields.ptbStopLossEnabled, 'true');
  assert.equal(form.fields.ptbStopLossGapUnit, 'usd');
  assert.equal(form.fields.priceToBeatCurrentPriceSource, 'binance');
  assert.equal(form.fields.ptbStopLossCurrentPriceSource, 'coinbase');
  assert.equal(form.ptbStopLossRuleRows.length, 2);
  assert.equal(form.ptbStopLossRuleRows[0]?.gapUsd, '12.5');
  assert.equal(form.ptbStopLossRuleRows[1]?.sizePct, '75');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.ptbStopLossGapUnit, 'usd');
  assert.deepEqual(rebuilt.ptbStopLossRules, [
    { gapUsd: 12.5, sizePct: 25 },
    { gapUsd: 3, sizePct: 75 },
  ]);
  assert.equal(rebuilt.ptbStopLossEnabled, true);
  assert.equal(rebuilt.priceToBeatCurrentPriceSource, 'binance');
  assert.equal(rebuilt.ptbStopLossCurrentPriceSource, 'coinbase');
  assert.equal(rebuilt.stagedSlReentryOnlyAfterAllStages, true);
});

test('action.place_order staged ptb stop-loss preserves cent unit through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    ptbStopLossGapUnit: 'cent',
    ptbStopLossRules: [
      { gapUsd: 20, sizePct: 60 },
      { gapUsd: 0, sizePct: 40 },
    ],
  });

  assert.equal(form.fields.ptbStopLossEnabled, 'true');
  assert.equal(form.fields.ptbStopLossGapUnit, 'cent');
  assert.equal(form.ptbStopLossRuleRows[0]?.gapUsd, '20');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.ptbStopLossGapUnit, 'cent');
  assert.deepEqual(rebuilt.ptbStopLossRules, [
    { gapUsd: 20, sizePct: 60 },
    { gapUsd: 0, sizePct: 40 },
  ]);
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
    ptbStopLossCurrentPriceSource: 'coinbase',
    ptbStopLossRules: [{ gapUsd: 3, sizePct: 100 }],
  });

  form.fields.ptbStopLossEnabled = 'false';

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal('ptbStopLossGapUsd' in rebuilt, false);
  assert.equal('ptbStopLossCurrentPriceSource' in rebuilt, false);
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
    pairProtectiveUnwindEnabled: false,
    pairIgnoreStopLossAfterLocked: true,
    notifyOnPairLocked: true,
    counterLegEnabled: true,
    counterLegSizeUsdc: 5,
    counterLegOutcomeLabel: 'opposite',
    counterLegTriggerCondition: 'level_below',
    counterLegTriggerPriceCent: 20,
    counterLegMaxPriceCent: 42,
    counterLegPriceToBeatGuardEnabled: true,
    counterLegPriceToBeatMode: 'manual',
    counterLegPriceToBeatCurrentPriceSource: 'coinbase',
    counterLegPriceToBeatMaxDiff: 10,
    counterLegPriceToBeatMaxDiffUnit: 'usd',
    counterLegExecutionFloorGuardEnabled: true,
    counterLegExecutionFloorPriceCent: 18,
    counterLegRetryOnPriceToBeatGuardBlock: true,
    tpEnabled: true,
    tpPriceCent: 95,
    tpRules: [{ priceCent: 95, sizePct: 100 }],
    notifyOnTpHit: true,
    counterLegTpEnabled: true,
    counterLegTpPriceCent: 82,
    counterLegTpRules: [{ priceCent: 82, sizePct: 100 }],
    counterLegNotifyOnTpHit: false,
    slEnabled: true,
    slPriceCent: 45,
    slTriggerPriceMode: 'composite_safe',
    ptbStopLossEnabled: true,
    ptbStopLossGapUsd: 0,
    ptbStopLossGapUnit: 'usd',
    ptbStopLossTimeDecayMode: 'relax',
    priceToBeatCurrentPriceSource: 'binance',
    ptbStopLossCurrentPriceSource: 'coinbase',
    ptbStopLossRules: [
      { gapUsd: 7, sizePct: 60 },
      { gapUsd: 0, sizePct: 40 },
    ],
    notifyOnSlHit: true,
    counterLegSlEnabled: true,
    counterLegSlPriceCent: 38,
    counterLegSlTriggerPriceMode: 'best_bid',
    counterLegPtbStopLossEnabled: true,
    counterLegPtbStopLossGapUsd: -2,
    counterLegPtbStopLossGapUnit: 'cent',
    counterLegPtbStopLossTimeDecayMode: 'tighten',
    counterLegPtbStopLossCurrentPriceSource: 'binance',
    counterLegNotifyOnSlHit: false,
    reenterOnSlHit: true,
    reentryMaxAttempts: 2,
    reentryCooldownSec: 15,
  });

  assert.equal(form.fields.mode, 'pair_lock');
  assert.equal(form.fields.pairSizingMode, 'manual');
  assert.equal(form.fields.counterLegOutcomeLabel, 'opposite');
  assert.equal(form.fields.pairMaxTotalCent, '90');
  assert.equal(form.fields.pairProtectiveUnwindEnabled, 'false');
  assert.equal(form.fields.pairIgnoreStopLossAfterLocked, 'true');
  assert.equal(form.fields.counterLegPriceToBeatMaxDiffUnit, 'usd');
  assert.equal(form.fields.counterLegPriceToBeatCurrentPriceSource, 'coinbase');
  assert.equal(form.fields.tpPriceCent, '95');
  assert.equal(form.fields.counterLegTpPriceCent, '82');
  assert.equal(form.fields.slPriceCent, '45');
  assert.equal(form.fields.ptbStopLossGapUsd, '0');
  assert.equal(form.fields.ptbStopLossGapUnit, 'usd');
  assert.equal(form.fields.priceToBeatCurrentPriceSource, 'binance');
  assert.equal(form.fields.ptbStopLossCurrentPriceSource, 'coinbase');
  assert.equal(form.fields.counterLegSlPriceCent, '38');
  assert.equal(form.fields.counterLegPtbStopLossGapUsd, '-2');
  assert.equal(form.fields.counterLegPtbStopLossGapUnit, 'cent');
  assert.equal(form.fields.counterLegPtbStopLossCurrentPriceSource, 'binance');
  assert.equal(form.tpRuleRows.length, 1);
  assert.equal(form.counterLegTpRuleRows.length, 1);
  assert.equal(form.ptbStopLossRuleRows.length, 2);

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.mode, 'pair_lock');
  assert.equal(rebuilt.pairMaxTotalCent, 90);
  assert.equal('pairTargetTotalCent' in rebuilt, false);
  assert.equal(rebuilt.pairSizingMode, 'manual');
  assert.equal(rebuilt.pairProtectiveUnwindEnabled, false);
  assert.equal(rebuilt.pairIgnoreStopLossAfterLocked, true);
  assert.equal(rebuilt.counterLegEnabled, true);
  assert.equal(rebuilt.counterLegSizeUsdc, 5);
  assert.equal(rebuilt.counterLegOutcomeLabel, 'opposite');
  assert.equal(rebuilt.counterLegTriggerCondition, 'level_below');
  assert.equal(rebuilt.counterLegTriggerPriceCent, 20);
  assert.equal(rebuilt.counterLegMaxPriceCent, 42);
  assert.equal(rebuilt.counterLegPriceToBeatGuardEnabled, true);
  assert.equal(rebuilt.counterLegPriceToBeatMode, 'manual');
  assert.equal(rebuilt.counterLegPriceToBeatCurrentPriceSource, 'coinbase');
  assert.equal(rebuilt.counterLegPriceToBeatMaxDiff, 10);
  assert.equal(rebuilt.counterLegPriceToBeatMaxDiffUnit, 'usd');
  assert.equal(rebuilt.counterLegExecutionFloorGuardEnabled, true);
  assert.equal(rebuilt.counterLegExecutionFloorPriceCent, 18);
  assert.equal(rebuilt.counterLegRetryOnPriceToBeatGuardBlock, true);
  assert.equal(rebuilt.tpEnabled, true);
  assert.equal(rebuilt.tpPriceCent, 95);
  assert.deepEqual(rebuilt.tpRules, [{ priceCent: 95, sizePct: 100 }]);
  assert.equal(rebuilt.notifyOnTpHit, true);
  assert.equal(rebuilt.counterLegTpEnabled, true);
  assert.equal(rebuilt.counterLegTpPriceCent, 82);
  assert.deepEqual(rebuilt.counterLegTpRules, [{ priceCent: 82, sizePct: 100 }]);
  assert.equal(rebuilt.counterLegNotifyOnTpHit, false);
  assert.equal(rebuilt.counterLegSlEnabled, true);
  assert.equal(rebuilt.counterLegSlPriceCent, 38);
  assert.equal(rebuilt.counterLegSlTriggerPriceMode, 'best_bid');
  assert.equal(rebuilt.counterLegPtbStopLossEnabled, true);
  assert.equal(rebuilt.counterLegPtbStopLossGapUsd, -2);
  assert.equal(rebuilt.counterLegPtbStopLossGapUnit, 'cent');
  assert.equal(rebuilt.counterLegPtbStopLossTimeDecayMode, 'tighten');
  assert.equal(rebuilt.counterLegNotifyOnSlHit, false);
  assert.equal(rebuilt.slEnabled, true);
  assert.equal(rebuilt.slPriceCent, 45);
  assert.equal(rebuilt.slTriggerPriceMode, 'composite_safe');
  assert.equal(rebuilt.ptbStopLossEnabled, true);
  assert.equal(rebuilt.ptbStopLossGapUsd, 0);
  assert.equal(rebuilt.ptbStopLossGapUnit, 'usd');
  assert.equal(rebuilt.ptbStopLossTimeDecayMode, 'relax');
  assert.equal(rebuilt.priceToBeatCurrentPriceSource, 'binance');
  assert.equal(rebuilt.ptbStopLossCurrentPriceSource, 'coinbase');
  assert.deepEqual(rebuilt.ptbStopLossRules, [
    { gapUsd: 7, sizePct: 60 },
    { gapUsd: 0, sizePct: 40 },
  ]);
  assert.equal(rebuilt.notifyOnSlHit, true);
  assert.equal(rebuilt.reenterOnSlHit, true);
  assert.equal(rebuilt.reentryMaxAttempts, 2);
  assert.equal(rebuilt.reentryCooldownSec, 15);
  assert.equal(rebuilt.counterLegPtbStopLossCurrentPriceSource, 'binance');
});

test('action.place_order pair_lock preserves primary PTB bump loss table and relax config through mapper form state', () => {
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
    pairMaxTotalCent: 90,
    counterLegEnabled: true,
    counterLegSizeUsdc: 5,
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'auto_vol_pct',
    priceToBeatStopLossBumpEnabled: true,
    priceToBeatStopLossBumpMode: 'loss_table',
    priceToBeatStopLossBumpUnit: 'cent',
    priceToBeatStopLossBumpLossRules: [
      { lossUsd: 1, bumpValue: 25 },
      { lossUsd: 2, bumpValue: 50 },
    ],
    priceToBeatMaxPriceRelaxMissCount: 5,
    priceToBeatMaxPriceRelaxHistoryCount: 7,
    priceToBeatMaxPriceRelaxMinValue: 10,
    priceToBeatMaxPriceRelaxMinUnit: 'cent',
    priceToBeatMaxPriceRelaxMinDepthUsd: 4,
    priceToBeatMaxPriceRelaxStepMode: 'absolute',
    priceToBeatMaxPriceRelaxStepValue: 2,
    priceToBeatMaxPriceRelaxStepUnit: 'cent',
    notifyOnPriceToBeatGapBlocked: true,
    retryOnPriceToBeatGuardBlock: true,
  });

  assert.equal(form.fields.priceToBeatGuardEnabled, 'true');
  assert.equal(form.fields.priceToBeatStopLossBumpMode, 'loss_table');
  assert.equal(form.ptbStopLossBumpLossRuleRows.length, 2);

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.mode, 'pair_lock');
  assert.equal(rebuilt.priceToBeatGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatMode, 'auto_vol_pct');
  assert.equal(rebuilt.priceToBeatStopLossBumpEnabled, true);
  assert.equal(rebuilt.priceToBeatStopLossBumpMode, 'loss_table');
  assert.equal(rebuilt.priceToBeatStopLossBumpUnit, 'cent');
  assert.deepEqual(rebuilt.priceToBeatStopLossBumpLossRules, [
    { lossUsd: 1, bumpValue: 25 },
    { lossUsd: 2, bumpValue: 50 },
  ]);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMissCount, 5);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxHistoryCount, 7);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMinValue, 10);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMinUnit, 'cent');
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMinDepthUsd, 4);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepMode, 'absolute');
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepValue, 2);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepUnit, 'cent');
  assert.equal(rebuilt.notifyOnPriceToBeatGapBlocked, true);
  assert.equal(rebuilt.retryOnPriceToBeatGuardBlock, true);
  assert.equal('priceToBeatStopLossBumpAmount' in rebuilt, false);
});

test('action.place_order pair_lock preserves fixed primary PTB bump without loss table rows', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'pair_lock',
    pairSizingMode: 'manual',
    side: 'buy',
    executionMode: 'limit',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    pairMaxTotalCent: 90,
    counterLegEnabled: true,
    counterLegSizeUsdc: 5,
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'manual',
    priceToBeatMaxDiff: 80,
    priceToBeatMaxDiffUnit: 'cent',
    priceToBeatStopLossBumpEnabled: true,
    priceToBeatStopLossBumpAmount: 10,
    priceToBeatStopLossBumpMaxValue: 30,
    priceToBeatStopLossBumpUnit: 'cent',
  });

  assert.equal(form.ptbStopLossBumpLossRuleRows.length, 0);

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.mode, 'pair_lock');
  assert.equal(rebuilt.priceToBeatGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatMode, 'manual');
  assert.equal(rebuilt.priceToBeatMaxDiff, 80);
  assert.equal(rebuilt.priceToBeatMaxDiffUnit, 'cent');
  assert.equal(rebuilt.priceToBeatStopLossBumpEnabled, true);
  assert.equal(rebuilt.priceToBeatStopLossBumpAmount, 10);
  assert.equal(rebuilt.priceToBeatStopLossBumpMaxValue, 30);
  assert.equal(rebuilt.priceToBeatStopLossBumpUnit, 'cent');
  assert.equal('priceToBeatStopLossBumpLossRules' in rebuilt, false);
});

test('action.place_order pair_lock keeps counter stop-loss fields empty without explicit config', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'pair_lock',
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    pairMaxTotalCent: 90,
    pairSizingMode: 'manual',
    counterLegEnabled: true,
    counterLegSizeUsdc: 5,
    slEnabled: true,
    slPriceCent: 45,
    slTriggerPriceMode: 'composite_safe',
    ptbStopLossEnabled: true,
    ptbStopLossGapUsd: 0,
    ptbStopLossTimeDecayMode: 'relax',
    notifyOnSlHit: true,
  });

  assert.equal(form.fields.counterLegSlEnabled ?? '', '');
  assert.equal(form.fields.counterLegTpEnabled ?? '', '');
  assert.equal(form.fields.counterLegTpPriceCent ?? '', '');
  assert.equal(form.fields.counterLegNotifyOnTpHit ?? '', '');
  assert.equal(form.fields.counterLegSlPriceCent ?? '', '');
  assert.equal(form.fields.counterLegSlTriggerPriceMode ?? '', '');
  assert.equal(form.fields.counterLegPtbStopLossEnabled ?? '', '');
  assert.equal(form.fields.counterLegPtbStopLossGapUsd ?? '', '');
  assert.equal(form.fields.counterLegPtbStopLossGapUnit ?? '', '');
  assert.equal(form.fields.counterLegPtbStopLossCurrentPriceSource ?? '', '');
  assert.equal(form.fields.counterLegPtbStopLossTimeDecayMode ?? '', '');
  assert.equal(form.fields.counterLegNotifyOnSlHit ?? '', '');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.ptbStopLossGapUnit, 'usd');
  assert.equal('counterLegSlEnabled' in rebuilt, false);
  assert.equal('counterLegTpEnabled' in rebuilt, false);
  assert.equal('counterLegTpPriceCent' in rebuilt, false);
  assert.equal('counterLegTpRules' in rebuilt, false);
  assert.equal('counterLegNotifyOnTpHit' in rebuilt, false);
  assert.equal('counterLegSlPriceCent' in rebuilt, false);
  assert.equal('counterLegSlTriggerPriceMode' in rebuilt, false);
  assert.equal('counterLegPtbStopLossEnabled' in rebuilt, false);
  assert.equal('counterLegPtbStopLossGapUsd' in rebuilt, false);
  assert.equal('counterLegPtbStopLossGapUnit' in rebuilt, false);
  assert.equal('counterLegPtbStopLossCurrentPriceSource' in rebuilt, false);
  assert.equal('counterLegPtbStopLossTimeDecayMode' in rebuilt, false);
  assert.equal('counterLegNotifyOnSlHit' in rebuilt, false);
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
  assert.equal(form.fields.pairProtectiveUnwindEnabled, 'true');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.pairSizingMode, 'auto_remaining_budget');
  assert.equal(rebuilt.pairTotalBudgetUsdc, 14);
  assert.equal(rebuilt.pairProtectiveUnwindEnabled, true);
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
    counterLegTpRuleRows: [],
    slRuleRows: [],
    ptbStopLossRuleRows: [],
    ptbStopLossBumpLossRuleRows: [],
    timeExitRuleRows: [],
    drawdownRuleRows: [],
  });

  assert.equal(rebuilt.counterLegPriceToBeatMode, 'auto_vol_pct');
  assert.equal('counterLegPriceToBeatMaxDiff' in rebuilt, false);
  assert.equal('counterLegPriceToBeatMaxDiffUnit' in rebuilt, false);
});

test('action.place_order pair_lock preserves primary staged stop loss, take profit, and advanced reentry fields', () => {
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
      ptbStopLossGapUnit: 'usd',
      priceToBeatGuardEnabled: 'true',
      priceToBeatMode: 'auto_vol_pct',
      priceToBeatStopLossBumpEnabled: 'true',
      priceToBeatStopLossBumpMode: 'loss_table',
      priceToBeatStopLossBumpUnit: 'cent',
      priceToBeatMaxPriceRelaxMissCount: '5',
      priceToBeatMaxPriceRelaxHistoryCount: '6',
      priceToBeatMaxPriceRelaxMinValue: '10',
      priceToBeatMaxPriceRelaxMinUnit: 'cent',
      priceToBeatMaxPriceRelaxMinDepthUsd: '4',
      priceToBeatMaxPriceRelaxStepMode: 'absolute',
      priceToBeatMaxPriceRelaxStepValue: '2',
      priceToBeatMaxPriceRelaxStepUnit: 'cent',
      notifyOnPriceToBeatGapBlocked: 'true',
      retryOnPriceToBeatGuardBlock: 'true',
      reenterOnSlHit: 'true',
      reentryMaxAttempts: '2',
      reentryCooldownSec: '15',
      tpEnabled: 'true',
      tpPriceCent: '95',
      counterLegTpEnabled: 'true',
      counterLegTpPriceCent: '82',
      reentryMinPriceCent: '40',
      reentryMaxPriceCent: '88',
      reentryPriceToBeatMaxDiff: '3',
      reentryPriceToBeatMaxDiffUnit: 'usd',
      reentrySkipCurrentWindow: 'true',
      reentryThresholdDecay: '0.8',
      reentryMaxPriceTightenBps: '500',
    },
    advancedJson: '',
    triggerSizeRows: [],
    keyValueDrafts: [],
    expressionRows: [],
    outcomeConditionRows: [],
    tpRuleRows: [{ id: 'tp-1', priceCent: '95', sizePct: '100' }],
    counterLegTpRuleRows: [{ id: 'counter-tp-1', priceCent: '82', sizePct: '100' }],
    slRuleRows: [{ id: 'sl-1', priceCent: '40', sizePct: '100' }],
    ptbStopLossRuleRows: [{ id: 'ptb-1', gapUsd: '0', sizePct: '100' }],
    ptbStopLossBumpLossRuleRows: [
      { id: 'bump-1', lossUsd: '1', bumpValue: '25' },
      { id: 'bump-2', lossUsd: '2', bumpValue: '50' },
    ],
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
  assert.equal(rebuilt.ptbStopLossGapUnit, 'usd');
  assert.equal(rebuilt.priceToBeatGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatMode, 'auto_vol_pct');
  assert.equal(rebuilt.priceToBeatStopLossBumpEnabled, true);
  assert.equal(rebuilt.priceToBeatStopLossBumpMode, 'loss_table');
  assert.equal(rebuilt.priceToBeatStopLossBumpUnit, 'cent');
  assert.deepEqual(rebuilt.priceToBeatStopLossBumpLossRules, [
    { lossUsd: 1, bumpValue: 25 },
    { lossUsd: 2, bumpValue: 50 },
  ]);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMissCount, 5);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxHistoryCount, 6);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMinValue, 10);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMinUnit, 'cent');
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxMinDepthUsd, 4);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepMode, 'absolute');
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepValue, 2);
  assert.equal(rebuilt.priceToBeatMaxPriceRelaxStepUnit, 'cent');
  assert.equal(rebuilt.notifyOnPriceToBeatGapBlocked, true);
  assert.equal(rebuilt.retryOnPriceToBeatGuardBlock, true);
  assert.equal(rebuilt.reenterOnSlHit, true);
  assert.equal(rebuilt.reentryMaxAttempts, 2);
  assert.equal(rebuilt.reentryCooldownSec, 15);
  assert.equal(rebuilt.tpEnabled, true);
  assert.equal(rebuilt.tpPriceCent, 95);
  assert.deepEqual(rebuilt.tpRules, [{ priceCent: 95, sizePct: 100 }]);
  assert.equal(rebuilt.counterLegTpEnabled, true);
  assert.equal(rebuilt.counterLegTpPriceCent, 82);
  assert.deepEqual(rebuilt.counterLegTpRules, [{ priceCent: 82, sizePct: 100 }]);
  assert.deepEqual(rebuilt.slRules, [{ priceCent: 40, sizePct: 100 }]);
  assert.deepEqual(rebuilt.ptbStopLossRules, [{ gapUsd: 0, sizePct: 100 }]);
  assert.equal('timeExitRules' in rebuilt, false);
  assert.equal(rebuilt.reentryMinPriceCent, 40);
  assert.equal(rebuilt.reentryMaxPriceCent, 88);
  assert.equal(rebuilt.reentryPriceToBeatMaxDiff, 3);
  assert.equal(rebuilt.reentryPriceToBeatMaxDiffUnit, 'usd');
  assert.equal(rebuilt.reentrySkipCurrentWindow, true);
  assert.equal(rebuilt.reentryThresholdDecay, 0.8);
  assert.equal(rebuilt.reentryMaxPriceTightenBps, 500);
});
