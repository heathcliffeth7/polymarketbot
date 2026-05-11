import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildNodeConfigFromForm,
  parseNodeConfigToForm,
} from '@/lib/trade-flow-config-mappers';

test('action.place_order autoTune config round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    autoTune: {
      enabled: true,
      mode: 'advice',
      sampleMarkets: 6,
      minEligibleMarkets: 4,
      cooldownMarketsAfterAdvice: 2,
      dedupeSameAdviceForMarkets: 4,
    },
  });

  assert.equal(form.fields.autoTuneEnabled, 'true');
  assert.equal(form.fields.autoTuneMode, 'advice');
  assert.equal(form.fields.autoTuneSampleMarkets, '6');
  assert.equal(form.fields.autoTuneMinEligibleMarkets, '4');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.deepEqual(rebuilt.autoTune, {
    enabled: true,
    mode: 'advice',
    sampleMarkets: 6,
    minEligibleMarkets: 4,
    cooldownMarketsAfterAdvice: 2,
    dedupeSameAdviceForMarkets: 4,
  });
  assert.equal('autoTuneEnabled' in rebuilt, false);
});

test('action.place_order disabled autoTune is dropped on rebuild', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    autoTune: {
      enabled: false,
      mode: 'advice',
      sampleMarkets: 6,
    },
  });

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal('autoTune' in rebuilt, false);
  assert.equal('autoTuneEnabled' in rebuilt, false);
});

test('action.place_order legacy flat autoTune fields normalize to nested config', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    autoTuneEnabled: true,
    autoTuneMode: 'advice',
    autoTuneSampleMarkets: 3,
    autoTuneMinEligibleMarkets: 2,
  });

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.deepEqual(rebuilt.autoTune, {
    enabled: true,
    mode: 'advice',
    sampleMarkets: 3,
    minEligibleMarkets: 2,
  });
  assert.equal('autoTuneSampleMarkets' in rebuilt, false);
});
