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

test('formatClientRequestError surfaces auth failures as a login prompt', () => {
  const error = new ClientRequestError(
    'Oturumun suresi doldu veya giris yapilmamis. Lutfen tekrar login ol.',
    {
      kind: 'http',
      endpoint: '/api/trade-flow/definitions/4331',
      status: 401,
      apiCode: 'auth_unauthorized',
      retryable: false,
    }
  );

  assert.equal(
    formatClientRequestError(error, 'Workflow yuklenemedi.'),
    'Oturumun suresi doldu veya giris yapilmamis. Lutfen tekrar login ol. (HTTP 401)'
  );
  assert.equal(hasClientRequestErrorCode(error, 'auth_unauthorized'), true);
});
