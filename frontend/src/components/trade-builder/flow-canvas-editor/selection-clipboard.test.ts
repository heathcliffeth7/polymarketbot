import assert from 'node:assert/strict';
import test from 'node:test';

import type { FlowNode } from '../flow-canvas-constants';
import { pasteSelectionClipboard } from './selection-clipboard';

function buildGenericPresetPlaceOrderNode(id: string, refKey: string): FlowNode {
  return {
    id,
    type: 'flowNode',
    position: { x: 0, y: 0 },
    data: {
      nodeType: 'action.place_order',
      config: {
        presetKind: 'place_order',
        refKey,
        side: 'buy',
        executionMode: 'market',
      },
    },
  };
}

test('pasteSelectionClipboard rewrites generic preset refKey when it points to the source node id', () => {
  const result = pasteSelectionClipboard(
    {
      nodes: [buildGenericPresetPlaceOrderNode('action_old', 'action_old')],
      edges: [],
      pasteCount: 0,
    },
    [],
    []
  );

  assert.ok(result, 'paste result should exist');
  const [pastedNode] = result.pastedNodes;
  assert.ok(pastedNode, 'pasted node should exist');
  assert.notEqual(pastedNode.id, 'action_old');
  assert.equal(pastedNode.data.config.refKey, pastedNode.id);
});

test('pasteSelectionClipboard rewrites generic preset refKey when it uses the placeholder marker', () => {
  const result = pasteSelectionClipboard(
    {
      nodes: [buildGenericPresetPlaceOrderNode('action_old', 'preset_place_order')],
      edges: [],
      pasteCount: 0,
    },
    [],
    []
  );

  assert.ok(result, 'paste result should exist');
  const [pastedNode] = result.pastedNodes;
  assert.ok(pastedNode, 'pasted node should exist');
  assert.equal(pastedNode.data.config.refKey, pastedNode.id);
});

test('pasteSelectionClipboard preserves custom shared refs that are not node keys', () => {
  const result = pasteSelectionClipboard(
    {
      nodes: [buildGenericPresetPlaceOrderNode('action_old', 'team_shared_buy')],
      edges: [],
      pasteCount: 0,
    },
    [],
    []
  );

  assert.ok(result, 'paste result should exist');
  const [pastedNode] = result.pastedNodes;
  assert.ok(pastedNode, 'pasted node should exist');
  assert.equal(pastedNode.data.config.refKey, 'team_shared_buy');
});
