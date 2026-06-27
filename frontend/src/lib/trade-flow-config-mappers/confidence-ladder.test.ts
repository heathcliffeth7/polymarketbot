import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildNodeConfigFromForm,
  parseNodeConfigToForm,
} from '@/lib/trade-flow-config-mappers';

test('action.place_order confidence ladder config round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'confidence_ladder_hedge_lock_v1',
    side: 'buy',
    executionMode: 'market',
    confidenceLadder: {
      baseProbeShares: 2,
      maxLossPerMarketUsdc: 5,
      hardNoChaseAbove: 0.93,
      hedge: {
        oppositePriceMax: 0.2,
        profitLockPairCostMax: 0.97,
      },
    },
  });

  assert.equal(form.fields.mode, 'confidence_ladder_hedge_lock_v1');
  assert.match(form.fields.confidenceLadder, /baseProbeShares/);
  form.fields.confidenceLadder = JSON.stringify({
    baseProbeShares: 3,
    maxLossPerMarketUsdc: 6,
    hardNoChaseAbove: 0.92,
    hedge: {
      oppositePriceMax: 0.18,
      profitLockPairCostMax: 0.96,
    },
  });

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  const ladder = rebuilt.confidenceLadder as Record<string, unknown>;
  const hedge = ladder.hedge as Record<string, unknown>;

  assert.equal(rebuilt.mode, 'confidence_ladder_hedge_lock_v1');
  assert.equal(rebuilt.side, 'buy');
  assert.equal('tpEnabled' in rebuilt, false);
  assert.equal('slEnabled' in rebuilt, false);
  assert.equal(ladder.baseProbeShares, 3);
  assert.equal(ladder.maxLossPerMarketUsdc, 6);
  assert.equal(ladder.hardNoChaseAbove, 0.92);
  assert.equal(hedge.oppositePriceMax, 0.18);
});

test('trigger.market_price confidence ladder binding round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('trigger.market_price', {
    marketMode: 'auto_scope',
    marketScope: 'btc_5m_updown',
    bindingMode: 'confidence_ladder_only',
    tokenId: 'should-strip',
    outcomeConditions: [{ tokenId: 'tok', outcomeLabel: 'Up' }],
  });

  assert.equal(form.fields.bindingMode, 'confidence_ladder_only');
  assert.equal(form.outcomeConditionRows.length, 0);

  const rebuilt = buildNodeConfigFromForm('trigger.market_price', form);
  assert.equal(rebuilt.bindingMode, 'confidence_ladder_only');
  assert.equal('tokenId' in rebuilt, false);
  assert.equal('outcomeConditions' in rebuilt, false);
});
