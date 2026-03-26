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
