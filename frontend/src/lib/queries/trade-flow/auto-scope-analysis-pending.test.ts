import assert from 'node:assert/strict';
import test from 'node:test';

import { __pendingAutoScopeAnalysisTestUtils } from './auto-scope-analysis-pending';

const NOW = '2026-04-27T23:30:00.000Z';

test('pending analysis hides permanent skip roots', () => {
  assert.equal(
    __pendingAutoScopeAnalysisTestUtils.shouldShowPendingAutoScopeAnalysisRow({
      hasAutoScopeUpstream: true,
      latestSkipReason: 'missing_auto_scope_trigger',
      filledOrUpdatedAt: '2026-04-27T23:29:00.000Z',
      nowIso: NOW,
    }),
    false
  );
});

test('pending analysis allows retryable skip roots during ttl', () => {
  assert.equal(
    __pendingAutoScopeAnalysisTestUtils.shouldShowPendingAutoScopeAnalysisRow({
      hasAutoScopeUpstream: false,
      latestSkipReason: 'missing_buy_fill_metrics',
      filledOrUpdatedAt: '2026-04-27T23:29:00.000Z',
      nowIso: NOW,
    }),
    true
  );
});

test('pending analysis requires auto-scope upstream when there is no retryable skip', () => {
  assert.equal(
    __pendingAutoScopeAnalysisTestUtils.shouldShowPendingAutoScopeAnalysisRow({
      hasAutoScopeUpstream: false,
      latestSkipReason: null,
      filledOrUpdatedAt: '2026-04-27T23:29:00.000Z',
      nowIso: NOW,
    }),
    false
  );
});

test('pending analysis hides stale unresolved rows outside ttl', () => {
  assert.equal(
    __pendingAutoScopeAnalysisTestUtils.shouldShowPendingAutoScopeAnalysisRow({
      hasAutoScopeUpstream: true,
      latestSkipReason: null,
      filledOrUpdatedAt: '2026-04-27T22:59:59.000Z',
      nowIso: NOW,
    }),
    false
  );
});
