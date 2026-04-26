import assert from 'node:assert/strict';
import test from 'node:test';

import { resolveAutoScopeTradeAnalysisDateFilters } from '@/lib/queries/trade-flow/analysis-time-range';

const NOW = new Date('2026-04-25T12:00:00.000Z');

test('resolveAutoScopeTradeAnalysisDateFilters resolves short relative windows', () => {
  const filters = resolveAutoScopeTradeAnalysisDateFilters({
    timeRangeRaw: '6h',
    fromRaw: '2026-01-01',
    toRaw: '2026-01-02',
    now: NOW,
  });

  assert.equal(filters.error, null);
  assert.equal(filters.timeRange, '6h');
  assert.equal(filters.from, '2026-04-25T06:00:00.000Z');
  assert.equal(filters.to, '2026-04-25T12:00:00.000Z');
});

test('resolveAutoScopeTradeAnalysisDateFilters keeps custom dates without a relative range', () => {
  const filters = resolveAutoScopeTradeAnalysisDateFilters({
    fromRaw: '2026-04-01T00:00:00.000Z',
    toRaw: '2026-04-02T23:59:59.999Z',
    now: NOW,
  });

  assert.equal(filters.error, null);
  assert.equal(filters.timeRange, 'custom');
  assert.equal(filters.from, '2026-04-01T00:00:00.000Z');
  assert.equal(filters.to, '2026-04-02T23:59:59.999Z');
});

test('resolveAutoScopeTradeAnalysisDateFilters rejects invalid time ranges', () => {
  const filters = resolveAutoScopeTradeAnalysisDateFilters({
    timeRangeRaw: '2h',
    now: NOW,
  });

  assert.match(filters.error, /timeRange must be one of/);
});

test('resolveAutoScopeTradeAnalysisDateFilters rejects invalid custom dates', () => {
  const filters = resolveAutoScopeTradeAnalysisDateFilters({
    timeRangeRaw: 'custom',
    fromRaw: 'not-a-date',
    now: NOW,
  });

  assert.equal(filters.error, 'from must be a valid date');
});
