import assert from 'node:assert/strict';
import test from 'node:test';

import {
  collectTriggerMarketPriceCustomRangeSnapshots,
  diffTriggerMarketPriceCustomRangeSnapshots,
} from '@/lib/trade-flow-config-mappers';

test('custom_range snapshot diff catches start/end mutation', () => {
  const before = collectTriggerMarketPriceCustomRangeSnapshots({
    context: {},
    nodes: [
      {
        key: 'trigger_market',
        type: 'trigger.market_price',
        positionX: 0,
        positionY: 0,
        config: {
          cycleWindowMode: 'custom_range',
          cycleWindowStartSec: 240,
          cycleWindowEndSec: 285,
          autoSellOnWindowEnd: true,
        },
      },
    ],
    edges: [],
  });
  const after = collectTriggerMarketPriceCustomRangeSnapshots({
    context: {},
    nodes: [
      {
        key: 'trigger_market',
        type: 'trigger.market_price',
        positionX: 0,
        positionY: 0,
        config: {
          cycleWindowMode: 'custom_range',
          cycleWindowStartSec: 0,
          cycleWindowEndSec: 285,
          autoSellOnWindowEnd: true,
        },
      },
    ],
    edges: [],
  });

  const diffs = diffTriggerMarketPriceCustomRangeSnapshots(before, after);
  assert.equal(diffs.length, 1);
  assert.equal(diffs[0]?.nodeKey, 'trigger_market');
  assert.equal(diffs[0]?.before?.startSec, 240);
  assert.equal(diffs[0]?.after?.startSec, 0);
});
