import assert from 'node:assert/strict';
import test from 'node:test';

import {
  ClientRequestError,
  formatClientRequestError,
  hasClientRequestErrorCode,
} from './http-client';

test('formatClientRequestError preserves server busy message for http errors', () => {
  const error = new ClientRequestError('Bu flow üzerinde başka bir işlem çalışıyor. Birkaç saniye bekleyip tekrar dene.', {
    kind: 'http',
    endpoint: '/api/trade-flow/definitions/4298',
    status: 423,
    apiCode: 'flow_definition_busy',
    retryable: true,
  });

  assert.equal(
    formatClientRequestError(error, 'fallback'),
    'Bu flow üzerinde başka bir işlem çalışıyor. Birkaç saniye bekleyip tekrar dene.'
  );
  assert.equal(hasClientRequestErrorCode(error, 'flow_definition_busy'), true);
});
