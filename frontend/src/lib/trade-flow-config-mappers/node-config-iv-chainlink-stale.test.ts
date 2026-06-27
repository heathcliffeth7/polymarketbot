import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildNodeConfigFromForm,
  parseNodeConfigToForm,
} from '@/lib/trade-flow-config-mappers';

test('action.place_order iv chainlink stale thresholds round-trip through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'btc-updown-5m-1774013100',
    tokenId: 'btc-up-token',
    outcomeLabel: 'Up',
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'iv_mismatch_edge',
    priceToBeatIvChainlinkStaleMs: 3500,
    priceToBeatIvEntryQualityChainlinkMaxAgeMs: 3500,
  });

  assert.equal(form.fields.priceToBeatIvChainlinkStaleMs, '3500');
  assert.equal(form.fields.priceToBeatIvEntryQualityChainlinkMaxAgeMs, '3500');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.priceToBeatIvChainlinkStaleMs, 3500);
  assert.equal(rebuilt.priceToBeatIvEntryQualityChainlinkMaxAgeMs, 3500);
});
