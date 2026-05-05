import assert from 'node:assert/strict';
import test from 'node:test';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import type { PtbStopLossRuleRow } from '@/lib/trade-flow-config-mappers';
import { PtbStopLossRuleSection } from './exit-sections';
import { PtbStopLossSection } from './ptb-stop-loss-section';

type CheckboxElement = React.ReactElement<{
  onChange?: (event: { target: { checked: boolean } }) => void;
}>;

function findCheckboxElement(node: React.ReactNode): CheckboxElement | null {
  if (!React.isValidElement(node)) return null;
  if (node.type === 'input') return node as CheckboxElement;

  const props = node.props as { children?: React.ReactNode };
  for (const child of React.Children.toArray(props.children)) {
    const found = findCheckboxElement(child);
    if (found) return found;
  }
  return null;
}

test('PtbStopLossSection explains that PTB stop-loss uses directional gap instead of counter token price', () => {
  const html = renderToStaticMarkup(
    React.createElement(PtbStopLossSection, {
      enabled: true,
      unit: 'usd',
      timeDecayMode: 'tighten',
      currentSourceOverride: '',
      inheritedCurrentSource: 'binance',
      rows: [{ id: 'ptb-1', gapUsd: '-10', sizePct: '100' }],
      onUpdateField: () => {},
      onUpdateRows: () => {},
    })
  );

  assert.match(html, /karsi token fiyati degil/i);
  assert.match(html, /secilen current kaynak/i);
  assert.match(html, /Up\/Yes icin PTB referansinin 10 altini/i);
  assert.match(html, /Down\/No icin PTB referansinin 10 ustunu bekler/i);
  assert.match(html, /Negatif esik, karsi yone overshoot bekler/i);
  assert.match(html, /PTB SL Current Kaynagi/i);
  assert.match(html, /Entry PTB kaynagi ile ayni/i);
  assert.match(html, /Binance/i);
  assert.match(html, /Bos birakilirsa yukaridaki PTB Current Kaynagi kullanilir/i);
});

test('PtbStopLossSection renders selected PTB stop-loss source override', () => {
  const html = renderToStaticMarkup(
    React.createElement(PtbStopLossSection, {
      enabled: true,
      unit: 'usd',
      timeDecayMode: 'tighten',
      currentSourceOverride: 'coinbase',
      inheritedCurrentSource: 'binance',
      rows: [],
      onUpdateField: () => {},
      onUpdateRows: () => {},
    })
  );

  assert.match(html, /PTB SL Current Kaynagi/i);
  assert.match(html, /Coinbase/i);
  assert.match(html, /Gap Eşiği/);
  assert.match(html, /Satış Yüzdesi/);
});

test('PtbStopLossSection creates a default gap row when enabled from an empty state', () => {
  let fieldUpdate: [string, string] | null = null;
  let updatedRows: PtbStopLossRuleRow[] | null = null;
  const element = PtbStopLossSection({
    enabled: false,
    unit: 'usd',
    timeDecayMode: 'tighten',
    currentSourceOverride: '',
    inheritedCurrentSource: 'binance',
    rows: [],
    onUpdateField: (key, value) => {
      fieldUpdate = [key, value];
    },
    onUpdateRows: (updater) => {
      updatedRows = updater([]);
    },
  });
  const checkbox = findCheckboxElement(element);
  assert.ok(checkbox);
  const onChange = checkbox.props.onChange;
  assert.ok(onChange);

  onChange({ target: { checked: true } });

  assert.deepEqual(fieldUpdate, ['ptbStopLossEnabled', 'true']);
  assert.equal(updatedRows?.length, 1);
  assert.equal(updatedRows?.[0]?.gapUsd, '');
  assert.equal(updatedRows?.[0]?.sizePct, '100');
});

test('PtbStopLossSection does not clear existing gap rows when disabled', () => {
  let fieldUpdate: [string, string] | null = null;
  let rowsUpdated = false;
  const existingRows: PtbStopLossRuleRow[] = [{ id: 'ptb-1', gapUsd: '0', sizePct: '100' }];
  const element = PtbStopLossSection({
    enabled: true,
    unit: 'usd',
    timeDecayMode: 'tighten',
    currentSourceOverride: '',
    inheritedCurrentSource: 'binance',
    rows: existingRows,
    onUpdateField: (key, value) => {
      fieldUpdate = [key, value];
    },
    onUpdateRows: () => {
      rowsUpdated = true;
    },
  });
  const checkbox = findCheckboxElement(element);
  assert.ok(checkbox);
  const onChange = checkbox.props.onChange;
  assert.ok(onChange);

  onChange({ target: { checked: false } });

  assert.deepEqual(fieldUpdate, ['ptbStopLossEnabled', 'false']);
  assert.equal(rowsUpdated, false);
});

test('PtbStopLossRuleSection explains staged negative rows with directional gap semantics', () => {
  const html = renderToStaticMarkup(
    React.createElement(PtbStopLossRuleSection, {
      unit: 'usd',
      rows: [{ id: 'ptb-1', gapUsd: '-10', sizePct: '100' }],
      onAdd: () => {},
      onUpdate: () => {},
      onRemove: () => {},
    })
  );

  assert.match(html, /karsi token fiyati degil, directional gap esigidir/i);
  assert.match(html, /Up\/Yes icin current &lt;= PTB - 10/i);
  assert.match(html, /Down\/No icin current &gt;= PTB \+ 10/i);
});
