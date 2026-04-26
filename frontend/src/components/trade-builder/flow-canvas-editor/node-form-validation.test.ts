import assert from 'node:assert/strict';
import test from 'node:test';

import type { NodeConfigFormState } from '@/lib/trade-flow-config-mappers';

import { validateNodeFormBeforeSave } from './node-form-validation';

function createBaseForm(): NodeConfigFormState {
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
    entryTimingProfileRows: [],
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

function createLossTableForm(): NodeConfigFormState {
  const form = createBaseForm();
  form.fields.side = 'buy';
  form.fields.priceToBeatStopLossBumpEnabled = 'true';
  form.fields.priceToBeatStopLossBumpMode = 'loss_table';
  return form;
}

test('validateNodeFormBeforeSave reports incomplete primary staged TP rows', () => {
  const form = createBaseForm();
  form.fields.side = 'buy';
  form.tpRuleRows = [{ id: 'tp-1', priceCent: '99', sizePct: '' }];

  const error = validateNodeFormBeforeSave('action.place_order', form);

  assert.equal(
    error,
    'Take Profit Kademeleri - Kademe #1 icin Boyut (%) eksik. Fiyat (cent) ve Boyut (%) birlikte doldurulmali.'
  );
});

test('validateNodeFormBeforeSave reports incomplete counter TP and staged SL rows', () => {
  const counterTpForm = createBaseForm();
  counterTpForm.fields.mode = 'pair_lock';
  counterTpForm.fields.side = 'buy';
  counterTpForm.counterLegTpRuleRows = [{ id: 'ctp-1', priceCent: '', sizePct: '100' }];

  assert.equal(
    validateNodeFormBeforeSave('action.place_order', counterTpForm),
    'Karsi Bacak Take Profit Kademeleri - Kademe #1 icin Fiyat (cent) eksik. Fiyat (cent) ve Boyut (%) birlikte doldurulmali.'
  );

  const stagedSlForm = createBaseForm();
  stagedSlForm.fields.mode = 'pair_lock';
  stagedSlForm.fields.side = 'buy';
  stagedSlForm.slRuleRows = [{ id: 'sl-1', priceCent: '45', sizePct: '' }];

  assert.equal(
    validateNodeFormBeforeSave('action.place_order', stagedSlForm),
    'Ilk Bacak Stop Loss Kademeleri - Kademe #1 icin Boyut (%) eksik. Fiyat (cent) ve Boyut (%) birlikte doldurulmali.'
  );
});

test('validateNodeFormBeforeSave allows complete staged exit rows', () => {
  const form = createBaseForm();
  form.fields.side = 'buy';
  form.tpRuleRows = [{ id: 'tp-1', priceCent: '99', sizePct: '100' }];

  const error = validateNodeFormBeforeSave('action.place_order', form);

  assert.equal(error, null);
});

test('validateNodeFormBeforeSave reports incomplete entry timing profile rows', () => {
  const form = createBaseForm();
  form.entryTimingProfileRows = [
    {
      id: 'etp-1',
      startRemainingSec: '90',
      endRemainingSec: '',
      maxPriceCent: '',
      priceToBeatTriggerMinGap: '',
      priceToBeatTriggerMaxGap: '',
      sizeUsdc: '',
    },
  ];

  const error = validateNodeFormBeforeSave('trigger.market_price', form);

  assert.equal(
    error,
    'Entry Timing Profiles - Satir #1 icin Baslangic ve Bitis saniyeleri birlikte ve gecerli doldurulmali.'
  );
});

test('validateNodeFormBeforeSave reports partial PTB bump loss table rows', () => {
  const missingLossForm = createLossTableForm();
  missingLossForm.ptbStopLossBumpLossRuleRows = [
    { id: 'loss-1', lossUsd: '', bumpValue: '25' },
  ];

  assert.equal(
    validateNodeFormBeforeSave('action.place_order', missingLossForm),
    'PTB Zarar Bazli Tablo - Kademe #1 icin Zarar (USD) eksik. Zarar (USD) ve Bump birlikte doldurulmali.'
  );

  const missingBumpForm = createLossTableForm();
  missingBumpForm.ptbStopLossBumpLossRuleRows = [
    { id: 'loss-1', lossUsd: '1', bumpValue: '' },
  ];

  assert.equal(
    validateNodeFormBeforeSave('action.place_order', missingBumpForm),
    'PTB Zarar Bazli Tablo - Kademe #1 icin Bump eksik. Zarar (USD) ve Bump birlikte doldurulmali.'
  );
});

test('validateNodeFormBeforeSave rejects non-positive or invalid PTB bump loss table values', () => {
  const invalidLossForm = createLossTableForm();
  invalidLossForm.ptbStopLossBumpLossRuleRows = [
    { id: 'loss-1', lossUsd: '0', bumpValue: '25' },
  ];

  assert.equal(
    validateNodeFormBeforeSave('action.place_order', invalidLossForm),
    "PTB Zarar Bazli Tablo - Kademe #1 icin Zarar (USD) 0'dan buyuk sayi olmali."
  );

  const invalidBumpForm = createLossTableForm();
  invalidBumpForm.ptbStopLossBumpLossRuleRows = [
    { id: 'loss-1', lossUsd: '1', bumpValue: 'abc' },
  ];

  assert.equal(
    validateNodeFormBeforeSave('action.place_order', invalidBumpForm),
    "PTB Zarar Bazli Tablo - Kademe #1 icin Bump 0'dan buyuk sayi olmali."
  );
});

test('validateNodeFormBeforeSave rejects unsorted PTB bump loss table rows', () => {
  const form = createLossTableForm();
  form.ptbStopLossBumpLossRuleRows = [
    { id: 'loss-1', lossUsd: '2', bumpValue: '50' },
    { id: 'loss-2', lossUsd: '1', bumpValue: '25' },
  ];

  assert.equal(
    validateNodeFormBeforeSave('action.place_order', form),
    'PTB Zarar Bazli Tablo - Kademe #2 icin Zarar (USD) onceki satirdan buyuk olmali.'
  );
});

test('validateNodeFormBeforeSave ignores fully empty PTB bump loss table rows when valid rows exist', () => {
  const form = createLossTableForm();
  form.ptbStopLossBumpLossRuleRows = [
    { id: 'loss-1', lossUsd: '1', bumpValue: '25' },
    { id: 'loss-2', lossUsd: '', bumpValue: '' },
  ];

  assert.equal(validateNodeFormBeforeSave('action.place_order', form), null);
});

test('validateNodeFormBeforeSave requires at least one complete PTB bump loss table row', () => {
  const form = createLossTableForm();
  form.ptbStopLossBumpLossRuleRows = [
    { id: 'loss-1', lossUsd: '', bumpValue: '' },
  ];

  assert.equal(
    validateNodeFormBeforeSave('action.place_order', form),
    'PTB Zarar Bazli Tablo - Loss-table modu icin en az bir tam kademe girilmeli.'
  );
});

test('validateNodeFormBeforeSave allows valid PTB bump loss table rows', () => {
  const form = createLossTableForm();
  form.ptbStopLossBumpLossRuleRows = [
    { id: 'loss-1', lossUsd: '1', bumpValue: '25' },
    { id: 'loss-2', lossUsd: '2', bumpValue: '50' },
  ];

  assert.equal(validateNodeFormBeforeSave('action.place_order', form), null);
});
