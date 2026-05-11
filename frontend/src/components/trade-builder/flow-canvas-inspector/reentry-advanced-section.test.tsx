import assert from 'node:assert/strict';
import test from 'node:test';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import { ReentryAdvancedSection } from './reentry-advanced-section';

function renderSection(overrides: {
  visible?: boolean;
  priceToBeatGuardChecked?: boolean;
  priceToBeatGuardMode?: string;
  fields?: Record<string, string>;
} = {}) {
  return renderToStaticMarkup(
    React.createElement(ReentryAdvancedSection, {
      visible: overrides.visible ?? true,
      fields: overrides.fields ?? {},
      priceToBeatGuardChecked: overrides.priceToBeatGuardChecked ?? true,
      priceToBeatGuardMode: overrides.priceToBeatGuardMode ?? 'manual',
      priceToBeatGuardUnit: 'usd',
      onUpdateField: () => {},
    })
  );
}

test('ReentryAdvancedSection renders PTB override controls when PTB guard is enabled', () => {
  const html = renderSection({
    fields: {
      reentryPriceToBeatMaxDiff: '3',
      reentryPriceToBeatMaxDiffUnit: 'usd',
      reentryCooldownSec: '0',
      reentryMaxPriceTightenBps: '500',
    },
  });

  assert.match(html, /SL Sonrasi Re-entry Detaylari/);
  assert.match(html, /Re-entry PTB Min Fark/);
  assert.match(html, /Re-entry PTB Birimi/);
  assert.match(html, /Cooldown \(sn\)/);
  assert.match(html, /MaxPrice Tighten \(bps\)/);
});

test('ReentryAdvancedSection hides PTB-only controls when PTB guard is disabled', () => {
  const html = renderSection({ priceToBeatGuardChecked: false });

  assert.match(html, /SL Sonrasi Re-entry Detaylari/);
  assert.doesNotMatch(html, /Re-entry PTB Min Fark/);
  assert.doesNotMatch(html, /Re-entry PTB Birimi/);
  assert.doesNotMatch(html, /PTB Decay/);
  assert.match(html, /Cooldown \(sn\)/);
});

test('ReentryAdvancedSection renders nothing when hidden', () => {
  const html = renderSection({ visible: false });

  assert.equal(html, '');
});
