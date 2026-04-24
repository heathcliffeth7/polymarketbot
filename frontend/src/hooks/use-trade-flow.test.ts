import assert from 'node:assert/strict';
import test from 'node:test';

import { buildTradeFlowAutoScopeAnalysisQuery } from '@/hooks/use-trade-flow';

test('buildTradeFlowAutoScopeAnalysisQuery includes filters and skips empty dates', () => {
  const query = buildTradeFlowAutoScopeAnalysisQuery({
    sortBy: 'pnl',
    sortDirection: 'asc',
    pnl: 'loss',
    position: 'open',
    from: '2026-04-01T00:00:00.000Z',
    to: undefined,
  });
  const params = new URLSearchParams(query);

  assert.equal(params.get('sortBy'), 'pnl');
  assert.equal(params.get('sortDirection'), 'asc');
  assert.equal(params.get('pnl'), 'loss');
  assert.equal(params.get('position'), 'open');
  assert.equal(params.get('from'), '2026-04-01T00:00:00.000Z');
  assert.equal(params.has('to'), false);
});
