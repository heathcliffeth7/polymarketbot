import assert from 'node:assert/strict';
import test from 'node:test';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import { EntryTimingProfilesSection } from './entry-timing-profiles-section';

test('EntryTimingProfilesSection renders timing window and override fields', () => {
  const html = renderToStaticMarkup(
    React.createElement(EntryTimingProfilesSection, {
      rows: [
        {
          id: 'etp-1',
          startRemainingSec: '90',
          endRemainingSec: '45',
          maxPriceCent: '60',
          priceToBeatTriggerMinGap: '10',
          priceToBeatTriggerMaxGap: '12',
          sizeUsdc: '1.5',
        },
      ],
      onAdd: () => {},
      onUpdate: () => {},
      onRemove: () => {},
    })
  );

  assert.match(html, /Entry Timing Profiles/);
  assert.match(html, /remainingSec/);
  assert.match(html, /maxPrice/);
  assert.match(html, /PTB Min Gap/);
  assert.match(html, /sizeUsdc/);
  assert.match(html, /Profil #1/);
});
