import assert from 'node:assert/strict';
import test from 'node:test';

import type { NodeConfigFormState } from '@/lib/trade-flow-config-mappers';
import { updateNodeFieldState, updateOutcomeConditionState } from './form-state';

function baseForm(): NodeConfigFormState {
  return {
    fields: { repeatMode: 'loop', onceScope: '' },
    triggerSizeRows: [],
    outcomeConditionRows: [
      {
        id: 'oc_1',
        tokenId: 'token',
        outcomeLabel: 'Up',
        triggerCondition: 'cross_above',
        triggerPriceCent: '71',
        maxPriceCent: '',
      },
    ],
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

test('updateOutcomeConditionState switches level triggers to once per market', () => {
  const next = updateOutcomeConditionState(baseForm(), 'oc_1', {
    triggerCondition: 'level_above',
  });

  assert.equal(next?.fields.repeatMode, 'once');
  assert.equal(next?.fields.onceScope, 'market');
  assert.equal(next?.outcomeConditionRows[0]?.triggerCondition, 'level_above');
});

test('updateOutcomeConditionState keeps repeat mode when switching to cross trigger', () => {
  const next = updateOutcomeConditionState(baseForm(), 'oc_1', {
    triggerCondition: 'cross_below',
  });

  assert.equal(next?.fields.repeatMode, 'loop');
  assert.equal(next?.fields.onceScope, '');
  assert.equal(next?.outcomeConditionRows[0]?.triggerCondition, 'cross_below');
});

test('updateNodeFieldState applies live gap collector mode defaults', () => {
  const form = baseForm();
  form.fields.side = 'sell';
  form.fields.executionMode = 'limit';
  form.fields.maxPriceCent = '70';

  const next = updateNodeFieldState(
    form,
    'action.place_order',
    'mode',
    'live_gap_collector_v1'
  );

  assert.equal(next?.fields.mode, 'live_gap_collector_v1');
  assert.equal(next?.fields.side, 'buy');
  assert.equal(next?.fields.executionMode, 'market');
  assert.equal(next?.fields.tpEnabled, 'true');
  assert.equal(next?.fields.tpPriceCent, '98');
  assert.equal(next?.fields.maxPriceCent, '93');
  assert.equal(next?.fields.liveGapCollectorEnabled, 'true');
  assert.equal(next?.fields.notifyOnLiveGapCollectorDecision, 'true');
  assert.equal(next?.fields.liveGapCollectorWindowStartSec, '220');
});
