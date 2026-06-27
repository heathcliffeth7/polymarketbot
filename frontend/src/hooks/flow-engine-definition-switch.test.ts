import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildDraftPersistSignature,
  shouldSkipUnchangedDraftSwitchSave,
} from './flow-engine-definition-switch';
import type { TradeFlowGraph } from '@/lib/types';

const graph: TradeFlowGraph = {
  context: { beta: 2, alpha: { z: true, a: 1 } },
  nodes: [
    {
      key: 'trigger',
      type: 'trigger_market_price',
      positionX: 10,
      positionY: 20,
      config: { outcome: 'YES', threshold: 0.5 },
    },
  ],
  edges: [
    {
      key: 'edge-1',
      source: 'trigger',
      target: 'action',
      type: 'default',
      condition: { mode: 'always' },
    },
  ],
};

function payload(overrides: Partial<Record<'name' | 'description' | 'graphJson', unknown>> = {}) {
  return {
    name: 'Workflow A',
    description: 'Draft',
    graphJson: graph,
    ...overrides,
  };
}

test('skips switch save when dirty flag is true but persisted payload is unchanged', () => {
  const signature = buildDraftPersistSignature(payload());

  assert.equal(
    shouldSkipUnchangedDraftSwitchSave({
      hydratedSignature: signature,
      payloadSignature: signature,
      shouldSave: true,
    }),
    true
  );
});

test('draft signatures ignore object key order while preserving content', () => {
  const first = buildDraftPersistSignature(payload());
  const second = buildDraftPersistSignature({
    graphJson: {
      edges: graph.edges,
      nodes: graph.nodes,
      context: { alpha: { a: 1, z: true }, beta: 2 },
    },
    description: 'Draft',
    name: 'Workflow A',
  });

  assert.equal(first, second);
});

test('draft signatures change when persisted graph or metadata changes', () => {
  const original = buildDraftPersistSignature(payload());
  const changedContext = buildDraftPersistSignature(payload({
    graphJson: { ...graph, context: { ...graph.context, beta: 3 } },
  }));
  const changedNode = buildDraftPersistSignature(payload({
    graphJson: {
      ...graph,
      nodes: [{ ...graph.nodes[0], config: { ...graph.nodes[0].config, threshold: 0.7 } }],
    },
  }));
  const changedEdge = buildDraftPersistSignature(payload({
    graphJson: {
      ...graph,
      edges: [{ ...graph.edges[0], condition: { mode: 'never' } }],
    },
  }));

  assert.notEqual(original, changedContext);
  assert.notEqual(original, changedNode);
  assert.notEqual(original, changedEdge);
  assert.notEqual(original, buildDraftPersistSignature(payload({ name: 'Workflow B' })));
  assert.notEqual(original, buildDraftPersistSignature(payload({ description: 'Changed' })));
});
