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
    counterLegTpRuleRows: [],
    slRuleRows: [],
    ptbStopLossRuleRows: [],
    ptbStopLossBumpLossRuleRows: [],
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

test('PairLockStaleConfigSection ignores supported zero-value reentry entries', () => {
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

test('PairLockStaleConfigSection ignores supported primary advanced reentry entries', () => {
  const form = baseForm();
  form.fields.reentryCooldownSec = '1';
  form.fields.reentryMaxPriceCent = '88';
  form.fields.reentryPriceToBeatMaxDiff = '3';
  form.fields.reentryMaxPriceTightenBps = '500';
  const html = renderToStaticMarkup(
    React.createElement(PairLockStaleConfigSection, {
      visible: true,
      form,
      onFormChange: () => {},
    })
  );

  assert.equal(html, '');
});

test('PairLockStaleConfigSection ignores supported pair-lock stop-loss fields', () => {
  const form = baseForm();
  form.fields.slEnabled = 'true';
  form.fields.slPriceCent = '45';
  form.slRuleRows = [{ id: 'sl-1', priceCent: '45', sizePct: '100' }];
  form.fields.ptbStopLossEnabled = 'true';
  form.fields.ptbStopLossGapUsd = '0';
  form.fields.ptbStopLossGapUnit = 'cent';
  form.fields.notifyOnSlHit = 'true';
  form.fields.reenterOnSlHit = 'true';
  form.fields.reentryMaxAttempts = '2';
  form.fields.reentryCooldownSec = '0';
  form.ptbStopLossRuleRows = [{ id: 'ptb-1', gapUsd: '7', sizePct: '100' }];
  const html = renderToStaticMarkup(
    React.createElement(PairLockStaleConfigSection, {
      visible: true,
      form,
      onFormChange: () => {},
    })
  );

  assert.equal(html, '');
});

test('PairLockStaleConfigSection keeps supported take-profit rows out of stale warning', () => {
  const form = baseForm();
  form.fields.tpEnabled = 'true';
  form.fields.tpPriceCent = '95';
  form.fields.notifyOnTpHit = 'true';
  form.tpRuleRows = [{ id: 'tp-1', priceCent: '95', sizePct: '100' }];
  form.fields.counterLegEnabled = 'true';
  form.fields.counterLegTpEnabled = 'true';
  form.fields.counterLegTpPriceCent = '82';
  form.fields.counterLegNotifyOnTpHit = 'true';
  form.counterLegTpRuleRows = [{ id: 'counter-tp-1', priceCent: '82', sizePct: '100' }];

  const html = renderToStaticMarkup(
    React.createElement(PairLockStaleConfigSection, {
      visible: true,
      form,
      onFormChange: () => {},
    })
  );

  assert.equal(html, '');
});
