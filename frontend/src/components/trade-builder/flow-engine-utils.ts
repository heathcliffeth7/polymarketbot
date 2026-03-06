import type { TradeFlowDefinitionDetail, TradeFlowGraph } from '@/lib/types';

export function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

export function deepCloneGraph(graph: TradeFlowGraph): TradeFlowGraph {
  return {
    context: isRecord(graph.context) ? JSON.parse(JSON.stringify(graph.context)) : {},
    nodes: graph.nodes.map((node) => ({
      ...node,
      config: isRecord(node.config) ? JSON.parse(JSON.stringify(node.config)) : {},
    })),
    edges: graph.edges.map((edge) => ({
      ...edge,
      condition:
        edge.condition && isRecord(edge.condition)
          ? (JSON.parse(JSON.stringify(edge.condition)) as Record<string, unknown>)
          : null,
    })),
  };
}

export function createSellBuyIfElseTemplate(
  marketSlug: string | null,
  outcome: { token_id: string; label: string } | null
): TradeFlowGraph {
  return {
    context: {
      sourceTradeId: 0,
      marketSlug: marketSlug || '',
      tokenId: outcome?.token_id || '',
      outcomeLabel: outcome?.label || '',
    },
    nodes: [
      {
        key: 'trigger_market',
        type: 'trigger.market_price',
        positionX: 80,
        positionY: 180,
        config: {
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          pollIntervalMs: 1000,
        },
      },
      {
        key: 'action_sell',
        type: 'action.place_order',
        positionX: 370,
        positionY: 120,
        config: {
          side: 'sell',
          executionMode: 'market',
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          outcomeLabel: outcome?.label || '',
          sizeUsdc: 25,
          minPriceDistanceCent: 1,
          maxTriggers: 1,
        },
      },
      {
        key: 'logic_if_rebuy',
        type: 'logic.if',
        positionX: 660,
        positionY: 180,
        config: {
          expression: { '<=': [{ var: 'market_price' }, 45] },
          comment: 'If market_price <= 45 ise yeniden alisa gec, degilse bekleme koluna git.',
        },
      },
      {
        key: 'action_buy',
        type: 'action.place_order',
        positionX: 980,
        positionY: 110,
        config: {
          side: 'buy',
          executionMode: 'market',
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          outcomeLabel: outcome?.label || '',
          sizeUsdc: 20,
          minPriceDistanceCent: 1,
          maxTriggers: 2,
        },
      },
      {
        key: 'action_wait',
        type: 'action.set_state',
        positionX: 980,
        positionY: 255,
        config: {
          statePatch: { state: 'waiting_reentry', reason: 'if_false_path' },
        },
      },
    ],
    edges: [
      { key: 'edge_1', source: 'trigger_market', target: 'action_sell', type: 'default', condition: null },
      { key: 'edge_2', source: 'action_sell', target: 'logic_if_rebuy', type: 'on_success', condition: null },
      { key: 'edge_3', source: 'logic_if_rebuy', target: 'action_buy', type: 'on_true', condition: null },
      { key: 'edge_4', source: 'logic_if_rebuy', target: 'action_wait', type: 'on_false', condition: null },
    ],
  };
}

export function buildDetailSnapshotKey(detail: TradeFlowDefinitionDetail | null): string | null {
  if (!detail?.draftVersion) return null;
  return `${detail.definition.id}:${detail.draftVersion.id}:${detail.definition.updated_at}`;
}
