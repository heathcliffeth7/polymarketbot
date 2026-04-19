import assert from 'node:assert/strict';
import test from 'node:test';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import { PairLockStaleConfigSection } from './pair-lock-stale-config-section';
import type { NodeConfigFormState } from '@/lib/trade-flow-config-mappers';

function baseForm(): NodeConfigFormState {
  return {
    fields: {},
    triggerSizeRows: [],
    outcomeConditionRows: [],
    drawdownRuleRows: [],
    tpRuleRows: [],
    slRuleRows: [],
    ptbStopLossRuleRows: [],
    timeExitRuleRows: [],
    expressionRows: [],
    expressionJoin: 'and',
    expressionSupported: false,
    nestedExprMode: false,
    nestedExprGroup: null,
    statePatchRows: [],
    advancedJson: '',
  };
}

test('PairLockStaleConfigSection hides zero-value reentry stale entries', () => {
  const form = baseForm();
  form.fields.reentryCooldownSec = '0';
  form.fields.reentryMaxPriceTightenBps = '0';
  const html = renderToStaticMarkup(
    React.createElement(PairLockStaleConfigSection, {
      visible: true,
      form,
      onFormChange: () => {},
    })
  );

  assert.doesNotMatch(html, /reentryCooldownSec=0/);
  assert.doesNotMatch(html, /reentryMaxPriceTightenBps=0/);
});

test('PairLockStaleConfigSection still renders non-zero reentry stale entries', () => {
  const form = baseForm();
  form.fields.reentryCooldownSec = '1';
  form.fields.reentryMaxPriceTightenBps = '500';
  const html = renderToStaticMarkup(
    React.createElement(PairLockStaleConfigSection, {
      visible: true,
      form,
      onFormChange: () => {},
    })
  );

  assert.match(html, /reentryMaxPriceTightenBps=500/);
  assert.match(html, /Uyumsuz Alanlari Temizle/);
});

test('PairLockStaleConfigSection ignores supported pair-lock stop-loss fields', () => {
  const form = baseForm();
  form.fields.slEnabled = 'true';
  form.fields.slPriceCent = '45';
  form.fields.ptbStopLossEnabled = 'true';
  form.fields.ptbStopLossGapUsd = '0';
  form.fields.notifyOnSlHit = 'true';
  form.fields.reenterOnSlHit = 'true';
  form.fields.reentryMaxAttempts = '2';
  form.fields.reentryCooldownSec = '0';
  const html = renderToStaticMarkup(
    React.createElement(PairLockStaleConfigSection, {
      visible: true,
      form,
      onFormChange: () => {},
    })
  );

  assert.equal(html, '');
});
