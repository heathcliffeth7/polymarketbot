import assert from 'node:assert/strict';
import test from 'node:test';

import {
  FLOW_DEFINITION_BUSY_CODE,
  FLOW_DEFINITION_BUSY_MESSAGE,
  FlowDefinitionBusyError,
  mapTradeFlowMutationHttpError,
} from './mutation-errors';

test('mapTradeFlowMutationHttpError maps FlowDefinitionBusyError to 423 locked', () => {
  const mapped = mapTradeFlowMutationHttpError(
    new FlowDefinitionBusyError(4298),
    'fallback'
  );

  assert.equal(mapped.status, 423);
  assert.deepEqual(mapped.body, {
    error: FLOW_DEFINITION_BUSY_MESSAGE,
    code: FLOW_DEFINITION_BUSY_CODE,
    retryable: true,
  });
});

test('mapTradeFlowMutationHttpError maps postgres lock timeout to 423 locked', () => {
  const mapped = mapTradeFlowMutationHttpError(
    { code: '55P03', message: 'canceling statement due to lock timeout' },
    'fallback'
  );

  assert.equal(mapped.status, 423);
  assert.deepEqual(mapped.body, {
    error: FLOW_DEFINITION_BUSY_MESSAGE,
    code: FLOW_DEFINITION_BUSY_CODE,
    retryable: true,
  });
});
