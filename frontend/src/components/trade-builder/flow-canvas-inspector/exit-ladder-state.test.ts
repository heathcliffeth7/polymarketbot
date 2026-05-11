import assert from 'node:assert/strict';
import test from 'node:test';

import { appendPrimaryTakeProfitRuleRow } from './exit-ladder-state';

test('appendPrimaryTakeProfitRuleRow defaults the first primary TP row to 100 percent', () => {
  const nextRows = appendPrimaryTakeProfitRuleRow([]);

  assert.equal(nextRows.length, 1);
  assert.equal(nextRows[0]?.priceCent, '');
  assert.equal(nextRows[0]?.sizePct, '100');
});

test('appendPrimaryTakeProfitRuleRow leaves later primary TP rows without an automatic size percent', () => {
  const nextRows = appendPrimaryTakeProfitRuleRow([
    { id: 'tp-1', priceCent: '99', sizePct: '100' },
  ]);

  assert.equal(nextRows.length, 2);
  assert.equal(nextRows[1]?.priceCent, '');
  assert.equal(nextRows[1]?.sizePct, '');
});
