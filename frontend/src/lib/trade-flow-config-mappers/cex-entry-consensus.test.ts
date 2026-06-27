import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildNodeConfigFromForm,
  parseNodeConfigToForm,
} from '@/lib/trade-flow-config-mappers';

function baseBuyConfig(extra: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'btc-updown-5m-1773319200',
    tokenId: 'tok-down',
    outcomeLabel: 'Down',
    ...extra,
  };
}

test('action.place_order defaults own-open-gap CEX entry consensus', () => {
  const form = parseNodeConfigToForm('action.place_order', baseBuyConfig({
    priceToBeatGuardEnabled: true,
    priceToBeatCurrentPriceSource: 'chainlink_cex_consensus',
  }));

  assert.equal(form.fields.cexEntryConsensusBasis, 'own_open_gap');
  assert.equal(form.fields.cexEntryConsensusMode, 'binance_coinbase');
  assert.equal(form.fields.cexEntryOpenGapThresholdUsd, '5');
  assert.equal(form.fields.cexEntryOpenGapMinVenues, '2');
  assert.equal(form.fields.cexEntryOpenGapAllowCleanPairWithoutAnchor, 'true');
  assert.equal(form.fields.cexEntryOpenGapRatioMin, '0.25');
  assert.equal(form.fields.cexEntryOpenGapSpreadFloorUsd, '0.20');
  assert.equal(form.fields.cexEntryOpenGapSpreadExpectedMoveMult, '0.75');
  assert.equal(form.fields.cexEntryChainlinkSanityCheck, 'true');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.cexEntryConsensusBasis, 'own_open_gap');
  assert.equal(rebuilt.cexEntryConsensusMode, 'binance_coinbase');
  assert.equal(rebuilt.cexEntryOpenGapThresholdUsd, 5);
  assert.equal(rebuilt.cexEntryOpenGapMinVenues, 2);
  assert.equal(rebuilt.cexEntryOpenGapAllowCleanPairWithoutAnchor, true);
  assert.equal(rebuilt.cexEntryChainlinkSanityCheck, true);
});

test('action.place_order uses asset-specific CEX open-gap defaults', () => {
  const ethForm = parseNodeConfigToForm('action.place_order', baseBuyConfig({
    marketSlug: 'eth-updown-5m-1773319200',
    priceToBeatGuardEnabled: true,
    priceToBeatCurrentPriceSource: 'chainlink_cex_consensus',
  }));
  const solForm = parseNodeConfigToForm('action.place_order', baseBuyConfig({
    marketSlug: 'sol-updown-5m-1773319200',
    priceToBeatGuardEnabled: true,
    priceToBeatCurrentPriceSource: 'chainlink_cex_consensus',
  }));
  const unknownForm = parseNodeConfigToForm('action.place_order', baseBuyConfig({
    marketSlug: 'xrp-updown-5m-1773319200',
    priceToBeatGuardEnabled: true,
    priceToBeatCurrentPriceSource: 'chainlink_cex_consensus',
  }));

  assert.equal(ethForm.fields.cexEntryOpenGapThresholdUsd, '0.175');
  assert.equal(solForm.fields.cexEntryOpenGapThresholdUsd, '0.0075');
  assert.equal(unknownForm.fields.cexEntryOpenGapThresholdUsd, '0.30');
});

test('action.place_order preserves asset-auto current-price CEX mode', () => {
  const form = parseNodeConfigToForm('action.place_order', baseBuyConfig({
    priceToBeatGuardEnabled: true,
    priceToBeatCurrentPriceSource: 'chainlink_cex_consensus',
    cexEntryConsensusBasis: 'current_price',
    cexEntryConsensusMode: 'asset_auto_plus_one_or_clean_pair',
  }));

  assert.equal(form.fields.cexEntryConsensusBasis, 'current_price');
  assert.equal(form.fields.cexEntryConsensusMode, 'asset_auto_plus_one_or_clean_pair');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.cexEntryConsensusBasis, 'current_price');
  assert.equal(rebuilt.cexEntryConsensusMode, 'asset_auto_plus_one_or_clean_pair');
});

test('action.place_order normalizes invalid CEX entry consensus values', () => {
  const form = parseNodeConfigToForm('action.place_order', baseBuyConfig({
    priceToBeatGuardEnabled: true,
    priceToBeatCurrentPriceSource: 'chainlink_cex_consensus',
    cexEntryConsensusBasis: 'bad',
    cexEntryConsensusMode: 'bad',
    cexEntryOpenGapThresholdUsd: -5,
    cexEntryOpenGapMinVenues: 0,
    cexEntryOpenGapRatioMin: -1,
    cexEntryOpenGapSpreadFloorUsd: -1,
    cexEntryOpenGapSpreadExpectedMoveMult: Number.NaN,
    cexEntryOpenGapAllowCleanPairWithoutAnchor: false,
    cexEntryChainlinkSanityCheck: false,
  }));

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.cexEntryConsensusBasis, 'own_open_gap');
  assert.equal(rebuilt.cexEntryConsensusMode, 'binance_coinbase');
  assert.equal(rebuilt.cexEntryOpenGapThresholdUsd, 5);
  assert.equal(rebuilt.cexEntryOpenGapMinVenues, 2);
  assert.equal(rebuilt.cexEntryOpenGapRatioMin, 0.25);
  assert.equal(rebuilt.cexEntryOpenGapSpreadFloorUsd, 0.2);
  assert.equal(rebuilt.cexEntryOpenGapSpreadExpectedMoveMult, 0.75);
  assert.equal(rebuilt.cexEntryOpenGapAllowCleanPairWithoutAnchor, false);
  assert.equal(rebuilt.cexEntryChainlinkSanityCheck, false);
});

test('action.place_order removes CEX entry consensus fields outside chainlink CEX source', () => {
  const form = parseNodeConfigToForm('action.place_order', baseBuyConfig({
    priceToBeatGuardEnabled: true,
    priceToBeatCurrentPriceSource: 'chainlink',
    cexEntryConsensusBasis: 'own_open_gap',
    cexEntryConsensusMode: 'open_gap_okx_plus_one_or_clean_pair',
    cexEntryOpenGapThresholdUsd: 0.3,
  }));

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.cexEntryConsensusBasis, undefined);
  assert.equal(rebuilt.cexEntryConsensusMode, undefined);
  assert.equal(rebuilt.cexEntryOpenGapThresholdUsd, undefined);
});
